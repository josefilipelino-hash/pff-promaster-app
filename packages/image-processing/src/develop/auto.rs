//! **Edição automática** por fotografia.
//!
//! Cada fotografia é medida (não há parâmetros fixos): histograma de luminância, chave da
//! cena, rostos, balanço de cinzentos por arestas, saturação média, nível de ruído (com ISO
//! e tamanho do sensor), inclinação do horizonte, aberração cromática e vinheta da objetiva,
//! olhos vermelhos e pó do sensor. Daí sai uma receita normal ([`Settings`]) — editável e
//! não destrutiva como qualquer outra — com a força escolhida em [`Intensity`].
//!
//! As correções são **parciais** de propósito (ex.: só ~70 % da distância à exposição
//! "média"), para não achatar o ambiente de uma festa à noite ou de um pôr do sol.

use super::color::{luma, mul3, neutralize, oetf, wb_matrix};
use super::filters::gaussian;
use super::geometry::GeoMap;
use super::settings::*;
use super::source::{Plane, Source};
use serde::{Deserialize, Serialize};

/// Intensidade da edição automática.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Intensity {
    VerySoft,
    Soft,
    #[default]
    Natural,
    Balanced,
    Strong,
    /// Força livre ([`AutoOptions::strength`]) e escolha dos grupos de ajustes.
    Custom,
}

impl Intensity {
    /// (fator das correções, acréscimo criativo de contraste/vibração/clareza).
    fn profile(self, custom: f32) -> (f32, f32) {
        match self {
            Intensity::VerySoft => (0.4, 0.0),
            Intensity::Soft => (0.65, 0.0),
            Intensity::Natural => (0.85, 0.0),
            Intensity::Balanced => (1.0, 5.0),
            Intensity::Strong => (1.2, 12.0),
            Intensity::Custom => {
                let k = custom.clamp(0.0, 150.0) / 100.0;
                (k, ((k - 1.0) * 24.0).clamp(0.0, 12.0))
            }
        }
    }
}

/// Quando usar o AI Denoise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiDenoise {
    /// Só a redução de ruído rápida.
    #[default]
    Off,
    /// Só nas fotografias com ruído alto (mais lento).
    WhenNeeded,
    /// Em todas as fotografias com algum ruído.
    Always,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AutoOptions {
    pub intensity: Intensity,
    /// Força em % (só em [`Intensity::Custom`]), 0…150.
    pub strength: f32,
    pub tone: bool,
    pub white_balance: bool,
    pub color: bool,
    pub sharpen: bool,
    pub denoise: bool,
    pub ai_denoise: AiDenoise,
    pub straighten: bool,
    pub crop: bool,
    pub lens: bool,
    pub red_eye: bool,
    pub dust: bool,
}

impl Default for AutoOptions {
    fn default() -> Self {
        AutoOptions {
            intensity: Intensity::Natural,
            strength: 100.0,
            tone: true,
            white_balance: true,
            color: true,
            sharpen: true,
            denoise: true,
            ai_denoise: AiDenoise::Off,
            straighten: true,
            crop: false,
            lens: true,
            red_eye: true,
            dust: false,
        }
    }
}

/// Rosto (caixa normalizada na fotografia orientada).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Informação externa à imagem.
#[derive(Debug, Clone, Default)]
pub struct AutoContext {
    pub iso: Option<u32>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub is_raw: bool,
    pub faces: Vec<FaceBox>,
    /// Centros e raios das íris (coordenadas de [`RedEye`]).
    pub eyes: Vec<RedEye>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoiseLevel {
    None,
    Low,
    Moderate,
    High,
    VeryHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseEstimate {
    /// Desvio-padrão do ruído de luminância (0–255, depois da exposição).
    pub luminance: f32,
    /// Ruído de cor (0–255).
    pub chroma: f32,
    pub iso: Option<u32>,
    /// Multiplicador pelo tamanho do sensor (1 = full frame; telemóvel ≈ 4).
    pub sensor_factor: f32,
    pub level: NoiseLevel,
    pub ai_recommended: bool,
}

/// O que foi medido e decidido (mostrado na interface).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoReport {
    pub exposure: f32,
    pub temperature: f32,
    pub tint: f32,
    pub noise: NoiseEstimate,
    pub tilt: Option<f32>,
    pub cropped: bool,
    pub red_eyes: usize,
    pub spots: usize,
    pub faces: usize,
}

fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f32 * p).round() as usize]
}

