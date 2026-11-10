//! Métricas técnicas clássicas sobre uma imagem RGB 8 bits (sRGB).
//!
//! As métricas de nitidez e ruído são calculadas numa versão em escala de cinzentos com
//! lado maior fixo ([`WORK_EDGE`]), para que os valores sejam comparáveis entre fotografias
//! de resoluções diferentes.

use image::{imageops, GrayImage, RgbImage};
use serde::{Deserialize, Serialize};

pub const WORK_EDGE: u32 = 1024;
const GRID_X: u32 = 6;
const GRID_Y: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalMetrics {
    /// Variância do Laplaciano da imagem inteira (log10).
    pub sharpness: f32,
    /// Variância do Laplaciano no mosaico mais nítido (log10) — onde está o foco.
    pub focus_peak: f32,
    /// Variância do Laplaciano no terço central (log10).
    pub focus_center: f32,
    /// 0–1: anisotropia dos gradientes nas zonas com textura (rastos de movimento).
    pub motion_blur: f32,
    /// Luminância média (0–1).
    pub brightness: f32,
    /// Fração de píxeis com algum canal ≥ 252 (realces queimados).
    pub highlights_clipped: f32,
    /// Fração de píxeis com todos os canais ≤ 6 (sombras bloqueadas).
    pub shadows_crushed: f32,
    /// Percentil 95 − percentil 5 da luminância (0–1).
    pub contrast: f32,
    /// Desvio-padrão estimado do ruído (escala 0–255, método de Immerkær).
    pub noise: f32,
    /// Inclinação do horizonte/verticais em graus (positivo = sentido horário), se detetada.
    pub tilt_degrees: Option<f32>,
}

pub fn to_work_gray(img: &RgbImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let s = WORK_EDGE as f32 / w.max(h) as f32;
    let small = if s < 1.0 {
        crate::resize::rgb(img, ((w as f32 * s) as u32).max(1), ((h as f32 * s) as u32).max(1))
    } else {
        img.clone()
    };
    imageops::grayscale(&small)
}

/// Variância do Laplaciano (kernel 4-vizinhos) numa região.
pub fn laplacian_variance(g: &GrayImage, x0: u32, y0: u32, x1: u32, y1: u32) -> f32 {
    let (w, h) = g.dimensions();
    let (x0, y0, x1, y1) = (x0.max(1), y0.max(1), x1.min(w - 1), y1.min(h - 1));
    if x1 <= x0 || y1 <= y0 {
        return 0.0;
    }
    let raw = g.as_raw();
    let at = |x: u32, y: u32| raw[(y * w + x) as usize] as f32;
    let (mut s, mut s2, mut n) = (0f64, 0f64, 0f64);
    for y in y0..y1 {
        for x in x0..x1 {
            let l = at(x - 1, y) + at(x + 1, y) + at(x, y - 1) + at(x, y + 1) - 4.0 * at(x, y);
            s += l as f64;
            s2 += (l * l) as f64;
            n += 1.0;
        }
    }
    let mean = s / n;
    (s2 / n - mean * mean).max(0.0) as f32
}

fn log_sharp(v: f32) -> f32 {
    (v + 1.0).log10()
}

/// Nitidez de uma região de uma imagem RGB, normalizada para um lado fixo (comparável
/// entre rostos de tamanhos diferentes). Devolve log10(variância do Laplaciano).
pub fn region_sharpness(img: &RgbImage, x: f32, y: f32, w: f32, h: f32, norm_edge: u32) -> f32 {
    let (iw, ih) = img.dimensions();
    let x0 = x.max(0.0).floor() as u32;
    let y0 = y.max(0.0).floor() as u32;
    let x1 = ((x + w).ceil() as u32).min(iw);
    let y1 = ((y + h).ceil() as u32).min(ih);
    if x1 <= x0 + 4 || y1 <= y0 + 4 {
        return 0.0;
    }
    let crop = imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0).to_image();
    let s = norm_edge as f32 / (x1 - x0).max(y1 - y0) as f32;
    // Só se reduz (nunca se amplia: ampliar inventa suavidade e baixa a nitidez medida).
    let crop = if s < 1.0 {
        crate::resize::rgb(
            &crop,
            ((crop.width() as f32 * s) as u32).max(8),
            ((crop.height() as f32 * s) as u32).max(8),
        )
    } else {
        crop
    };
    let g = imageops::grayscale(&crop);
    log_sharp(laplacian_variance(&g, 0, 0, g.width(), g.height()))
}

