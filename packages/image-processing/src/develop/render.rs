//! Render da edição.
//!
//! 1. **Guia** (lado maior fixo, [`GUIDE_EDGE`]): a mesma imagem a baixa resolução, usada
//!    pelas operações de grande raio — sombras/realces locais, clareza e dehaze. Como tem
//!    sempre o mesmo tamanho, o render rápido e o final dão o mesmo aspeto.
//! 2. **Por píxel** (paralelo por linhas): geometria + objetiva (uma reamostragem) →
//!    vinheta da objetiva → balanço de brancos + exposição → dehaze → sombras/realces →
//!    domínio de visualização → curvas (pretos/brancos/contraste/paramétrica/pontos) →
//!    clareza → vibração/saturação/HSL → curvas por canal.
//! 3. **Filtros espaciais** à resolução do render: textura, redução de ruído, nitidez.
//! 4. **Saída**: vinheta, grão → sRGB 8 bits (RGBA) + histograma.

use super::color::{eotf, luma, matmul, mul3, oetf, wb_matrix, Transfer, REC2020_TO_SRGB};
use super::curves::CurveLuts;
use super::filters::{gaussian, min_filter, sample_plane};
use super::geometry::{output_size, GeoMap};
use super::settings::{Settings, HSL_CENTERS};
use super::source::Source;
use rayon::prelude::*;
use std::sync::OnceLock;

/// Lado maior da guia de baixa resolução.
pub const GUIDE_EDGE: u32 = 384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    /// Durante a interação: amostragem do nível da pirâmide mais próximo, sem redução de ruído.
    Fast,
    /// Depois de parar: amostragem com sobreamostragem e todos os filtros.
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    pub max_width: u32,
    pub max_height: u32,
    pub quality: Quality,
    /// Mostra a imagem toda (sem recorte) — usado pela ferramenta de recorte.
    pub uncropped: bool,
}

/// Resultado: RGBA 8 bits sRGB + histograma RGB (256 classes por canal).
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub histogram: [[u32; 256]; 3],
}

fn transfer() -> &'static Transfer {
    static T: OnceLock<Transfer> = OnceLock::new();
    T.get_or_init(Transfer::new)
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Parâmetros pré-calculados para o passo por píxel.
struct Prep<'a> {
    s: &'a Settings,
    geo: GeoMap,
    /// Balanço de brancos + exposição (Rec. 2020 linear).
    wbx: [[f32; 3]; 3],
    curves: CurveLuts,
    hsl_on: bool,
    color_on: bool,
    /// Tamanho de um píxel de saída em coordenadas normalizadas (sobreamostragem).
    du: f32,
    dv: f32,
    /// Retoque em unidades do lado maior: escala de u e v.
    ru: f32,
    rv: f32,
    /// Tonalização: vetores de cor (sem luminância) das sombras e dos realces, já escalados.
    grade: Option<([f32; 3], [f32; 3], f32)>,
    lut: Option<(std::sync::Arc<super::lut::Lut>, f32)>,
    /// Correção do "heal" por mancha (razão destino/origem por canal).
    heal: Vec<[f32; 3]>,
    /// Remoção de objetos: zona rasterizada, razão do heal, parâmetros.
    patches: Vec<(std::sync::Arc<super::local::Raster>, [f32; 3], super::settings::Patch)>,
}

/// Média de um plano numa coroa (ou zona) definida por `inside(u, v)`, deslocada (ox, oy).
fn ring_mean(src: &Source, pts: &[(f32, f32)], ox: f32, oy: f32) -> Option<[f32; 3]> {
    let pl = src.level_for(1024.0);
    let (mut acc, mut n) = ([0.0f32; 3], 0.0);
    for &(u, v) in pts {
        if let Some(q) = pl.sample(u + ox, v + oy) {
            for c in 0..3 {
                acc[c] += q[c];
            }
            n += 1.0;
        }
    }
    (n > 0.0).then(|| acc.map(|a| a / n))
}

/// Razão de correção do heal: luz e cor à volta do destino ÷ à volta da origem.
fn heal_ratio(src: &Source, ring: &[(f32, f32)], ox: f32, oy: f32) -> [f32; 3] {
    match (ring_mean(src, ring, 0.0, 0.0), ring_mean(src, ring, ox, oy)) {
        (Some(d), Some(o)) => std::array::from_fn(|c| (d[c] / o[c].max(1e-5)).clamp(0.05, 20.0)),
        _ => [1.0; 3],
    }
}

/// Cor pura de um matiz sem a sua luminância (para tonalizar sem mudar o brilho).
fn chroma_of(hue: f32) -> [f32; 3] {
    let c = hsv_to_rgb(hue, 1.0, 1.0);
    let l = luma(c);
    c.map(|v| v - l)
}

