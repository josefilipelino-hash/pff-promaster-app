//! Céu (heurística clássica, sem modelo): píxeis claros e lisos, azuis ou brancos/cinzentos
//! (nublado), **ligados ao topo** da imagem. Falha em céus muito escuros (noite), com
//! muitas árvores à frente ou quando não toca no topo — nesses casos a máscara sai vazia ou
//! incompleta e pode ser corrigida com o pincel.

use super::color::luma;
use super::filters::{gaussian, guided_filter};
use image::RgbImage;

/// Probabilidade de céu por píxel (mesmas dimensões de `img`).
pub fn sky_mask(img: &RgbImage) -> Vec<f32> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let px: Vec<[f32; 3]> = img
        .pixels()
        .map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0])
        .collect();
    let l: Vec<f32> = px.iter().map(|p| luma(*p)).collect();
    let bl = gaussian(&l, w, h, 1.5);
    let grad: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let gx = bl[y * w + (x + 1).min(w - 1)] - bl[y * w + x.saturating_sub(1)];
            let gy = bl[(y + 1).min(h - 1) * w + x] - bl[y.saturating_sub(1) * w + x];
            (gx * gx + gy * gy).sqrt()
        })
        .collect();
    let tex = gaussian(&grad, w, h, 3.0);
    let cand: Vec<bool> = (0..w * h)
        .map(|i| {
            let p = px[i];
            let mx = p[0].max(p[1]).max(p[2]);
            let mn = p[0].min(p[1]).min(p[2]);
            let sat = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
            let blue = p[2] >= p[0] && p[2] >= p[1] * 0.95 && sat > 0.08;
            let overcast = sat < 0.12 && l[i] > 0.55;
            l[i] > 0.3 && tex[i] < 0.03 && (blue || overcast)
        })
        .collect();
    // Preenchimento a partir do topo (4-ligado).
    let mut seen = vec![false; w * h];
    let mut stack: Vec<usize> = (0..w).filter(|&x| cand[x]).collect();
    for &i in &stack {
        seen[i] = true;
    }
    while let Some(i) = stack.pop() {
        let (x, y) = (i % w, i / w);
        let mut push = |j: usize| {
            if !seen[j] && cand[j] {
                seen[j] = true;
                stack.push(j);
            }
        };
        if x > 0 {
            push(i - 1);
        }
        if x + 1 < w {
            push(i + 1);
        }
        if y > 0 {
            push(i - w);
        }
        if y + 1 < h {
            push(i + w);
        }
    }
    // Nuvens: claras, pouco saturadas e com mais textura — só se forem contíguas ao céu já
    // encontrado (crescimento a partir dele).
    let cloud: Vec<bool> = (0..w * h)
        .map(|i| {
            let p = px[i];
            let mx = p[0].max(p[1]).max(p[2]);
            let mn = p[0].min(p[1]).min(p[2]);
            let sat = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
            l[i] > 0.42 && sat < 0.18 && tex[i] < 0.09
        })
        .collect();
    let mut stack: Vec<usize> = (0..w * h).filter(|&i| seen[i]).collect();
    while let Some(i) = stack.pop() {
        let (x, y) = (i % w, i / w);
        for j in [
            (x > 0).then(|| i - 1),
            (x + 1 < w).then(|| i + 1),
            (y > 0).then(|| i - w),
            (y + 1 < h).then(|| i + w),
        ]
        .into_iter()
        .flatten()
        {
            if !seen[j] && (cloud[j] || cand[j]) {
                seen[j] = true;
                stack.push(j);
            }
        }
    }
    let area = seen.iter().filter(|&&s| s).count() as f32 / (w * h) as f32;
    if area < 0.05 {
        return vec![0.0; w * h];
    }
    let m: Vec<f32> = seen.iter().map(|&s| if s { 1.0 } else { 0.0 }).collect();
    guided_filter(&l, &m, w, h, (w.max(h) / 128).max(2), 1e-3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn blue_sky_above_a_building_and_none_indoors() {
        let img = RgbImage::from_fn(400, 300, |x, y| {
            if y < 120 && !(150..220).contains(&x) {
                Rgb([110, 160, 225])
            } else if y < 120 {
                Rgb([90, 80, 70]) // torre que toca no topo
            } else {
                let t = ((x / 6 + y / 6) % 2) as u8 * 60;
                Rgb([60 + t, 90 + t, 50])
            }
        });
        let m = sky_mask(&img);
        let at = |x: usize, y: usize| m[y * 400 + x];
        assert!(at(50, 40) > 0.9 && at(350, 100) > 0.9);
        assert!(at(185, 40) < 0.2, "torre");
        assert!(at(50, 250) < 0.1, "relva");
        // Interior escuro: sem céu.
        let dark =
            RgbImage::from_fn(200, 150, |x, y| Rgb([(40 + x % 7) as u8, (35 + y % 5) as u8, 30]));
        assert!(sky_mask(&dark).iter().all(|&v| v == 0.0));
    }
}
