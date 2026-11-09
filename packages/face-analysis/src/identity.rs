//! Identidade facial: alinhamento por 5 pontos para o modelo de referência ArcFace
//! (112×112) e embedding de 128 dimensões com o **SFace** (OpenCV Zoo, Apache-2.0).
//!
//! O vetor serve apenas para agrupar rostos da mesma pessoa **dentro do projeto**; fica no
//! catálogo local e nunca sai do computador.

use image::RgbImage;
use tract_onnx::prelude::*;

/// Posições de referência (112×112) usadas no treino do SFace/ArcFace: olho esquerdo e
/// direito (na imagem), ponta do nariz, cantos esquerdo e direito da boca.
pub const ARCFACE_TEMPLATE: [(f32, f32); 5] = [
    (38.2946, 51.6963),
    (73.5318, 51.5014),
    (56.0252, 71.7366),
    (41.5493, 92.3655),
    (70.7299, 92.2041),
];

/// Os 5 pontos a partir da malha de 478 pontos do MediaPipe.
pub fn five_points(landmarks: &[(f32, f32)]) -> Option<[(f32, f32); 5]> {
    if landmarks.len() < 468 {
        return None;
    }
    let mean = |ids: &[usize]| {
        let n = ids.len() as f32;
        let (sx, sy) =
            ids.iter().fold((0.0, 0.0), |a, &i| (a.0 + landmarks[i].0, a.1 + landmarks[i].1));
        (sx / n, sy / n)
    };
    Some([
        mean(&[33, 133, 159, 145]), // olho à esquerda na imagem (direito da pessoa)
        mean(&[362, 263, 386, 374]), // olho à direita na imagem
        landmarks[1],               // ponta do nariz
        landmarks[61],              // canto da boca à esquerda
        landmarks[291],             // canto da boca à direita
    ])
}

/// Transformação de semelhança (escala + rotação + translação) que leva `src` para `dst`
/// por mínimos quadrados (Umeyama). Devolve (a, b, tx, ty) com
/// x' = a·x − b·y + tx, y' = b·x + a·y + ty.
pub fn similarity(src: &[(f32, f32)], dst: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    let n = src.len() as f32;
    let (mx, my) = src.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
    let (ux, uy) = dst.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
    let (mut sxx, mut sxy, mut var) = (0.0f32, 0.0f32, 0.0f32);
    for (s, d) in src.iter().zip(dst) {
        let (x, y) = (s.0 - mx, s.1 - my);
        let (u, v) = (d.0 - ux, d.1 - uy);
        sxx += x * u + y * v;
        sxy += x * v - y * u;
        var += x * x + y * y;
    }
    let var = var.max(1e-6);
    let (a, b) = (sxx / var, sxy / var);
    (a, b, ux - (a * mx - b * my), uy - (b * mx + a * my))
}

/// Recorte alinhado 112×112 (bilinear; fora da imagem: preto).
pub fn align(img: &RgbImage, pts: &[(f32, f32); 5]) -> RgbImage {
    let (a, b, tx, ty) = similarity(pts, &ARCFACE_TEMPLATE);
    // Inversa: x = (a(u−tx) + b(v−ty)) / (a²+b²), y = (−b(u−tx) + a(v−ty)) / (a²+b²).
    let det = (a * a + b * b).max(1e-12);
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    let px = |x: i64, y: i64| -> [f32; 3] {
        if x < 0 || y < 0 || x >= iw || y >= ih {
            return [0.0; 3];
        }
        let p = img.get_pixel(x as u32, y as u32).0;
        [p[0] as f32, p[1] as f32, p[2] as f32]
    };
    RgbImage::from_fn(112, 112, |ox, oy| {
        let (u, v) = (ox as f32 - tx, oy as f32 - ty);
        let x = (a * u + b * v) / det;
        let y = (-b * u + a * v) / det;
        let (x0, y0) = (x.floor() as i64, y.floor() as i64);
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let (p00, p10, p01, p11) = (px(x0, y0), px(x0 + 1, y0), px(x0, y0 + 1), px(x0 + 1, y0 + 1));
        let mut o = [0u8; 3];
        for i in 0..3 {
            let top = p00[i] * (1.0 - fx) + p10[i] * fx;
            let bot = p01[i] * (1.0 - fx) + p11[i] * fx;
            o[i] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
        }
        image::Rgb(o)
    })
}

/// Embedding SFace (normalizado, norma L2 = 1) de um recorte alinhado 112×112.
/// Entrada do modelo: NCHW, RGB 0–255 (a normalização está dentro do modelo).
pub fn embed(plan: &crate::Plan, aligned: &RgbImage) -> TractResult<Vec<f32>> {
    let mut data = vec![0f32; 3 * 112 * 112];
    for (x, y, p) in aligned.enumerate_pixels() {
        for c in 0..3 {
            data[c * 112 * 112 + (y * 112 + x) as usize] = p.0[c] as f32;
        }
    }
    let t: Tensor = tract_ndarray::Array4::from_shape_vec((1, 3, 112, 112), data)?.into();
    let out = plan.run(tvec!(t.into()))?;
    let v: Vec<f32> = out[0].to_plain_array_view::<f32>()?.iter().copied().collect();
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
    Ok(v.into_iter().map(|x| x / n).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_recovers_a_known_transform() {
        // Rodar 30°, escalar 2×, deslocar (5, −3).
        let (s, c) = (30f32.to_radians().sin(), 30f32.to_radians().cos());
        let src: Vec<(f32, f32)> = ARCFACE_TEMPLATE.to_vec();
        let dst: Vec<(f32, f32)> = src
            .iter()
            .map(|p| (2.0 * (c * p.0 - s * p.1) + 5.0, 2.0 * (s * p.0 + c * p.1) - 3.0))
            .collect();
        let (a, b, tx, ty) = similarity(&src, &dst);
        assert!((a - 2.0 * c).abs() < 1e-3 && (b - 2.0 * s).abs() < 1e-3, "{a} {b}");
        assert!((tx - 5.0).abs() < 1e-2 && (ty + 3.0).abs() < 1e-2);
    }

    #[test]
    fn align_maps_template_points_onto_themselves() {
        // Imagem 112×112 com um ponto branco em cada posição do modelo: alinhar com os
        // próprios pontos é a identidade.
        let mut img = RgbImage::new(112, 112);
        for p in ARCFACE_TEMPLATE {
            img.put_pixel(p.0.round() as u32, p.1.round() as u32, image::Rgb([255, 255, 255]));
        }
        let out = align(&img, &ARCFACE_TEMPLATE);
        assert_eq!(out.get_pixel(38, 52), img.get_pixel(38, 52));
        assert_eq!(out.as_raw(), img.as_raw());
    }
}