impl<'a> Prep<'a> {
    fn new(src: &Source, s: &'a Settings, uncropped: bool) -> Self {
        let gain = 2f32.powf(s.light.exposure.clamp(-5.0, 5.0));
        let mut wbx = wb_matrix(s.wb.temperature, s.wb.tint);
        for row in &mut wbx {
            for v in row.iter_mut() {
                *v *= gain;
            }
        }
        let h = &s.hsl;
        let long = src.width().max(src.height()) as f32;
        let g = &s.grading;
        let grade = (g.shadows_sat > 0.0 || g.highlights_sat > 0.0).then(|| {
            (
                chroma_of(g.shadows_hue).map(|v| v * g.shadows_sat / 100.0 * 0.3),
                chroma_of(g.highlights_hue).map(|v| v * g.highlights_sat / 100.0 * 0.3),
                g.balance.clamp(-100.0, 100.0) / 100.0,
            )
        });
        let lut = (s.lut.amount > 0.0)
            .then(|| super::lut::get(&s.lut.id))
            .flatten()
            .map(|l| (l, s.lut.amount.clamp(0.0, 100.0) / 100.0));
        // Heal: coroa à volta de cada mancha (1,1–1,5 × raio).
        let heal = s
            .retouch
            .spots
            .iter()
            .map(|sp| {
                if !sp.heal {
                    return [1.0; 3];
                }
                let ring: Vec<(f32, f32)> = (0..24)
                    .flat_map(|k| {
                        let a = k as f32 / 24.0 * std::f32::consts::TAU;
                        [1.15f32, 1.45].map(|f| {
                            (
                                sp.x + a.cos() * sp.r * f / (src.width() as f32 / long),
                                sp.y + a.sin() * sp.r * f / (src.height() as f32 / long),
                            )
                        })
                    })
                    .collect();
                heal_ratio(src, &ring, sp.sx - sp.x, sp.sy - sp.y)
            })
            .collect();
        let (pru, prv) = (src.width() as f32 / long, src.height() as f32 / long);
        let patches = s
            .retouch
            .patches
            .iter()
            .filter(|pt| !pt.stroke.points.is_empty() && pt.opacity > 0.0)
            .map(|pt| {
                let key = super::local::hash_of(&pt.stroke, &format!("patch{pru}{prv}"));
                let r = super::local::cached(key, || {
                    super::local::rasterize_strokes(
                        std::slice::from_ref(&pt.stroke),
                        super::local::Raster::full(pru / prv),
                        pru,
                        prv,
                    )
                });
                // Coroa: células na transição da borda.
                let ring: Vec<(f32, f32)> = if pt.heal {
                    let mut v = Vec::new();
                    for j in (0..r.h).step_by(2) {
                        for i in (0..r.w).step_by(2) {
                            let m = r.data[j * r.w + i];
                            if (0.02..0.35).contains(&m) {
                                v.push(r.cell_uv(i, j));
                            }
                        }
                    }
                    v
                } else {
                    vec![]
                };
                let ratio = if pt.heal { heal_ratio(src, &ring, pt.dx, pt.dy) } else { [1.0; 3] };
                (r, ratio, pt.clone())
            })
            .collect();
        Prep {
            heal,
            patches,
            grade,
            lut,
            ru: src.width() as f32 / long,
            rv: src.height() as f32 / long,
            s,
            geo: GeoMap::new(src.width(), src.height(), &s.geometry, &s.lens, uncropped),
            wbx,
            curves: CurveLuts::new(&s.light, &s.curve),
            hsl_on: h.hue.iter().chain(&h.saturation).chain(&h.luminance).any(|v| *v != 0.0),
            color_on: s.color.saturation != 0.0 || s.color.vibrance != 0.0,
            du: 0.0,
            dv: 0.0,
        }
    }

    /// Amostra o original (com geometria, aberração cromática e vinheta da objetiva) e
    /// aplica balanço de brancos + exposição. `None` fora da imagem.
    #[inline]
    fn linear(&self, src: &super::source::Plane, u: f32, v: f32, ss: u32) -> Option<[f32; 3]> {
        let mut acc = [0.0f32; 3];
        let mut n = 0.0;
        let step = 1.0 / ss as f32;
        for sy in 0..ss {
            for sx in 0..ss {
                let (du, dv) = ((sx as f32 + 0.5) * step - 0.5, (sy as f32 + 0.5) * step - 0.5);
                let (pts, r) = self.geo.map(u + du * self.du, v + dv * self.dv);
                let g = src.sample(pts[1].0, pts[1].1)?;
                let rr =
                    if pts[0] == pts[1] { g[0] } else { src.sample_channel(pts[0].0, pts[0].1, 0) };
                let bb =
                    if pts[2] == pts[1] { g[2] } else { src.sample_channel(pts[2].0, pts[2].1, 2) };
                let mut p = [rr, g[1], bb];
                if !self.s.retouch.spots.is_empty()
                    || !self.s.retouch.red_eyes.is_empty()
                    || !self.patches.is_empty()
                {
                    p = self.retouch(src, &pts, p);
                }
                let lv = self.s.lens.vignetting;
                if lv != 0.0 {
                    let m = 0.5 + (50.0 - self.s.lens.vignetting_midpoint) / 100.0;
                    let k = 1.0 + lv / 100.0 * 1.2 * (r / m.max(0.2)).powi(2).min(4.0) * 0.5;
                    p = [p[0] * k, p[1] * k, p[2] * k];
                }
                for i in 0..3 {
                    acc[i] += p[i];
                }
                n += 1.0;
            }
        }
        Some(mul3(&self.wbx, [acc[0] / n, acc[1] / n, acc[2] / n]))
    }