/// Fator de ruído pelo tamanho do sensor, a partir da marca/modelo (tabela aproximada;
/// 1,2 se desconhecido). Mais alto = mais ruído para o mesmo ISO.
pub fn sensor_factor(make: Option<&str>, model: Option<&str>) -> f32 {
    let make = make.unwrap_or("").to_lowercase();
    let model = model.unwrap_or("").to_lowercase();
    let has = |xs: &[&str]| xs.iter().any(|x| model.contains(x));
    // Número a seguir a um prefixo (ex.: "d850" → 850).
    let num_after = |prefix: &str| -> Option<u32> {
        let i = model.find(prefix)? + prefix.len();
        let digits: String = model[i..].chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok()
    };
    let phone = ["apple", "google", "samsung", "huawei", "xiaomi", "oneplus", "oppo", "motorola"];
    if phone.iter().any(|m| make.contains(m)) || has(&["iphone", "pixel", "sm-"]) {
        return 4.0;
    }
    if has(&["rx100", "g7 x", "g5 x", "zv-1", "rx10", "dc-tz", "dmc-tz"]) {
        return 2.5;
    }
    if ["olympus", "om digital"].iter().any(|m| make.contains(m))
        || (make.contains("panasonic") && has(&["dc-g", "dmc-g"]))
    {
        return 1.6;
    }
    if make.contains("fujifilm") {
        return if has(&["gfx"]) { 0.8 } else { 1.3 };
    }
    if make.contains("nikon") {
        if has(&[
            "z 5", "z 6", "z 7", "z 8", "z 9", "z5", "z6", "z7", "z8", "z9", "z f", "zf", "df",
        ]) {
            return 1.0;
        }
        if has(&["z 50", "z50", "z fc", "zfc", "z 30", "z30"]) {
            return 1.3;
        }
        return match num_after("d") {
            Some(n) if n < 10 => 1.0,                   // D3, D4, D5, D6
            Some(n) if (600..=900).contains(&n) => 1.0, // D600…D850
            Some(_) => 1.3,                             // D90, D500, D3500, D7500…
            None => 1.2,
        };
    }
    if make.contains("canon") {
        if has(&["5d", "6d", "1d", "eos r5", "eos r6", "eos r3", "eos r8", "eos r1"])
            || model.trim_end() == "canon eos r"
            || model.trim_end() == "eos r"
            || model.ends_with("eos rp")
        {
            return 1.0;
        }
        return 1.3;
    }
    if make.contains("sony") {
        if has(&["ilce-7", "ilce-9", "ilce-1"]) {
            return 1.0;
        }
        if has(&["ilce-6", "ilce-5", "ilce-3", "nex-"]) {
            return 1.3;
        }
    }
    if make.contains("leica") || make.contains("hasselblad") {
        return 1.0;
    }
    1.2
}

/// Ruído pelo método de Immerkær (1996) só nas zonas homogéneas (as 40 % de menor
/// gradiente) de até 9 janelas, à resolução da fonte. Devolve (luminância, cor) em 0–255.
pub fn measure_noise(src: &Source, gain: f32) -> (f32, f32) {
    let p = &src.levels[0];
    let win = 320.min(p.w).min(p.h);
    if win < 16 {
        return (0.0, 0.0);
    }
    let mut lum_est = Vec::new();
    let mut chroma_est = Vec::new();
    for gy in 0..3 {
        for gx in 0..3 {
            let x0 = (p.w - win) * gx / 2;
            let y0 = (p.h - win) * gy / 2;
            // Canais: luminância e diferenças de cor, no domínio de visualização (×255).
            let disp: Vec<[f32; 3]> = (0..win * win)
                .map(|i| {
                    let q = p.at(x0 + i % win, y0 + i / win);
                    let d = q.map(|c| oetf((c * gain).clamp(0.0, 1.0)) * 255.0);
                    [luma(d), d[0] - d[1], d[2] - d[1]]
                })
                .collect();
            let at = |x: usize, y: usize, c: usize| disp[y * win + x][c];
            let mut cand: Vec<(f32, [f32; 3])> = Vec::with_capacity((win - 2) * (win - 2));
            for y in 1..win - 1 {
                for x in 1..win - 1 {
                    let gxv = at(x + 1, y, 0) - at(x - 1, y, 0);
                    let gyv = at(x, y + 1, 0) - at(x, y - 1, 0);
                    let mut lap = [0.0; 3];
                    for (c, l) in lap.iter_mut().enumerate() {
                        *l = (at(x - 1, y - 1, c)
                            + at(x + 1, y - 1, c)
                            + at(x - 1, y + 1, c)
                            + at(x + 1, y + 1, c)
                            - 2.0
                                * (at(x, y - 1, c)
                                    + at(x - 1, y, c)
                                    + at(x + 1, y, c)
                                    + at(x, y + 1, c))
                            + 4.0 * at(x, y, c))
                        .abs();
                    }
                    // Exclui zonas saturadas ou totalmente pretas (sem ruído visível).
                    let l = at(x, y, 0);
                    if (3.0..252.0).contains(&l) {
                        cand.push((gxv.abs() + gyv.abs(), lap));
                    }
                }
            }
            if cand.len() < 500 {
                continue;
            }
            cand.sort_by(|a, b| a.0.total_cmp(&b.0));
            let n = cand.len() * 2 / 5;
            let k = (std::f32::consts::PI / 2.0).sqrt() / 6.0;
            let mean = |c: usize| cand[..n].iter().map(|v| v.1[c]).sum::<f32>() / n as f32 * k;
            lum_est.push(mean(0));
            chroma_est.push((mean(1) + mean(2)) / 2.0);
        }
    }
    if lum_est.is_empty() {
        return (0.0, 0.0);
    }
    // Mediana: robusta a janelas com textura.
    lum_est.sort_by(f32::total_cmp);
    chroma_est.sort_by(f32::total_cmp);
    (lum_est[lum_est.len() / 2], chroma_est[chroma_est.len() / 2])
}

/// Classificação do ruído (com ISO e sensor).
pub fn classify_noise(lum: f32, chroma: f32, iso: Option<u32>, factor: f32) -> NoiseEstimate {
    let eff = iso.map(|i| i as f32 * factor);
    let score = lum.max(chroma * 0.6);
    let level = match score {
        s if s < 1.5 => NoiseLevel::None,
        s if s < 2.5 => NoiseLevel::Low,
        s if s < 4.5 => NoiseLevel::Moderate,
        s if s < 7.0 => NoiseLevel::High,
        _ => NoiseLevel::VeryHigh,
    };
    let ai_recommended = matches!(level, NoiseLevel::High | NoiseLevel::VeryHigh)
        || (eff.is_some_and(|e| e >= 6400.0) && score >= 2.5);
    NoiseEstimate { luminance: lum, chroma, iso, sensor_factor: factor, level, ai_recommended }
}

