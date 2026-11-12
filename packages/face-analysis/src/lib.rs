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
//! | `face_identity.onnx` (SFace, OpenCV Zoo) | 112×112 alinhado | identidade facial (128-D), opcional |
//! | `scene_classifier.onnx` (EfficientNet-Lite2) | 260×260 | 1000 classes ImageNet (cena/objetos), opcional |
//!
//! O pré e pós-processamento reproduz o do MediaPipe (âncoras SSD, NMS ponderado, recorte
//! ×1,5 rodado pelos olhos, subconjunto de 146 pontos) e é validado contra os resultados
//! de referência publicados pela Google (`portrait.jpg`).

pub mod denoise;
mod detector;
pub mod identity;
mod mesh;
mod models;
pub mod scene;
pub mod segment;
mod tensor;

/// Modelo otimizado e pronto a executar.
pub(crate) type Plan = std::sync::Arc<tract_onnx::prelude::TypedRunnableModel>;

pub use denoise::{Denoiser, DENOISE_MODEL_FILE};
pub use detector::FaceDetection;
pub use mesh::{blendshape_index, FaceMesh, BLENDSHAPE_NAMES};
pub use models::{
    find_models_dir, VisionModels, MAX_IDENTITIES_PER_PHOTO, MIN_FACE_PX_FOR_IDENTITY, MODEL_FILES,
    OPTIONAL_MODEL_FILES,
};

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
    /// Profundidade relativa de cada ponto (píxeis; menor = mais perto da câmara).
    #[serde(skip)]
    pub depth: Vec<f32>,
    /// Identidade (SFace, 128-D normalizado), se o modelo existir e o rosto for grande o suficiente.
    #[serde(skip)]
    pub identity: Option<Vec<f32>>,
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