    /// Manchas (clonagem suave a partir da zona de origem) e olhos vermelhos, no original.
    #[inline]
    fn retouch(
        &self,
        src: &super::source::Plane,
        pts: &[(f32, f32); 3],
        mut p: [f32; 3],
    ) -> [f32; 3] {
        let (u, v) = pts[1];
        for (k, sp) in self.s.retouch.spots.iter().enumerate() {
            let (dx, dy) = ((u - sp.x) * self.ru, (v - sp.y) * self.rv);
            let r = sp.r.max(1e-5);
            let d2 = dx * dx + dy * dy;
            if d2 >= r * r {
                continue;
            }
            let inner = r * (1.0 - sp.feather.clamp(0.0, 100.0) / 100.0);
            let w = (1.0 - smoothstep(inner, r, d2.sqrt())) * sp.opacity.clamp(0.0, 100.0) / 100.0;
            let (ox, oy) = (sp.sx - sp.x, sp.sy - sp.y);
            let ratio = self.heal.get(k).copied().unwrap_or([1.0; 3]);
            let q: [f32; 3] = std::array::from_fn(|c| {
                src.sample_channel(pts[c].0 + ox, pts[c].1 + oy, c) * ratio[c]
            });
            p = std::array::from_fn(|c| p[c] + (q[c] - p[c]) * w);
        }
        for (r, ratio, pt) in &self.patches {
            let w = r.sample(u, v) * pt.opacity.clamp(0.0, 100.0) / 100.0;
            if w <= 0.0 {
                continue;
            }
            let q: [f32; 3] = std::array::from_fn(|c| {
                src.sample_channel(pts[c].0 + pt.dx, pts[c].1 + pt.dy, c) * ratio[c]
            });
            p = std::array::from_fn(|c| p[c] + (q[c] - p[c]) * w);
        }
        for e in &self.s.retouch.red_eyes {
            let (dx, dy) = ((u - e.x) * self.ru, (v - e.y) * self.rv);
            let r = e.r.max(1e-5);
            let d2 = dx * dx + dy * dy;
            if d2 >= r * r {
                continue;
            }
            // Razão vermelho/(verde, azul) no domínio de visualização: só a pupila vermelha
            // (a íris castanha e a pele à volta ficam iguais).
            let m = (p[1] + p[2]) * 0.5;
            let (dr, dm) = (oetf(p[0].clamp(0.0, 1.0)), oetf(p[1].max(p[2]).clamp(0.0, 1.0)));
            if dr <= dm * 1.3 || dr < 0.12 {
                continue;
            }
            let k =
                smoothstep(1.3, 2.0, dr / dm.max(1e-4)) * (1.0 - smoothstep(0.7 * r, r, d2.sqrt()));
            // Pupila: neutra e escura (o reflexo branco do flash não é vermelho e mantém-se).
            let t = p[1].min(p[2]).min(m) * 0.8;
            p = p.map(|c| c + (t - c) * k);
        }
        p
    }
}

/// Guia: luminância desfocada (sombras/realces), transmissão do dehaze e luminância de
/// visualização desfocada (clareza).
struct Guide {
    w: usize,
    h: usize,
    /// Luz atmosférica (dehaze).
    atmos: [f32; 3],
    transmission: Vec<f32>,
    base_lum: Vec<f32>,
    clarity_base: Vec<f32>,
}

fn dehaze(p: [f32; 3], atmos: [f32; 3], t: f32, amount: f32) -> [f32; 3] {
    if amount == 0.0 {
        return p;
    }
    if amount > 0.0 {
        let tt = t.max(0.1);
        let j: [f32; 3] = std::array::from_fn(|i| ((p[i] - atmos[i]) / tt + atmos[i]).max(0.0));
        std::array::from_fn(|i| p[i] + (j[i] - p[i]) * amount)
    } else {
        // Acrescentar névoa: mistura com a luz atmosférica onde a transmissão é baixa.
        let k = -amount * 0.6 * (1.0 - t * 0.5);
        std::array::from_fn(|i| p[i] + (atmos[i] - p[i]) * k)
    }
}

/// Ganho das sombras/realces a partir da luminância local (guia), em EV.
#[inline]
fn local_tone_ev(s: &Settings, base: f32) -> f32 {
    let (hl, sh) = (s.light.highlights / 100.0, s.light.shadows / 100.0);
    if hl == 0.0 && sh == 0.0 {
        return 0.0;
    }
    let ev = (base.max(1e-5) / 0.18).log2();
    let w_sh = 1.0 - smoothstep(-4.5, 0.0, ev);
    let w_hl = smoothstep(-0.5, 2.6, ev);
    sh * 1.6 * w_sh + hl * 1.6 * w_hl
}