/// Amostra linear da fonte (nível ≈ `edge` px) com orientação e retoque ignorados.
fn small_plane(src: &Source, edge: f32) -> &Plane {
    src.level_for(edge)
}

/// Estimativa do iluminante por "cinzento das arestas" (van de Weijer et al., 2007,
/// Minkowski p = 6) combinada com o cinzento médio dos píxeis pouco saturados.
fn estimate_illuminant(p: &Plane) -> Option<[f32; 3]> {
    let (w, h) = (p.w, p.h);
    if w < 8 || h < 8 {
        return None;
    }
    let chans: Vec<Vec<f32>> = (0..3)
        .map(|c| gaussian(&p.px.iter().map(|q| q[c]).collect::<Vec<_>>(), w, h, 1.0))
        .collect();
    let maxv = p.px.iter().map(|q| q[0].max(q[1]).max(q[2])).fold(0.0f32, f32::max);
    let mut edge = [0.0f64; 3];
    let mut gray = [0.0f64; 3];
    let mut ng = 0.0f64;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let q = p.px[i];
            let mx = q[0].max(q[1]).max(q[2]);
            if mx >= maxv * 0.95 || mx < 0.01 {
                continue;
            }
            for c in 0..3 {
                let ch = &chans[c];
                let g = (ch[i + 1] - ch[i - 1]).abs() + (ch[i + w] - ch[i - w]).abs();
                edge[c] += (g as f64).powi(6);
            }
            let mn = q[0].min(q[1]).min(q[2]);
            if (mx - mn) / mx < 0.35 {
                for c in 0..3 {
                    gray[c] += q[c] as f64;
                }
                ng += 1.0;
            }
        }
    }
    let e: [f32; 3] = edge.map(|v| v.powf(1.0 / 6.0) as f32);
    if e.iter().any(|v| *v <= 0.0) {
        return None;
    }
    let mut est = e.map(|v| v / e[1]);
    let _ = (gray, ng);
    // Refinamento por "píxeis cinzentos": depois de corrigir com a estimativa atual, os
    // píxeis quase neutros (paredes, roupa branca, nuvens) definem o iluminante.
    for thr in [0.15f32, 0.1] {
        let (mut acc, mut n) = ([0.0f64; 3], 0.0f64);
        for q in &p.px {
            let mx = q[0].max(q[1]).max(q[2]);
            if mx >= maxv * 0.95 || mx < 0.02 {
                continue;
            }
            let c = [q[0] / est[0], q[1] / est[1], q[2] / est[2]];
            let (cm, cn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
            if (cm - cn) / cm.max(1e-6) < thr {
                for k in 0..3 {
                    acc[k] += q[k] as f64;
                }
                n += 1.0;
            }
        }
        if n < p.px.len() as f64 * 0.02 || acc[1] <= 0.0 {
            break;
        }
        est = std::array::from_fn(|k| (acc[k] / acc[1]) as f32);
    }
    Some(est)
}

/// Aberração cromática lateral: procura a escala radial do vermelho e do azul (unidades de
/// [`Lens::ca_red`]) que maximiza a correlação com o verde nas arestas fora do centro.
pub fn estimate_chromatic_aberration(src: &Source) -> (f32, f32) {
    let p = &src.levels[0];
    let (w, h) = (p.w, p.h);
    if w < 64 || h < 64 {
        return (0.0, 0.0);
    }
    // Pontos de aresta (gradiente forte do verde) na coroa exterior.
    let mut pts: Vec<(f32, f32, f32)> = Vec::new();
    let stride = (w.max(h) / 1500).max(1);
    for y in (2..h - 2).step_by(stride) {
        for x in (2..w - 2).step_by(stride) {
            let (u, v) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            let (dx, dy) = (u - 0.5, v - 0.5);
            if dx * dx + dy * dy < 0.09 {
                continue;
            }
            let g = |xx: usize, yy: usize| p.at(xx, yy)[1];
            let gr = (g(x + 1, y) - g(x - 1, y)).abs() + (g(x, y + 1) - g(x, y - 1)).abs();
            pts.push((u, v, gr));
        }
    }
    if pts.len() < 200 {
        return (0.0, 0.0);
    }
    pts.sort_by(|a, b| b.2.total_cmp(&a.2));
    pts.truncate((pts.len() / 50).clamp(200, 40_000));
    if pts.last().is_some_and(|p| p.2 < 0.02) {
        pts.retain(|p| p.2 >= 0.02);
        if pts.len() < 200 {
            return (0.0, 0.0);
        }
    }
    let ncc = |a: &[f32], b: &[f32]| {
        let n = a.len() as f32;
        let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
        let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
        for (x, y) in a.iter().zip(b) {
            sab += (x - ma) * (y - mb);
            saa += (x - ma) * (x - ma);
            sbb += (y - mb) * (y - mb);
        }
        sab / (saa * sbb).sqrt().max(1e-12)
    };
    let green: Vec<f32> =
        pts.iter().map(|&(u, v, _)| p.sample(u, v).map(|q| q[1]).unwrap_or(0.0)).collect();
    let best = |ch: usize| -> f32 {
        let score = |amount: f32| {
            let lens = if ch == 0 {
                Lens { ca_red: amount, ..Default::default() }
            } else {
                Lens { ca_blue: amount, ..Default::default() }
            };
            let geo = GeoMap::new(w, h, &Geometry::default(), &lens, false);
            let vals: Vec<f32> = pts
                .iter()
                .map(|&(u, v, _)| {
                    let (m, _) = geo.map(u, v);
                    p.sample_channel(m[ch].0, m[ch].1, ch)
                })
                .collect();
            ncc(&vals, &green)
        };
        let base = score(0.0);
        let (mut bx, mut bs) = (0.0, base);
        for k in -12..=12 {
            let a = k as f32 * 5.0;
            let s = score(a);
            if s > bs {
                (bx, bs) = (a, s);
            }
        }
        // Refinamento parabólico.
        let (l, r) = (score(bx - 2.5), score(bx + 2.5));
        let den = l - 2.0 * bs + r;
        let off = if den < -1e-9 { (0.5 * (l - r) / den * 2.5).clamp(-2.5, 2.5) } else { 0.0 };
        let a = bx + off;
        // Só se a melhoria for clara e a correção relevante.
        if bs - base > 0.002 && a.abs() >= 4.0 {
            a.clamp(-60.0, 60.0)
        } else {
            0.0
        }
    };
    (best(0), best(2))
}

