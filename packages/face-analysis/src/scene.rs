//! Classificador de cena/objetos: EfficientNet-Lite2 do MediaPipe (Apache-2.0), 1000
//! classes ImageNet. Dá pistas como “gown”, “suit”, “church”, “altar”, “Loafer”,
//! “microphone”, “plate”, “limousine”, “stage”… usadas na classificação de momentos.

use image::RgbImage;
use tract_onnx::prelude::*;

pub const SCENE_INPUT: u32 = 260;
pub static IMAGENET_LABELS: &str = include_str!("../../ai-analysis/src/imagenet_labels.txt");

pub fn labels() -> Vec<&'static str> {
    IMAGENET_LABELS.lines().collect()
}

/// Índice de uma etiqueta ImageNet (comparação exata).
pub fn label_index(name: &str) -> Option<usize> {
    IMAGENET_LABELS.lines().position(|l| l == name)
}

/// Probabilidades das 1000 classes. Entrada NHWC 260×260, (v − 127) / 128.
pub fn classify(plan: &crate::Plan, img: &RgbImage) -> TractResult<Vec<f32>> {
    let small = if img.dimensions() == (SCENE_INPUT, SCENE_INPUT) {
        img.clone()
    } else {
        momento_ai_analysis::resize::rgb(img, SCENE_INPUT, SCENE_INPUT)
    };
    let data: Vec<f32> = small.as_raw().iter().map(|&v| (v as f32 - 127.0) / 128.0).collect();
    let t: Tensor = tract_ndarray::Array4::from_shape_vec(
        (1, SCENE_INPUT as usize, SCENE_INPUT as usize, 3),
        data,
    )?
    .into();
    let out = plan.run(tvec!(t.into()))?;
    Ok(out[0].to_plain_array_view::<f32>()?.iter().copied().collect())
}

/// As `k` classes mais prováveis: (índice, probabilidade).
pub fn top_k(probs: &[f32], k: usize) -> Vec<(u16, f32)> {
    let mut idx: Vec<usize> = (0..probs.len()).collect();
    idx.sort_by(|&a, &b| probs[b].total_cmp(&probs[a]));
    idx.into_iter().take(k).map(|i| (i as u16, probs[i])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_the_1000_imagenet_classes() {
        let l = labels();
        assert_eq!(l.len(), 1000);
        assert_eq!(label_index("suit"), Some(834));
        assert_eq!(label_index("groom"), Some(982));
        assert!(label_index("church").is_some() && label_index("Loafer").is_some());
    }

    #[test]
    fn top_k_orders_by_probability() {
        assert_eq!(top_k(&[0.1, 0.7, 0.2], 2), vec![(1, 0.7), (2, 0.2)]);
    }
}
