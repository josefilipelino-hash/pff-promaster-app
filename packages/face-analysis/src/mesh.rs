//! Pontos faciais (478) e blendshapes (52), como em `face_landmarks_detector_graph.cc` e
//! `face_blendshapes_graph.cc` do MediaPipe.

use crate::detector::FaceDetection;
use crate::tensor::{crop_rotated, to_nhwc};
use image::RgbImage;
use tract_onnx::prelude::*;

pub const MESH_INPUT: u32 = 256;

/// Subconjunto de 146 pontos usado pelo modelo de blendshapes (`kLandmarksSubsetIdxs`).
pub const BLENDSHAPE_SUBSET: [usize; 146] = [
    0, 1, 4, 5, 6, 7, 8, 10, 13, 14, 17, 21, 33, 37, 39, 40, 46, 52, 53, 54, 55, 58, 61, 63, 65,
    66, 67, 70, 78, 80, 81, 82, 84, 87, 88, 91, 93, 95, 103, 105, 107, 109, 127, 132, 133, 136,
    144, 145, 146, 148, 149, 150, 152, 153, 154, 155, 157, 158, 159, 160, 161, 162, 163, 168, 172,
    173, 176, 178, 181, 185, 191, 195, 197, 234, 246, 249, 251, 263, 267, 269, 270, 276, 282, 283,
    284, 285, 288, 291, 293, 295, 296, 297, 300, 308, 310, 311, 312, 314, 317, 318, 321, 323, 324,
    332, 334, 336, 338, 356, 361, 362, 365, 373, 374, 375, 377, 378, 379, 380, 381, 382, 384, 385,
    386, 387, 388, 389, 390, 397, 398, 400, 402, 405, 409, 415, 454, 466, 468, 469, 470, 471, 472,
    473, 474, 475, 476, 477,
];

/// Nomes dos 52 coeficientes (`kBlendshapeNames`).
pub const BLENDSHAPE_NAMES: [&str; 52] = [
    "_neutral",
    "browDownLeft",
    "browDownRight",
    "browInnerUp",
    "browOuterUpLeft",
    "browOuterUpRight",
    "cheekPuff",
    "cheekSquintLeft",
    "cheekSquintRight",
    "eyeBlinkLeft",
    "eyeBlinkRight",
    "eyeLookDownLeft",
    "eyeLookDownRight",
    "eyeLookInLeft",
    "eyeLookInRight",
    "eyeLookOutLeft",
    "eyeLookOutRight",
    "eyeLookUpLeft",
    "eyeLookUpRight",
    "eyeSquintLeft",
    "eyeSquintRight",
    "eyeWideLeft",
    "eyeWideRight",
    "jawForward",
    "jawLeft",
    "jawOpen",
    "jawRight",
    "mouthClose",
    "mouthDimpleLeft",
    "mouthDimpleRight",
    "mouthFrownLeft",
    "mouthFrownRight",
    "mouthFunnel",
    "mouthLeft",
    "mouthLowerDownLeft",
    "mouthLowerDownRight",
    "mouthPressLeft",
    "mouthPressRight",
    "mouthPucker",
    "mouthRight",
    "mouthRollLower",
    "mouthRollUpper",
    "mouthShrugLower",
    "mouthShrugUpper",
    "mouthSmileLeft",
    "mouthSmileRight",
    "mouthStretchLeft",
    "mouthStretchRight",
    "mouthUpperUpLeft",
    "mouthUpperUpRight",
    "noseSneerLeft",
    "noseSneerRight",
];

pub fn blendshape_index(name: &str) -> Option<usize> {
    BLENDSHAPE_NAMES.iter().position(|n| *n == name)
}

/// Pontos faciais em píxeis da imagem analisada.
#[derive(Debug, Clone)]
pub struct FaceMesh {
    pub points: Vec<(f32, f32, f32)>,
    pub presence: f32,
}

/// Retângulo rodado da região do rosto (DetectionsToRects + RectTransformation ×1,5).
pub fn face_roi(d: &FaceDetection) -> (f32, f32, f32, f32, f32) {
    let (l, r) = (d.keypoints[0], d.keypoints[1]);
    // Ângulo que deixa os olhos na horizontal (MediaPipe: target 0 − atan2(−dy, dx)).
    let angle = (r.1 - l.1).atan2(r.0 - l.0);
    let (cx, cy) = (d.x + d.w / 2.0, d.y + d.h / 2.0);
    (cx, cy, d.w * 1.5, d.h * 1.5, angle)
}

pub fn run_mesh(model: &crate::Plan, img: &RgbImage, d: &FaceDetection) -> TractResult<FaceMesh> {
    let (cx, cy, w, h, angle) = face_roi(d);
    let crop = crop_rotated(img, cx, cy, w, h, angle, MESH_INPUT);
    let out = model.run(tvec!(to_nhwc(&crop, 0.0, 1.0).into()))?;
    let lm = out[0].to_plain_array_view::<f32>()?;
    let lm = lm.as_slice().ok_or_else(|| anyhow::anyhow!("tensor não contíguo"))?;
    let presence_logit =
        out[1].to_plain_array_view::<f32>()?.iter().next().copied().unwrap_or(-10.0);
    let (sin, cos) = angle.sin_cos();
    let points = lm
        .chunks_exact(3)
        .map(|p| {
            let u = p[0] / MESH_INPUT as f32 - 0.5;
            let v = p[1] / MESH_INPUT as f32 - 0.5;
            let (lx, ly) = (u * w, v * h);
            (cx + lx * cos - ly * sin, cy + lx * sin + ly * cos, p[2] / MESH_INPUT as f32 * w)
        })
        .collect();
    Ok(FaceMesh { points, presence: 1.0 / (1.0 + (-presence_logit).exp()) })
}

/// Blendshapes: os 146 pontos em píxeis da imagem (LandmarksToTensor com IMAGE_SIZE).
pub fn run_blendshapes(model: &crate::Plan, mesh: &FaceMesh) -> TractResult<Vec<f32>> {
    let mut data = Vec::with_capacity(146 * 2);
    for &i in BLENDSHAPE_SUBSET.iter() {
        let p = mesh.points.get(i).copied().unwrap_or((0.0, 0.0, 0.0));
        data.push(p.0);
        data.push(p.1);
    }
    let t: Tensor = tract_ndarray::Array3::from_shape_vec((1, 146, 2), data)?.into();
    let out = model.run(tvec!(t.into()))?;
    Ok(out[0].to_plain_array_view::<f32>()?.iter().copied().collect())
}