/// Vinheta da objetiva (só RAW, sem perfil): cantos claramente mais escuros que o centro nos
/// quatro quadrantes → correção positiva (0…35).
fn estimate_lens_vignetting(p: &Plane) -> f32 {
    let (w, h) = (p.w, p.h);
    let mut center = (0.0, 0.0);
    let mut corners = [(0.0f32, 0.0f32); 4];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = ((x as f32 + 0.5) / w as f32 - 0.5, (y as f32 + 0.5) / h as f32 - 0.5);
            let r = (u * u + v * v).sqrt() / std::f32::consts::FRAC_1_SQRT_2;
            let l = luma(p.at(x, y));
            if r < 0.35 {
                center.0 += l;
                center.1 += 1.0;
            } else if r > 0.85 {
                let q = (u > 0.0) as usize + 2 * (v > 0.0) as usize;
                corners[q].0 += l;
                corners[q].1 += 1.0;
            }
        }
    }
    if center.1 == 0.0 || corners.iter().any(|c| c.1 == 0.0) {
        return 0.0;
    }
    let c = center.0 / center.1;
    let rat: Vec<f32> = corners.iter().map(|k| k.0 / k.1 / c.max(1e-5)).collect();
    let (mn, mx) = rat.iter().fold((f32::MAX, 0.0f32), |a, &r| (a.0.min(r), a.1.max(r)));
    // Os quatro cantos parecidos (a cena não explica a diferença) e todos escuros.
    if mx < 0.75 && mn > mx * 0.75 {
        ((0.8 - (mn + mx) / 2.0) * 120.0).clamp(0.0, 35.0)
    } else {
        0.0
    }
}

/// Olhos vermelhos: para cada íris, píxeis **muito** vermelhos (no domínio de visualização,
/// onde uma íris castanha ou a pele não passam o limiar) concentrados no centro.
pub fn detect_red_eyes(src: &Source, eyes: &[RedEye]) -> Vec<RedEye> {
    let p = &src.levels[0];
    let long = p.w.max(p.h) as f32;
    let mut out = Vec::new();
    for e in eyes {
        let r_px = (e.r * long).max(1.5);
        let (cx, cy) = (e.x * p.w as f32, e.y * p.h as f32);
        let (mut red, mut n, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0);
        let rr = r_px.ceil() as i32;
        for dy in -rr..=rr {
            for dx in -rr..=rr {
                if (dx * dx + dy * dy) as f32 > r_px * r_px {
                    continue;
                }
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if x < 0 || y < 0 || x >= p.w as i32 || y >= p.h as i32 {
                    continue;
                }
                n += 1.0;
                let q = p.at(x as usize, y as usize).map(|c| oetf(c.clamp(0.0, 1.0)));
                if is_red_eye_pixel(q) {
                    red += 1.0;
                    sx += dx as f32;
                    sy += dy as f32;
                }
            }
        }
        if n == 0.0 || red / n < 0.15 {
            continue;
        }
        // Os píxeis vermelhos formam a pupila: concentrados à volta do centro da íris.
        let (mx, my) = (sx / red, sy / red);
        if (mx * mx + my * my).sqrt() > 0.45 * r_px {
            continue;
        }
        out.push(RedEye { x: e.x, y: e.y, r: e.r * 1.25 });
    }
    out
}

/// Vermelho de flash na pupila (valores de visualização 0–1): vermelho dominante e saturado.
fn is_red_eye_pixel(q: [f32; 3]) -> bool {
    let m = q[1].max(q[2]);
    q[0] > 0.25 && q[0] > 2.0 * m && q[0] - m > 0.2
}

