//! Segmentação local (Fase 8) para máscaras de IA — modelos MediaPipe (Apache-2.0):
//!
//! - `segment_multiclass.onnx` (*selfie multiclass* 256×256): fundo, cabelo, pele do corpo,
//!   pele do rosto, roupa, outros (acessórios) → pessoa, cabelo, pele, rosto, roupa, fundo;
//! - `segment_object.onnx` (*magic touch* 512×512): o objeto/pessoa sob um ponto clicado.
//!
//! As saídas são de baixa resolução: a aplicação refina as bordas com um *guided filter*
//! guiado pela própria fotografia.

use image::{imageops, RgbImage};
use momento_core_types::{CoreError, CoreResult};
use std::path::Path;
use tract_onnx::prelude::*;

pub const MULTICLASS_FILE: &str = "segment_multiclass.onnx";
pub const OBJECT_FILE: &str = "segment_object.onnx";

/// Classes do modelo multiclasse.
pub const BACKGROUND: usize = 0;
pub const HAIR: usize = 1;
pub const BODY_SKIN: usize = 2;
pub const FACE_SKIN: usize = 3;
pub const CLOTHES: usize = 4;
pub const OTHERS: usize = 5;

/// Probabilidades por classe numa grelha com a proporção da fotografia.
pub struct ClassMaps {
    pub w: usize,
    pub h: usize,
    /// `probs[classe][y * w + x]`.
    pub probs: Vec<Vec<f32>>,
}

/// Mapa de probabilidade de uma só classe/objeto.
pub struct ProbMap {
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

pub struct Segmenter {
    multiclass: Option<crate::Plan>,
    object: Option<crate::Plan>,
}

fn load(path: &Path, shape: [usize; 4]) -> CoreResult<Option<crate::Plan>> {
    if !path.is_file() {
        return Ok(None);
    }
    let err = |e: TractError| CoreError::Other(format!("modelo {}: {e:#}", path.display()));
    let p = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(err)?
        .with_input_fact(0, f32::fact(shape).into())
        .map_err(err)?
        .into_optimized()
        .map_err(err)?
        .into_runnable()
        .map_err(err)?;
    Ok(Some(p))
}

/// Imagem colocada no canto superior esquerdo de um quadrado `n`×`n` (como no MediaPipe).
fn letterbox(img: &RgbImage, n: u32) -> (Vec<f32>, usize, usize) {
    let s = n as f32 / img.width().max(img.height()) as f32;
    let (w, h) = (
        ((img.width() as f32 * s).round() as u32).max(1),
        ((img.height() as f32 * s).round() as u32).max(1),
    );
    let r = imageops::resize(img, w, h, imageops::FilterType::Triangle);
    let mut out = vec![0f32; (n * n * 3) as usize];
    for (x, y, p) in r.enumerate_pixels() {
        let i = ((y * n + x) * 3) as usize;
        for c in 0..3 {
            out[i + c] = p[c] as f32 / 255.0;
        }
    }
    (out, w as usize, h as usize)
}

impl Segmenter {
    pub fn load(dir: &Path) -> CoreResult<Self> {
        Ok(Segmenter {
            multiclass: load(&dir.join(MULTICLASS_FILE), [1, 256, 256, 3])?,
            object: load(&dir.join(OBJECT_FILE), [1, 512, 512, 4])?,
        })
    }

    pub fn has_multiclass(&self) -> bool {
        self.multiclass.is_some()
    }

    pub fn has_object(&self) -> bool {
        self.object.is_some()
    }

