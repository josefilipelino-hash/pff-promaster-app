//! Carregamento dos modelos e API de alto nível.

use crate::detector::{Detector, FaceDetection};
use crate::mesh::{run_blendshapes, run_mesh};
use crate::tensor::to_nhwc;
use crate::FaceResult;
use image::{imageops, RgbImage};
use momento_core_types::{CoreError, CoreResult};
use std::path::{Path, PathBuf};
use tract_onnx::prelude::*;
use tract_onnx::tract_hir::infer::Factoid;

pub const MODEL_FILES: [&str; 4] = [
    "face_detector.onnx",
    "face_landmarks_detector.onnx",
    "face_blendshapes.onnx",
    "image_embedder.onnx",
];

/// Procura a pasta dos modelos: `MOMENTO_MODELS_DIR`, recursos da aplicação, pasta `models/`
/// do repositório (desenvolvimento).
pub fn find_models_dir(extra: &[PathBuf]) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(d) = std::env::var("MOMENTO_MODELS_DIR") {
        candidates.push(PathBuf::from(d));
    }
    candidates.extend_from_slice(extra);
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models"));
    candidates.into_iter().find(|d| MODEL_FILES.iter().all(|f| d.join(f).is_file()))
}

fn load(path: &Path) -> CoreResult<crate::Plan> {
    let err = |e: TractError| CoreError::Other(format!("modelo {}: {e:#}", path.display()));
    let mut m = tract_onnx::onnx().model_for_path(path).map_err(err)?;
    for i in 0..m.input_outlets().map_err(err)?.len() {
        let f = m.input_fact(i).map_err(err)?.clone();
        let dims: Vec<usize> = f
            .shape
            .dims()
            .map(|d| d.concretize().and_then(|d| d.to_i64().ok()).map(|v| v as usize).unwrap_or(1))
            .collect();
        m.set_input_fact(i, f32::fact(&dims).into()).map_err(err)?;
    }
    m.into_optimized().map_err(err)?.into_runnable().map_err(err)
}

/// Modelos carregados. Cada instância deve ser usada por uma thread (tract não é Sync
/// durante a execução); cria-se uma por thread de análise.
pub struct VisionModels {
    detector: Detector,
    mesh: crate::Plan,
    blend: crate::Plan,
    embedder: crate::Plan,
}

impl VisionModels {
    pub fn load(dir: &Path) -> CoreResult<Self> {
        Ok(VisionModels {
            detector: Detector::new(load(&dir.join(MODEL_FILES[0]))?),
            mesh: load(&dir.join(MODEL_FILES[1]))?,
            blend: load(&dir.join(MODEL_FILES[2]))?,
            embedder: load(&dir.join(MODEL_FILES[3]))?,
        })
    }

    pub fn detect_faces(&self, img: &RgbImage, multiscale: bool) -> CoreResult<Vec<FaceDetection>> {
        self.detector
            .detect(img, multiscale)
            .map_err(|e| CoreError::Other(format!("deteção de rostos: {e:#}")))
    }

    /// Deteção + pontos + blendshapes para cada rosto com presença ≥ 0,5.
    pub fn analyze_faces(&self, img: &RgbImage, multiscale: bool) -> CoreResult<Vec<FaceResult>> {
        let mut out = Vec::new();
        for d in self.detect_faces(img, multiscale)? {
            let mesh = run_mesh(&self.mesh, img, &d)
                .map_err(|e| CoreError::Other(format!("pontos faciais: {e:#}")))?;
            if mesh.presence < 0.5 {
                continue;
            }
            let blendshapes = run_blendshapes(&self.blend, &mesh)
                .map_err(|e| CoreError::Other(format!("blendshapes: {e:#}")))?;
            out.push(FaceResult {
                detection: d,
                presence: mesh.presence,
                blendshapes,
                landmarks: mesh.points.iter().map(|p| (p.0, p.1)).collect(),
            });
        }
        Ok(out)
    }

    /// Vetor visual normalizado (norma L2 = 1) da imagem inteira.
    pub fn embed(&self, img: &RgbImage) -> CoreResult<Vec<f32>> {
        let small = imageops::resize(img, 224, 224, imageops::FilterType::Triangle);
        let out = self
            .embedder
            .run(tvec!(to_nhwc(&small, -1.0, 1.0).into()))
            .map_err(|e| CoreError::Other(format!("embedding: {e:#}")))?;
        let v: Vec<f32> = out[0]
            .to_plain_array_view::<f32>()
            .map_err(|e| CoreError::Other(e.to_string()))?
            .iter()
            .copied()
            .collect();
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        Ok(v.into_iter().map(|x| x / n).collect())
    }
}

#[cfg(test)]
mod tests;
