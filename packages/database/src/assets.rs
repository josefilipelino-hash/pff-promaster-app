//! Fotografias (assets), variante principal e classificação.
//!
//! Cada asset tem uma variante principal (`variant.is_master = 1`). A classificação
//! pertence à variante, para que as cópias virtuais (Fase 4) tenham classificação própria.
//! Na interface da Fase 1, “fotografia” = asset + variante principal; o identificador
//! exposto é o `AssetId`.

use crate::{db_err, now_unix, Catalog, CLOCK_JOIN_SQL, CORRECTED_ORDER_SQL, CORRECTED_TIME_SQL};

/// Variante designada pela chave `?3` (principal do asset, ou cópia virtual).
const VARIANT_OF_KEY_3: &str = "(CASE WHEN ?3 >= 1099511627776 THEN ?3 - 1099511627776 ELSE (SELECT id FROM variant WHERE asset_id = ?3 AND is_master = 1) END)";
use momento_core_types::{AssetId, CoreError, CoreResult, ImportSessionId};
use rusqlite::{params, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Etiquetas de cor permitidas.
pub const COLORS: &[&str] = &["red", "yellow", "green", "blue", "purple"];

/// Estado da miniatura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThumbState {
    Pending = 0,
    Ready = 1,
    Failed = 2,
}

impl ThumbState {
    fn from_i64(v: i64) -> Self {
        match v {
            1 => ThumbState::Ready,
            2 => ThumbState::Failed,
            _ => ThumbState::Pending,
        }
    }
}

/// Dados de uma fotografia nova, obtidos pelo motor de importação.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewAsset {
    pub path: String,
    pub source_path: Option<String>,
    pub file_name: String,
    pub file_size: u64,
    pub file_mtime: Option<i64>,
    pub content_hash: [u8; 32],
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub orientation: u8,
    pub capture_time: Option<String>,
    pub capture_offset: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub camera_serial: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
    pub focal_length: Option<f64>,
    pub focal_length_35mm: Option<u32>,
    pub import_session_id: Option<ImportSessionId>,
    pub bit_depth: Option<u8>,
    pub color_space: Option<String>,
    pub codec: Option<String>,
    pub codec_detail: Option<String>,
    /// Dimensões da preview embutida (`LxA`), quando existe.
    pub embedded_preview: Option<String>,
    /// `None` = suporte completo; `Some(nota)` = parcial, com o motivo.
    pub partial_note: Option<String>,
    /// Latitude/longitude (graus decimais).
    pub gps: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertResult {
    Inserted(AssetId),
    /// Já existe uma fotografia com o mesmo conteúdo (hash BLAKE3).
    DuplicateContent(AssetId),
    /// O mesmo caminho já está no catálogo.
    DuplicatePath(AssetId),
}

/// Linha compacta para a grelha (milhares de linhas enviadas de uma vez).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoRow {
    /// Chave da fotografia na interface (ver `momento_core_types::decode_key`).
    pub id: AssetId,
    /// Asset (ficheiro) desta fotografia (igual a `id` na versão principal).
    pub asset: AssetId,
    pub variant: i64,
    /// Nome da cópia virtual (`None` = versão principal).
    pub virtual_copy: Option<String>,
    /// Tem ajustes de edição.
    pub edited: bool,
    pub file_name: String,
    pub capture_time: Option<String>,
    /// Dimensões já orientadas (troca largura/altura para orientações 5–8).
    pub width: u32,
    pub height: u32,
    pub stars: u8,
    /// -1 rejeitada, 0 sem marca, 1 favorita.
    pub flag: i8,
    pub color: Option<String>,
    pub thumb: ThumbState,
    /// Suporte parcial (ex.: RAW só com preview embutida).
    pub partial: bool,
    pub format: String,
}

/// Detalhes completos para o painel de informação.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoDetails {
    pub id: AssetId,
    pub path: String,
    pub source_path: Option<String>,
    pub file_name: String,
    pub file_size: u64,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub orientation: u8,
    pub capture_time: Option<String>,
    pub capture_offset: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub camera_serial: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
    pub focal_length: Option<f64>,
    pub focal_length_35mm: Option<u32>,
    pub imported_at: i64,
    pub thumb_error: Option<String>,
    pub bit_depth: Option<u8>,
    pub color_space: Option<String>,
    pub codec: Option<String>,
    pub codec_detail: Option<String>,
    pub embedded_preview: Option<String>,
    pub partial_note: Option<String>,
    /// Hora corrigida pela sincronização de câmaras (igual a `capture_time` sem correção).
    pub corrected_time: Option<String>,
    /// Correção aplicada à câmara desta fotografia (ms).
    pub clock_offset_ms: i64,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
}