fn build_guide(src: &Source, p: &Prep, uncropped: bool) -> Guide {
    let s = p.s;
    let (w, h) =
        output_size(src.width(), src.height(), &s.geometry, uncropped, GUIDE_EDGE, GUIDE_EDGE);
    let (w, h) = (w as usize, h as usize);
    let plane = src.level_for(GUIDE_EDGE as f32 * 2.0);
    let mut lin = vec![[0.0f32; 3]; w * h];
    let mut pp = Prep::new(src, s, uncropped);
    pp.du = 1.0 / w as f32;
    pp.dv = 1.0 / h as f32;
    let pp = pp;
    lin.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            *o = pp
                .linear(plane, (x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32, 1)
                .unwrap_or([0.0; 3]);
        }
    });
    // Dehaze (He et al., simplificado): canal escuro → luz atmosférica e transmissão.
    let (mut atmos, mut transmission) = ([1.0; 3], vec![1.0; w * h]);
    if s.effects.dehaze != 0.0 {
        let dark: Vec<f32> = lin.iter().map(|q| q[0].min(q[1]).min(q[2])).collect();
        let dc = min_filter(&dark, w, h, (w.max(h) / 60).max(2));
        let mut idx: Vec<usize> = (0..dc.len()).collect();
        idx.sort_by(|&a, &b| dc[b].total_cmp(&dc[a]));
        let top = &idx[..(idx.len() / 500).max(1)];
        atmos = std::array::from_fn(|i| {
            (top.iter().map(|&k| lin[k][i]).sum::<f32>() / top.len() as f32).max(0.05)
        });
        let norm: Vec<f32> =
            lin.iter().map(|q| (0..3).map(|i| q[i] / atmos[i]).fold(f32::MAX, f32::min)).collect();
        let dn = min_filter(&norm, w, h, (w.max(h) / 60).max(2));
        let t: Vec<f32> = dn.iter().map(|v| 1.0 - 0.95 * v).collect();
        transmission = gaussian(&t, w, h, w.max(h) as f32 / 40.0);
    }
    let amount = s.effects.dehaze / 100.0;
    let lum: Vec<f32> =
        lin.iter().zip(&transmission).map(|(q, &t)| luma(dehaze(*q, atmos, t, amount))).collect();
    let base_lum = gaussian(&lum, w, h, w.max(h) as f32 / 30.0);
    // Base da clareza: luminância de visualização (depois do tom) desfocada.
    let tr = transfer();
    let disp: Vec<f32> = lin
        .iter()
        .zip(&transmission)
        .zip(&base_lum)
        .map(|((q, &t), &b)| {
            let mut v = dehaze(*q, atmos, t, amount);
            let g = 2f32.powf(local_tone_ev(s, b));
            v = [v[0] * g, v[1] * g, v[2] * g];
            let d: [f32; 3] = std::array::from_fn(|i| p.curves.apply(i, tr.to_display.get(v[i])));
            luma(d)
        })
        .collect();
    let clarity_base = gaussian(&disp, w, h, w.max(h) as f32 / 60.0);
    Guide { w, h, atmos, transmission, base_lum, clarity_base }
}

/// Peso das bandas HSL para um matiz (interpolação linear entre centros vizinhos).
#[inline]
fn hsl_weights(hue: f32) -> [(usize, f32); 2] {
    let n = HSL_CENTERS.len();
    for i in 0..n {
        let a = HSL_CENTERS[i];
        let b = if i + 1 < n { HSL_CENTERS[i + 1] } else { 360.0 };
        if hue >= a && hue < b {
            let t = (hue - a) / (b - a);
            return [(i, 1.0 - t), ((i + 1) % n, t)];
        }
    }
    [(0, 1.0), (1, 0.0)]
}