/// Manchas do sensor: pequenas zonas redondas, suavemente mais escuras, em áreas lisas e
/// claras (céu, paredes). Trabalha na razão entre a imagem e o seu fundo desfocado (anula
/// degradês); só aceita candidatas redondas com contorno homogéneo.
pub fn detect_dust(src: &Source) -> Vec<Spot> {
    let p = src.level_for(1600.0);
    let (w, h) = (p.w, p.h);
    if w < 64 || h < 64 {
        return Vec::new();
    }
    let lum: Vec<f32> = p.px.iter().map(|q| luma(*q)).collect();
    let fine = gaussian(&lum, w, h, 1.0);
    let bg = gaussian(&lum, w, h, 16.0);
    let q: Vec<f32> = fine.iter().zip(&bg).map(|(f, b)| f / b.max(1e-4)).collect();
    // Textura à volta (desvio médio da razão num raio grande): as manchas só se procuram
    // em zonas lisas, onde se veem.
    let dev: Vec<f32> = q.iter().map(|v| (v - 1.0).abs()).collect();
    let texture = gaussian(&dev, w, h, 24.0);
    let long = w.max(h) as f32;
    let at = |x: isize, y: isize| -> Option<f32> {
        (x >= 0 && y >= 0 && x < w as isize && y < h as isize)
            .then(|| q[y as usize * w + x as usize])
    };
    // Média e desvio de q numa coroa.
    let ring = |x: usize, y: usize, r0: f32, r1: f32| -> (f32, f32, usize) {
        let (mut s, mut s2, mut n) = (0.0f32, 0.0f32, 0usize);
        let rr = r1.ceil() as isize;
        for dy in -rr..=rr {
            for dx in -rr..=rr {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d < r0 || d > r1 {
                    continue;
                }
                if let Some(v) = at(x as isize + dx, y as isize + dy) {
                    s += v;
                    s2 += v * v;
                    n += 1;
                }
            }
        }
        if n == 0 {
            return (1.0, 1.0, 0);
        }
        let m = s / n as f32;
        (m, (s2 / n as f32 - m * m).max(0.0).sqrt(), n)
    };
    let mut cands: Vec<(usize, usize, f32)> = Vec::new();
    for y in 3..h - 3 {
        for x in 3..w - 3 {
            let i = y * w + x;
            let v = q[i];
            if !(0.5..=0.975).contains(&v) || bg[i] < 0.1 || texture[i] > 0.02 {
                continue;
            }
            let is_min = (-2..=2isize).all(|dy| {
                (-2..=2isize).all(|dx| {
                    let j = (y as isize + dy) as usize * w + (x as isize + dx) as usize;
                    j == i || q[j] > v || (q[j] == v && j > i)
                })
            });
            if is_min {
                cands.push((x, y, 1.0 - v));
            }
        }
    }
    cands.sort_by(|a, b| b.2.total_cmp(&a.2));
    let mut spots: Vec<(f32, f32, f32)> = Vec::new();
    for &(x, y, _) in cands.iter().take(5000) {
        if spots
            .iter()
            .any(|s| ((s.0 - x as f32).powi(2) + (s.1 - y as f32).powi(2)).sqrt() < s.2 * 3.0)
        {
            continue;
        }
        let center = q[y * w + x];
        let (far, _, _) = ring(x, y, 20.0, 24.0);
        // Raio: primeira coroa onde o escurecimento cai para 20 %.
        let mut r = 0.0;
        for k in 1..=18 {
            let (m, _, _) = ring(x, y, k as f32 - 0.5, k as f32 + 0.5);
            if far - m < (far - center) * 0.2 {
                r = k as f32;
                break;
            }
        }
        if !(1.5..=18.0).contains(&r) {
            continue;
        }
        let (am, asd, an) = ring(x, y, r * 1.5, r * 2.8);
        if an < 20 {
            continue;
        }
        let depth = 1.0 - center / am;
        if depth < 0.03 || asd > 0.012 || depth < 4.0 * asd {
            continue;
        }
        // Redonda: os quatro quadrantes escurecem de forma parecida.
        let quad = |sx: isize, sy: isize| {
            let (mut s, mut n) = (0.0f32, 0.0f32);
            let rr = (r * 0.6).max(1.0) as isize;
            for dy in 0..=rr {
                for dx in 0..=rr {
                    if let Some(v) = at(x as isize + sx * dx, y as isize + sy * dy) {
                        s += v;
                        n += 1.0;
                    }
                }
            }
            1.0 - s / n.max(1.0) / am
        };
        let qd = [quad(1, 1), quad(-1, 1), quad(1, -1), quad(-1, -1)];
        let (qmin, qmax) = qd.iter().fold((f32::MAX, f32::MIN), |a, &v| (a.0.min(v), a.1.max(v)));
        if qmin < qmax * 0.35 {
            continue;
        }
        spots.push((x as f32, y as f32, r));
        if spots.len() >= 25 {
            break;
        }
    }
    // Origem da clonagem: zona vizinha (8 direções) com o mesmo nível e sem textura.
    spots
        .into_iter()
        .map(|(x, y, r)| {
            let rr = r * 1.3 + 1.0;
            let mut best = (x + rr * 2.2, y, f32::MAX);
            for k in 0..8 {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                let (sx, sy) = (x + a.cos() * rr * 2.3, y + a.sin() * rr * 2.3);
                if sx < rr || sy < rr || sx >= w as f32 - rr || sy >= h as f32 - rr {
                    continue;
                }
                let (m, sd, _) = ring(sx as usize, sy as usize, 0.0, rr);
                let cost = (m - 1.0).abs() + sd * 3.0;
                if cost < best.2 {
                    best = (sx, sy, cost);
                }
            }
            Spot {
                x: x / w as f32,
                y: y / h as f32,
                r: rr / long,
                sx: best.0 / w as f32,
                sy: best.1 / h as f32,
            }
        })
        .collect()
}

/// Posição no enquadramento (0–1, depois da geometria) de um ponto da fotografia orientada,
/// invertendo numericamente o mapa da geometria (Newton).
fn forward(geo: &GeoMap, x: f32, y: f32) -> Option<(f32, f32)> {
    let (mut u, mut v) = (x, y);
    for _ in 0..12 {
        let p = geo.map(u, v).0[1];
        let (ex, ey) = (p.0 - x, p.1 - y);
        if ex.abs() < 1e-5 && ey.abs() < 1e-5 {
            break;
        }
        let h = 1e-3;
        let pu = geo.map(u + h, v).0[1];
        let pv = geo.map(u, v + h).0[1];
        let (a, b, c, d) = ((pu.0 - p.0) / h, (pv.0 - p.0) / h, (pu.1 - p.1) / h, (pv.1 - p.1) / h);
        let det = a * d - b * c;
        if det.abs() < 1e-9 {
            return None;
        }
        u -= (d * ex - b * ey) / det;
        v -= (-c * ex + a * ey) / det;
    }
    Some((u, v))
}

