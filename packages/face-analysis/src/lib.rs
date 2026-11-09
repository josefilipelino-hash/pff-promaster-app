//! # momento-face-analysis
//!
//! Inferência **local** (nenhum dado sai do computador) com modelos MediaPipe da Google
//! (Apache-2.0), convertidos para ONNX por `scripts/prepare-models.py` e executados pelo
//! `tract-onnx` (Rust puro, sem bibliotecas nativas — igual em macOS e Windows).
//!
//! | Modelo | Entrada | Uso |
//! |---|---|---|
//! | `face_detector.onnx` (BlazeFace short-range) | 128×128 | caixas e 6 pontos por rosto |
//! | `face_landmarks_detector.onnx` | 256×256 | 478 pontos faciais + presença |
//! | `face_blendshapes.onnx` | 146 pontos | 52 coeficientes (olhos fechados, sorriso, …) |
//! | `image_embedder.onnx` (MobileNetV3-small) | 224×224 | vetor visual de 1024 dimensões |
//!
//! O pré e pós-processamento reproduz o do MediaPipe (âncoras SSD, NMS ponderado, recorte
//! ×1,5 rodado pelos olhos, subconjunto de 146 pontos) e é validado contra os resultados
//! de referência publicados pela Google (`portrait.jpg`).

mod detector;
mod mesh;
mod models;
mod tensor;

/// Modelo otimizado e pronto a executar.
pub(crate) type Plan = std::sync::Arc<tract_onnx::prelude::TypedRunnableModel>;

pub use detector::FaceDetection;
pub use mesh::{blendshape_index, FaceMesh, BLENDSHAPE_NAMES};
pub use models::{find_models_dir, VisionModels, MODEL_FILES};

/// Resultado completo para um rosto.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceResult {
    pub detection: FaceDetection,
    /// Probabilidade de existir um rosto no recorte (0–1).
    pub presence: f32,
    /// 52 coeficientes de expressão (ordem de [`BLENDSHAPE_NAMES`]).
    pub blendshapes: Vec<f32>,
    /// 478 pontos (x, y) em píxeis da imagem analisada.
    #[serde(skip)]
    pub landmarks: Vec<(f32, f32)>,
}

impl FaceResult {
    fn blend(&self, name: &str) -> f32 {
        blendshape_index(name).and_then(|i| self.blendshapes.get(i).copied()).unwrap_or(0.0)
    }
    /// Olho esquerdo/direito fechado (coeficiente `eyeBlink*`, 0 aberto – 1 fechado).
    pub fn eye_blink(&self) -> (f32, f32) {
        (self.blend("eyeBlinkLeft"), self.blend("eyeBlinkRight"))
    }
    /// Sorriso (média de `mouthSmileLeft/Right`).
    pub fn smile(&self) -> f32 {
        (self.blend("mouthSmileLeft") + self.blend("mouthSmileRight")) / 2.0
    }
}
