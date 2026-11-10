//! Carregamento dos modelos e API de alto nível.

use crate::detector::{Detector, FaceDetection};
use crate::mesh::{run_blendshapes, run_mesh};
use crate::tensor::to_nhwc;
use crate::FaceResult;
use image::RgbImage;
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
/// Modelos opcionais (Fase 4): sem eles, não há agrupamento de pessoas nem pistas de cena.
pub const OPTIONAL_MODEL_FILES: [&str; 2] = ["face_identity.onnx", "scene_classifier.onnx"];
/// Largura mínima de um rosto (px) para calcular a identidade (abaixo disso é pouco fiável).
pub const MIN_FACE_PX_FOR_IDENTITY: f32 = 40.0;
/// No máximo, identidade para os N maiores rostos de cada fotografia.
pub const MAX_IDENTITIES_PER_PHOTO: usize = 12;

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

fn optional(path: &Path) -> CoreResult<Option<crate::Plan>> {
    if path.is_file() {
        load(path).map(Some)
    } else {
        Ok(None)
    }
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
    identity: Option<crate::Plan>,
    scene: Option<crate::Plan>,
}

impl VisionModels {
    pub fn load(dir: &Path) -> CoreResult<Self> {
        Ok(VisionModels {
            detector: Detector::new(load(&dir.join(MODEL_FILES[0]))?),
            mesh: load(&dir.join(MODEL_FILES[1]))?,
            blend: load(&dir.join(MODEL_FILES[2]))?,
            embedder: load(&dir.join(MODEL_FILES[3]))?,
            identity: optional(&dir.join(OPTIONAL_MODEL_FILES[0]))?,
            scene: optional(&dir.join(OPTIONAL_MODEL_FILES[1]))?,
        })
    }

    pub fn has_identity(&self) -> bool {
        self.identity.is_some()
    }

    pub fn has_scene(&self) -> bool {
        self.scene.is_some()
    }

    /// Probabilidades ImageNet (cena/objetos), se o classificador existir.
    pub fn classify_scene(&self, img: &RgbImage) -> CoreResult<Option<Vec<f32>>> {
        match &self.scene {
            Some(p) => crate::scene::classify(p, img)
                .map(Some)
                .map_err(|e| CoreError::Other(format!("cena: {e:#}"))),
            None => Ok(None),
        }
    }

    /// Identidade de um rosto a partir dos seus pontos faciais.
    pub fn face_identity(
        &self,
        img: &RgbImage,
        landmarks: &[(f32, f32)],
    ) -> CoreResult<Option<Vec<f32>>> {
        let (Some(p), Some(pts)) = (&self.identity, crate::identity::five_points(landmarks)) else {
            return Ok(None);
        };
        let aligned = crate::identity::align(img, &pts);
        crate::identity::embed(p, &aligned)
            .map(Some)
            .map_err(|e| CoreError::Other(format!("identidade: {e:#}")))
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
                identity: None,
            });
        }
        // Identidade só para os maiores rostos com tamanho suficiente.
        if self.identity.is_some() {
            let mut order: Vec<usize> = (0..out.len()).collect();
            order.sort_by(|&a, &b| out[b].detection.w.total_cmp(&out[a].detection.w));
            for &i in order.iter().take(MAX_IDENTITIES_PER_PHOTO) {
                if out[i].detection.w >= MIN_FACE_PX_FOR_IDENTITY {
                    out[i].identity = self.face_identity(img, &out[i].landmarks)?;
                }
            }
        }
        Ok(out)
    }

    /// Vetor visual normalizado (norma L2 = 1) da imagem inteira.
    pub fn embed(&self, img: &RgbImage) -> CoreResult<Vec<f32>> {
        let small = momento_ai_analysis::resize::rgb(img, 224, 224);
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