/// Recorte automático (opcional): só com pessoas. Mantém a proporção, nunca corta rostos nem
/// ombros, e procura pôr o olhar do rosto principal perto do terço superior.
fn auto_crop(
    src: &Source,
    g: &Geometry,
    lens: &Lens,
    faces: &[FaceBox],
    max_cut: f32,
) -> Option<Crop> {
    if faces.is_empty() || max_cut <= 0.0 {
        return None;
    }
    let geo = GeoMap::new(
        src.width(),
        src.height(),
        &Geometry { crop: Crop::default(), ..g.clone() },
        lens,
        false,
    );
    // Caixas no enquadramento (cabeça + ombros).
    let mut boxes = Vec::new();
    for f in faces {
        let corners = [(f.x, f.y), (f.x + f.w, f.y), (f.x, f.y + f.h), (f.x + f.w, f.y + f.h)];
        let mapped: Vec<(f32, f32)> =
            corners.iter().filter_map(|&(x, y)| forward(&geo, x, y)).collect();
        if mapped.len() < 4 {
            return None;
        }
        let (x0, x1) =
            mapped.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
        let (y0, y1) =
            mapped.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
        let (fw, fh) = (x1 - x0, y1 - y0);
        boxes.push((
            x0 - 0.7 * fw,
            y0 - 0.45 * fh,
            x1 + 0.7 * fw,
            y1 + 1.6 * fh,
            (x0 + x1) / 2.0,
            y0 + 0.4 * fh,
            fw * fh,
        ));
    }
    let main = boxes.iter().copied().max_by(|a, b| a.6.total_cmp(&b.6))?;
    let sx0 = boxes.iter().map(|b| b.0).fold(f32::MAX, f32::min).max(0.0);
    let sy0 = boxes.iter().map(|b| b.1).fold(f32::MAX, f32::min).max(0.0);
    let sx1 = boxes.iter().map(|b| b.2).fold(f32::MIN, f32::max).min(1.0);
    let sy1 = boxes.iter().map(|b| b.3).fold(f32::MIN, f32::max).min(1.0);
    let thirds = |t: f32, opts: &[f32]| opts.iter().map(|o| (t - o).abs()).fold(f32::MAX, f32::min);
    let score = |c: &Crop| {
        let ex = (main.4 - c.x) / c.w;
        let ey = (main.5 - c.y) / c.h;
        let hx = if boxes.len() == 1 {
            thirds(ex, &[1.0 / 3.0, 0.5, 2.0 / 3.0])
        } else {
            (ex - 0.5).abs() * 0.5
        };
        thirds(ey, &[1.0 / 3.0]) + hx + (1.0 - c.w) * 0.35
    };
    let full = Crop::default();
    let mut best = (full, score(&full));
    let mut s = 1.0;
    while s >= 1.0 - max_cut - 1e-4 {
        let steps = 20;
        for iy in 0..=steps {
            for ix in 0..=steps {
                let c = Crop {
                    x: (1.0 - s) * ix as f32 / steps as f32,
                    y: (1.0 - s) * iy as f32 / steps as f32,
                    w: s,
                    h: s,
                };
                if c.x > sx0 + 1e-4
                    || c.y > sy0 + 1e-4
                    || c.x + c.w < sx1 - 1e-4
                    || c.y + c.h < sy1 - 1e-4
                {
                    continue;
                }
                let sc = score(&c);
                if sc < best.1 {
                    best = (c, sc);
                }
            }
        }
        s -= 0.025;
    }
    (best.0 != full && score(&full) - best.1 > 0.02).then_some(best.0)
}

/// Medições de tom (domínio de visualização) para um ganho de exposição.
struct Tone {
    p005: f32,
    p995: f32,
    std: f32,
    above_90: f32,
    below_20: f32,
    crushed: f32,
    mean_sat: f32,
}

fn tone_stats(px: &[[f32; 3]], wb: &[[f32; 3]; 3], gain: f32) -> Tone {
    let mut lum = Vec::with_capacity(px.len());
    let mut sat = 0.0;
    for q in px {
        let c = mul3(wb, *q).map(|v| oetf((v * gain).clamp(0.0, 1.0)));
        let mx = c[0].max(c[1]).max(c[2]);
        let mn = c[0].min(c[1]).min(c[2]);
        if mx > 0.05 {
            sat += (mx - mn) / mx;
        }
        lum.push(luma(c));
    }
    let n = lum.len().max(1) as f32;
    let mean = lum.iter().sum::<f32>() / n;
    let std = (lum.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / n).sqrt();
    let frac = |f: &dyn Fn(f32) -> bool| lum.iter().filter(|&&l| f(l)).count() as f32 / n;
    let above_90 = frac(&|l| l > 0.9);
    let below_20 = frac(&|l| l < 0.2);
    let crushed = frac(&|l| l < 0.01);
    lum.sort_by(f32::total_cmp);
    Tone {
        p005: percentile(&lum, 0.005),
        p995: percentile(&lum, 0.995),
        std,
        above_90,
        below_20,
        crushed,
        mean_sat: sat / n,
    }
}