fn noise_immerkaer(g: &GrayImage) -> f32 {
    let (w, h) = g.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let raw = g.as_raw();
    let at = |x: u32, y: u32| raw[(y * w + x) as usize] as f32;
    let (mut sum, mut n) = (0f64, 0f64);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            // Ignorar arestas fortes (o operador mede o ruído em zonas uniformes).
            let gx = at(x + 1, y) - at(x - 1, y);
            let gy = at(x, y + 1) - at(x, y - 1);
            if gx.abs() + gy.abs() > 24.0 {
                continue;
            }
            let v = at(x - 1, y - 1) - 2.0 * at(x, y - 1) + at(x + 1, y - 1) - 2.0 * at(x - 1, y)
                + 4.0 * at(x, y)
                - 2.0 * at(x + 1, y)
                + at(x - 1, y + 1)
                - 2.0 * at(x, y + 1)
                + at(x + 1, y + 1);
            sum += v.abs() as f64;
            n += 1.0;
        }
    }
    if n < 100.0 {
        return 0.0;
    }
    ((std::f64::consts::PI / 2.0).sqrt() * sum / (6.0 * n)) as f32
}

/// Coerência dos gradientes (0 isotrópico – 1 todos na mesma direção) e energia.
fn coherence(g: &GrayImage, x0: u32, y0: u32, x1: u32, y1: u32) -> (f32, f32) {
    let w = g.width();
    let raw = g.as_raw();
    let at = |x: u32, y: u32| raw[(y * w + x) as usize] as f32;
    let (mut jxx, mut jyy, mut jxy) = (0f64, 0f64, 0f64);
    let (x0, y0, x1, y1) = (x0.max(1), y0.max(1), x1.min(w - 1), y1.min(g.height() - 1));
    for y in y0..y1 {
        for x in x0..x1 {
            let gx = (at(x + 1, y) - at(x - 1, y)) as f64;
            let gy = (at(x, y + 1) - at(x, y - 1)) as f64;
            jxx += gx * gx;
            jyy += gy * gy;
            jxy += gx * gy;
        }
    }
    let tr = jxx + jyy;
    if tr <= 0.0 {
        return (0.0, 0.0);
    }
    let n = ((x1 - x0) * (y1 - y0)).max(1) as f64;
    ((((jxx - jyy).powi(2) + 4.0 * jxy * jxy).sqrt() / tr) as f32, (tr / n) as f32)
}

