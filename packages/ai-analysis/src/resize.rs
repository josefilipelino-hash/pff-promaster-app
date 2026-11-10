//! Redução rápida de imagens (filtro triangular, como `imageops::FilterType::Triangle`).
//!
//! Medido na Fase 12 (`perf`, análise de fotografias de 24 MP): `imageops::resize` era
//! 51 % do tempo da análise — mais do que os modelos de IA. Esta versão faz o mesmo
//! filtro separável com pesos pré-calculados em vírgula fixa e linhas em paralelo; o
//! resultado difere de `imageops` em no máximo ±2 níveis (teste abaixo).

use image::{GrayImage, ImageBuffer, Pixel, RgbImage};
use rayon::prelude::*;

const SHIFT: u32 = 14;
const ONE: i32 = 1 << SHIFT;

/// Pesos (início, pesos em vírgula fixa) por píxel de saída.
fn weights(src: usize, dst: usize) -> Vec<(usize, Vec<i32>)> {
    let ratio = src as f32 / dst as f32;
    let scale = ratio.max(1.0);
    let support = scale; // triângulo de raio 1, alargado na redução
    (0..dst)
        .map(|i| {
            let c = (i as f32 + 0.5) * ratio;
            let lo = ((c - support).floor().max(0.0)) as usize;
            let hi = ((c + support).ceil() as usize).min(src);
            let mut w: Vec<f32> =
                (lo..hi).map(|j| (1.0 - ((j as f32 + 0.5 - c) / scale).abs()).max(0.0)).collect();
            let sum: f32 = w.iter().sum();
            if sum > 0.0 {
                w.iter_mut().for_each(|v| *v /= sum);
            }
            // Vírgula fixa com soma exata (o resto vai para o maior peso).
            let mut q: Vec<i32> = w.iter().map(|v| (v * ONE as f32).round() as i32).collect();
            let diff = ONE - q.iter().sum::<i32>();
            if let Some((k, _)) = q.iter().enumerate().max_by_key(|(_, v)| **v) {
                q[k] += diff;
            }
            (lo, q)
        })
        .collect()
}

fn resize_impl<P: Pixel<Subpixel = u8> + Send + Sync>(
    img: &ImageBuffer<P, Vec<u8>>,
    w: u32,
    h: u32,
) -> ImageBuffer<P, Vec<u8>> {
    let ch = P::CHANNEL_COUNT as usize;
    let (sw, sh) = (img.width() as usize, img.height() as usize);
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let wx = weights(sw, w);
    let wy = weights(sh, h);
    let src = img.as_raw();
    // Passo horizontal: todas as linhas da origem → largura final (i32, vírgula fixa).
    let mut tmp = vec![0i32; w * ch * sh];
    tmp.par_chunks_mut(w * ch).enumerate().for_each(|(y, row)| {
        let line = &src[y * sw * ch..(y + 1) * sw * ch];
        for (x, (lo, ws)) in wx.iter().enumerate() {
            for c in 0..ch {
                let mut acc = 0i32;
                for (k, wt) in ws.iter().enumerate() {
                    acc += line[(lo + k) * ch + c] as i32 * wt;
                }
                row[x * ch + c] = acc;
            }
        }
    });
    // Passo vertical.
    let mut out = vec![0u8; w * h * ch];
    out.par_chunks_mut(w * ch).enumerate().for_each(|(y, row)| {
        let (lo, ws) = &wy[y];
        for (i, o) in row.iter_mut().enumerate() {
            let mut acc = 0i64;
            for (k, wt) in ws.iter().enumerate() {
                acc += tmp[(lo + k) * w * ch + i] as i64 * *wt as i64;
            }
            let v = (acc + (1i64 << (2 * SHIFT - 1))) >> (2 * SHIFT);
            *o = v.clamp(0, 255) as u8;
        }
    });
    ImageBuffer::from_raw(w as u32, h as u32, out).expect("dimensões coerentes")
}

/// Reduz (ou amplia) uma imagem RGB para `w`×`h`.
pub fn rgb(img: &RgbImage, w: u32, h: u32) -> RgbImage {
    resize_impl(img, w, h)
}

/// Reduz (ou amplia) uma imagem em tons de cinzento para `w`×`h`.
pub fn gray(img: &GrayImage, w: u32, h: u32) -> GrayImage {
    resize_impl(img, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::imageops;

    fn noise(w: u32, h: u32) -> RgbImage {
        let mut s = 1u32;
        RgbImage::from_fn(w, h, |x, y| {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let n = (s >> 24) as u8;
            image::Rgb([(x * 7 + y) as u8 ^ n, (y * 3) as u8, n])
        })
    }

    #[test]
    fn matches_imageops_triangle_within_two_levels() {
        let img = noise(1031, 677);
        for (w, h) in [(800, 525), (256, 168), (224, 224), (64, 42), (1031, 677)] {
            let a = imageops::resize(&img, w, h, imageops::FilterType::Triangle);
            let b = rgb(&img, w, h);
            let worst =
                a.as_raw().iter().zip(b.as_raw()).map(|(x, y)| (*x as i32 - *y as i32).abs()).max();
            assert!(worst.unwrap() <= 2, "{w}×{h}: diferença {worst:?}");
        }
        let g = imageops::grayscale(&img);
        let a = imageops::resize(&g, 300, 197, imageops::FilterType::Triangle);
        let b = gray(&g, 300, 197);
        let worst =
            a.as_raw().iter().zip(b.as_raw()).map(|(x, y)| (*x as i32 - *y as i32).abs()).max();
        assert!(worst.unwrap() <= 2, "cinzento: {worst:?}");
    }
}
