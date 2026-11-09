//! Sincronização de hora entre câmaras: uma correção (ms) por câmara, somada à hora EXIF.
//! A hora original nunca é alterada (nem no catálogo nem nos ficheiros).

use crate::{db_err, Catalog, CAMERA_KEY_SQL};
use momento_core_types::{AssetId, CoreResult};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraRow {
    /// Marca + modelo + n.º de série (vazio = sem EXIF de câmara).
    pub key: String,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub photos: u64,
    /// Primeira/última hora EXIF original.
    pub first_time: Option<String>,
    pub last_time: Option<String>,
    pub offset_ms: i64,
    pub reference: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraOffset {
    pub key: String,
    pub offset_ms: i64,
    pub reference: bool,
}

/// Fotografia para a sugestão automática de correções.
#[derive(Debug, Clone, PartialEq)]
pub struct ClockSample {
    pub asset: AssetId,
    pub camera: String,
    pub capture_time: String,
}

impl Catalog {
    pub fn list_cameras(&self) -> CoreResult<Vec<CameraRow>> {
        let mut st = self
            .connection()
            .prepare(&format!(
                "SELECT {CAMERA_KEY_SQL} AS k, max(a.camera_make), max(a.camera_model), max(a.camera_serial),
                        count(*), min(a.capture_time), max(a.capture_time),
                        coalesce(max(cc.offset_ms), 0), coalesce(max(cc.reference), 0)
                 FROM asset a LEFT JOIN camera_clock cc ON cc.camera_key = {CAMERA_KEY_SQL}
                 GROUP BY k ORDER BY count(*) DESC, k"
            ))
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(CameraRow {
                    key: r.get(0)?,
                    make: r.get(1)?,
                    model: r.get(2)?,
                    serial: r.get(3)?,
                    photos: r.get::<_, i64>(4)? as u64,
                    first_time: r.get(5)?,
                    last_time: r.get(6)?,
                    offset_ms: r.get(7)?,
                    reference: r.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Substitui as correções (uma única câmara de referência; a referência tem correção 0).
    pub fn set_camera_offsets(&mut self, offsets: &[CameraOffset]) -> CoreResult<()> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        tx.execute("DELETE FROM camera_clock", []).map_err(db_err)?;
        {
            let mut ins = tx
                .prepare(
                    "INSERT INTO camera_clock(camera_key, offset_ms, reference) VALUES(?1, ?2, ?3)",
                )
                .map_err(db_err)?;
            let mut seen_ref = false;
            for o in offsets {
                let reference = o.reference && !seen_ref;
                seen_ref |= reference;
                let off = if reference { 0 } else { o.offset_ms };
                if off != 0 || reference {
                    ins.execute(params![o.key, off, reference as i64]).map_err(db_err)?;
                }
            }
        }
        tx.commit().map_err(db_err)
    }

    pub fn camera_offsets(&self) -> CoreResult<HashMap<String, i64>> {
        let mut st = self
            .connection()
            .prepare("SELECT camera_key, offset_ms FROM camera_clock")
            .map_err(db_err)?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Hora **original** e câmara das fotografias analisadas (para comparar câmaras).
    pub fn clock_samples(&self) -> CoreResult<Vec<ClockSample>> {
        let mut st = self
            .connection()
            .prepare(&format!(
                "SELECT a.id, {CAMERA_KEY_SQL}, a.capture_time FROM asset a
                 WHERE a.capture_time IS NOT NULL AND a.analysis_state = 1 ORDER BY a.capture_time"
            ))
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(ClockSample {
                    asset: AssetId(r.get(0)?),
                    camera: r.get(1)?,
                    capture_time: r.get(2)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::tests::new_asset;

    #[test]
    fn offsets_reorder_the_timeline_without_touching_exif() {
        let mut c = Catalog::open_in_memory().unwrap();
        let mut sony = new_asset(1);
        sony.camera_make = Some("SONY".into());
        sony.camera_model = Some("ILCE-7M4".into());
        sony.capture_time = Some("2026-06-15T14:03:50".into()); // relógio adiantado 3min42s
        let mut canon = new_asset(2);
        canon.capture_time = Some("2026-06-15T14:00:30".into());
        c.insert_assets(&[sony, canon]).unwrap();
        let names = |c: &Catalog| {
            c.list_photos().unwrap().into_iter().map(|p| p.file_name).collect::<Vec<_>>()
        };
        assert_eq!(names(&c), ["IMG_0002.jpg", "IMG_0001.jpg"]);
        let cams = c.list_cameras().unwrap();
        assert_eq!(cams.len(), 2);
        c.set_camera_offsets(&[
            CameraOffset { key: "Canon EOS R5".into(), offset_ms: 999, reference: true },
            CameraOffset {
                key: "SONY ILCE-7M4".into(),
                offset_ms: -(3 * 60 + 42) * 1000,
                reference: false,
            },
        ])
        .unwrap();
        // Sony corrigida: 14:00:08 → passa à frente da Canon (14:00:30).
        assert_eq!(names(&c), ["IMG_0001.jpg", "IMG_0002.jpg"]);
        let p = c.list_photos().unwrap();
        assert_eq!(p[0].capture_time.as_deref(), Some("2026-06-15T14:00:08.000"));
        let d = c.photo_details(p[0].id).unwrap().unwrap();
        assert_eq!(d.capture_time.as_deref(), Some("2026-06-15T14:03:50"), "EXIF original intacto");
        assert_eq!(d.clock_offset_ms, -222_000);
        let cams = c.list_cameras().unwrap();
        let canon = cams.iter().find(|k| k.key == "Canon EOS R5").unwrap();
        assert!(canon.reference && canon.offset_ms == 0, "a referência fica sempre a 0");
        assert_eq!(c.camera_offsets().unwrap().get("SONY ILCE-7M4"), Some(&-222_000));
    }
}
