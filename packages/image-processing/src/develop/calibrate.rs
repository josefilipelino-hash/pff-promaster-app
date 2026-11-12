//! Calibração de cor de câmaras que a LibRaw ainda não conhece (sem matriz câmara→sRGB,
//! ex.: Canon EOS C50 na LibRaw 0.22). Sem matriz, os valores da câmara são tratados como
//! Rec. 2020 e as cores saem com uma dominante forte (magenta/verde).
//!
//! A câmara grava dentro do RAW uma preview JPEG com as cores certas. Comparando-a com os
//! dados do sensor (já com o balanço de brancos da câmara) estima-se uma matriz 3×3
//! câmara → Rec. 2020 linear:
//! - por cromaticidade (cada par normalizado pela sua soma), para não depender da curva de
//!   tons da câmara;
//! - matriz completa: também corrige o balanço de brancos (sem perfil, a LibRaw não aplica
//!   o da câmara e os dados chegam com dominante verde do sensor);
//! - só com píxeis de meios-tons (sem sombras com ruído nem realces cortados na preview).

use super::color::{eotf, mul3, SRGB_TO_REC2020};
use super::source::Plane;
use image::RgbImage;

/// Matriz estimada e qualidade do ajuste.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibration {
    pub matrix: [[f32; 3]; 3],
    /// Pares de píxeis usados.
    pub samples: usize,
    /// Erro médio de cromaticidade antes e depois (0–1, menor é melhor).
    pub error_before: f32,
    pub error_after: f32,
    /// Correlação (0–1) entre a luminância do RAW e a da preview no enquadramento escolhido:
    /// mede se os píxeis comparados são mesmo os mesmos pontos da imagem.
    pub alignment: f32,
    /// Enquadramento usado (para diagnóstico).
    pub framing: &'static str,
}

fn norm(v: [f32; 3]) -> Option<[f32; 3]> {
    let s = v[0] + v[1] + v[2];
    (s > 1e-6 && v.iter().all(|c| c.is_finite())).then(|| v.map(|c| c / s))
}

/// Correlação mínima para confiar no alinhamento RAW ↔ preview.
const MIN_ALIGNMENT: f32 = 0.85;