/// Inclinação dominante de linhas quase horizontais/verticais (histograma de orientação
/// das arestas fortes). Só devolve um valor quando há linhas suficientemente dominantes.
fn tilt(g: &GrayImage) -> Option<f32> {
    let (w, h) = g.dimensions();
    let raw = g.as_raw();
    let at = |x: u32, y: u32| raw[(y * w + x) as usize] as f32;
    const BINS: usize = 81; // −10° … +10° em passos de 0,25°
    let mut samples: Vec<(f32, f32)> = Vec::new(); // (magnitude, ângulo da linha)
    for y in (2..h - 2).step_by(2) {
        for x in (2..w - 2).step_by(2) {
            let gx = (at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1))
                - (at(x - 1, y - 1) + 2.0 * at(x - 1, y) + at(x - 1, y + 1));
            let gy = (at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1))
                - (at(x - 1, y - 1) + 2.0 * at(x, y - 1) + at(x + 1, y - 1));
            let mag = (gx * gx + gy * gy).sqrt();
            if mag < 30.0 {
                continue;
            }
            // Ângulo da linha (perpendicular ao gradiente), dobrado para [−45°, 45°].
            let mut a = gy.atan2(gx).to_degrees() + 90.0;
            while a > 45.0 {
                a -= 90.0;
            }
            while a < -45.0 {
                a += 90.0;
            }
            samples.push((mag, a));
        }
    }
    if samples.len() < 200 {
        return None;
    }
    // Só as arestas mais fortes (10 % superiores) contam: são as linhas estruturais.
    let mut mags: Vec<f32> = samples.iter().map(|s| s.0).collect();
    mags.sort_by(|a, b| a.total_cmp(b));
    let thr = mags[mags.len() * 9 / 10];
    let mut hist = [0f64; BINS];
    let mut total = 0f64;
    for &(mag, a) in samples.iter().filter(|s| s.0 >= thr) {
        total += mag as f64;
        if a.abs() <= 10.0 {
            let b = ((a + 10.0) * 4.0).round() as usize;
            hist[b.min(BINS - 1)] += mag as f64;
        }
    }
    if total <= 0.0 {
        return None;
    }
    // Suavização e pico.
    let smooth: Vec<f64> = (0..BINS)
        .map(|i| (i.saturating_sub(2)..(i + 3).min(BINS)).map(|j| hist[j]).sum())
        .collect();
    let (peak, &val) = smooth.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    let share = val / total;
    // Ângulo refinado: centroide ponderado do histograma à volta do pico (±1°).
    let lo = peak.saturating_sub(4);
    let hi = (peak + 5).min(BINS);
    let wsum: f64 = hist[lo..hi].iter().sum();
    let centroid = (lo..hi).map(|i| i as f64 * hist[i]).sum::<f64>() / wsum.max(1e-9);
    // Linhas dominantes: o pico concentra ≥ 12 % da energia das arestas fortes.
    (share >= 0.12).then(|| centroid as f32 / 4.0 - 10.0)
}

pub fn technical(img: &RgbImage) -> TechnicalMetrics {
    let g = to_work_gray(img);
    let (w, h) = g.dimensions();
    let sharpness = log_sharp(laplacian_variance(&g, 0, 0, w, h));
    let (tw, th) = (w / GRID_X, h / GRID_Y);
    let mut tiles = Vec::new();
    let mut coh = Vec::new();
    for ty in 0..GRID_Y {
        for tx in 0..GRID_X {
            let (x0, y0) = (tx * tw, ty * th);
            tiles.push(log_sharp(laplacian_variance(&g, x0, y0, x0 + tw, y0 + th)));
            let (c, e) = coherence(&g, x0, y0, x0 + tw, y0 + th);
            if e > 30.0 {
                coh.push(c);
            }
        }
    }
    let focus_peak = tiles.iter().copied().fold(0.0, f32::max);
    let focus_center = log_sharp(laplacian_variance(&g, w / 3, h / 3, 2 * w / 3, 2 * h / 3));
    coh.sort_by(|a, b| a.total_cmp(b));
    let median_coh = if coh.is_empty() { 0.0 } else { coh[coh.len() / 2] };
    // Movimento: gradientes com direção dominante em toda a imagem E pouca nitidez.
    let softness = (1.0 - ((focus_peak - 1.5) / 1.5).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let motion_blur = ((median_coh - 0.25) / 0.5).clamp(0.0, 1.0) * (0.3 + 0.7 * softness);

    // Exposição em sRGB (luminância Rec.709 sobre valores codificados).
    let mut hist = [0u32; 256];
    let (mut hi, mut lo) = (0u64, 0u64);
    let px = img.pixels().len().max(1) as f64;
    for p in img.pixels() {
        let [r, g, b] = p.0;
        let y = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) as usize;
        hist[y.min(255)] += 1;
        if r.max(g).max(b) >= 252 {
            hi += 1;
        }
        if r.max(g).max(b) <= 6 {
            lo += 1;
        }
    }
    let pct = |p: f64| {
        let target = (px * p) as u64;
        let mut acc = 0u64;
        for (i, &c) in hist.iter().enumerate() {
            acc += c as u64;
            if acc >= target {
                return i as f32 / 255.0;
            }
        }
        1.0
    };
    let brightness = hist.iter().enumerate().map(|(i, &c)| i as f64 * c as f64).sum::<f64>() as f32
        / px as f32
        / 255.0;

    TechnicalMetrics {
        sharpness,
        focus_peak,
        focus_center,
        motion_blur,
        brightness,
        highlights_clipped: (hi as f64 / px) as f32,
        shadows_crushed: (lo as f64 / px) as f32,
        contrast: pct(0.95) - pct(0.05),
        noise: noise_immerkaer(&g),
        tilt_degrees: tilt(&g),
    }
}

