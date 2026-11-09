//! Sugestão automática da correção de hora entre câmaras.
//!
//! Para cada câmara, procura na câmara de referência a fotografia **visualmente mais
//! parecida** (embedding) dentro de uma janela de ±2 h; cada par é um “voto” na diferença
//! de horas. A sugestão é a moda robusta dos votos (caixas de 5 s, refinada pela mediana).
//! Sem votos suficientes e concordantes, não há sugestão (nunca se inventa uma correção).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const WINDOW_MS: i64 = 2 * 3600 * 1000;
const MIN_SIM: f32 = 0.86;
const BIN_MS: i64 = 5_000;
const MIN_VOTES: usize = 3;
/// No máximo, fotografias amostradas por câmara.
const MAX_SAMPLES: usize = 400;

#[derive(Debug, Clone, PartialEq)]
pub struct ClockPhoto {
    pub camera: String,
    /// Hora EXIF original (ms).
    pub time_ms: i64,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OffsetSuggestion {
    pub camera: String,
    /// Correção a somar à hora desta câmara (ms).
    pub offset_ms: i64,
    /// Pares que concordam com a sugestão.
    pub votes: usize,
    /// Fração dos pares encontrados que concordam (0–1).
    pub agreement: f32,
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn suggest(photos: &[ClockPhoto], reference: &str) -> Vec<OffsetSuggestion> {
    let mut by_cam: HashMap<&str, Vec<&ClockPhoto>> = HashMap::new();
    for p in photos.iter().filter(|p| !p.embedding.is_empty()) {
        by_cam.entry(p.camera.as_str()).or_default().push(p);
    }
    let Some(mut refs) = by_cam.get(reference).cloned() else { return Vec::new() };
    refs.sort_by_key(|p| p.time_ms);
    let mut out = Vec::new();
    let mut cams: Vec<&&str> = by_cam.keys().filter(|c| **c != reference).collect();
    cams.sort();
    for cam in cams {
        let list = &by_cam[*cam];
        let step = (list.len() / MAX_SAMPLES).max(1);
        let mut votes: Vec<(i64, f32)> = Vec::new();
        for p in list.iter().step_by(step) {
            let lo = refs.partition_point(|r| r.time_ms < p.time_ms - WINDOW_MS);
            let hi = refs.partition_point(|r| r.time_ms <= p.time_ms + WINDOW_MS);
            let best = refs[lo..hi]
                .iter()
                .map(|r| (r, dot(&r.embedding, &p.embedding)))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((r, s)) = best {
                if s >= MIN_SIM {
                    votes.push((r.time_ms - p.time_ms, s));
                }
            }
        }
        if votes.len() < MIN_VOTES {
            continue;
        }
        // Moda ponderada com caixas de 5 s (a caixa e as duas vizinhas).
        let mut bins: HashMap<i64, f32> = HashMap::new();
        for (d, s) in &votes {
            *bins.entry(d.div_euclid(BIN_MS)).or_default() += s;
        }
        let score = |b: i64| {
            bins.get(&(b - 1)).unwrap_or(&0.0)
                + bins.get(&b).unwrap_or(&0.0)
                + bins.get(&(b + 1)).unwrap_or(&0.0)
        };
        let Some(&mode) =
            bins.keys().max_by(|a, b| score(**a).total_cmp(&score(**b)).then(b.cmp(a)))
        else {
            continue;
        };
        let mut near: Vec<i64> = votes
            .iter()
            .map(|v| v.0)
            .filter(|d| (d.div_euclid(BIN_MS) - mode).abs() <= 1)
            .collect();
        if near.len() < MIN_VOTES {
            continue;
        }
        near.sort();
        let median = near[near.len() / 2];
        let agreement = near.len() as f32 / votes.len() as f32;
        if agreement < 0.4 {
            continue;
        }
        out.push(OffsetSuggestion {
            camera: cam.to_string(),
            offset_ms: (median as f64 / 1000.0).round() as i64 * 1000,
            votes: near.len(),
            agreement,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emb(scene: u32, jitter: u32) -> Vec<f32> {
        let v: Vec<f32> = (0..64u32)
            .map(|i| {
                ((i.wrapping_mul(2_654_435_761) ^ scene.wrapping_mul(97)) % 1000) as f32 / 1000.0
                    - 0.5
                    + ((i + jitter) % 5) as f32 * 0.01
            })
            .collect();
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.into_iter().map(|x| x / n).collect()
    }

    /// O dia do exemplo: Canon correta, Sony adiantada 3min42s, DJI atrasada 1min15s.
    #[test]
    fn recovers_sony_plus_3m42_and_dji_minus_1m15() {
        let mut photos = Vec::new();
        for scene in 0..40u32 {
            let t = scene as i64 * 600_000; // um momento a cada 10 min
            for k in 0..3 {
                photos.push(ClockPhoto {
                    camera: "Canon EOS R5".into(),
                    time_ms: t + k * 2000,
                    embedding: emb(scene, k as u32),
                });
            }
            photos.push(ClockPhoto {
                camera: "SONY ILCE-7M4".into(),
                time_ms: t + 1500 + 222_000,
                embedding: emb(scene, 7),
            });
            if scene % 2 == 0 {
                photos.push(ClockPhoto {
                    camera: "DJI FC3582".into(),
                    time_ms: t + 3000 - 75_000,
                    embedding: emb(scene, 9),
                });
            }
        }
        let s = suggest(&photos, "Canon EOS R5");
        let get = |c: &str| s.iter().find(|x| x.camera == c).unwrap();
        // A Sony está +3min42s → correção −3min42s (±3 s do desfasamento entre disparos).
        assert!((get("SONY ILCE-7M4").offset_ms + 222_000).abs() <= 3000, "{s:?}");
        assert!((get("DJI FC3582").offset_ms - 75_000).abs() <= 3000, "{s:?}");
        assert!(get("SONY ILCE-7M4").agreement > 0.8);
    }

    #[test]
    fn no_suggestion_without_visual_matches() {
        let mut photos: Vec<ClockPhoto> = (0..20)
            .map(|i| ClockPhoto {
                camera: "A".into(),
                time_ms: i * 60_000,
                embedding: emb(i as u32, 0),
            })
            .collect();
        photos.extend((0..20).map(|i| ClockPhoto {
            camera: "B".into(),
            time_ms: i * 60_000,
            embedding: emb(1000 + i as u32, 0),
        }));
        assert!(suggest(&photos, "A").is_empty());
    }
}
