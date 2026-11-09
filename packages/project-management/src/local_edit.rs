//! Edição localizada (Fase 8): máscaras de IA guardadas no projeto, regiões do retrato,
//! mapeamento de pontos entre a vista e a fotografia, origem automática do heal.
//!
//! As máscaras de IA são **imagens** (`<projeto>/masks/<id>.png`, 8 bits, lado maior 1024)
//! calculadas uma vez e referenciadas pela receita; podem ser apagadas da receita a qualquer
//! momento sem afetar o original. Os pontos faciais do retrato ficam na receita (contornos),
//! não no catálogo de pessoas.

use crate::session::ProjectSession;
use image::{GrayImage, Luma, RgbImage};
use momento_core_types::{CoreError, CoreResult};
use momento_face_analysis::segment::{self, Segmenter};
use momento_face_analysis::VisionModels;
use momento_image_processing::develop::filters::guided_filter;
use momento_image_processing::develop::geometry::{source_to_view, view_to_source};
use momento_image_processing::develop::local::{raster_from_gray, register_raster};
use momento_image_processing::develop::{self, FaceRegions, Quality, Settings};
use serde::Serialize;
use std::sync::Arc;

/// Lado maior das máscaras guardadas.
pub const MASK_EDGE: u32 = 1024;

/// Máscara criada.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaskRasterInfo {
    pub raster: String,
    pub kind: String,
    /// Fração da imagem coberta (0–1): 0 = nada encontrado.
    pub coverage: f32,
}

const FACE_OVAL: [usize; 36] = [
    10, 338, 297, 332, 284, 251, 389, 356, 454, 323, 361, 288, 397, 365, 379, 378, 400, 377, 152,
    148, 176, 149, 150, 136, 172, 58, 132, 93, 234, 127, 162, 21, 54, 103, 67, 109,
];
const LEFT_EYE: [usize; 16] =
    [263, 249, 390, 373, 374, 380, 381, 382, 362, 398, 384, 385, 386, 387, 388, 466];
const RIGHT_EYE: [usize; 16] =
    [33, 7, 163, 144, 145, 153, 154, 155, 133, 173, 157, 158, 159, 160, 161, 246];
const LEFT_BROW: [usize; 10] = [276, 283, 282, 295, 285, 300, 293, 334, 296, 336];
const RIGHT_BROW: [usize; 10] = [46, 53, 52, 65, 55, 70, 63, 105, 66, 107];
const LIPS_OUTER: [usize; 20] =
    [61, 146, 91, 181, 84, 17, 314, 405, 321, 375, 291, 409, 270, 269, 267, 0, 37, 39, 40, 185];
const LIPS_INNER: [usize; 20] =
    [78, 95, 88, 178, 87, 14, 317, 402, 318, 324, 308, 415, 310, 311, 312, 13, 82, 81, 80, 191];
/// Pálpebra inferior (canto a canto) de cada olho.
const LEFT_LOWER: [usize; 9] = [263, 249, 390, 373, 374, 380, 381, 382, 362];
const RIGHT_LOWER: [usize; 9] = [33, 7, 163, 144, 145, 153, 154, 155, 133];

/// Envolvente convexa (sobrancelhas: os pontos não estão por ordem de contorno).
fn hull(mut p: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut lower: Vec<[f32; 2]> = Vec::new();
    for &q in &p {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], q) <= 0.0 {
            lower.pop();
        }
        lower.push(q);
    }
    let mut upper: Vec<[f32; 2]> = Vec::new();
    for &q in p.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], q) <= 0.0 {
            upper.pop();
        }
        upper.push(q);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Regiões do retrato a partir dos 478 pontos (em píxeis de uma imagem `w`×`h`).
pub fn face_regions(l: &[(f32, f32)], w: f32, h: f32) -> Option<FaceRegions> {
    if l.len() < 478 {
        return None;
    }
    let p = |i: usize| [l[i].0 / w, l[i].1 / h];
    let poly = |ix: &[usize]| ix.iter().map(|&i| p(i)).collect::<Vec<_>>();
    // Olheiras: faixa abaixo da pálpebra inferior, com a altura de ~35 % da largura do olho.
    let under = |lower: &[usize]| -> Vec<[f32; 2]> {
        let (a, b) = (l[lower[0]], l[lower[lower.len() - 1]]);
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let len = (ex * ex + ey * ey).sqrt().max(1.0);
        // Normal "para baixo" no rosto (perpendicular ao eixo do olho, sentido +y).
        let (mut nx, mut ny) = (-ey / len, ex / len);
        if ny < 0.0 {
            nx = -nx;
            ny = -ny;
        }
        let off = len * 0.35;
        let top: Vec<[f32; 2]> = lower
            .iter()
            .map(|&i| [(l[i].0 + nx * len * 0.05) / w, (l[i].1 + ny * len * 0.05) / h])
            .collect();
        let mut bottom: Vec<[f32; 2]> =
            lower.iter().map(|&i| [(l[i].0 + nx * off) / w, (l[i].1 + ny * off) / h]).collect();
        bottom.reverse();
        top.into_iter().chain(bottom).collect()
    };
    let long = w.max(h);
    let iris = |c: usize, e: usize| {
        let r = ((l[c].0 - l[e].0).powi(2) + (l[c].1 - l[e].1).powi(2)).sqrt();
        [l[c].0 / w, l[c].1 / h, r / long]
    };
    Some(FaceRegions {
        oval: poly(&FACE_OVAL),
        left_eye: poly(&LEFT_EYE),
        right_eye: poly(&RIGHT_EYE),
        left_brow: hull(poly(&LEFT_BROW)),
        right_brow: hull(poly(&RIGHT_BROW)),
        lips_outer: poly(&LIPS_OUTER),
        lips_inner: poly(&LIPS_INNER),
        under_left: under(&LEFT_LOWER),
        under_right: under(&RIGHT_LOWER),
        left_iris: iris(473, 474),
        right_iris: iris(468, 469),
    })
}