/// Localização de um original para servir previews.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSource {
    pub path: String,
    pub format: String,
    pub orientation: u8,
}

pub(crate) fn oriented(w: u32, h: u32, orientation: u8) -> (u32, u32) {
    if (5..=8).contains(&orientation) {
        (h, w)
    } else {
        (w, h)
    }
}

impl Catalog {
    /// Insere fotografias numa transação. Conflitos (mesmo conteúdo ou mesmo caminho) não
    /// abortam o lote: são devolvidos por item.
    pub fn insert_assets(&mut self, items: &[NewAsset]) -> CoreResult<Vec<InsertResult>> {
        let now = now_unix();
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        let mut out = Vec::with_capacity(items.len());
        {
            let mut ins = tx
                .prepare(
                    "INSERT INTO asset(path, source_path, file_name, file_size, file_mtime,
                        content_hash, format, width, height, orientation, capture_time,
                        capture_offset, camera_make, camera_model, camera_serial, lens, iso,
                        exposure_time, f_number, focal_length, focal_length_35mm,
                        import_session_id, imported_at, bit_depth, color_space, codec,
                        codec_detail, embedded_preview, support, support_note, gps_lat, gps_lon)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                            ?29, ?30, ?31, ?32)",
                )
                .map_err(db_err)?;
            let mut var = tx
                .prepare("INSERT INTO variant(asset_id, is_master, created_at) VALUES(?1, 1, ?2)")
                .map_err(db_err)?;
            let mut rat = tx
                .prepare("INSERT INTO rating(variant_id, updated_at) VALUES(?1, ?2)")
                .map_err(db_err)?;
            let mut by_hash =
                tx.prepare("SELECT id FROM asset WHERE content_hash = ?1").map_err(db_err)?;
            let mut by_path = tx.prepare("SELECT id FROM asset WHERE path = ?1").map_err(db_err)?;

            for a in items {
                let res = ins.execute(params![
                    a.path,
                    a.source_path,
                    a.file_name,
                    a.file_size as i64,
                    a.file_mtime,
                    &a.content_hash[..],
                    a.format,
                    a.width,
                    a.height,
                    a.orientation,
                    a.capture_time,
                    a.capture_offset,
                    a.camera_make,
                    a.camera_model,
                    a.camera_serial,
                    a.lens,
                    a.iso,
                    a.exposure_time,
                    a.f_number,
                    a.focal_length,
                    a.focal_length_35mm,
                    a.import_session_id.map(|s| s.0),
                    now,
                    a.bit_depth,
                    a.color_space,
                    a.codec,
                    a.codec_detail,
                    a.embedded_preview,
                    if a.partial_note.is_some() { "partial" } else { "full" },
                    a.partial_note,
                    a.gps.map(|g| g.0),
                    a.gps.map(|g| g.1)
                ]);
                match res {
                    Ok(_) => {
                        let id = tx.last_insert_rowid();
                        var.execute(params![id, now]).map_err(db_err)?;
                        rat.execute(params![tx.last_insert_rowid(), now]).map_err(db_err)?;
                        out.push(InsertResult::Inserted(AssetId(id)));
                    }
                    Err(rusqlite::Error::SqliteFailure(e, _))
                        if e.code == ErrorCode::ConstraintViolation =>
                    {
                        let existing: Option<i64> = by_path
                            .query_row([&a.path], |r| r.get(0))
                            .optional()
                            .map_err(db_err)?;
                        if let Some(id) = existing {
                            out.push(InsertResult::DuplicatePath(AssetId(id)));
                            continue;
                        }
                        let existing: Option<i64> = by_hash
                            .query_row([&a.content_hash[..]], |r| r.get(0))
                            .optional()
                            .map_err(db_err)?;
                        match existing {
                            Some(id) => out.push(InsertResult::DuplicateContent(AssetId(id))),
                            None => {
                                return Err(CoreError::Database(format!(
                                    "restrição violada ao inserir {}",
                                    a.path
                                )))
                            }
                        }
                    }
                    Err(e) => return Err(db_err(e)),
                }
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(out)
    }

