//! Repositório da análise (Fase 3): métricas, rostos, embeddings, grupos e recomendações.
//! Os tipos são primitivos para não acoplar a base de dados aos crates de análise.

use crate::{db_err, now_unix, Catalog, CLOCK_JOIN_SQL, CORRECTED_TIME_SQL};
use momento_core_types::{AssetId, CoreResult};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Versão do algoritmo de análise (mudar obriga a reanalisar).
pub const ANALYSIS_VERSION: i64 = 2;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRow {
    pub models: Option<String>,
    pub width: u32,
    pub height: u32,
    pub sharpness: f32,
    pub focus_peak: f32,
    pub focus_center: f32,
    pub motion_blur: f32,
    pub brightness: f32,
    pub highlights_clipped: f32,
    pub shadows_crushed: f32,
    pub contrast: f32,
    pub noise: f32,
    pub tilt_degrees: Option<f32>,
    pub phash: u64,
    pub dhash: u64,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceRow {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub px: f32,
    pub det_score: f32,
    pub presence: f32,
    pub blink_left: f32,
    pub blink_right: f32,
    pub smile: f32,
    pub jaw_open: f32,
    pub sharpness: f32,
    pub eye_sharpness: f32,
    pub cut: bool,
    #[serde(skip)]
    pub blendshapes: Vec<f32>,
    /// Identificador do rosto no catálogo (0 antes de gravar).
    pub id: i64,
    /// Pessoa atribuída (agrupamento ou utilizador).
    pub person_id: Option<i64>,
    /// Identidade SFace (128-D); vazia se o rosto for pequeno ou o modelo não existir.
    #[serde(skip)]
    pub identity: Vec<f32>,
}

/// Resultado completo de uma fotografia, gravado numa transação.
#[derive(Debug, Clone, Default)]
pub struct AnalysisResult {
    pub asset: AssetId,
    pub row: AnalysisRow,
    pub faces: Vec<FaceRow>,
    pub embedding: Option<(String, Vec<f32>)>,
    /// Classes ImageNet mais prováveis (índice, probabilidade).
    pub scene: Vec<(u16, f32)>,
}

/// Dados para o motor de seleção.
#[derive(Debug, Clone, Default)]
pub struct SelectionRow {
    pub asset: AssetId,
    pub path: String,
    pub format: String,
    pub file_name: String,
    pub capture_time: Option<String>,
    pub camera: Option<String>,
    pub analysis: AnalysisRow,
    pub faces: Vec<FaceRow>,
    pub embedding: Option<Vec<f32>>,
    pub user_flag: i8,
    pub gps: Option<(f64, f64)>,
    pub iso: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRow {
    pub id: i64,
    pub kind: String,
    pub size: u32,
    /// Membros pela ordem de recomendação (melhor primeiro).
    pub members: Vec<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationRow {
    pub asset: AssetId,
    pub label: String,
    /// Índice do grupo na lista `groups` passada a `save_selection` (convertido em id).
    pub group_index: Option<usize>,
    pub rank: u32,
    pub group_size: u32,
    pub summary: String,
    /// JSON `[{code, value?}]`.
    pub reasons: String,
    pub focus: f32,
    pub motion: f32,
    pub exposure: f32,
    pub noise: f32,
    pub eyes: Option<f32>,
    pub expression: Option<f32>,
    pub composition: f32,
    pub technical: f32,
    pub people: Option<f32>,
    pub overall: f32,
}

/// Etiqueta resumida para a grelha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationBrief {
    pub asset: AssetId,
    pub label: String,
    pub group_id: Option<i64>,
    pub rank: u32,
    pub group_size: u32,
}

/// Detalhe para o painel de informação.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisDetails {
    pub analysis: Option<AnalysisRow>,
    pub faces: Vec<FaceRow>,
    pub recommendation: Option<RecommendationRow>,
    pub group_id: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisCounts {
    pub total: u64,
    pub analyzed: u64,
    pub failed: u64,
    pub pending: u64,
}

fn f32s_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn blob_to_f32s(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn read_analysis(r: &rusqlite::Row, o: usize) -> rusqlite::Result<AnalysisRow> {
    Ok(AnalysisRow {
        models: r.get(o)?,
        width: r.get(o + 1)?,
        height: r.get(o + 2)?,
        sharpness: r.get(o + 3)?,
        focus_peak: r.get(o + 4)?,
        focus_center: r.get(o + 5)?,
        motion_blur: r.get(o + 6)?,
        brightness: r.get(o + 7)?,
        highlights_clipped: r.get(o + 8)?,
        shadows_crushed: r.get(o + 9)?,
        contrast: r.get(o + 10)?,
        noise: r.get(o + 11)?,
        tilt_degrees: r.get(o + 12)?,
        phash: r.get::<_, i64>(o + 13)? as u64,
        dhash: r.get::<_, i64>(o + 14)? as u64,
        duration_ms: r.get(o + 15)?,
    })
}

const ANALYSIS_COLS: &str = "a.models, a.width, a.height, a.sharpness, a.focus_peak, a.focus_center, a.motion_blur, a.brightness, a.highlights_clipped, a.shadows_crushed, a.contrast, a.noise, a.tilt_degrees, a.phash, a.dhash, a.duration_ms";
const FACE_COLS: &str = "x, y, w, h, px, det_score, presence, blink_left, blink_right, smile, jaw_open, sharpness, eye_sharpness, cut, blendshapes";

const FACE_READ_COLS: &str = "x, y, w, h, px, det_score, presence, blink_left, blink_right, smile, jaw_open, sharpness, eye_sharpness, cut, blendshapes, id, person_id";

pub(crate) fn scene_to_blob(v: &[(u16, f32)]) -> Vec<u8> {
    v.iter()
        .flat_map(|(i, p)| [i.to_le_bytes().as_slice(), p.to_le_bytes().as_slice()].concat())
        .collect()
}
pub(crate) fn blob_to_scene(b: &[u8]) -> Vec<(u16, f32)> {
    b.chunks_exact(6)
        .map(|c| (u16::from_le_bytes([c[0], c[1]]), f32::from_le_bytes([c[2], c[3], c[4], c[5]])))
        .collect()
}

/// Rosto anterior com decisão do utilizador: (caixa, pessoa, origem).
type KeptFace = ((f32, f32, f32, f32), Option<i64>, i64);

fn iou(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> f32 {
    let x1 = a.0.max(b.0);
    let y1 = a.1.max(b.1);
    let x2 = (a.0 + a.2).min(b.0 + b.2);
    let y2 = (a.1 + a.3).min(b.1 + b.3);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let u = a.2 * a.3 + b.2 * b.3 - inter;
    if u <= 0.0 {
        0.0
    } else {
        inter / u
    }
}

fn read_face(r: &rusqlite::Row, o: usize) -> rusqlite::Result<FaceRow> {
    Ok(FaceRow {
        x: r.get(o)?,
        y: r.get(o + 1)?,
        w: r.get(o + 2)?,
        h: r.get(o + 3)?,
        px: r.get(o + 4)?,
        det_score: r.get(o + 5)?,
        presence: r.get(o + 6)?,
        blink_left: r.get(o + 7)?,
        blink_right: r.get(o + 8)?,
        smile: r.get(o + 9)?,
        jaw_open: r.get(o + 10)?,
        sharpness: r.get(o + 11)?,
        eye_sharpness: r.get(o + 12)?,
        cut: r.get::<_, i64>(o + 13)? != 0,
        blendshapes: r
            .get::<_, Option<Vec<u8>>>(o + 14)?
            .map(|b| blob_to_f32s(&b))
            .unwrap_or_default(),
        id: r.get(o + 15)?,
        person_id: r.get(o + 16)?,
        identity: Vec::new(),
    })
}

impl Catalog {
    /// Fotografias por analisar (inclui as de versões antigas do algoritmo).
    pub fn pending_analysis_ids(&self) -> CoreResult<Vec<AssetId>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT id FROM asset WHERE analysis_state = 0 AND thumb_state != 2
                 ORDER BY capture_time IS NULL, capture_time, file_name, id",
            )
            .map_err(db_err)?;
        let rows = st.query_map([], |r| Ok(AssetId(r.get(0)?))).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn analysis_counts(&self) -> CoreResult<AnalysisCounts> {
        self.connection()
            .query_row(
                "SELECT count(*), sum(analysis_state = 1), sum(analysis_state = 2), sum(analysis_state = 0) FROM asset",
                [],
                |r| {
                    Ok(AnalysisCounts {
                        total: r.get::<_, i64>(0)? as u64,
                        analyzed: r.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64,
                        failed: r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
                        pending: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as u64,
                    })
                },
            )
            .map_err(db_err)
    }

    /// Grava resultados de análise em lote (uma transação).
    pub fn save_analyses(
        &mut self,
        results: &[AnalysisResult],
        failures: &[(AssetId, String)],
    ) -> CoreResult<()> {
        let now = now_unix();
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        {
            let mut del_f = tx.prepare("DELETE FROM face WHERE asset_id = ?1").map_err(db_err)?;
            // Decisões do utilizador sobre pessoas (confirmar/excluir) sobrevivem à reanálise:
            // passam para o novo rosto na mesma posição (IoU ≥ 0,5).
            let mut old_f = tx
                .prepare("SELECT x, y, w, h, person_id, person_source FROM face WHERE asset_id = ?1 AND person_source != 0")
                .map_err(db_err)?;
            let mut ins_s = tx
                .prepare("INSERT OR REPLACE INTO scene(asset_id, top) VALUES(?1, ?2)")
                .map_err(db_err)?;
            let mut ins_a = tx
                .prepare(
                    "INSERT OR REPLACE INTO analysis(asset_id, version, analyzed_at, models, width, height, sharpness,
                        focus_peak, focus_center, motion_blur, brightness, highlights_clipped, shadows_crushed,
                        contrast, noise, tilt_degrees, phash, dhash, face_count, duration_ms)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
                )
                .map_err(db_err)?;
            let mut ins_f = tx
                .prepare(&format!("INSERT INTO face(asset_id, {FACE_COLS}, identity, person_id, person_source) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)"))
                .map_err(db_err)?;
            let mut ins_e = tx
                .prepare(
                    "INSERT OR REPLACE INTO embedding(asset_id, model, vector) VALUES(?1, ?2, ?3)",
                )
                .map_err(db_err)?;
            let mut state = tx
                .prepare("UPDATE asset SET analysis_state = ?1, analysis_error = ?2 WHERE id = ?3")
                .map_err(db_err)?;
            for res in results {
                let a = &res.row;
                let kept: Vec<KeptFace> = old_f
                    .query_map([res.asset.0], |r| {
                        Ok(((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?), r.get(4)?, r.get(5)?))
                    })
                    .map_err(db_err)?
                    .collect::<Result<_, _>>()
                    .map_err(db_err)?;
                del_f.execute([res.asset.0]).map_err(db_err)?;
                if !res.scene.is_empty() {
                    ins_s
                        .execute(params![res.asset.0, scene_to_blob(&res.scene)])
                        .map_err(db_err)?;
                }
                ins_a
                    .execute(params![
                        res.asset.0,
                        ANALYSIS_VERSION,
                        now,
                        a.models,
                        a.width,
                        a.height,
                        a.sharpness,
                        a.focus_peak,
                        a.focus_center,
                        a.motion_blur,
                        a.brightness,
                        a.highlights_clipped,
                        a.shadows_crushed,
                        a.contrast,
                        a.noise,
                        a.tilt_degrees,
                        a.phash as i64,
                        a.dhash as i64,
                        res.faces.len() as i64,
                        a.duration_ms
                    ])
                    .map_err(db_err)?;
                for f in &res.faces {
                    let carried = kept
                        .iter()
                        .map(|k| (iou(k.0, (f.x, f.y, f.w, f.h)), k))
                        .filter(|(o, _)| *o >= 0.5)
                        .max_by(|a, b| a.0.total_cmp(&b.0))
                        .map(|(_, k)| (k.1, k.2));
                    let (pid, src) = carried.unwrap_or((None, 0));
                    ins_f
                        .execute(params![
                            res.asset.0,
                            f.x,
                            f.y,
                            f.w,
                            f.h,
                            f.px,
                            f.det_score,
                            f.presence,
                            f.blink_left,
                            f.blink_right,
                            f.smile,
                            f.jaw_open,
                            f.sharpness,
                            f.eye_sharpness,
                            f.cut as i64,
                            (!f.blendshapes.is_empty()).then(|| f32s_to_blob(&f.blendshapes)),
                            (!f.identity.is_empty()).then(|| f32s_to_blob(&f.identity)),
                            pid,
                            src
                        ])
                        .map_err(db_err)?;
                }
                if let Some((model, v)) = &res.embedding {
                    ins_e.execute(params![res.asset.0, model, f32s_to_blob(v)]).map_err(db_err)?;
                }
                state.execute(params![1, Option::<String>::None, res.asset.0]).map_err(db_err)?;
            }
            for (id, err) in failures {
                state.execute(params![2, err, id.0]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    /// Todas as fotografias analisadas, com rostos, embedding e decisão do utilizador.
    pub fn selection_rows(&self) -> CoreResult<Vec<SelectionRow>> {
        let c = self.connection();
        let mut st = c
            .prepare(&format!(
                "SELECT s.id, s.path, s.format, s.file_name, {corrected},
                        trim(coalesce(s.camera_make, '') || ' ' || coalesce(s.camera_model, '') || ' ' || coalesce(s.camera_serial, '')),
                        r.flag, {ANALYSIS_COLS}, s.gps_lat, s.gps_lon, s.iso
                 FROM asset s
                 JOIN analysis a ON a.asset_id = s.id
                 JOIN variant v ON v.asset_id = s.id AND v.is_master = 1
                 JOIN rating r ON r.variant_id = v.id
                 {clock}
                 WHERE s.analysis_state = 1
                 ORDER BY s.id",
                corrected = CORRECTED_TIME_SQL.replace("a.capture_time", "s.capture_time"),
                clock = CLOCK_JOIN_SQL.replace("a.camera", "s.camera"),
            ))
            .map_err(db_err)?;
        let mut rows: Vec<SelectionRow> = st
            .query_map([], |r| {
                Ok(SelectionRow {
                    asset: AssetId(r.get(0)?),
                    path: r.get(1)?,
                    format: r.get(2)?,
                    file_name: r.get(3)?,
                    capture_time: r.get(4)?,
                    camera: r.get::<_, String>(5).ok().filter(|s| !s.is_empty()),
                    user_flag: r.get(6)?,
                    analysis: read_analysis(r, 7)?,
                    gps: match (r.get::<_, Option<f64>>(23)?, r.get::<_, Option<f64>>(24)?) {
                        (Some(a), Some(b)) => Some((a, b)),
                        _ => None,
                    },
                    iso: r.get(25)?,
                    ..Default::default()
                })
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        let index: std::collections::HashMap<i64, usize> =
            rows.iter().enumerate().map(|(i, r)| (r.asset.0, i)).collect();
        let mut fst = c
            .prepare(&format!("SELECT asset_id, {FACE_READ_COLS} FROM face ORDER BY asset_id, id"))
            .map_err(db_err)?;
        let faces =
            fst.query_map([], |r| Ok((r.get::<_, i64>(0)?, read_face(r, 1)?))).map_err(db_err)?;
        for f in faces {
            let (id, face) = f.map_err(db_err)?;
            if let Some(&i) = index.get(&id) {
                rows[i].faces.push(face);
            }
        }
        let mut est = c.prepare("SELECT asset_id, vector FROM embedding").map_err(db_err)?;
        let embs = est
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))
            .map_err(db_err)?;
        for e in embs {
            let (id, v) = e.map_err(db_err)?;
            if let Some(&i) = index.get(&id) {
                rows[i].embedding = Some(blob_to_f32s(&v));
            }
        }
        Ok(rows)
    }

    /// Substitui grupos e recomendações (resultado de uma execução do motor de seleção).
    pub fn save_selection(
        &mut self,
        groups: &[(String, usize)],
        recs: &[RecommendationRow],
    ) -> CoreResult<()> {
        let now = now_unix();
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        tx.execute_batch("DELETE FROM recommendation; DELETE FROM photo_group;").map_err(db_err)?;
        let mut ids = Vec::with_capacity(groups.len());
        {
            let mut ins_g =
                tx.prepare("INSERT INTO photo_group(kind, size) VALUES(?1, ?2)").map_err(db_err)?;
            for (kind, size) in groups {
                ins_g.execute(params![kind, *size as i64]).map_err(db_err)?;
                ids.push(tx.last_insert_rowid());
            }
            let mut ins_r = tx
                .prepare(
                    "INSERT INTO recommendation(asset_id, label, group_id, rank, group_size, summary, reasons, focus,
                        motion, exposure, noise, eyes, expression, composition, technical, people, overall, computed_at)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                )
                .map_err(db_err)?;
            for r in recs {
                ins_r
                    .execute(params![
                        r.asset.0,
                        r.label,
                        r.group_index.and_then(|g| ids.get(g).copied()),
                        r.rank,
                        r.group_size,
                        r.summary,
                        r.reasons,
                        r.focus,
                        r.motion,
                        r.exposure,
                        r.noise,
                        r.eyes,
                        r.expression,
                        r.composition,
                        r.technical,
                        r.people,
                        r.overall,
                        now
                    ])
                    .map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    pub fn list_recommendations(&self) -> CoreResult<Vec<RecommendationBrief>> {
        let mut st = self
            .connection()
            .prepare("SELECT asset_id, label, group_id, rank, group_size FROM recommendation ORDER BY asset_id")
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(RecommendationBrief {
                    asset: AssetId(r.get(0)?),
                    label: r.get(1)?,
                    group_id: r.get(2)?,
                    rank: r.get(3)?,
                    group_size: r.get(4)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Etiquetas das fotografias de um grupo, por ordem.
    pub fn group_labels(&self, group: i64) -> CoreResult<Vec<(AssetId, String)>> {
        let mut st = self
            .connection()
            .prepare("SELECT asset_id, label FROM recommendation WHERE group_id = ?1 ORDER BY rank")
            .map_err(db_err)?;
        let rows =
            st.query_map([group], |r| Ok((AssetId(r.get(0)?), r.get(1)?))).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn labels_for(&self, ids: &[AssetId]) -> CoreResult<Vec<(AssetId, String)>> {
        let mut st = self
            .connection()
            .prepare("SELECT label FROM recommendation WHERE asset_id = ?1")
            .map_err(db_err)?;
        let mut out = Vec::new();
        for id in ids {
            if let Some(l) =
                st.query_row([id.0], |r| r.get::<_, String>(0)).optional().map_err(db_err)?
            {
                out.push((*id, l));
            }
        }
        Ok(out)
    }

    pub fn list_groups(&self) -> CoreResult<Vec<GroupRow>> {
        let c = self.connection();
        let mut st =
            c.prepare("SELECT id, kind, size FROM photo_group ORDER BY id").map_err(db_err)?;
        let mut groups: Vec<GroupRow> = st
            .query_map([], |r| {
                Ok(GroupRow {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    size: r.get(2)?,
                    members: Vec::new(),
                })
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        let index: std::collections::HashMap<i64, usize> =
            groups.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
        let mut m = c
            .prepare("SELECT group_id, asset_id FROM recommendation WHERE group_id IS NOT NULL ORDER BY group_id, rank")
            .map_err(db_err)?;
        let rows =
            m.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).map_err(db_err)?;
        for row in rows {
            let (g, a) = row.map_err(db_err)?;
            if let Some(&i) = index.get(&g) {
                groups[i].members.push(AssetId(a));
            }
        }
        Ok(groups)
    }

    pub fn analysis_details(&self, id: AssetId) -> CoreResult<AnalysisDetails> {
        let c = self.connection();
        let analysis = c
            .query_row(
                &format!("SELECT {ANALYSIS_COLS} FROM analysis a WHERE a.asset_id = ?1"),
                [id.0],
                |r| read_analysis(r, 0),
            )
            .optional()
            .map_err(db_err)?;
        let mut st = c
            .prepare(&format!(
                "SELECT {FACE_READ_COLS} FROM face WHERE asset_id = ?1 ORDER BY w * h DESC"
            ))
            .map_err(db_err)?;
        let faces = st
            .query_map([id.0], |r| read_face(r, 0))
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        let rec = c
            .query_row(
                "SELECT label, group_id, rank, group_size, summary, reasons, focus, motion, exposure, noise, eyes,
                        expression, composition, technical, people, overall
                 FROM recommendation WHERE asset_id = ?1",
                [id.0],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(1)?,
                        RecommendationRow {
                            asset: id,
                            label: r.get(0)?,
                            group_index: None,
                            rank: r.get(2)?,
                            group_size: r.get(3)?,
                            summary: r.get(4)?,
                            reasons: r.get(5)?,
                            focus: r.get(6)?,
                            motion: r.get(7)?,
                            exposure: r.get(8)?,
                            noise: r.get(9)?,
                            eyes: r.get(10)?,
                            expression: r.get(11)?,
                            composition: r.get(12)?,
                            technical: r.get(13)?,
                            people: r.get(14)?,
                            overall: r.get(15)?,
                        },
                    ))
                },
            )
            .optional()
            .map_err(db_err)?;
        let error = c
            .query_row("SELECT analysis_error FROM asset WHERE id = ?1", [id.0], |r| r.get(0))
            .optional()
            .map_err(db_err)?
            .flatten();
        let (group_id, recommendation) = match rec {
            Some((g, r)) => (g, Some(r)),
            None => (None, None),
        };
        Ok(AnalysisDetails { analysis, faces, recommendation, group_id, error })
    }

    /// Apaga toda a análise (métricas, rostos, embeddings, recomendações) e marca as
    /// fotografias como por analisar. Não toca nas classificações do utilizador.
    pub fn clear_analysis(&mut self) -> CoreResult<()> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        tx.execute_batch(
            "DELETE FROM recommendation; DELETE FROM photo_group; DELETE FROM embedding; DELETE FROM scene;
             DELETE FROM face WHERE person_source = 0; DELETE FROM analysis; UPDATE asset SET analysis_state = 0, analysis_error = NULL;",
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    /// Aplica a decisão “aceitar recomendação” a um conjunto: favorita para `picks`,
    /// rejeitada para `rejects` — só em fotografias ainda sem decisão do utilizador.
    pub fn accept_recommendations(
        &mut self,
        picks: &[AssetId],
        rejects: &[AssetId],
    ) -> CoreResult<usize> {
        let now = now_unix();
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        let mut n = 0;
        {
            let mut st = tx
                .prepare(
                    "UPDATE rating SET flag = ?1, updated_at = ?2
                     WHERE flag = 0 AND variant_id = (SELECT id FROM variant WHERE asset_id = ?3 AND is_master = 1)",
                )
                .map_err(db_err)?;
            for id in picks {
                n += st.execute(params![1, now, id.0]).map_err(db_err)?;
            }
            for id in rejects {
                n += st.execute(params![-1, now, id.0]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(n)
    }

    /// Pontuação global (0–1) de cada fotografia analisada (álbum automático).
    pub fn overall_scores(&self) -> CoreResult<std::collections::HashMap<AssetId, f32>> {
        let c = self.connection();
        let mut st = c.prepare("SELECT asset_id, overall FROM recommendation").map_err(db_err)?;
        let rows = st
            .query_map([], |r| Ok((AssetId(r.get(0)?), r.get::<_, f64>(1)? as f32)))
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn load_setting(&self, key: &str) -> CoreResult<Option<String>> {
        self.connection()
            .query_row("SELECT value FROM catalog_info WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .map_err(db_err)
    }

    pub fn save_setting(&self, key: &str, value: &str) -> CoreResult<()> {
        self.connection()
            .execute(
                "INSERT INTO catalog_info(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, value],
            )
            .map_err(db_err)?;
        Ok(())
    }
}

pub(crate) fn blob_to_f32s_pub(b: &[u8]) -> Vec<f32> {
    blob_to_f32s(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::tests::new_asset;
    use crate::assets::InsertResult;

    #[test]
    fn analysis_roundtrip_selection_and_accept() {
        let mut c = Catalog::open_in_memory().unwrap();
        let ids: Vec<AssetId> = c
            .insert_assets(&[new_asset(1), new_asset(2), new_asset(3)])
            .unwrap()
            .into_iter()
            .map(|r| match r {
                InsertResult::Inserted(id) => id,
                _ => panic!(),
            })
            .collect();
        assert_eq!(c.pending_analysis_ids().unwrap().len(), 3);
        let face = FaceRow {
            x: 0.1,
            y: 0.2,
            w: 0.1,
            h: 0.15,
            px: 120.0,
            blink_left: 0.1,
            smile: 0.7,
            blendshapes: vec![0.5; 52],
            ..Default::default()
        };
        let res = AnalysisResult {
            asset: ids[0],
            row: AnalysisRow {
                width: 2048,
                height: 1365,
                sharpness: 2.1,
                phash: u64::MAX - 3,
                ..Default::default()
            },
            faces: vec![face.clone(), face],
            embedding: Some(("mobilenet_v3_small".into(), vec![0.25; 1024])),
            scene: Vec::new(),
        };
        c.save_analyses(&[res], &[(ids[2], "falhou".into())]).unwrap();
        let counts = c.analysis_counts().unwrap();
        assert_eq!((counts.analyzed, counts.failed, counts.pending), (1, 1, 1));
        let rows = c.selection_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].faces.len(), 2);
        assert_eq!(rows[0].faces[0].blendshapes.len(), 52);
        assert_eq!(rows[0].embedding.as_ref().unwrap().len(), 1024);
        assert_eq!(
            rows[0].analysis.phash,
            u64::MAX - 3,
            "u64 preservado através de INTEGER com sinal"
        );

        let rec = |a: AssetId, label: &str, g: Option<usize>, rank: u32| RecommendationRow {
            asset: a,
            label: label.into(),
            group_index: g,
            rank,
            group_size: 2,
            summary: "s".into(),
            reasons: "[]".into(),
            overall: 0.5,
            ..Default::default()
        };
        c.save_selection(
            &[("sequence".into(), 2)],
            &[rec(ids[0], "best", Some(0), 0), rec(ids[1], "reject", Some(0), 1)],
        )
        .unwrap();
        let groups = c.list_groups().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(c.list_recommendations().unwrap().len(), 2);
        assert_eq!(c.group_labels(groups[0].id).unwrap()[1].1, "reject");
        assert_eq!(c.labels_for(&[ids[0], ids[2]]).unwrap().len(), 1);
        assert_eq!(groups[0].members, vec![ids[0], ids[1]]);
        let d = c.analysis_details(ids[0]).unwrap();
        assert_eq!(d.recommendation.unwrap().label, "best");
        assert_eq!(d.faces.len(), 2);
        assert_eq!(c.analysis_details(ids[2]).unwrap().error.as_deref(), Some("falhou"));

        // Aceitar: não altera decisões já tomadas pelo utilizador.
        c.set_flag(&[ids[1]], 1).unwrap();
        let n = c.accept_recommendations(&[ids[0]], &[ids[1]]).unwrap();
        assert_eq!(n, 1);
        let photos = c.list_photos().unwrap();
        assert_eq!(photos.iter().find(|p| p.id == ids[0]).unwrap().flag, 1);
        assert_eq!(
            photos.iter().find(|p| p.id == ids[1]).unwrap().flag,
            1,
            "decisão do utilizador mantida"
        );

        c.clear_analysis().unwrap();
        assert_eq!(c.analysis_counts().unwrap().pending, 3);
        assert!(c.list_groups().unwrap().is_empty());
        assert_eq!(c.analysis_details(ids[0]).unwrap().faces.len(), 0);
    }
}