/// Probabilidade de baixa resolução → máscara 8 bits com as arestas da fotografia.
fn refine(prob: &[f32], pw: usize, ph: usize, img: &RgbImage) -> GrayImage {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let up: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = (
                (i % w) as f32 / w as f32 * pw as f32 - 0.5,
                (i / w) as f32 / h as f32 * ph as f32 - 0.5,
            );
            let (x0, y0) = (x.clamp(0.0, (pw - 1) as f32), y.clamp(0.0, (ph - 1) as f32));
            let (xi, yi) = (x0 as usize, y0 as usize);
            let (x1, y1) = ((xi + 1).min(pw - 1), (yi + 1).min(ph - 1));
            let (fx, fy) = (x0 - xi as f32, y0 - yi as f32);
            let a = prob[yi * pw + xi] * (1.0 - fx) + prob[yi * pw + x1] * fx;
            let b = prob[y1 * pw + xi] * (1.0 - fx) + prob[y1 * pw + x1] * fx;
            a * (1.0 - fy) + b * fy
        })
        .collect();
    let guide: Vec<f32> = img
        .pixels()
        .map(|p| (0.2627 * p[0] as f32 + 0.678 * p[1] as f32 + 0.0593 * p[2] as f32) / 255.0)
        .collect();
    let q = guided_filter(&guide, &up, w, h, (w.max(h) / 128).max(4), 1e-3);
    GrayImage::from_fn(w as u32, h as u32, |x, y| {
        Luma([(q[y as usize * w + x as usize] * 255.0).round() as u8])
    })
}

impl ProjectSession {
    fn neutral_image(self: &Arc<Self>, key: i64, edge: u32) -> CoreResult<RgbImage> {
        let cat = self.open_catalog()?;
        let (_, asset) = cat.resolve_key(key)?;
        let src = self.edits.source(&cat, self.registry(), asset)?;
        Ok(develop::render_rgb(&src, &Settings::default(), edge, Quality::Final))
    }