    /// Hashes de conteúdo já no catálogo (deteção de duplicados antes de copiar).
    pub fn content_hashes(&self) -> CoreResult<HashSet<[u8; 32]>> {
        let mut st = self.connection().prepare("SELECT content_hash FROM asset").map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                let v: Vec<u8> = r.get(0)?;
                Ok(v)
            })
            .map_err(db_err)?;
        let mut set = HashSet::new();
        for v in rows {
            let v = v.map_err(db_err)?;
            if let Ok(h) = <[u8; 32]>::try_from(v.as_slice()) {
                set.insert(h);
            }
        }
        Ok(set)
    }

    /// Caminhos de origem já importados (`path` e `source_path`).
    pub fn known_paths(&self) -> CoreResult<HashSet<String>> {
        let mut st = self
            .connection()
            .prepare("SELECT path FROM asset UNION SELECT source_path FROM asset WHERE source_path IS NOT NULL")
            .map_err(db_err)?;
        let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn photo_count(&self) -> CoreResult<u64> {
        self.connection()
            .query_row("SELECT count(*) FROM asset", [], |r| r.get::<_, i64>(0))
            .map(|n| n as u64)
            .map_err(db_err)
    }

    /// Todas as fotografias, por ordem cronológica **corrigida** (sincronização de câmaras).
    /// `capture_time` é a hora corrigida.
    pub fn list_photos(&self) -> CoreResult<Vec<PhotoRow>> {
        let mut st = self
            .connection()
            .prepare(&format!(
                                "SELECT CASE WHEN v.is_master = 1 THEN a.id ELSE 1099511627776 + v.id END,
                        a.file_name, {CORRECTED_TIME_SQL}, a.width, a.height, a.orientation,
                        r.stars, r.flag, r.color, a.thumb_state, a.support, a.format,
                        a.id, v.id, CASE WHEN v.is_master = 1 THEN NULL ELSE v.name END,
                        EXISTS(SELECT 1 FROM edit e WHERE e.variant_id = v.id AND e.recipe != '{{\"v\":1}}')
                 FROM asset a
                 JOIN variant v ON v.asset_id = a.id
                 JOIN rating r ON r.variant_id = v.id
                 {CLOCK_JOIN_SQL}
                 ORDER BY {CORRECTED_ORDER_SQL}, v.is_master DESC, v.id"
            ))
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                let (w, h) = oriented(r.get(3)?, r.get(4)?, r.get(5)?);
                Ok(PhotoRow {
                    id: AssetId(r.get(0)?),
                    file_name: r.get(1)?,
                    capture_time: r.get(2)?,
                    width: w,
                    height: h,
                    stars: r.get(6)?,
                    flag: r.get(7)?,
                    color: r.get(8)?,
                    thumb: ThumbState::from_i64(r.get(9)?),
                    partial: r.get::<_, String>(10)? == "partial",
                    format: r.get(11)?,
                    asset: AssetId(r.get(12)?),
                    variant: r.get(13)?,
                    virtual_copy: r.get(14)?,
                    edited: r.get::<_, i64>(15)? != 0,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn photo_details(&self, id: AssetId) -> CoreResult<Option<PhotoDetails>> {
        self.connection()
            .query_row(
                &format!(
                    "SELECT id, path, source_path, file_name, file_size, format, width, height,
                        orientation, capture_time, capture_offset, camera_make, camera_model,
                        camera_serial, lens, iso, exposure_time, f_number, focal_length,
                        focal_length_35mm, imported_at, thumb_error, bit_depth, color_space,
                        codec, codec_detail, embedded_preview, support_note, {CORRECTED_TIME_SQL},
                        coalesce(cc.offset_ms, 0), gps_lat, gps_lon
                 FROM asset a {CLOCK_JOIN_SQL} WHERE a.id = ?1"
                ),
                [id.0],
                |r| {
                    Ok(PhotoDetails {
                        id: AssetId(r.get(0)?),
                        path: r.get(1)?,
                        source_path: r.get(2)?,
                        file_name: r.get(3)?,
                        file_size: r.get::<_, i64>(4)? as u64,
                        format: r.get(5)?,
                        width: r.get(6)?,
                        height: r.get(7)?,
                        orientation: r.get(8)?,
                        capture_time: r.get(9)?,
                        capture_offset: r.get(10)?,
                        camera_make: r.get(11)?,
                        camera_model: r.get(12)?,
                        camera_serial: r.get(13)?,
                        lens: r.get(14)?,
                        iso: r.get(15)?,
                        exposure_time: r.get(16)?,
                        f_number: r.get(17)?,
                        focal_length: r.get(18)?,
                        focal_length_35mm: r.get(19)?,
                        imported_at: r.get(20)?,
                        thumb_error: r.get(21)?,
                        bit_depth: r.get(22)?,
                        color_space: r.get(23)?,
                        codec: r.get(24)?,
                        codec_detail: r.get(25)?,
                        embedded_preview: r.get(26)?,
                        partial_note: r.get(27)?,
                        corrected_time: r.get(28)?,
                        clock_offset_ms: r.get(29)?,
                        gps_lat: r.get(30)?,
                        gps_lon: r.get(31)?,
                    })
                },
            )
            .optional()
            .map_err(db_err)
    }

    pub fn asset_source(&self, id: AssetId) -> CoreResult<Option<AssetSource>> {
        self.connection()
            .query_row("SELECT path, format, orientation FROM asset WHERE id = ?1", [id.0], |r| {
                Ok(AssetSource { path: r.get(0)?, format: r.get(1)?, orientation: r.get(2)? })
            })
            .optional()
            .map_err(db_err)
    }

    fn update_rating_column(
        &mut self,
        ids: &[AssetId],
        column: &str,
        value: &dyn rusqlite::ToSql,
    ) -> CoreResult<usize> {
        let sql = format!(
                        "UPDATE rating SET {column} = ?1, updated_at = ?2 WHERE variant_id = {VARIANT_OF_KEY_3}"
        );
        let now = now_unix();
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        let mut n = 0;
        {
            let mut st = tx.prepare(&sql).map_err(db_err)?;
            for id in ids {
                n += st.execute(params![value, now, id.0]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(n)
    }

    pub fn set_stars(&mut self, ids: &[AssetId], stars: u8) -> CoreResult<usize> {
        if stars > 5 {
            return Err(CoreError::Other(format!("estrelas inválidas: {stars}")));
        }
        self.update_rating_column(ids, "stars", &stars)
    }

    pub fn set_flag(&mut self, ids: &[AssetId], flag: i8) -> CoreResult<usize> {
        if !(-1..=1).contains(&flag) {
            return Err(CoreError::Other(format!("marca inválida: {flag}")));
        }
        self.update_rating_column(ids, "flag", &flag)
    }

    pub fn set_color(&mut self, ids: &[AssetId], color: Option<&str>) -> CoreResult<usize> {
        if let Some(c) = color {
            if !COLORS.contains(&c) {
                return Err(CoreError::Other(format!("cor inválida: {c}")));
            }
        }
        self.update_rating_column(ids, "color", &color)
    }

    /// Fotografias cuja miniatura ainda não foi gerada (retoma após fecho).
    pub fn pending_thumbnail_ids(&self) -> CoreResult<Vec<AssetId>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT id FROM asset WHERE thumb_state = 0
                 ORDER BY capture_time IS NULL, capture_time, file_name, id",
            )
            .map_err(db_err)?;
        let rows = st.query_map([], |r| Ok(AssetId(r.get(0)?))).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn set_thumb_states(
        &mut self,
        results: &[(AssetId, ThumbState, Option<String>)],
    ) -> CoreResult<()> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        {
            let mut st = tx
                .prepare("UPDATE asset SET thumb_state = ?1, thumb_error = ?2 WHERE id = ?3")
                .map_err(db_err)?;
            for (id, s, e) in results {
                st.execute(params![*s as i64, e, id.0]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    /// Marca todas as miniaturas como pendentes (ex.: cache apagada manualmente).
    pub fn reset_thumbnails(&self) -> CoreResult<()> {
        self.connection()
            .execute("UPDATE asset SET thumb_state = 0, thumb_error = NULL", [])
            .map_err(db_err)?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn new_asset(n: u8) -> NewAsset {
        NewAsset {
            path: format!("/fotos/IMG_{n:04}.jpg"),
            file_name: format!("IMG_{n:04}.jpg"),
            file_size: 1000 + n as u64,
            content_hash: [n; 32],
            format: "jpeg".into(),
            width: 6000,
            height: 4000,
            orientation: 1,
            capture_time: Some(format!("2026-06-15T14:00:{:02}", n % 60)),
            camera_make: Some("Canon".into()),
            camera_model: Some("EOS R5".into()),
            iso: Some(800),
            ..Default::default()
        }
    }

    #[test]
    fn insert_detects_duplicates_by_content_and_path() {
        let mut c = Catalog::open_in_memory().unwrap();
        let a = new_asset(1);
        let mut same_content = new_asset(1);
        same_content.path = "/outra/pasta/copia.jpg".into();
        let same_path = NewAsset { content_hash: [9; 32], ..new_asset(1) };
        let res = c.insert_assets(&[a, same_content, same_path, new_asset(2)]).unwrap();
        let first = match res[0] {
            InsertResult::Inserted(id) => id,
            r => panic!("{r:?}"),
        };
        assert_eq!(res[1], InsertResult::DuplicateContent(first));
        assert_eq!(res[2], InsertResult::DuplicatePath(first));
        assert!(matches!(res[3], InsertResult::Inserted(_)));
        assert_eq!(c.photo_count().unwrap(), 2);
        assert!(c.content_hashes().unwrap().contains(&[1; 32]));
        assert!(c.known_paths().unwrap().contains("/fotos/IMG_0001.jpg"));
    }

    #[test]
    fn ratings_persist_after_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        let ids: Vec<AssetId> = {
            let mut c = Catalog::open(&path).unwrap();
            let res = c.insert_assets(&(1..=5).map(new_asset).collect::<Vec<_>>()).unwrap();
            let ids: Vec<_> = res
                .iter()
                .map(|r| match r {
                    InsertResult::Inserted(id) => *id,
                    _ => panic!(),
                })
                .collect();
            c.set_stars(&ids[0..2], 4).unwrap();
            c.set_flag(&ids[1..3], 1).unwrap();
            c.set_flag(&ids[4..5], -1).unwrap();
            c.set_color(&ids[0..1], Some("red")).unwrap();
            c.set_color(&ids[0..1], None).unwrap();
            c.set_color(&ids[3..4], Some("blue")).unwrap();
            ids
        };
        let c = Catalog::open(&path).unwrap();
        let rows = c.list_photos().unwrap();
        let get = |id: AssetId| rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!(get(ids[0]).stars, 4);
        assert_eq!(get(ids[0]).color, None);
        assert_eq!(get(ids[1]).flag, 1);
        assert_eq!(get(ids[3]).color.as_deref(), Some("blue"));
        assert_eq!(get(ids[4]).flag, -1);
        assert_eq!(get(ids[2]).stars, 0);
    }

    #[test]
    fn rejects_invalid_ratings() {
        let mut c = Catalog::open_in_memory().unwrap();
        assert!(c.set_stars(&[AssetId(1)], 6).is_err());
        assert!(c.set_flag(&[AssetId(1)], 2).is_err());
        assert!(c.set_color(&[AssetId(1)], Some("pink")).is_err());
    }

    #[test]
    fn list_orders_by_capture_time_and_orients_dimensions() {
        let mut c = Catalog::open_in_memory().unwrap();
        let mut late = new_asset(1);
        late.capture_time = Some("2026-06-15T20:00:00".into());
        late.orientation = 6;
        let mut none = new_asset(2);
        none.capture_time = None;
        let early = new_asset(3);
        c.insert_assets(&[late, none, early]).unwrap();
        let rows = c.list_photos().unwrap();
        let names: Vec<_> = rows.iter().map(|r| r.file_name.as_str()).collect();
        assert_eq!(names, ["IMG_0003.jpg", "IMG_0001.jpg", "IMG_0002.jpg"]);
        assert_eq!((rows[1].width, rows[1].height), (4000, 6000));
    }

    #[test]
    fn thumbnail_states() {
        let mut c = Catalog::open_in_memory().unwrap();
        c.insert_assets(&[new_asset(1), new_asset(2)]).unwrap();
        let pending = c.pending_thumbnail_ids().unwrap();
        assert_eq!(pending.len(), 2);
        c.set_thumb_states(&[
            (pending[0], ThumbState::Ready, None),
            (pending[1], ThumbState::Failed, Some("x".into())),
        ])
        .unwrap();
        assert!(c.pending_thumbnail_ids().unwrap().is_empty());
        assert_eq!(c.photo_details(pending[1]).unwrap().unwrap().thumb_error.as_deref(), Some("x"));
        c.reset_thumbnails().unwrap();
        assert_eq!(c.pending_thumbnail_ids().unwrap().len(), 2);
    }
}