/// Edição automática de uma fotografia. `base` são os ajustes atuais: mantêm-se a rotação
/// de 90°, o espelho, a proporção, o HSL, a curva de pontos, a vinheta criativa e o grão.
pub fn auto_edit(
    src: &Source,
    base: &Settings,
    ctx: &AutoContext,
    o: &AutoOptions,
) -> (Settings, AutoReport) {
    let (k, creative) = o.intensity.profile(o.strength);
    let on = |flag: bool| flag;
    let mut s = base.clone();
    let small = small_plane(src, 640.0);
    let px = &small.px;

    // Balanço de brancos (correção parcial; mais contida quando a estimativa é extrema).
    let (mut temp, mut tint) = (0.0, 0.0);
    if on(o.white_balance) {
        if let Some(ill) = estimate_illuminant(small) {
            let (t, n) = neutralize(ill);
            // A matiz (verde/magenta) raramente é a causa: corrige-se menos.
            let n = n * 0.6;
            let conf = if t.abs() > 60.0 || n.abs() > 40.0 { 0.5 } else { 1.0 };
            // Arrefecer menos do que aquecer: preserva a luz quente de interiores.
            let warm_keep = if t < 0.0 { 0.7 } else { 1.0 };
            temp = (t * 0.75 * conf * warm_keep * k.min(1.0)).clamp(-60.0, 60.0);
            tint = (n * 0.75 * conf * k.min(1.0)).clamp(-25.0, 25.0);
        }
        s.wb = WhiteBalance { temperature: round1(temp), tint: round1(tint) };
    }
    let wbm = wb_matrix(s.wb.temperature, s.wb.tint);

    // Exposição: chave da cena (parcial, mantém o ambiente) e rostos.
    let lum_lin: Vec<f32> = px.iter().map(|q| luma(mul3(&wbm, *q))).collect();
    let mut sorted = lum_lin.clone();
    sorted.sort_by(f32::total_cmp);
    let median = percentile(&sorted, 0.5).max(1e-4);
    let p995 = percentile(&sorted, 0.995).max(1e-4);
    let mut ev: f32;
    let faces = &ctx.faces;
    if on(o.tone) {
        // Desvios pequenos podem ser intencionais (ambiente): conserva-se até 0,4 EV deles;
        // os grandes são erros de exposição e corrigem-se quase por completo.
        let dev = (0.17 / median).log2();
        let ev_scene = dev.signum() * (dev.abs() - 0.4 * dev.abs().min(1.0));
        let face_lum = face_mean(small, &wbm, faces);
        ev = ev_scene;
        if let Some(fl) = face_lum {
            let ev_face = (0.2 / fl.max(1e-4)).log2();
            ev = if ev_face > ev_scene {
                ev_scene + ((ev_face - ev_scene) * 0.5).min(1.0)
            } else {
                ev_scene + (ev_face - ev_scene) * 0.3
            };
        }
        // Não queimar mais realces do que o original.
        if ev > 0.0 {
            ev = ev.min((0.98 / p995).log2().max(0.0) + 0.6);
        }
        if percentile(&sorted, 0.95) < 0.05 {
            ev = ev.min(1.5);
        }
        // A intensidade só modera o primeiro EV (gosto); erros grandes corrigem-se sempre.
        ev = (ev.signum() * (ev.abs().min(1.0) * k + (ev.abs() - 1.0).max(0.0))).clamp(-2.0, 2.5);
        s.light.exposure = round2(ev);
    }
    let gain = 2f32.powf(s.light.exposure);
    let t = tone_stats(px, &wbm, gain);
    if on(o.tone) {
        let l = &mut s.light;
        l.highlights = if t.above_90 > 0.02 || t.p995 > 0.97 {
            -((t.above_90 - 0.02).max(0.0) * 400.0 + 20.0).min(70.0)
        } else {
            0.0
        };
        l.shadows = if t.below_20 > 0.25 { ((t.below_20 - 0.2) * 120.0).min(50.0) } else { 0.0 };
        if let Some(fl) = face_mean(small, &wbm, faces) {
            if oetf((fl * gain).min(1.0)) < 0.35 {
                l.shadows = (l.shadows + 15.0).min(60.0);
            }
        }
        l.whites = if t.p995 < 0.85 {
            ((0.95 - t.p995) * 150.0).min(40.0)
        } else if t.above_90 > 0.03 && t.p995 > 0.985 {
            -10.0
        } else {
            0.0
        };
        l.blacks = if t.p005 > 0.07 {
            -((t.p005 - 0.03) * 250.0).min(40.0)
        } else if t.crushed > 0.02 {
            (t.crushed * 300.0).min(25.0)
        } else {
            0.0
        };
        // Contraste: sobe nas imagens baças; só desce nas muito duras.
        let ct = if t.std < 0.24 {
            ((0.24 - t.std) * 150.0).min(30.0)
        } else if t.std > 0.31 {
            -((t.std - 0.31) * 100.0).min(15.0)
        } else {
            0.0
        };
        l.contrast = ct + creative * 0.6;
        for v in [&mut l.highlights, &mut l.shadows, &mut l.whites, &mut l.blacks, &mut l.contrast]
        {
            *v = round1(*v * k);
        }
        // Curva: S suave (mais quando a imagem é baça).
        let soft = ((0.24 - t.std) * 60.0).clamp(0.0, 8.0) + creative * 0.4;
        s.curve.lights = round1(soft * k);
        s.curve.darks = round1(-soft * k);
        s.curve.highlights = 0.0;
        s.curve.shadows = 0.0;
        // Névoa / baço: só quando os pretos estão muito levantados e a cor apagada.
        s.effects.dehaze = if t.p005 > 0.12 && t.mean_sat < 0.15 {
            round1(((t.p005 - 0.08) * 150.0).min(25.0) * k)
        } else {
            0.0
        };
        // Clareza: nunca positiva em retratos de perto (pele).
        let big_face = faces.iter().any(|f| f.w * f.h > 0.05);
        s.effects.clarity = if big_face { 0.0 } else { round1(creative * 0.8) };
    }
    if on(o.color) {
        let vib = ((0.30 - t.mean_sat) * 120.0).clamp(-10.0, 30.0) + creative;
        s.color.vibrance = round1(vib * k);
        s.color.saturation =
            if t.mean_sat > 0.45 { round1(-5.0 * k) } else { round1(creative * 0.3) };
    }

    // Ruído (medido à resolução da fonte, depois da exposição) + ISO e sensor.
    let factor = sensor_factor(ctx.make.as_deref(), ctx.model.as_deref());
    let (nl, nc) = measure_noise(src, gain);
    let noise = classify_noise(nl, nc, ctx.iso, factor);
    let eff_iso = ctx.iso.map(|i| i as f32 * factor);
    if on(o.denoise) {
        let mut lum_nr = ((nl - 1.2) * 10.0).clamp(0.0, 70.0);
        let cap = match eff_iso {
            Some(e) if e <= 400.0 => 12.0,
            Some(e) if e <= 1600.0 => 35.0,
            Some(_) => 70.0,
            None => 50.0,
        };
        lum_nr = lum_nr.min(cap);
        if noise.level == NoiseLevel::None {
            lum_nr = 0.0;
        }
        let mut col_nr = ((nc - 0.8) * 12.0).clamp(0.0, 60.0);
        if ctx.is_raw && eff_iso.is_some_and(|e| e >= 800.0) {
            col_nr = col_nr.max(15.0);
        }
        let use_ai = match o.ai_denoise {
            AiDenoise::Off => false,
            AiDenoise::WhenNeeded => noise.ai_recommended,
            AiDenoise::Always => noise.level != NoiseLevel::None,
        };
        let d = &mut s.detail;
        if use_ai {
            d.ai_denoise = round1((20.0 + nl * 6.0).clamp(25.0, 80.0));
            lum_nr *= 0.25;
            col_nr *= 0.5;
        } else {
            d.ai_denoise = 0.0;
        }
        d.noise_luminance = round1(lum_nr);
        d.noise_color = round1(col_nr);
        d.noise_detail = if nl < 3.0 { 65.0 } else { 55.0 };
    }
    if on(o.sharpen) {
        let d = &mut s.detail;
        let (amount, radius, masking): (f32, f32, f32) =
            if ctx.is_raw { (40.0, 1.0, 25.0) } else { (15.0, 0.8, 30.0) };
        let noisy = matches!(noise.level, NoiseLevel::High | NoiseLevel::VeryHigh);
        let portrait = faces.iter().any(|f| f.w * f.h > 0.03);
        d.sharpen_amount = round1(amount * k * if noisy { 0.7 } else { 1.0 });
        d.sharpen_radius = radius;
        d.sharpen_detail = 25.0;
        d.sharpen_masking =
            (masking + if noisy { 30.0 } else { 0.0 } + if portrait { 15.0 } else { 0.0 })
                .min(90.0);
    }

    // Objetiva (sem perfis): aberração cromática medida; vinheta só em RAW.
    if on(o.lens) {
        let (cr, cb) = estimate_chromatic_aberration(src);
        s.lens.ca_red = round1(cr);
        s.lens.ca_blue = round1(cb);
        s.lens.vignetting = if ctx.is_raw { round1(estimate_lens_vignetting(small)) } else { 0.0 };
    }

    // Horizonte: só inclinações pequenas (as grandes costumam ser intencionais).
    let mut tilt = None;
    if on(o.straighten) {
        let probe =
            Settings { geometry: Geometry { straighten: 0.0, ..s.geometry.clone() }, ..s.clone() };
        tilt = super::auto_straighten(src, &probe);
        s.geometry.straighten = match tilt {
            Some(a) if (0.25..=10.0).contains(&a.abs()) => round2(a),
            _ => 0.0,
        };
    }
    // Recorte automático (opcional, por omissão desligado).
    let mut cropped = false;
    if o.crop {
        let max_cut = (0.15 * k).clamp(0.05, 0.25);
        let g = Geometry { crop: Crop::default(), ..s.geometry.clone() };
        if let Some(c) = auto_crop(src, &g, &s.lens, faces, max_cut) {
            s.geometry.crop = c;
            cropped = true;
        } else {
            s.geometry.crop = Crop::default();
        }
    }

    // Retoque.
    if o.red_eye {
        s.retouch.red_eyes = detect_red_eyes(src, &ctx.eyes);
    }
    if o.dust {
        s.retouch.spots = detect_dust(src);
    }
    let report = AutoReport {
        exposure: s.light.exposure,
        temperature: s.wb.temperature,
        tint: s.wb.tint,
        noise,
        tilt,
        cropped,
        red_eyes: s.retouch.red_eyes.len(),
        spots: s.retouch.spots.len(),
        faces: faces.len(),
    };
    (s, report)
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}
fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// Luminância linear média do interior dos rostos (ponderada pela área).
fn face_mean(p: &Plane, wb: &[[f32; 3]; 3], faces: &[FaceBox]) -> Option<f32> {
    let (mut s, mut n) = (0.0, 0.0);
    for f in faces {
        let (x0, x1) = ((f.x + f.w * 0.2) * p.w as f32, (f.x + f.w * 0.8) * p.w as f32);
        let (y0, y1) = ((f.y + f.h * 0.25) * p.h as f32, (f.y + f.h * 0.85) * p.h as f32);
        for y in (y0.max(0.0) as usize)..(y1.min(p.h as f32) as usize) {
            for x in (x0.max(0.0) as usize)..(x1.min(p.w as f32) as usize) {
                s += luma(mul3(wb, p.at(x, y)));
                n += 1.0;
            }
        }
    }
    (n >= 4.0).then(|| s / n)
}

#[cfg(test)]
mod tests;
