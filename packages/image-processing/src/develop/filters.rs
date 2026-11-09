//! Filtros espaciais em planos f32 (paralelos por linhas).

use rayon::prelude::*;

/// Desfocagem de caixa separável (soma corrida), raio em píxeis, bordas replicadas.
pub fn box_blur(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    if r == 0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let mut tmp = vec![0.0f32; w * h];
    let norm = 1.0 / (2 * r + 1) as f32;
    tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let s = &src[y * w..(y + 1) * w];
        let at = |i: isize| s[i.clamp(0, w as isize - 1) as usize];
        let mut acc: f32 = (-(r as isize)..=r as isize).map(at).sum();
        for (x, o) in row.iter_mut().enumerate() {
            *o = acc * norm;
            acc += at(x as isize + r as isize + 1) - at(x as isize - r as isize);
        }
    });
    let mut out = vec![0.0f32; w * h];
    // Vertical: por colunas, em blocos de linhas de saída.
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let y0 = y as isize;
        for (x, o) in row.iter_mut().enumerate() {
            let mut acc = 0.0;
            for k in -(r as isize)..=r as isize {
                let yy = (y0 + k).clamp(0, h as isize - 1) as usize;
                acc += tmp[yy * w + x];
            }
            *o = acc * norm;
        }
    });
    out
}

/// Aproximação gaussiana: três caixas (σ ≈ r·√(1/3·3)…). `sigma` em píxeis.
pub fn gaussian(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    if sigma < 0.3 {
        return src.to_vec();
    }
    if sigma < 1.5 {
        return small_gaussian(src, w, h, sigma);
    }
    // Três passagens de caixa com raio r: σ² = 3·((2r+1)² − 1)/12.
    let r = (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) / 2.0).round().max(1.0) as usize;
    let a = box_blur(src, w, h, r);
    let b = box_blur(&a, w, h, r);
    box_blur(&b, w, h, r)
}

/// Convolução gaussiana exata separável (para raios pequenos, ex.: nitidez).
pub fn small_gaussian(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let k = (sigma * 3.0).ceil().max(1.0) as isize;
    let mut kern: Vec<f32> =
        (-k..=k).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let s: f32 = kern.iter().sum();
    kern.iter_mut().for_each(|v| *v /= s);
    let mut tmp = vec![0.0f32; w * h];
    tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut acc = 0.0;
            for (j, kv) in kern.iter().enumerate() {
                let xx = (x as isize + j as isize - k).clamp(0, w as isize - 1) as usize;
                acc += src[y * w + xx] * kv;
            }
            *o = acc;
        }
    });
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut acc = 0.0;
            for (j, kv) in kern.iter().enumerate() {
                let yy = (y as isize + j as isize - k).clamp(0, h as isize - 1) as usize;
                acc += tmp[yy * w + x] * kv;
            }
            *o = acc;
        }
    });
    out
}

/// Mínimo numa janela quadrada (separável), para o “canal escuro” do dehaze.
pub fn min_filter(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            tmp[y * w + x] = src[y * w + a..=y * w + b].iter().copied().fold(f32::MAX, f32::min);
        }
    }
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            out[y * w + x] = (a..=b).map(|yy| tmp[yy * w + x]).fold(f32::MAX, f32::min);
        }
    }
    out
}

/// Amostragem bilinear de um plano em coordenadas normalizadas (0–1).
#[inline]
pub fn sample_plane(p: &[f32], w: usize, h: usize, u: f32, v: f32) -> f32 {
    let x = (u * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
    let y = (v * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x as usize, y as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let a = p[y0 * w + x0] * (1.0 - fx) + p[y0 * w + x1] * fx;
    let b = p[y1 * w + x0] * (1.0 - fx) + p[y1 * w + x1] * fx;
    a * (1.0 - fy) + b * fy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blurs_preserve_mean_and_reduce_variance() {
        let (w, h) = (64, 48);
        let src: Vec<f32> =
            (0..w * h).map(|i| if (i / w + i % w) % 2 == 0 { 1.0 } else { 0.0 }).collect();
        for out in
            [box_blur(&src, w, h, 2), gaussian(&src, w, h, 3.0), small_gaussian(&src, w, h, 0.8)]
        {
            let m: f32 = out.iter().sum::<f32>() / out.len() as f32;
            assert!((m - 0.5).abs() < 0.02, "{m}");
            let var: f32 = out.iter().map(|v| (v - 0.5).powi(2)).sum::<f32>() / out.len() as f32;
            assert!(var < 0.2, "{var}");
        }
        let mins = min_filter(&src, w, h, 1);
        assert!(mins.iter().all(|&v| v == 0.0));
    }
}