fn rgb_to_hsv(c: [f32; 3]) -> (f32, f32, f32) {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let d = mx - mn;
    let h = if d <= 1e-6 {
        0.0
    } else if mx == c[0] {
        60.0 * (((c[1] - c[2]) / d).rem_euclid(6.0))
    } else if mx == c[1] {
        60.0 * ((c[2] - c[0]) / d + 2.0)
    } else {
        60.0 * ((c[0] - c[1]) / d + 4.0)
    };
    (h, if mx <= 1e-6 { 0.0 } else { d / mx }, mx)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let c = v * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

/// Cor (domínio de visualização): vibração, saturação e HSL.
#[inline]
fn color_ops(p: &Prep, mut d: [f32; 3]) -> [f32; 3] {
    let s = p.s;
    if p.color_on {
        let l = luma(d);
        let (h, sat, _) = rgb_to_hsv(d);
        let mut k = 1.0 + s.color.saturation / 100.0;
        let vib = s.color.vibrance / 100.0;
        if vib != 0.0 {
            // Mais efeito nas cores pouco saturadas; protege tons de pele quando positivo.
            let skin = if vib > 0.0 && (10.0..=50.0).contains(&h) { 0.5 } else { 1.0 };
            k *= 1.0 + vib * (1.0 - sat).powi(2) * skin;
        }
        d = std::array::from_fn(|i| l + (d[i] - l) * k.max(0.0));
    }
    if p.hsl_on {
        let (h, sat, v) = rgb_to_hsv([d[0].max(0.0), d[1].max(0.0), d[2].max(0.0)]);
        if sat > 1e-4 {
            let w = hsl_weights(h);
            let mut dh = 0.0;
            let mut ds = 0.0;
            let mut dl = 0.0;
            for (i, wt) in w {
                dh += wt * s.hsl.hue[i];
                ds += wt * s.hsl.saturation[i];
                dl += wt * s.hsl.luminance[i];
            }
            let nh = h + dh / 100.0 * 30.0;
            let ns = (sat * (1.0 + ds / 100.0)).clamp(0.0, 1.0);
            let nv = (v * (1.0 + dl / 100.0 * 0.5 * sat)).max(0.0);
            d = hsv_to_rgb(nh, ns, nv);
        }
    }
    if s.bw.enabled {
        // Preto e branco: luminância com a mistura por cor (mais efeito nas cores saturadas).
        let l = luma(d);
        let (h, sat, _) = rgb_to_hsv([d[0].max(0.0), d[1].max(0.0), d[2].max(0.0)]);
        let adj: f32 = hsl_weights(h).iter().map(|&(i, w)| w * s.bw.mix[i] / 100.0).sum();
        let g = (l * (1.0 + adj * sat.min(1.0) * 0.9)).max(0.0);
        d = [g, g, g];
    }
    if let Some((sh, hl, bal)) = p.grade {
        let l = luma(d).clamp(0.0, 1.0);
        let ws = ((1.0 - l).powi(2) * (1.0 - bal * 0.6)).max(0.0);
        let wh = (l * l * (1.0 + bal * 0.6)).max(0.0);
        d = std::array::from_fn(|i| d[i] + sh[i] * ws + hl[i] * wh);
    }
    d
}

/// Ruído de valor determinístico (grão), coerente entre resoluções.
#[inline]
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0 - 0.5
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = hash(xi, yi, seed) * (1.0 - sx) + hash(xi + 1, yi, seed) * sx;
    let b = hash(xi, yi + 1, seed) * (1.0 - sx) + hash(xi + 1, yi + 1, seed) * sx;
    a * (1.0 - sy) + b * sy
}

pub fn render(src: &Source, s: &Settings, o: &RenderOptions) -> Rendered {
    render_with_overlay(src, s, o, None)
}