/// Pontuação de exposição 0–1 (1 = bem exposta, sem recortes significativos).
pub fn exposure_score(m: &TechnicalMetrics) -> f32 {
    // Fotografias de eventos aceitam bastante variação de luminosidade média (low key,
    // high key): penaliza-se sobretudo os extremos e os recortes.
    let b = m.brightness;
    let level = if b < 0.12 {
        b / 0.12 * 0.6
    } else if b < 0.25 {
        0.6 + (b - 0.12) / 0.13 * 0.4
    } else if b <= 0.7 {
        1.0
    } else if b <= 0.85 {
        1.0 - (b - 0.7) / 0.15 * 0.4
    } else {
        0.6 - ((b - 0.85) / 0.15).min(1.0) * 0.6
    };
    let clip_hi = ((m.highlights_clipped - 0.01) / 0.09).clamp(0.0, 1.0); // 1 % tolerado, 10 % grave
    let clip_lo = ((m.shadows_crushed - 0.05) / 0.25).clamp(0.0, 1.0); // sombras profundas são comuns
    (level * (1.0 - 0.6 * clip_hi) * (1.0 - 0.3 * clip_lo)).clamp(0.0, 1.0)
}

/// Nitidez em escala 0–1 a partir de log10(variância do Laplaciano) a 1024 px.
pub fn sharpness_score(log_var: f32) -> f32 {
    // ≤ 1,3 (≈ 20) muito desfocada; ≥ 2,6 (≈ 400) nítida.
    ((log_var - 1.3) / 1.3).clamp(0.0, 1.0)
}