    /// Cria uma máscara de IA (`person`, `subject`, `face`, `skin`, `hair`, `clothes`,
    /// `background`, `object` com `point`) ou de céu (`sky`). Guardada em `<projeto>/masks`.
    pub fn create_mask_raster(
        self: &Arc<Self>,
        key: i64,
        kind: &str,
        point: Option<[f32; 2]>,
    ) -> CoreResult<MaskRasterInfo> {
        let img = self.neutral_image(key, MASK_EDGE)?;
        let (w, h) = (img.width() as usize, img.height() as usize);
        let missing = || {
            CoreError::Other("modelo de segmentação em falta (scripts/prepare-models.py)".into())
        };
        let seg = || -> CoreResult<Segmenter> {
            let dir = crate::session::models_dir().ok_or_else(missing)?;
            Segmenter::load(&dir)
        };
        let gray: GrayImage = match kind {
            "sky" => {
                let m = develop::sky::sky_mask(&img);
                GrayImage::from_fn(w as u32, h as u32, |x, y| {
                    Luma([(m[y as usize * w + x as usize] * 255.0).round() as u8])
                })
            }
            "object" | "subject" => {
                let s = seg()?;
                let mut out = None;
                if kind == "subject" {
                    // Sujeito = as pessoas, quando existem; senão o objeto ao centro.
                    if let Some(c) = s.classes(&img)? {
                        let person: Vec<f32> =
                            c.probs[segment::BACKGROUND].iter().map(|b| 1.0 - b).collect();
                        if person.iter().sum::<f32>() / person.len() as f32 > 0.02 {
                            out = Some(refine(&person, c.w, c.h, &img));
                        }
                    }
                }
                match out {
                    Some(g) => g,
                    None => {
                        let [u, v] = point.unwrap_or([0.5, 0.5]);
                        let o = s.object_at(&img, u, v)?.ok_or_else(missing)?;
                        refine(&o.data, o.w, o.h, &img)
                    }
                }
            }
            _ => {
                let s = seg()?;
                let c = s.classes(&img)?.ok_or_else(missing)?;
                let pick = |ix: &[usize]| -> Vec<f32> {
                    (0..c.w * c.h)
                        .map(|i| ix.iter().map(|&k| c.probs[k][i]).sum::<f32>().min(1.0))
                        .collect()
                };
                let prob = match kind {
                    "person" => c.probs[segment::BACKGROUND].iter().map(|b| 1.0 - b).collect(),
                    "background" => c.probs[segment::BACKGROUND].clone(),
                    "hair" => pick(&[segment::HAIR]),
                    "face" => pick(&[segment::FACE_SKIN]),
                    "skin" => pick(&[segment::FACE_SKIN, segment::BODY_SKIN]),
                    "clothes" => pick(&[segment::CLOTHES, segment::OTHERS]),
                    other => {
                        return Err(CoreError::Other(format!(
                            "tipo de máscara desconhecido: {other}"
                        )))
                    }
                };
                refine(&prob, c.w, c.h, &img)
            }
        };
        let coverage = gray.pixels().map(|p| p[0] as f32 / 255.0).sum::<f32>() / (w * h) as f32;
        let mut png = Vec::new();
        image::DynamicImage::ImageLuma8(gray.clone())
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| CoreError::Other(e.to_string()))?;
        let id = blake3::hash(&png).to_hex()[..32].to_string();
        let dir = self.paths.root.join("masks");
        std::fs::create_dir_all(&dir).map_err(|e| CoreError::Other(e.to_string()))?;
        momento_image_processing::write_atomic(&dir.join(format!("{id}.png")), &png)?;
        register_raster(&id, raster_from_gray(&gray));
        Ok(MaskRasterInfo { raster: id, kind: kind.into(), coverage })
    }

    /// Rostos para o retrato (regiões da malha facial local).
    pub fn detect_portrait(self: &Arc<Self>, key: i64) -> CoreResult<Vec<FaceRegions>> {
        let dir = crate::session::models_dir()
            .ok_or_else(|| CoreError::Other("modelos de rostos em falta".into()))?;
        let models = VisionModels::load(&dir)?;
        let img = self.neutral_image(key, 1600)?;
        let (w, h) = (img.width() as f32, img.height() as f32);
        Ok(models
            .analyze_faces(&img, true)?
            .iter()
            .filter_map(|f| face_regions(&f.landmarks, w, h))
            .collect())
    }

    /// Converte pontos entre a vista (0–1, depois da geometria) e a fotografia orientada.
    pub fn map_points(
        self: &Arc<Self>,
        key: i64,
        settings: &Settings,
        points: &[[f32; 2]],
        to_source: bool,
        uncropped: bool,
    ) -> CoreResult<Vec<Option<[f32; 2]>>> {
        let cat = self.open_catalog()?;
        let (_, asset) = cat.resolve_key(key)?;
        let src = self.edits.source(&cat, self.registry(), asset)?;
        let (sw, sh) = (src.width(), src.height());
        let (g, l) = (&settings.geometry, &settings.lens);
        Ok(points
            .iter()
            .map(|p| {
                if to_source {
                    let (x, y) = view_to_source(sw, sh, g, l, uncropped, p[0], p[1]);
                    Some([x, y])
                } else {
                    source_to_view(sw, sh, g, l, uncropped, p[0], p[1]).map(|(u, v)| [u, v])
                }
            })
            .collect())
    }

    /// Origem automática para remover uma mancha em (x, y) com raio `r` (lado maior).
    pub fn suggest_heal_source(
        self: &Arc<Self>,
        key: i64,
        x: f32,
        y: f32,
        r: f32,
    ) -> CoreResult<[f32; 2]> {
        let cat = self.open_catalog()?;
        let (_, asset) = cat.resolve_key(key)?;
        let src = self.edits.source(&cat, self.registry(), asset)?;
        let (sx, sy) = develop::local::suggest_source(&src, x, y, r);
        Ok([sx, sy])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_and_regions() {
        let h = hull(vec![[0.0, 0.0], [1.0, 0.0], [0.5, 0.2], [1.0, 1.0], [0.0, 1.0], [0.5, 0.5]]);
        assert_eq!(h.len(), 4);
        // Malha sintética: todos os pontos num círculo; regiões com os tamanhos certos.
        let pts: Vec<(f32, f32)> = (0..478)
            .map(|i| {
                let a = i as f32 * 0.1;
                (500.0 + a.cos() * 100.0, 400.0 + a.sin() * 100.0)
            })
            .collect();
        let r = face_regions(&pts, 1000.0, 800.0).unwrap();
        assert_eq!((r.oval.len(), r.lips_inner.len(), r.under_left.len()), (36, 20, 18));
        assert!(r.left_iris[2] > 0.0);
        assert!(face_regions(&pts[..400], 1000.0, 800.0).is_none());
    }
}