/// Render com a máscara `overlay` (índice em `s.masks`) mostrada a vermelho.
pub fn render_with_overlay(
    src: &Source,
    s: &Settings,
    o: &RenderOptions,
    overlay: Option<usize>,
) -> Rendered {
    let (w, h) =
        output_size(src.width(), src.height(), &s.geometry, o.uncropped, o.max_width, o.max_height);
    let (w, h) = (w as usize, h as usize);
    let guide = build_guide(src, &Prep::new(src, s, o.uncropped), o.uncropped);
    let mut p = Prep::new(src, s, o.uncropped);
    p.du = 1.0 / w as f32;
    p.dv = 1.0 / h as f32;
    // Nível da pirâmide: ~1 píxel de origem por píxel (rápido) ou 2 com sobreamostragem 2×2.
    let crop_frac = if o.uncropped { 1.0 } else { s.geometry.crop.w.max(s.geometry.crop.h) };
    let needed = w.max(h) as f32 / crop_frac.max(0.05);
    let (plane, ss) = match o.quality {
        Quality::Fast => (src.level_for(needed), 1),
        Quality::Final => {
            let l = src.level_for(needed * 1.5);
            (l, if l.w.max(l.h) as f32 > needed * 1.3 { 2 } else { 1 })
        }
    };
    let p = p;
    let tr = transfer();
    let amount = s.effects.dehaze / 100.0;
    let clarity = s.effects.clarity / 100.0;
    let mut disp = vec![[0.0f32; 3]; w * h];
    let mut valid = vec![true; w * h];
    disp.par_chunks_mut(w).zip(valid.par_chunks_mut(w)).enumerate().for_each(|(y, (row, vrow))| {
        let v = (y as f32 + 0.5) / h as f32;
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32;
            let Some(mut q) = p.linear(plane, u, v, ss) else {
                vrow[x] = false;
                row[x] = [0.0; 3];
                continue;
            };
            if amount != 0.0 {
                let t = sample_plane(&guide.transmission, guide.w, guide.h, u, v);
                q = dehaze(q, guide.atmos, t, amount);
            }
            let b = sample_plane(&guide.base_lum, guide.w, guide.h, u, v);
            let ev = local_tone_ev(s, b);
            if ev != 0.0 {
                let g = 2f32.powf(ev);
                q = [q[0] * g, q[1] * g, q[2] * g];
            }
            let mut d: [f32; 3] = std::array::from_fn(|i| tr.to_display.get(q[i]));
            if !p.curves.identity {
                d = std::array::from_fn(|i| p.curves.apply(i, d[i]));
            }
            if clarity != 0.0 {
                let l = luma(d);
                let base = sample_plane(&guide.clarity_base, guide.w, guide.h, u, v);
                let mid = 1.0 - (2.0 * l - 1.0).powi(2);
                let nl = (l + clarity * 0.9 * (l - base) * mid).max(0.0);
                let k = if l > 1e-4 { nl / l } else { 1.0 };
                d = [d[0] * k, d[1] * k, d[2] * k];
            }
            row[x] = color_ops(&p, d);
        }
    });
    // Ajustes locais e retrato: posição de cada píxel na fotografia.
    let long = src.width().max(src.height()) as f32;
    let (ru, rv) = (src.width() as f32 / long, src.height() as f32 / long);
    let masks = super::local::prepare(&s.masks, ru, rv);
    let portrait = super::portrait::prepare(&s.portrait, ru, rv);
    let overlay_mask = overlay.and_then(|i| s.masks.get(i)).map(|m| {
        let mut m = m.clone();
        m.enabled = true;
        m.adjust = Default::default();
        super::local::prepare(std::slice::from_ref(&m), ru, rv)
    });
    let need_uv = !masks.is_empty() || portrait.is_some() || overlay_mask.is_some();
    let uv: Vec<Option<[f32; 2]>> = if need_uv {
        (0..w * h)
            .into_par_iter()
            .map(|i| {
                if !valid[i] {
                    return None;
                }
                let (x, y) = (i % w, i / w);
                let (pts, _) = p.geo.map((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                Some([pts[1].0, pts[1].1])
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut local = super::local::LocalSpatial::default();
    if !masks.is_empty() {
        let k = masks.len();
        let mut vals = vec![0.0f32; w * h * k];
        vals.par_chunks_mut(k).zip(disp.par_iter_mut()).enumerate().for_each(|(i, (mv, d))| {
            let Some([u, v]) = uv[i] else { return };
            for (j, m) in masks.iter().enumerate() {
                let val = m.value(u, v, *d, ru, rv);
                mv[j] = val;
                *d = m.apply(*d, val, tr);
            }
        });
        for (j, m) in masks.iter().enumerate() {
            if m.spatial {
                local.planes.push(((0..w * h).map(|i| vals[i * k + j]).collect(), m.adjust));
            }
        }
    }
    spatial(&mut disp, w, h, s, src, o.quality, &local);
    if let Some(pp) = &portrait {
        // Píxeis do render por unidade do lado maior (medido ao centro).
        let c = (h / 2) * w + w / 2;
        let ppu = match (uv.get(c).copied().flatten(), uv.get(c + 1).copied().flatten()) {
            (Some(a), Some(b)) => {
                let d = (((b[0] - a[0]) * ru).powi(2) + ((b[1] - a[1]) * rv).powi(2)).sqrt();
                if d > 0.0 {
                    1.0 / d
                } else {
                    w.max(h) as f32
                }
            }
            _ => w.max(h) as f32,
        };
        super::portrait::apply(pp, &mut disp, w, h, &uv, ppu);
    }
    // Sobreposição da máscara: calculada aqui, aplicada aos bytes **depois** do histograma
    // (o histograma mostra sempre a fotografia, não o vermelho).
    let overlay_vals: Option<Vec<f32>> = overlay_mask.as_ref().and_then(|v| v.first()).map(|om| {
        disp.par_iter()
            .enumerate()
            .map(|(i, d)| uv[i].map(|[u, v]| om.value(u, v, *d, ru, rv) * 0.6).unwrap_or(0.0))
            .collect()
    });
    // Saída: vinheta, grão, sRGB 8 bits, histograma.
    let to_srgb = matmul(&REC2020_TO_SRGB, &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let e = &s.effects;
    let grain_px = (src.full_width.max(src.full_height) as f32 / w.max(h) as f32).max(1.0);
    let gsize = 1.0 + e.grain_size / 100.0 * 4.0;
    let mut rgba = vec![0u8; w * h * 4];
    let hist = rgba
        .par_chunks_mut(w * 4)
        .enumerate()
        .map(|(y, row)| {
            let mut hist = [[0u32; 256]; 3];
            let v = (y as f32 + 0.5) / h as f32;
            for x in 0..w {
                let i = y * w + x;
                if !valid[i] {
                    row[x * 4..x * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
                    continue;
                }
                let mut d = disp[i];
                let u = (x as f32 + 0.5) / w as f32;
                if e.vignette != 0.0 {
                    let round = e.vignette_roundness / 100.0;
                    let ax = 1.0 + round.min(0.0) * -0.5;
                    let ay = 1.0 + round.max(0.0) * 0.5;
                    let (dx, dy) = ((u - 0.5) * 2.0 * ax, (v - 0.5) * 2.0 * ay);
                    let r = (dx * dx + dy * dy).sqrt() / std::f32::consts::SQRT_2;
                    let m0 = 0.15 + e.vignette_midpoint / 100.0 * 0.7;
                    let f = 0.05 + e.vignette_feather / 100.0 * 0.8;
                    let mask = smoothstep(m0 - f * 0.5, m0 + f * 0.5, r);
                    let k = e.vignette / 100.0 * mask;
                    d = if k < 0.0 {
                        d.map(|c| c * (1.0 + k))
                    } else {
                        d.map(|c| c + (1.0 - c) * k)
                    };
                }
                if e.grain != 0.0 {
                    // Coordenadas na grelha da resolução total → grão com o mesmo tamanho a qualquer zoom.
                    let (gx, gy) = (x as f32 * grain_px / gsize, y as f32 * grain_px / gsize);
                    let mut n = value_noise(gx, gy, 7);
                    let rough = e.grain_roughness / 100.0;
                    n = n * (1.0 - rough * 0.5) + value_noise(gx * 2.3, gy * 2.3, 11) * rough * 0.5;
                    let l = luma(d);
                    let amp = e.grain / 100.0 * 0.22 * (0.3 + 0.7 * 4.0 * l * (1.0 - l));
                    d = d.map(|c| c + n * amp);
                }
                let lin = [tr.to_linear.get(d[0]), tr.to_linear.get(d[1]), tr.to_linear.get(d[2])];
                let mut out = mul3(&to_srgb, lin);
                if let Some((lut, amt)) = &p.lut {
                    // LUT em sRGB codificado (0–1), como nos editores de vídeo.
                    let e = out.map(|c| oetf(c.clamp(0.0, 1.0)));
                    let l = lut.apply(e);
                    out =
                        std::array::from_fn(|i| eotf((e[i] + (l[i] - e[i]) * amt).clamp(0.0, 1.0)));
                }
                for c in 0..3 {
                    let b = tr.encode_u8(out[c]);
                    row[x * 4 + c] = b;
                    hist[c][b as usize] += 1;
                }
                row[x * 4 + 3] = 255;
            }
            hist
        })
        .reduce(
            || [[0u32; 256]; 3],
            |mut a, b| {
                for c in 0..3 {
                    for k in 0..256 {
                        a[c][k] += b[c][k];
                    }
                }
                a
            },
        );
    if let Some(ov) = overlay_vals {
        rgba.par_chunks_mut(4).zip(ov.par_iter()).for_each(|(px, &m)| {
            if m > 0.0 {
                px[0] = (px[0] as f32 + (217.0 - px[0] as f32) * m) as u8;
                px[1] = (px[1] as f32 * (1.0 - m)) as u8;
                px[2] = (px[2] as f32 * (1.0 - m)) as u8;
            }
        });
    }
    Rendered { width: w as u32, height: h as u32, rgba, histogram: hist }
}

/// Textura, redução de ruído (luminância e cor) e nitidez, à resolução do render.
fn spatial(
    disp: &mut [[f32; 3]],
    w: usize,
    h: usize,
    s: &Settings,
    src: &Source,
    q: Quality,
    local: &super::local::LocalSpatial,
) {
    let d = &s.detail;
    let tex = s.effects.texture / 100.0;
    let nl = if q == Quality::Final { d.noise_luminance / 100.0 } else { 0.0 };
    // Redução de ruído e nitidez só no render final (no rápido não se veem à escala do ecrã
    // e custam mais do que o resto do pipeline).
    let nc = if q == Quality::Final { d.noise_color / 100.0 } else { 0.0 };
    // Píxeis do render por píxel do original (raios definidos à resolução total).
    let scale = w.max(h) as f32 / src.full_width.max(src.full_height) as f32;
    let sharp_sigma = d.sharpen_radius * scale.max(0.0);
    let sharpen = q == Quality::Final && d.sharpen_amount > 0.0 && sharp_sigma >= 0.25;
    let final_q = q == Quality::Final;
    let local_tex = local.any(|a| a.texture);
    let local_cl = local.any(|a| a.clarity);
    let local_sh = final_q && local.any(|a| a.sharpness);
    let local_nr = final_q && local.any(|a| a.noise.max(0.0));
    if tex == 0.0
        && nl == 0.0
        && nc == 0.0
        && !sharpen
        && !local_tex
        && !local_cl
        && !local_sh
        && !local_nr
    {
        return;
    }
    let mut y: Vec<f32> = disp.iter().map(|p| luma(*p)).collect();
    let y0 = y.clone();
    if nc > 0.0 {
        // Ruído de cor: desfoca a crominância (cor − luminância), mantém a luminância.
        let sigma = 1.0 + nc * 3.0 * (scale * 4.0).clamp(0.5, 1.0);
        let chans: Vec<Vec<f32>> =
            (0..3).map(|c| disp.iter().zip(&y).map(|(p, l)| p[c] - l).collect()).collect();
        let blurred: Vec<Vec<f32>> = chans.iter().map(|ch| gaussian(ch, w, h, sigma)).collect();
        for (i, p) in disp.iter_mut().enumerate() {
            for c in 0..3 {
                p[c] = y[i] + chans[c][i] + (blurred[c][i] - chans[c][i]) * nc.min(1.0);
            }
        }
    }
    if nl > 0.0 {
        // Filtro sigma: desfoca onde a diferença é pequena (ruído), preserva arestas.
        // Preservação do detalhe: 50 = comportamento base; mais alto → limiar mais baixo e
        // parte do detalhe fino devolvida.
        let keep = d.noise_detail.clamp(0.0, 100.0) / 100.0;
        let b = gaussian(&y, w, h, 1.0 + nl * 2.0);
        let sig = (0.02 + nl * 0.16) * (1.4 - keep * 0.8);
        let back = ((keep - 0.5) * 0.5).max(0.0);
        for (v, bb) in y.iter_mut().zip(&b) {
            let diff = *v - bb;
            let wgt = (-(diff * diff) / (sig * sig)).exp();
            let delta = (bb - *v) * wgt * nl.min(1.0);
            *v += delta * (1.0 - back);
        }
    }
    if tex != 0.0 {
        let b = gaussian(&y, w, h, (w.max(h) as f32 * 0.004).max(1.0));
        for (v, bb) in y.iter_mut().zip(&b) {
            let mid = 1.0 - (2.0 * *v - 1.0).clamp(-1.0, 1.0).powi(2);
            *v += tex * 0.8 * (*v - bb) * (0.4 + 0.6 * mid);
        }
    }
    if sharpen {
        let b = gaussian(&y, w, h, sharp_sigma);
        let thr = (1.0 - d.sharpen_detail / 100.0) * 0.02;
        // Máscara de arestas a partir do gradiente da imagem suavizada.
        let mask_on = d.sharpen_masking > 0.0;
        let grad: Vec<f32> = if mask_on {
            (0..w * h)
                .map(|i| {
                    let (x, yy) = (i % w, i / w);
                    let gx = b[yy * w + (x + 1).min(w - 1)] - b[yy * w + x.saturating_sub(1)];
                    let gy = b[(yy + 1).min(h - 1) * w + x] - b[yy.saturating_sub(1) * w + x];
                    (gx * gx + gy * gy).sqrt()
                })
                .collect()
        } else {
            Vec::new()
        };
        let m_t = d.sharpen_masking / 100.0 * 0.08;
        let amt = d.sharpen_amount / 100.0 * 1.2;
        for i in 0..w * h {
            let hp = y[i] - b[i];
            let a = hp.abs();
            let hp = if a < thr { hp * (a / thr.max(1e-6)) } else { hp };
            let m = if mask_on { smoothstep(m_t * 0.5, m_t, grad[i]) } else { 1.0 };
            y[i] += amt * hp * m;
        }
    }
    // Ajustes locais espaciais (máscaras).
    if local_nr {
        let b = gaussian(&y, w, h, 2.0);
        for i in 0..w * h {
            let a = local.amount(i, |a| a.noise.max(0.0)).min(1.0);
            if a > 0.0 {
                let diff = y[i] - b[i];
                let wgt = (-(diff * diff) / (0.12 * 0.12)).exp();
                y[i] += (b[i] - y[i]) * wgt * a;
            }
        }
    }
    if local_tex {
        let b = gaussian(&y, w, h, (w.max(h) as f32 * 0.004).max(1.0));
        for i in 0..w * h {
            let a = local.amount(i, |a| a.texture);
            if a != 0.0 {
                let mid = 1.0 - (2.0 * y[i] - 1.0).clamp(-1.0, 1.0).powi(2);
                y[i] += a * 0.8 * (y[i] - b[i]) * (0.4 + 0.6 * mid);
            }
        }
    }
    if local_cl {
        let b = gaussian(&y, w, h, w.max(h) as f32 / 60.0);
        for i in 0..w * h {
            let a = local.amount(i, |a| a.clarity);
            if a != 0.0 {
                let mid = 1.0 - (2.0 * y[i] - 1.0).clamp(-1.0, 1.0).powi(2);
                y[i] = (y[i] + a * 0.9 * (y[i] - b[i]) * mid).max(0.0);
            }
        }
    }
    if local_sh {
        let b = gaussian(&y, w, h, (scale * 1.0).max(0.6));
        for i in 0..w * h {
            let a = local.amount(i, |a| a.sharpness);
            if a != 0.0 {
                // Positivo: nitidez; negativo: suaviza.
                y[i] += if a > 0.0 { a * 1.2 * (y[i] - b[i]) } else { -a * (b[i] - y[i]) };
            }
        }
    }
    for (i, p) in disp.iter_mut().enumerate() {
        let (l0, l1) = (y0[i], y[i].max(0.0));
        if l0 > 1e-4 {
            let k = l1 / l0;
            *p = p.map(|c| c * k);
        } else {
            *p = [l1, l1, l1];
        }
    }
}

/// Amostra do original (Rec. 2020 linear, antes do balanço) em coordenadas normalizadas do
/// resultado — para o seletor de balanço de brancos. Média numa pequena janela.
pub fn sample_for_white_balance(src: &Source, s: &Settings, u: f32, v: f32) -> Option<[f32; 3]> {
    let neutral = Settings { wb: Default::default(), light: Default::default(), ..s.clone() };
    let mut p = Prep::new(src, &neutral, false);
    p.du = 0.0;
    p.dv = 0.0;
    let plane = src.level_for(1024.0);
    let mut acc = [0.0f32; 3];
    let mut n = 0.0;
    for dy in -2..=2 {
        for dx in -2..=2 {
            if let Some(q) = p.linear(plane, u + dx as f32 * 0.002, v + dy as f32 * 0.002, 1) {
                for i in 0..3 {
                    acc[i] += q[i];
                }
                n += 1.0;
            }
        }
    }
    (n > 0.0).then(|| acc.map(|a| a / n))
}
