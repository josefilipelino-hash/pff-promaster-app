//! Deteção de rostos BlazeFace (short-range) com o pós-processamento do MediaPipe
//! (`face_detector_graph.cc`): âncoras SSD de 4 camadas (strides 8,16,16,16), coordenadas
//! na ordem x,y,w,h, sigmoide com corte em ±100, limiar 0,5 e NMS ponderado (IoU 0,3).
//!
//! O modelo short-range foi treinado para rostos grandes (até ~2 m). Para fotografias de
//! eventos, a imagem é também analisada em mosaicos sobrepostos (multi-escala), o que
//! permite encontrar rostos mais pequenos.

use crate::tensor::to_nhwc;
use image::{imageops, RgbImage};
use serde::Serialize;
use tract_onnx::prelude::*;

pub const INPUT: u32 = 128;
const MIN_SCORE: f32 = 0.5;
const NMS_IOU: f32 = 0.3;

/// Rosto detetado, em píxeis da imagem analisada.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceDetection {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub score: f32,
    /// Olho esquerdo (do observador), olho direito, nariz, boca, orelha esq., orelha dir.
    pub keypoints: [(f32, f32); 6],
}

impl FaceDetection {
    fn iou(&self, o: &FaceDetection) -> f32 {
        let x1 = self.x.max(o.x);
        let y1 = self.y.max(o.y);
        let x2 = (self.x + self.w).min(o.x + o.w);
        let y2 = (self.y + self.h).min(o.y + o.h);
        let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
        let union = self.w * self.h + o.w * o.h - inter;
        if union <= 0.0 {
            0.0
        } else {
            inter / union
        }
    }
}

/// Âncoras (cx, cy) normalizadas, pela ordem do `SsdAnchorsCalculator`.
pub fn anchors() -> Vec<(f32, f32)> {
    let strides = [8u32, 16, 16, 16];
    let mut out = Vec::with_capacity(896);
    let mut layer = 0;
    while layer < strides.len() {
        // Camadas consecutivas com o mesmo stride partilham a grelha: 2 âncoras por camada
        // (aspect ratio 1.0 + escala interpolada).
        let mut last = layer;
        let mut per_cell = 0;
        while last < strides.len() && strides[last] == strides[layer] {
            per_cell += 2;
            last += 1;
        }
        let fm = INPUT.div_ceil(strides[layer]);
        for y in 0..fm {
            for x in 0..fm {
                for _ in 0..per_cell {
                    out.push(((x as f32 + 0.5) / fm as f32, (y as f32 + 0.5) / fm as f32));
                }
            }
        }
        layer = last;
    }
    out
}

pub struct Detector {
    model: crate::Plan,
    anchors: Vec<(f32, f32)>,
}

impl Detector {
    pub fn new(model: crate::Plan) -> Self {
        Detector { model, anchors: anchors() }
    }

    /// Deteções numa única passagem (imagem redimensionada com letterbox para 128×128).
    fn detect_once(&self, img: &RgbImage) -> TractResult<Vec<FaceDetection>> {
        let (w, h) = img.dimensions();
        let s = INPUT as f32 / w.max(h) as f32;
        let (nw, nh) =
            (((w as f32 * s).round() as u32).max(1), ((h as f32 * s).round() as u32).max(1));
        let resized = imageops::resize(img, nw, nh, imageops::FilterType::Triangle);
        let mut boxed = RgbImage::new(INPUT, INPUT);
        let (px, py) = ((INPUT - nw) / 2, (INPUT - nh) / 2);
        imageops::replace(&mut boxed, &resized, px as i64, py as i64);
        let out = self.model.run(tvec!(to_nhwc(&boxed, -1.0, 1.0).into()))?;
        let reg = out[0].to_plain_array_view::<f32>()?;
        let cls = out[1].to_plain_array_view::<f32>()?;
        let reg = reg.as_slice().ok_or_else(|| anyhow::anyhow!("tensor não contíguo"))?;
        let cls = cls.as_slice().ok_or_else(|| anyhow::anyhow!("tensor não contíguo"))?;
        let to_img = |nx: f32, ny: f32| {
            ((nx * INPUT as f32 - px as f32) / s, (ny * INPUT as f32 - py as f32) / s)
        };
        let mut dets = Vec::new();
        for (i, &(ax, ay)) in self.anchors.iter().enumerate() {
            let score = 1.0 / (1.0 + (-cls[i].clamp(-100.0, 100.0)).exp());
            if score < MIN_SCORE {
                continue;
            }
            let r = &reg[i * 16..i * 16 + 16];
            let (cx, cy) = (r[0] / 128.0 + ax, r[1] / 128.0 + ay);
            let (bw, bh) = (r[2] / 128.0, r[3] / 128.0);
            let (x0, y0) = to_img(cx - bw / 2.0, cy - bh / 2.0);
            let (x1, y1) = to_img(cx + bw / 2.0, cy + bh / 2.0);
            let mut kp = [(0.0, 0.0); 6];
            for (k, p) in kp.iter_mut().enumerate() {
                *p = to_img(r[4 + 2 * k] / 128.0 + ax, r[5 + 2 * k] / 128.0 + ay);
            }
            dets.push(FaceDetection { x: x0, y: y0, w: x1 - x0, h: y1 - y0, score, keypoints: kp });
        }
        Ok(dets)
    }

