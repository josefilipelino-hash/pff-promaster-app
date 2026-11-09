//! Fase 7 — dados para a consistência visual: características de luz por fotografia
//! (`look_feature`) e o que é preciso para agrupar (câmara, hora corrigida, exposição,
//! momento).

use crate::{db_err, now_unix, Catalog, CLOCK_JOIN_SQL, CORRECTED_ORDER_SQL, CORRECTED_TIME_SQL};
use momento_core_types::{AssetId, CoreResult};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Características medidas numa miniatura (e no EXIF).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookFeature {
    /// Luminosidade da cena (EV a ISO 100) a partir do EXIF, se existir.
    pub ev: Option<f32>,
    /// Dominante de cor residual (temperatura relativa que a neutralizaria; negativo = luz quente).
    pub warmth: f32,
    pub tint: f32,
    /// Mediana da luminância da miniatura (0–1).
    pub key: f32,
    /// Flash disparado (EXIF), se conhecido.
    pub flash: Option<bool>,
}

/// Uma fotografia para agrupar.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyItem {
    pub id: AssetId,
    pub file_name: String,
    pub camera: String,
    /// Hora corrigida (sincronização de câmaras).
    pub time: Option<String>,
    pub moment: Option<i64>,
    pub feature: Option<LookFeature>,
}

/// Fotografia ainda sem características: caminho do original e formato.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingLook {
    pub id: AssetId,
    pub path: String,
    pub format: String,
    pub orientation: u8,
    pub iso: Option<u32>,
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
}

/// EV a ISO 100 (luminosidade da cena) a partir de abertura, velocidade e ISO.
pub fn scene_ev(iso: Option<u32>, t: Option<f64>, n: Option<f64>) -> Option<f32> {
    let (iso, t, n) = (iso? as f64, t?, n?);
    (iso > 0.0 && t > 0.0 && n > 0.0).then(|| ((n * n / t).log2() - (iso / 100.0).log2()) as f32)
}

impl Catalog {
    pub fn pending_look_features(&self) -> CoreResult<Vec<PendingLook>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT a.id, a.path, a.format, a.orientation, a.iso, a.exposure_time, a.f_number FROM asset a
                 WHERE a.status = 'ok' AND NOT EXISTS (SELECT 1 FROM look_feature f WHERE f.asset_id = a.id)
                 ORDER BY a.id",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(PendingLook {
                    id: AssetId(r.get(0)?),
                    path: r.get(1)?,
                    format: r.get(2)?,
                    orientation: r.get::<_, i64>(3)? as u8,
                    iso: r.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                    exposure_time: r.get(5)?,
                    f_number: r.get(6)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn save_look_features(&self, rows: &[(AssetId, LookFeature)]) -> CoreResult<()> {
        let tx = self.connection().unchecked_transaction().map_err(db_err)?;
        for (id, f) in rows {
            tx.execute(
                "INSERT OR REPLACE INTO look_feature(asset_id, ev, warmth, tint, key, flash, computed_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id.0, f.ev, f.warmth, f.tint, f.key, f.flash, now_unix()],
            )
            .map_err(db_err)?;
        }
        tx.commit().map_err(db_err)
    }

    /// Todas as fotografias com o necessário para agrupar, por ordem cronológica corrigida.
    pub fn consistency_items(&self) -> CoreResult<Vec<ConsistencyItem>> {
        let mut st = self
            .connection()
            .prepare(&format!(
                "SELECT a.id, a.file_name,
                        trim(coalesce(a.camera_make, '') || ' ' || coalesce(a.camera_model, '')),
                        {CORRECTED_TIME_SQL},
                        (SELECT m.moment_id FROM asset_moment m WHERE m.asset_id = a.id),
                        f.ev, f.warmth, f.tint, f.key, f.flash
                 FROM asset a
                 {CLOCK_JOIN_SQL}
                 LEFT JOIN look_feature f ON f.asset_id = a.id
                 WHERE a.status = 'ok'
                 ORDER BY {CORRECTED_ORDER_SQL}"
            ))
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                let warmth: Option<f32> = r.get(6)?;
                Ok(ConsistencyItem {
                    id: AssetId(r.get(0)?),
                    file_name: r.get(1)?,
                    camera: r.get(2)?,
                    time: r.get(3)?,
                    moment: r.get(4)?,
                    feature: match warmth {
                        Some(w) => Some(LookFeature {
                            ev: r.get(5)?,
                            warmth: w,
                            tint: r.get(7)?,
                            key: r.get(8)?,
                            flash: r.get(9)?,
                        }),
                        None => None,
                    },
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Caixas dos rostos de uma fotografia (normalizadas na imagem orientada).
    pub fn face_boxes(&self, asset: AssetId) -> CoreResult<Vec<[f32; 4]>> {
        let mut st = self
            .connection()
            .prepare("SELECT x, y, w, h FROM face WHERE asset_id = ?1 ORDER BY w DESC LIMIT 12")
            .map_err(db_err)?;
        let rows = st
            .query_map(params![asset.0], |r| {
                Ok([
                    r.get::<_, f64>(0)? as f32,
                    r.get::<_, f64>(1)? as f32,
                    r.get::<_, f64>(2)? as f32,
                    r.get::<_, f64>(3)? as f32,
                ])
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_ev_values() {
        // Sol: f/16, 1/100, ISO 100 → ≈ 14,6 EV; interior: f/2,8, 1/60, ISO 3200 → ≈ 3,9.
        let sun = scene_ev(Some(100), Some(0.01), Some(16.0)).unwrap();
        let inside = scene_ev(Some(3200), Some(1.0 / 60.0), Some(2.8)).unwrap();
        assert!((sun - 14.64).abs() < 0.05 && (inside - 3.88).abs() < 0.05, "{sun} {inside}");
        assert_eq!(scene_ev(None, Some(0.01), Some(2.0)), None);
    }
}