/// Estima a matriz que leva `camera` (linear, já orientado) às cores de `preview` (sRGB 8
/// bits da mesma fotografia). A preview pode ter barras pretas ou outro enquadramento (ex.:
/// câmaras de cinema): retiram-se as barras e escolhe-se, entre os enquadramentos possíveis,
/// o que melhor coincide com o RAW. `None` se nenhum coincidir bem, se houver poucos píxeis
/// utilizáveis ou se o ajuste não melhorar nada.
pub fn calibrate_from_preview(camera: &Plane, preview: &RgbImage) -> Option<Calibration> {
    if camera.w < 16 || camera.h < 16 {
        return None;
    }
    let preview = trim_black_borders(preview);
    let rotated = image::imageops::rotate180(&preview);
    let mut candidates: Vec<(&'static str, Plane, &RgbImage)> =
        vec![("inteiro", camera.clone(), &preview)];
    let (ac, ap) = (
        camera.w as f32 / camera.h as f32,
        preview.width() as f32 / preview.height().max(1) as f32,
    );
    if (ac / ap - 1.0).abs() > 0.005 {
        candidates.push(("recortado à proporção da preview", crop_to_aspect(camera, ap), &preview));
    }
    let n = candidates.len();
    for i in 0..n {
        let (name, plane, _) = &candidates[i];
        let label = if *name == "inteiro" {
            "inteiro, preview rodada 180°"
        } else {
            "recortado, preview rodada 180°"
        };
        candidates.push((label, plane.clone(), &rotated));
    }
    let (framing, plane, pv, alignment) = candidates
        .into_iter()
        .map(|(f, p, pv)| {
            let a = luminance_correlation(&p, pv);
            (f, p, pv, a)
        })
        .max_by(|a, b| a.3.total_cmp(&b.3))?;
    if alignment < MIN_ALIGNMENT {
        return None;
    }
    fit(&plane, pv).map(|c| Calibration { alignment, framing, ..c })
}

/// Retira margens pretas (barras) da preview.
fn trim_black_borders(img: &RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let dark_row =
        |y: u32| (0..w).step_by(4).all(|x| img.get_pixel(x, y).0.iter().all(|&c| c < 10));
    let dark_col =
        |x: u32| (0..h).step_by(4).all(|y| img.get_pixel(x, y).0.iter().all(|&c| c < 10));
    let (mut top, mut bottom, mut left, mut right) = (0, h, 0, w);
    while top < h / 3 && dark_row(top) {
        top += 1;
    }
    while bottom > 2 * h / 3 && dark_row(bottom - 1) {
        bottom -= 1;
    }
    while left < w / 3 && dark_col(left) {
        left += 1;
    }
    while right > 2 * w / 3 && dark_col(right - 1) {
        right -= 1;
    }
    if (top, bottom, left, right) == (0, h, 0, w) {
        return img.clone();
    }
    image::imageops::crop_imm(img, left, top, right - left, bottom - top).to_image()
}

/// Recorte central do plano para a proporção `aspect` (largura/altura).
fn crop_to_aspect(p: &Plane, aspect: f32) -> Plane {
    let (w, h) = (p.w as f32, p.h as f32);
    let (cw, ch) = if w / h > aspect {
        ((h * aspect).round() as usize, p.h)
    } else {
        (p.w, (w / aspect).round() as usize)
    };
    let (x0, y0) = ((p.w - cw.min(p.w)) / 2, (p.h - ch.min(p.h)) / 2);
    let mut px = Vec::with_capacity(cw * ch);
    for y in y0..y0 + ch.min(p.h) {
        px.extend_from_slice(&p.px[y * p.w + x0..y * p.w + x0 + cw.min(p.w)]);
    }
    Plane { w: cw.min(p.w), h: ch.min(p.h), px }
}

/// Correlação de Pearson entre log-luminâncias do plano e da preview, numa grelha pequena.
fn luminance_correlation(p: &Plane, preview: &RgbImage) -> f32 {
    const GW: usize = 64;
    let gh = ((GW as f32 * p.h as f32 / p.w as f32).round() as usize).clamp(8, 128);
    let pv = image::imageops::resize(
        preview,
        GW as u32,
        gh as u32,
        image::imageops::FilterType::Triangle,
    );
    let lum = |v: [f32; 3]| 0.2627 * v[0] + 0.6780 * v[1] + 0.0593 * v[2];
    let (mut a, mut b) = (Vec::with_capacity(GW * gh), Vec::with_capacity(GW * gh));
    for gy in 0..gh {
        for gx in 0..GW {
            // Média do bloco correspondente no plano.
            let (x0, x1) = (gx * p.w / GW, ((gx + 1) * p.w / GW).max(gx * p.w / GW + 1));
            let (y0, y1) = (gy * p.h / gh, ((gy + 1) * p.h / gh).max(gy * p.h / gh + 1));
            let (mut s, mut n) = (0f32, 0f32);
            for y in (y0..y1.min(p.h)).step_by(2) {
                for x in (x0..x1.min(p.w)).step_by(2) {
                    s += lum(p.px[y * p.w + x]);
                    n += 1.0;
                }
            }
            let q = pv.get_pixel(gx as u32, gy as u32).0.map(|c| eotf(c as f32 / 255.0));
            a.push((s / n.max(1.0) + 1e-4).ln());
            b.push((lum(q) + 1e-4).ln());
        }
    }
    let n = a.len() as f32;
    let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
    let (mut sab, mut saa, mut sbb) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(&b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    if saa <= 0.0 || sbb <= 0.0 {
        return 0.0;
    }
    sab / (saa.sqrt() * sbb.sqrt())
}

/// Ajuste com a preview já no mesmo enquadramento que `camera`.
fn fit(camera: &Plane, preview: &RgbImage) -> Option<Calibration> {
    let pv = image::imageops::resize(
        preview,
        camera.w as u32,
        camera.h as u32,
        image::imageops::FilterType::Triangle,
    );
    // Pares (câmara, alvo) normalizados.
    let mut pairs: Vec<([f32; 3], [f32; 3])> = Vec::new();
    let mut raws: Vec<[f32; 3]> = Vec::new();
    let step = ((camera.w * camera.h) / 20_000).max(1);
    let max_cam =
        camera.px.iter().step_by(step).map(|p| p[0].max(p[1]).max(p[2])).fold(0f32, f32::max);
    for (i, p) in camera.px.iter().enumerate().step_by(step) {
        let (x, y) = ((i % camera.w) as u32, (i / camera.w) as u32);
        let q = pv.get_pixel(x, y).0;
        // Meios-tons: nem sombras (ruído, curva) nem realces (cortados ou comprimidos).
        if q.iter().any(|&c| !(12..=242).contains(&c)) || p.iter().any(|&c| c >= 0.98 * max_cam) {
            continue;
        }
        let target = mul3(&SRGB_TO_REC2020, q.map(|c| eotf(c as f32 / 255.0)));
        if let (Some(a), Some(b)) = (norm(*p), norm(target)) {
            pairs.push((a, b));
            raws.push(*p);
        }
    }
    if pairs.len() < 200 {
        return None;
    }
    // Matriz completa (9 valores), por mínimos quadrados em cromaticidade: corrige também o
    // balanço de brancos, porque sem perfil a LibRaw também não aplica o da câmara (os dados
    // chegam com o verde do sensor muito mais forte — dominante verde/oliva).
    let mut a = [[0f64; 3]; 3];
    let mut b = [[0f64; 3]; 3];
    for (x, y) in &pairs {
        for r in 0..3 {
            for c in 0..3 {
                a[r][c] += (x[r] * x[c]) as f64;
                b[r][c] += (x[r] * y[c]) as f64;
            }
        }
    }
    let inv = invert3(a)?;
    let mut m = [[0f32; 3]; 3];
    for (out, row) in m.iter_mut().enumerate() {
        for (k, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|r| inv[k][r] * b[r][out]).sum::<f64>() as f32;
        }
    }
    // Mesma luminância média (a exposição base já foi calculada sobre os dados da câmara).
    let lum = |v: [f32; 3]| 0.2627 * v[0] + 0.6780 * v[1] + 0.0593 * v[2];
    let (l0, l1) = raws
        .iter()
        .fold((0f64, 0f64), |(a0, a1), p| (a0 + lum(*p) as f64, a1 + lum(mul3(&m, *p)) as f64));
    if l1.is_nan() || l1 <= 0.0 {
        return None;
    }
    let k = (l0 / l1) as f32;
    for v in m.iter_mut().flatten() {
        *v *= k;
    }
    if m.iter().flatten().any(|v| !v.is_finite() || v.abs() > 8.0) {
        return None;
    }
    let err = |f: &dyn Fn([f32; 3]) -> [f32; 3]| {
        pairs
            .iter()
            .map(|(x, y)| {
                let z = norm(f(*x)).unwrap_or(*x);
                (0..3).map(|i| (z[i] - y[i]).abs()).sum::<f32>()
            })
            .sum::<f32>()
            / pairs.len() as f32
    };
    let before = err(&|x| x);
    let after = err(&|x| mul3(&m, x));
    (after < before).then_some(Calibration {
        matrix: m,
        samples: pairs.len(),
        error_before: before,
        error_after: after,
        alignment: 1.0,
        framing: "",
    })
}

/// Inversa de uma matriz 3×3 (`None` se for singular).
fn invert3(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-18 {
        return None;
    }
    let c =
        |r0: usize, c0: usize, r1: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    Some([
        [c(1, 1, 2, 2) / det, -c(0, 1, 2, 2) / det, c(0, 1, 1, 2) / det],
        [-c(1, 0, 2, 2) / det, c(0, 0, 2, 2) / det, -c(0, 0, 1, 2) / det],
        [c(1, 0, 2, 1) / det, -c(0, 0, 2, 1) / det, c(0, 0, 1, 1) / det],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::color::{oetf, REC2020_TO_SRGB};

    /// Uma "câmara" desconhecida (matriz qualquer) e a preview que ela própria gravaria (com
    /// curva de tons): a calibração recupera as cores verdadeiras.
    #[test]
    fn recovers_the_colors_of_an_unknown_camera() {
        // Cena com cores variadas em Rec. 2020 linear (dentro do sRGB).
        let (w, h) = (160usize, 120usize);
        // Zonas de cor (blocos de 10×10) com luz variada — estrutura como numa fotografia.
        let mut s = 0x9e37_79b9u32;
        let mut rnd = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s % 1000) as f32 / 1000.0
        };
        let blocks: Vec<[f32; 3]> = (0..(w / 10) * (h / 10))
            .map(|_| {
                let l = 0.15 + 0.7 * rnd();
                [l * (0.3 + 0.7 * rnd()), l * (0.3 + 0.7 * rnd()), l * (0.3 + 0.7 * rnd())]
            })
            .collect();
        let truth: Vec<[f32; 3]> = (0..w * h)
            .map(|i| mul3(&SRGB_TO_REC2020, blocks[(i / w / 10) * (w / 10) + (i % w) / 10]))
            .collect();
        // Câmara desconhecida: mistura os canais E sem balanço de brancos (verde do sensor
        // ~2× mais forte, azul fraco — como os dados que a LibRaw dá sem perfil).
        let cam_from_truth = [[0.40, 0.15, 0.03], [0.30, 1.45, 0.20], [0.03, 0.18, 0.45]];
        let camera = Plane { w, h, px: truth.iter().map(|t| mul3(&cam_from_truth, *t)).collect() };
        // Preview da câmara: cores verdadeiras com uma curva de tons e sRGB 8 bits.
        let preview = RgbImage::from_fn(w as u32, h as u32, |x, y| {
            let t = mul3(&REC2020_TO_SRGB, truth[y as usize * w + x as usize]);
            image::Rgb(
                t.map(|c| (oetf((c * 1.15).powf(0.92).clamp(0.0, 1.0)) * 255.0).round() as u8),
            )
        });
        let cal = calibrate_from_preview(&camera, &preview).expect("calibra");
        assert!(cal.error_after < cal.error_before * 0.35, "{cal:?}");
        // O cinzento da cena (que a câmara vê esverdeado) volta a ser cinzento.
        let g = norm(mul3(&cal.matrix, mul3(&cam_from_truth, [0.3, 0.3, 0.3]))).unwrap();
        assert!(g.iter().all(|c| (c - 1.0 / 3.0).abs() < 0.02), "{g:?}");
        // Cores recuperadas (cromaticidade).
        // (A curva de tons por canal da preview muda um pouco a cor por si só: nenhuma matriz
        // a desfaz; compara-se com o erro sem calibração.)
        let (mut worst, mut worst_raw) = (0f32, 0f32);
        for (c, t) in camera.px.iter().zip(&truth).step_by(97) {
            let b = norm(*t).unwrap();
            let d = |z: [f32; 3]| (0..3).map(|i| (z[i] - b[i]).abs()).fold(0.0, f32::max);
            worst = worst.max(d(norm(mul3(&cal.matrix, *c)).unwrap()));
            worst_raw = worst_raw.max(d(norm(*c).unwrap()));
        }
        assert!(
            worst < 0.08 && worst < worst_raw * 0.3,
            "pior erro {worst} (sem calibração {worst_raw})"
        );
    }

    #[test]
    fn refuses_without_enough_usable_pixels() {
        let camera = Plane { w: 32, h: 32, px: vec![[0.5, 0.5, 0.5]; 32 * 32] };
        let black = RgbImage::new(32, 32);
        assert!(calibrate_from_preview(&camera, &black).is_none());
    }
}