    /// Deteção multi-escala: imagem inteira + mosaicos 2×2 e 3×3 sobrepostos.
    pub fn detect(&self, img: &RgbImage, tiles: bool) -> TractResult<Vec<FaceDetection>> {
        let mut all = self.detect_once(img)?;
        if tiles {
            let (w, h) = img.dimensions();
            let long = w.max(h) as f32;
            for &(frac, n) in &[(0.6f32, 2u32), (0.4, 3)] {
                let side = (long * frac).round() as u32;
                if side < 96 {
                    continue;
                }
                let step_x =
                    if n > 1 { (w.saturating_sub(side)) as f32 / (n - 1) as f32 } else { 0.0 };
                let step_y =
                    if n > 1 { (h.saturating_sub(side)) as f32 / (n - 1) as f32 } else { 0.0 };
                for ty in 0..n {
                    for tx in 0..n {
                        let ox = (tx as f32 * step_x).round() as u32;
                        let oy = (ty as f32 * step_y).round() as u32;
                        let tw = side.min(w - ox);
                        let th = side.min(h - oy);
                        let tile = imageops::crop_imm(img, ox, oy, tw, th).to_image();
                        for mut d in self.detect_once(&tile)? {
                            // Rostos cortados pela margem do mosaico são descartados (a imagem
                            // inteira ou outro mosaico vê-os completos).
                            let m = 2.0;
                            let cut_left = d.x < m && ox > 0;
                            let cut_top = d.y < m && oy > 0;
                            let cut_right = d.x + d.w > tw as f32 - m && ox + tw < w;
                            let cut_bottom = d.y + d.h > th as f32 - m && oy + th < h;
                            if cut_left || cut_top || cut_right || cut_bottom {
                                continue;
                            }
                            d.x += ox as f32;
                            d.y += oy as f32;
                            for k in d.keypoints.iter_mut() {
                                k.0 += ox as f32;
                                k.1 += oy as f32;
                            }
                            all.push(d);
                        }
                    }
                }
            }
        }
        Ok(weighted_nms(all))
    }
}

/// NMS ponderado do MediaPipe: agrupa deteções sobrepostas e faz a média pesada pelo score.
pub fn weighted_nms(mut dets: Vec<FaceDetection>) -> Vec<FaceDetection> {
    dets.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut out = Vec::new();
    let mut used = vec![false; dets.len()];
    for i in 0..dets.len() {
        if used[i] {
            continue;
        }
        let mut cluster = vec![i];
        for j in i + 1..dets.len() {
            if !used[j] && dets[i].iou(&dets[j]) > NMS_IOU {
                cluster.push(j);
            }
        }
        let total: f32 = cluster.iter().map(|&k| dets[k].score).sum();
        let mut m = FaceDetection { score: dets[i].score, ..dets[i] };
        let avg = |f: &dyn Fn(&FaceDetection) -> f32| {
            cluster.iter().map(|&k| f(&dets[k]) * dets[k].score).sum::<f32>() / total
        };
        m.x = avg(&|d| d.x);
        m.y = avg(&|d| d.y);
        m.w = avg(&|d| d.w);
        m.h = avg(&|d| d.h);
        for k in 0..6 {
            m.keypoints[k] = (avg(&|d| d.keypoints[k].0), avg(&|d| d.keypoints[k].1));
        }
        for &k in &cluster {
            used[k] = true;
        }
        out.push(m);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_layout_matches_mediapipe() {
        let a = anchors();
        assert_eq!(a.len(), 896);
        assert_eq!(a[0], (0.5 / 16.0, 0.5 / 16.0));
        assert_eq!(a[1], a[0], "2 âncoras por célula na camada de stride 8");
        assert_eq!(a[512], (0.5 / 8.0, 0.5 / 8.0), "início da grelha 8×8 com 6 âncoras");
        assert_eq!(a[517], a[512]);
        assert_ne!(a[518], a[512]);
    }

    #[test]
    fn nms_merges_overlapping() {
        let d = |x: f32, s: f32| FaceDetection {
            x,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            score: s,
            keypoints: [(0.0, 0.0); 6],
        };
        let out = weighted_nms(vec![d(0.0, 0.9), d(1.0, 0.6), d(50.0, 0.7)]);
        assert_eq!(out.len(), 2);
        assert!((out[0].x - 0.4).abs() < 1e-4, "média ponderada: {}", out[0].x);
        assert_eq!(out[0].score, 0.9);
    }
}
