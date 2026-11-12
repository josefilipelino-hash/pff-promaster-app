//! Calibração de cor de câmaras que a LibRaw ainda não conhece (sem matriz câmara→sRGB,
//! ex.: Canon EOS C50 na LibRaw 0.22). Sem matriz, os valores da câmara são tratados como
//! Rec. 2020 e as cores saem com uma dominante forte (magenta/verde).
//!
//! A câmara grava dentro do RAW uma preview JPEG com as cores certas. Comparando-a com os
//! dados do sensor (já com o balanço de brancos da câmara) estima-se uma matriz 3×3
//! câmara → Rec. 2020 linear:
//! - por cromaticidade (cada par normalizado pela sua soma), para não depender da curva de
//!   tons da câmara;
//! - com as linhas a somar 1, para que um cinzento continue cinzento (o balanço da câmara já
//!   tornou o branco neutro);
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
}

fn norm(v: [f32; 3]) -> Option<[f32; 3]> {
    let s = v[0] + v[1] + v[2];
    (s > 1e-6 && v.iter().all(|c| c.is_finite())).then(|| v.map(|c| c / s))
}

/// Estima a matriz que leva `camera` (linear, balanceado, já orientado) às cores de `preview`
/// (sRGB 8 bits, a mesma imagem e orientação; é redimensionada para o tamanho de `camera`).
/// `None` se houver poucos píxeis utilizáveis ou o ajuste não melhorar nada.
pub fn calibrate_from_preview(camera: &Plane, preview: &RgbImage) -> Option<Calibration> {
    if camera.w < 16 || camera.h < 16 {
        return None;
    }
    let pv = image::imageops::resize(
        preview,
        camera.w as u32,
        camera.h as u32,
        image::imageops::FilterType::Triangle,
    );
    // Pares (câmara, alvo) normalizados.
    let mut pairs: Vec<([f32; 3], [f32; 3])> = Vec::new();
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
        }
    }
    if pairs.len() < 200 {
        return None;
    }
    // Cada linha: y_c = a·x0 + b·x1 + (1−a−b)·x2  ⇒  y_c − x2 = a(x0−x2) + b(x1−x2).
    let mut m = [[0f32; 3]; 3];
    for (c, row) in m.iter_mut().enumerate() {
        let (mut s11, mut s12, mut s22, mut t1, mut t2) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for (x, y) in &pairs {
            let (u, v) = ((x[0] - x[2]) as f64, (x[1] - x[2]) as f64);
            let r = (y[c] - x[2]) as f64;
            s11 += u * u;
            s12 += u * v;
            s22 += v * v;
            t1 += u * r;
            t2 += v * r;
        }
        let det = s11 * s22 - s12 * s12;
        if det.abs() < 1e-12 {
            return None;
        }
        let a = ((t1 * s22 - t2 * s12) / det) as f32;
        let b = ((s11 * t2 - s12 * t1) / det) as f32;
        *row = [a, b, 1.0 - a - b];
    }
    if m.iter().flatten().any(|v| !v.is_finite() || v.abs() > 4.0) {
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
    })
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
        let mut truth = Vec::with_capacity(w * h);
        let mut s = 0x9e37_79b9u32;
        for _ in 0..w * h {
            let mut rnd = || {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s % 1000) as f32 / 1000.0
            };
            let srgb = [0.05 + 0.6 * rnd(), 0.05 + 0.6 * rnd(), 0.05 + 0.6 * rnd()];
            truth.push(mul3(&SRGB_TO_REC2020, srgb));
        }
        // Câmara: mistura os canais (linhas a somar 1: o balanço já tornou o branco neutro).
        let cam_from_truth = [[0.70, 0.25, 0.05], [0.15, 0.75, 0.10], [0.05, 0.30, 0.65]];
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
        // Cinzento continua cinzento.
        let g = mul3(&cal.matrix, [0.3, 0.3, 0.3]);
        assert!(g.iter().all(|c| (c - 0.3).abs() < 1e-4), "{g:?}");
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
            worst < 0.06 && worst < worst_raw * 0.5,
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