/// Ruído em escala 0–1 (1 = sem ruído visível).
pub fn noise_score(sigma: f32) -> f32 {
    (1.0 - ((sigma - 2.0) / 8.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    fn textured(w: u32, h: u32, seed: u32) -> RgbImage {
        // Textura rica (padrão xorshift suavizado) — fotografia nítida sintética.
        let mut x = seed.max(1);
        let base = RgbImage::from_fn(w, h, |_, _| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let v = 60 + (x % 140) as u8;
            Rgb([v, v.saturating_sub(10), v.saturating_add(10)])
        });
        imageops::blur(&base, 0.8)
    }

    #[test]
    fn blur_lowers_sharpness_and_focus() {
        let sharp = textured(1200, 800, 3);
        let soft = imageops::blur(&sharp, 4.0);
        let (a, b) = (technical(&sharp), technical(&soft));
        assert!(a.sharpness > b.sharpness + 0.8, "{} vs {}", a.sharpness, b.sharpness);
        assert!(a.focus_peak > b.focus_peak);
        assert!(sharpness_score(a.focus_peak) > sharpness_score(b.focus_peak));
    }

    #[test]
    fn focus_peak_finds_the_sharp_region() {
        // Fundo desfocado com um sujeito nítido num canto (pouca profundidade de campo).
        let mut img = imageops::blur(&textured(1200, 800, 5), 5.0);
        let subject = textured(300, 250, 9);
        imageops::replace(&mut img, &subject, 100, 100);
        let m = technical(&img);
        assert!(m.focus_peak > m.sharpness + 0.3, "pico {} global {}", m.focus_peak, m.sharpness);
    }

    #[test]
    fn motion_blur_is_detected() {
        let sharp = textured(1000, 700, 7);
        // Rasto horizontal: média de 25 cópias deslocadas.
        let moved = RgbImage::from_fn(1000, 700, |x, y| {
            let mut acc = [0u32; 3];
            for d in 0..25u32 {
                let p = sharp.get_pixel((x + d).min(999), y).0;
                for c in 0..3 {
                    acc[c] += p[c] as u32;
                }
            }
            Rgb([(acc[0] / 25) as u8, (acc[1] / 25) as u8, (acc[2] / 25) as u8])
        });
        let (a, b) = (technical(&sharp), technical(&moved));
        assert!(
            b.motion_blur > 0.4 && b.motion_blur > a.motion_blur + 0.3,
            "{} vs {}",
            a.motion_blur,
            b.motion_blur
        );
    }

    #[test]
    fn exposure_and_clipping() {
        let ok = RgbImage::from_fn(400, 300, |x, y| {
            Rgb([(60 + x % 120) as u8, (70 + y % 100) as u8, 100])
        });
        let blown = RgbImage::from_fn(400, 300, |x, _| {
            if x < 160 {
                Rgb([255, 255, 255])
            } else {
                Rgb([200, 200, 190])
            }
        });
        let dark =
            RgbImage::from_fn(
                400,
                300,
                |x, _| if x < 200 { Rgb([0, 0, 0]) } else { Rgb([20, 15, 10]) },
            );
        let (a, b, c) = (technical(&ok), technical(&blown), technical(&dark));
        assert!(b.highlights_clipped > 0.35 && a.highlights_clipped < 0.01);
        assert!(c.shadows_crushed > 0.45);
        assert!(exposure_score(&a) > 0.9);
        assert!(exposure_score(&b) < 0.5, "{}", exposure_score(&b));
        assert!(exposure_score(&c) < 0.4, "{}", exposure_score(&c));
    }

    #[test]
    fn noise_estimate_grows_with_noise() {
        let clean = RgbImage::from_fn(800, 600, |x, y| {
            let v = ((x + y) / 8 % 200) as u8 + 20;
            Rgb([v, v, v])
        });
        let mut s = 1u32;
        let noisy = RgbImage::from_fn(800, 600, |x, y| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let n = (s % 31) as i32 - 15;
            let v = (((x + y) / 8 % 200) as i32 + 20 + n).clamp(0, 255) as u8;
            Rgb([v, v, v])
        });
        let (a, b) = (technical(&clean).noise, technical(&noisy).noise);
        assert!(b > a + 4.0, "limpa {a} ruidosa {b}");
        assert!(noise_score(b) < noise_score(a));
    }

    #[test]
    fn tilt_detection_on_rotated_lines() {
        // Riscas horizontais rodadas 4° (como um horizonte inclinado).
        let lines = RgbImage::from_fn(1200, 800, |x, y| {
            let yy = y as f32 - (x as f32 - 600.0) * (4.0f32).to_radians().tan();
            let v = (120.0 + 80.0 * (yy * std::f32::consts::TAU / 80.0).sin()) as u8;
            Rgb([v, v, v])
        });
        let t = technical(&lines).tilt_degrees.expect("inclinação detetada");
        assert!((t.abs() - 4.0).abs() < 0.6, "{t}");
        let straight = RgbImage::from_fn(1200, 800, |_, y| {
            let v = (120.0 + 80.0 * (y as f32 * std::f32::consts::TAU / 80.0).sin()) as u8;
            Rgb([v, v, v])
        });
        let s = technical(&straight).tilt_degrees;
        assert!(s.is_some_and(|v| v.abs() < 0.3), "{s:?}");
    }
}
