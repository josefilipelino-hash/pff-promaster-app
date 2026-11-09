//! Conversão de imagens em tensores NHWC f32.

use image::RgbImage;
use tract_onnx::prelude::*;

/// `(v / 255) * (max − min) + min` por canal.
pub fn to_nhwc(img: &RgbImage, min: f32, max: f32) -> Tensor {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let scale = (max - min) / 255.0;
    let data: Vec<f32> = img.as_raw().iter().map(|&v| v as f32 * scale + min).collect();
    tract_ndarray::Array4::from_shape_vec((1, h, w, 3), data).expect("dimensões").into()
}

/// Amostragem bilinear de um retângulo rodado (centro, tamanho, ângulo em radianos)
/// para uma imagem `out`×`out`. Fora da imagem: preto (como o BORDER_ZERO do MediaPipe).
pub fn crop_rotated(
    img: &RgbImage,
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    angle: f32,
    out: u32,
) -> RgbImage {
    let (sin, cos) = angle.sin_cos();
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    let px = |x: i64, y: i64| -> [f32; 3] {
        if x < 0 || y < 0 || x >= iw || y >= ih {
            return [0.0; 3];
        }
        let p = img.get_pixel(x as u32, y as u32).0;
        [p[0] as f32, p[1] as f32, p[2] as f32]
    };
    RgbImage::from_fn(out, out, |ox, oy| {
        let u = (ox as f32 + 0.5) / out as f32 - 0.5;
        let v = (oy as f32 + 0.5) / out as f32 - 0.5;
        let (lx, ly) = (u * w, v * h);
        let x = cx + lx * cos - ly * sin - 0.5;
        let y = cy + lx * sin + ly * cos - 0.5;
        let (x0, y0) = (x.floor() as i64, y.floor() as i64);
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let (a, b, c, d) = (px(x0, y0), px(x0 + 1, y0), px(x0, y0 + 1), px(x0 + 1, y0 + 1));
        let mut o = [0u8; 3];
        for i in 0..3 {
            let top = a[i] * (1.0 - fx) + b[i] * fx;
            let bot = c[i] * (1.0 - fx) + d[i] * fx;
            o[i] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
        }
        image::Rgb(o)
    })
}