    /// Probabilidades das 6 classes.
    pub fn classes(&self, img: &RgbImage) -> CoreResult<Option<ClassMaps>> {
        let Some(plan) = &self.multiclass else { return Ok(None) };
        let err = |e: TractError| CoreError::Other(format!("segmentação: {e:#}"));
        let (x, w, h) = letterbox(img, 256);
        let t = Tensor::from_shape(&[1, 256, 256, 3], &x).map_err(err)?;
        let out = plan.run(tvec!(t.into())).map_err(err)?;
        let o = out[0].to_plain_array_view::<f32>().map_err(err)?;
        let o = o.as_slice().ok_or_else(|| CoreError::Other("segmentação: saída".into()))?;
        let mut probs = vec![vec![0f32; w * h]; 6];
        for y in 0..h {
            for xx in 0..w {
                let base = (y * 256 + xx) * 6;
                let v = &o[base..base + 6];
                // As saídas já são probabilidades; normaliza por segurança.
                let sum: f32 = v.iter().map(|a| a.max(0.0)).sum::<f32>().max(1e-6);
                for c in 0..6 {
                    probs[c][y * w + xx] = v[c].max(0.0) / sum;
                }
            }
        }
        Ok(Some(ClassMaps { w, h, probs }))
    }

    /// Objeto sob o ponto (u, v) (coordenadas normalizadas da imagem).
    pub fn object_at(&self, img: &RgbImage, u: f32, v: f32) -> CoreResult<Option<ProbMap>> {
        let Some(plan) = &self.object else { return Ok(None) };
        let err = |e: TractError| CoreError::Other(format!("segmentação: {e:#}"));
        let (rgb, w, h) = letterbox(img, 512);
        let (cx, cy) = (u * w as f32, v * h as f32);
        let mut x = vec![0f32; 512 * 512 * 4];
        for i in 0..512 * 512 {
            let (px, py) = ((i % 512) as f32, (i / 512) as f32);
            x[i * 4..i * 4 + 3].copy_from_slice(&rgb[i * 3..i * 3 + 3]);
            // Ponto clicado como mancha gaussiana (σ = 6 px), como no exemplo do MediaPipe.
            x[i * 4 + 3] = (-((px - cx).powi(2) + (py - cy).powi(2)) / 72.0).exp();
        }
        let t = Tensor::from_shape(&[1, 512, 512, 4], &x).map_err(err)?;
        let out = plan.run(tvec!(t.into())).map_err(err)?;
        let o = out[0].to_plain_array_view::<f32>().map_err(err)?;
        let o = o.as_slice().ok_or_else(|| CoreError::Other("segmentação: saída".into()))?;
        let mut data = vec![0f32; w * h];
        for y in 0..h {
            for xx in 0..w {
                data[y * w + xx] = o[y * 512 + xx].clamp(0.0, 1.0);
            }
        }
        Ok(Some(ProbMap { w, h, data }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Retrato de referência do MediaPipe (`portrait.jpg`): pessoa, rosto e cabelo onde
    /// devem estar. Requer os modelos e `MOMENTO_FACES_DIR`.
    #[test]
    fn portrait_classes_and_object() {
        let (Some(dir), Ok(faces)) =
            (crate::find_models_dir(&[]), std::env::var("MOMENTO_FACES_DIR"))
        else {
            eprintln!("modelos ou fotografias em falta: teste ignorado");
            return;
        };
        let seg = Segmenter::load(&dir).unwrap();
        if !seg.has_multiclass() || !seg.has_object() {
            eprintln!("modelos de segmentação em falta: teste ignorado");
            return;
        }
        let img = image::open(Path::new(&faces).join("portrait.jpg")).unwrap().to_rgb8();
        let c = seg.classes(&img).unwrap().unwrap();
        let at = |cls: usize, u: f32, v: f32| {
            c.probs[cls][((v * c.h as f32) as usize).min(c.h - 1) * c.w
                + ((u * c.w as f32) as usize).min(c.w - 1)]
        };
        assert!(at(FACE_SKIN, 0.5, 0.22) > 0.5, "rosto");
        assert!(at(HAIR, 0.5, 0.06) > 0.3, "cabelo");
        assert!(at(CLOTHES, 0.5, 0.75) > 0.5, "roupa");
        assert!(at(BACKGROUND, 0.04, 0.3) > 0.5, "fundo (bandeira)");
        let o = seg.object_at(&img, 0.5, 0.75).unwrap().unwrap();
        let ot =
            |u: f32, v: f32| o.data[((v * o.h as f32) as usize) * o.w + (u * o.w as f32) as usize];
        assert!(ot(0.5, 0.6) > 0.5 && ot(0.03, 0.5) < 0.5, "objeto = pessoa clicada");
    }
}
