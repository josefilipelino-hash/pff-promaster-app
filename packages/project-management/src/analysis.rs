//! Análise local em segundo plano e motor de seleção.
//!
//! Por fotografia (numa imagem orientada com lado maior ≤ [`WORK_EDGE`]):
//! 1. métricas técnicas clássicas (nitidez, foco, movimento, exposição, ruído, horizonte);
//! 2. hashes percetuais (pHash, dHash);
//! 3. com os modelos disponíveis: rostos (deteção multiescala, 478 pontos, 52 blendshapes),
//!    nitidez do rosto e dos olhos, rosto cortado, e um embedding visual de 1024 dimensões.
//!
//! Tudo corre **neste computador**: nenhuma imagem ou resultado sai da máquina. Sem os
//! modelos, a análise continua só com as métricas técnicas (e a interface avisa).
//!
//! Os resultados ficam no catálogo; o trabalho por fazer (`analysis_state = 0`) é retomado
//! na próxima abertura. No fim de cada execução, o motor de seleção recalcula grupos e
//! recomendações — que **nunca** apagam nem alteram fotografias.

use crate::paths::{CacheLevel, ProjectPaths};
use image::{imageops, RgbImage};
use momento_ai_analysis::selection::{self, FaceInfo, Label, PhotoInput, Settings};
use momento_ai_analysis::{hash, metrics};
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_database::analysis::{
    AnalysisResult, AnalysisRow, FaceRow, RecommendationRow, SelectionRow,
};
use momento_database::Catalog;
use momento_face_analysis::{blendshape_index, FaceResult, VisionModels};
use momento_image_codecs::{format_from_id, CodecRegistry, FormatFamily};
use momento_image_processing::apply_orientation;
use momento_task_queue::WorkQueue;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Lado maior da imagem analisada.
pub const WORK_EDGE: u32 = 2048;
/// Identificador do modelo de embedding guardado no catálogo.
pub const EMBEDDING_MODEL: &str = "mediapipe-mobilenet_v3_small";
const SETTINGS_KEY: &str = "selection_settings";

/// Progresso enviado à interface.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    pub done: u64,
    pub failed: u64,
    pub total: u64,
    pub remaining: u64,
    /// `true` no último lote, depois de recalculada a seleção.
    pub finished: bool,
}

/// Contagens por etiqueta, para a barra lateral.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSummary {
    pub best: u64,
    pub alternative: u64,
    pub review: u64,
    pub reject: u64,
    pub duplicate: u64,
    pub groups: u64,
    pub sequences: u64,
    pub duration_ms: u64,
}

pub type ProgressCallback = Arc<dyn Fn(AnalysisProgress) + Send + Sync>;

// ------------------------------------------------------------------ análise de uma foto

/// Imagem de trabalho orientada: a preview de ecrã em cache (já orientada, rápida) ou,
/// se não existir, o original descodificado a resolução reduzida.
fn work_image(
    paths: &ProjectPaths,
    catalog: &Catalog,
    registry: &CodecRegistry,
    id: AssetId,
) -> CoreResult<RgbImage> {
    let preview = paths.cached(CacheLevel::Preview, id);
    let img = match image::open(&preview) {
        Ok(i) if preview.is_file() => i.to_rgb8(),
        _ => {
            let src = catalog
                .asset_source(id)?
                .ok_or_else(|| CoreError::Other("fotografia removida".into()))?;
            let format = format_from_id(&src.format)
                .ok_or_else(|| CoreError::UnsupportedFormat(src.format.clone()))?;
            let data = std::fs::read(&src.path)
                .map_err(|e| CoreError::Other(format!("original não encontrado ({e})")))?;
            let p = registry.probe(&data, format)?;
            let (long, short) = (p.width.max(p.height), p.width.min(p.height));
            let min_short = ((WORK_EDGE as u64 * short as u64) / long.max(1) as u64).max(1) as u32;
            let d = registry.decode(&data, format, Some(min_short))?;
            apply_orientation(d.image, src.orientation)
        }
    };
    let (w, h) = img.dimensions();
    if w.max(h) <= WORK_EDGE {
        return Ok(img);
    }
    let s = WORK_EDGE as f32 / w.max(h) as f32;
    Ok(imageops::resize(
        &img,
        ((w as f32 * s) as u32).max(1),
        ((h as f32 * s) as u32).max(1),
        imageops::FilterType::Triangle,
    ))
}

/// Pontos do contorno de cada olho na malha de 478 pontos do MediaPipe.
const EYE_POINTS: [[usize; 8]; 2] =
    [[33, 133, 159, 145, 160, 144, 158, 153], [362, 263, 386, 374, 385, 380, 387, 373]];

fn face_row(img: &RgbImage, f: &FaceResult) -> FaceRow {
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let d = &f.detection;
    let blend =
        |n: &str| blendshape_index(n).and_then(|i| f.blendshapes.get(i).copied()).unwrap_or(0.0);
    // Região dos olhos: caixa dos dois contornos, alargada 60 %.
    let eye_pts: Vec<(f32, f32)> =
        EYE_POINTS.iter().flatten().filter_map(|&i| f.landmarks.get(i).copied()).collect();
    let eye_sharpness = if eye_pts.len() == 16 {
        let (x0, x1) =
            eye_pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
        let (y0, y1) =
            eye_pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
        let (w, h) = ((x1 - x0) * 1.6, ((y1 - y0) * 1.6).max((x1 - x0) * 0.4));
        metrics::region_sharpness(
            img,
            (x0 + x1) / 2.0 - w / 2.0,
            (y0 + y1) / 2.0 - h / 2.0,
            w,
            h,
            128,
        )
    } else {
        0.0
    };
    // Cortado: a caixa sai mais de 8 % da imagem, ou >10 % dos pontos ficam fora.
    let margin_x = d.w * 0.08;
    let margin_y = d.h * 0.08;
    let box_out = d.x < -margin_x
        || d.y < -margin_y
        || d.x + d.w > iw + margin_x
        || d.y + d.h > ih + margin_y;
    let outside =
        f.landmarks.iter().filter(|p| p.0 < 0.0 || p.1 < 0.0 || p.0 >= iw || p.1 >= ih).count();
    let cut = box_out || outside * 10 > f.landmarks.len().max(1);
    let (bl, br) = f.eye_blink();
    FaceRow {
        x: d.x / iw,
        y: d.y / ih,
        w: d.w / iw,
        h: d.h / ih,
        px: d.w,
        det_score: d.score,
        presence: f.presence,
        blink_left: bl,
        blink_right: br,
        smile: f.smile(),
        jaw_open: blend("jawOpen"),
        sharpness: metrics::region_sharpness(img, d.x, d.y, d.w, d.h, 128),
        eye_sharpness,
        cut,
        blendshapes: f.blendshapes.clone(),
    }
}

/// Analisa uma fotografia (sem gravar).
pub fn analyze_one(
    paths: &ProjectPaths,
    catalog: &Catalog,
    registry: &CodecRegistry,
    models: Option<&VisionModels>,
    id: AssetId,
) -> CoreResult<AnalysisResult> {
    let t0 = Instant::now();
    let img = work_image(paths, catalog, registry, id)?;
    let m = metrics::technical(&img);
    let (faces, embedding) = match models {
        Some(models) => {
            let faces = models.analyze_faces(&img, true)?;
            let faces: Vec<FaceRow> = faces.iter().map(|f| face_row(&img, f)).collect();
            (faces, Some((EMBEDDING_MODEL.to_string(), models.embed(&img)?)))
        }
        None => (Vec::new(), None),
    };
    Ok(AnalysisResult {
        asset: id,
        row: AnalysisRow {
            models: models.map(|_| "mediapipe-face-landmarker+mobilenet_v3_small".to_string()),
            width: img.width(),
            height: img.height(),
            sharpness: m.sharpness,
            focus_peak: m.focus_peak,
            focus_center: m.focus_center,
            motion_blur: m.motion_blur,
            brightness: m.brightness,
            highlights_clipped: m.highlights_clipped,
            shadows_crushed: m.shadows_crushed,
            contrast: m.contrast,
            noise: m.noise,
            tilt_degrees: m.tilt_degrees,
            phash: hash::phash(&img),
            dhash: hash::dhash(&img),
            duration_ms: t0.elapsed().as_millis() as u32,
        },
        faces,
        embedding,
    })
}

// ------------------------------------------------------------------ seleção

/// `AAAA-MM-DDTHH:MM:SS[.fff]` (hora local da câmara) → milissegundos desde 1970 “locais”.
/// Só as diferenças importam (agrupamento), por isso o fuso é irrelevante.
pub fn capture_ms(s: &str) -> Option<i64> {
    let (date, time) = s.split_once('T').or_else(|| s.split_once(' '))?;
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, mo, da) = (d.next()??, d.next()??, d.next()??);
    let (hms, frac) = time.split_once('.').unwrap_or((time, ""));
    let mut t = hms.split(':').map(|x| x.parse::<i64>().ok());
    let (h, mi, se) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
    let frac: String = frac.chars().take_while(|c| c.is_ascii_digit()).take(3).collect();
    let ms = if frac.is_empty() {
        0
    } else {
        frac.parse::<i64>().ok()? * 10_i64.pow(3 - frac.len() as u32)
    };
    // Dias desde 1970-01-01 (algoritmo de Howard Hinnant).
    let (y2, m2) = if mo <= 2 { (y - 1, mo + 9) } else { (y, mo - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + da - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + h) * 60 + mi) * 60_000 + se * 1000 + ms)
}

fn stem_key(path: &str) -> String {
    let p = Path::new(path);
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let dir = p.parent().map(|d| d.to_string_lossy().to_lowercase()).unwrap_or_default();
    format!("{dir}/{stem}")
}

fn file_seq(file_name: &str) -> Option<u64> {
    let stem = Path::new(file_name).file_stem()?.to_string_lossy().into_owned();
    let digits: String = stem
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

pub fn photo_input(r: &SelectionRow) -> PhotoInput {
    let a = &r.analysis;
    PhotoInput {
        id: r.asset.0,
        capture_ms: r.capture_time.as_deref().and_then(capture_ms),
        camera: r.camera.clone(),
        stem_key: stem_key(&r.path),
        file_seq: file_seq(&r.file_name),
        is_raw: format_from_id(&r.format).is_some_and(|f| f.family() == FormatFamily::Raw),
        metrics: metrics::TechnicalMetrics {
            sharpness: a.sharpness,
            focus_peak: a.focus_peak,
            focus_center: a.focus_center,
            motion_blur: a.motion_blur,
            brightness: a.brightness,
            highlights_clipped: a.highlights_clipped,
            shadows_crushed: a.shadows_crushed,
            contrast: a.contrast,
            noise: a.noise,
            tilt_degrees: a.tilt_degrees,
        },
        faces: r
            .faces
            .iter()
            .map(|f| FaceInfo {
                x: f.x,
                y: f.y,
                w: f.w,
                h: f.h,
                px: f.px,
                blink_left: f.blink_left,
                blink_right: f.blink_right,
                smile: f.smile,
                jaw_open: f.jaw_open,
                sharpness: f.sharpness,
                eye_sharpness: f.eye_sharpness,
                presence: f.presence,
                cut: f.cut,
            })
            .collect(),
        phash: a.phash,
        embedding: r.embedding.clone(),
        user_flag: r.user_flag,
    }
}

pub fn load_settings(catalog: &Catalog) -> Settings {
    catalog
        .load_setting(SETTINGS_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(catalog: &Catalog, s: &Settings) -> CoreResult<()> {
    catalog.save_setting(
        SETTINGS_KEY,
        &serde_json::to_string(s).map_err(|e| CoreError::Other(e.to_string()))?,
    )
}

fn label_id(l: Label) -> &'static str {
    match l {
        Label::Best => "best",
        Label::Alternative => "alternative",
        Label::Review => "review",
        Label::Reject => "reject",
        Label::Duplicate => "duplicate",
    }
}

/// Recalcula grupos e recomendações a partir da análise gravada.
pub fn recompute_selection(catalog: &mut Catalog) -> CoreResult<SelectionSummary> {
    let t0 = Instant::now();
    let settings = load_settings(catalog);
    let rows = catalog.selection_rows()?;
    let photos: Vec<PhotoInput> = rows.iter().map(photo_input).collect();
    let (groups, recs) = selection::select(&photos, settings);
    let group_rows: Vec<(String, usize)> = groups
        .iter()
        .map(|g| {
            let kind = match g.kind {
                selection::GroupKind::Sequence => "sequence",
                selection::GroupKind::Duplicate => "duplicate",
                selection::GroupKind::RawJpeg => "raw_jpeg",
            };
            (kind.to_string(), g.members.len())
        })
        .collect();
    let mut summary = SelectionSummary {
        groups: groups.len() as u64,
        sequences: groups.iter().filter(|g| g.kind == selection::GroupKind::Sequence).count()
            as u64,
        ..Default::default()
    };
    let rec_rows: Vec<RecommendationRow> = recs
        .iter()
        .map(|r| {
            match r.label {
                Label::Best => summary.best += 1,
                Label::Alternative => summary.alternative += 1,
                Label::Review => summary.review += 1,
                Label::Reject => summary.reject += 1,
                Label::Duplicate => summary.duplicate += 1,
            }
            let s = &r.scores;
            RecommendationRow {
                asset: AssetId(r.id),
                label: label_id(r.label).into(),
                group_index: r.group,
                rank: r.rank as u32,
                group_size: r.group_size as u32,
                summary: r.summary.clone(),
                reasons: serde_json::to_string(&r.reasons).unwrap_or_else(|_| "[]".into()),
                focus: s.focus,
                motion: s.motion,
                exposure: s.exposure,
                noise: s.noise,
                eyes: s.eyes,
                expression: s.expression,
                composition: s.composition,
                technical: s.technical,
                people: s.people,
                overall: s.overall,
            }
        })
        .collect();
    catalog.save_selection(&group_rows, &rec_rows)?;
    summary.duration_ms = t0.elapsed().as_millis() as u64;
    Ok(summary)
}

// ------------------------------------------------------------------ serviço

pub struct AnalysisService {
    queue: Arc<WorkQueue<AssetId>>,
    workers: Vec<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
    cancelled: Arc<AtomicBool>,
}

impl AnalysisService {
    /// Inicia `threads` threads de análise. Com `models_dir = None`, só métricas técnicas.
    pub fn start(
        paths: ProjectPaths,
        registry: Arc<CodecRegistry>,
        models_dir: Option<PathBuf>,
        ids: Vec<AssetId>,
        threads: usize,
        on_progress: ProgressCallback,
    ) -> CoreResult<Self> {
        let queue = Arc::new(WorkQueue::new());
        let total = queue.push(ids) as u64;
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<Result<AnalysisResult, (AssetId, String)>>();
        let mut workers = Vec::new();
        for i in 0..threads.max(1) {
            let (queue, paths, registry, tx, dir) =
                (queue.clone(), paths.clone(), registry.clone(), tx.clone(), models_dir.clone());
            let catalog = Catalog::open_existing(&paths.catalog())?;
            workers.push(
                std::thread::Builder::new()
                    .name(format!("analysis-{i}"))
                    .spawn(move || {
                        // Uma instância dos modelos por thread (a execução do tract não é partilhável).
                        let models = dir.and_then(|d| match VisionModels::load(&d) {
                            Ok(m) => Some(m),
                            Err(e) => {
                                tracing::error!(erro = %e, "modelos de IA não carregados; só análise técnica");
                                None
                            }
                        });
                        while let Some(id) = queue.pop() {
                            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                analyze_one(&paths, &catalog, &registry, models.as_ref(), id)
                            }));
                            let r = match r {
                                Ok(Ok(res)) => Ok(res),
                                Ok(Err(e)) => Err((id, e.to_string())),
                                Err(_) => Err((id, "falha interna na análise".to_string())),
                            };
                            if tx.send(r).is_err() {
                                break;
                            }
                        }
                    })
                    .map_err(CoreError::Io)?,
            );
        }
        drop(tx);

        let mut catalog = Catalog::open_existing(&paths.catalog())?;
        let wq = queue.clone();
        let writer = std::thread::Builder::new()
            .name("analysis-writer".into())
            .spawn(move || {
                let mut ok: Vec<AnalysisResult> = Vec::new();
                let mut bad: Vec<(AssetId, String)> = Vec::new();
                let (mut done, mut failed) = (0u64, 0u64);
                let mut last = Instant::now();
                loop {
                    let msg = rx.recv_timeout(Duration::from_millis(250));
                    let disconnected = matches!(msg, Err(mpsc::RecvTimeoutError::Disconnected));
                    match msg {
                        Ok(Ok(r)) => ok.push(r),
                        Ok(Err(e)) => {
                            tracing::warn!(asset = e.0 .0, erro = %e.1, "análise falhou");
                            bad.push(e)
                        }
                        Err(_) => {}
                    }
                    let n = (ok.len() + bad.len()) as u64;
                    if n > 0
                        && (n >= 32 || last.elapsed() >= Duration::from_millis(500) || disconnected)
                    {
                        if let Err(e) = catalog.save_analyses(&ok, &bad) {
                            tracing::error!(erro = %e, "falha ao gravar a análise");
                        }
                        done += n;
                        failed += bad.len() as u64;
                        ok.clear();
                        bad.clear();
                        last = Instant::now();
                        on_progress(AnalysisProgress {
                            done,
                            failed,
                            total,
                            remaining: wq.len() as u64,
                            finished: false,
                        });
                    }
                    if disconnected {
                        break;
                    }
                }
                // Recalcula a seleção mesmo após cancelamento (com o que já foi analisado).
                if let Err(e) = recompute_selection(&mut catalog) {
                    tracing::error!(erro = %e, "falha ao calcular a seleção");
                }
                on_progress(AnalysisProgress {
                    done,
                    failed,
                    total,
                    remaining: wq.len() as u64,
                    finished: true,
                });
            })
            .map_err(CoreError::Io)?;
        Ok(AnalysisService { queue, workers, writer: Some(writer), cancelled })
    }

    pub fn add(&self, ids: impl IntoIterator<Item = AssetId>) -> usize {
        self.queue.push(ids)
    }

    pub fn is_finished(&self) -> bool {
        self.writer.as_ref().is_none_or(|w| w.is_finished())
    }

    pub fn remaining(&self) -> usize {
        self.queue.len()
    }

    /// Para depois da fotografia em curso. O resto fica por analisar no catálogo.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.queue.close();
    }

    /// Fecha a fila quando esvaziar: as threads terminam e o escritor recalcula a seleção.
    fn close_when_empty(&self) {
        let q = self.queue.clone();
        std::thread::spawn(move || {
            while !q.is_empty() {
                std::thread::sleep(Duration::from_millis(100));
            }
            q.close();
        });
    }

    pub fn wait(mut self) {
        self.join();
    }

    fn join(&mut self) {
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }
}

impl Drop for AnalysisService {
    fn drop(&mut self) {
        self.cancel();
        self.join();
    }
}

/// Inicia o serviço e garante que termina quando a fila esvaziar.
pub fn run(
    paths: ProjectPaths,
    registry: Arc<CodecRegistry>,
    models_dir: Option<PathBuf>,
    ids: Vec<AssetId>,
    threads: usize,
    on_progress: ProgressCallback,
) -> CoreResult<AnalysisService> {
    let s = AnalysisService::start(paths, registry, models_dir, ids, threads, on_progress)?;
    s.close_when_empty();
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_time_parsing() {
        let a = capture_ms("2026-06-15T14:00:21.03").unwrap();
        let b = capture_ms("2026-06-15T14:00:22").unwrap();
        assert_eq!(b - a, 970);
        assert_eq!(capture_ms("1970-01-01T00:00:00"), Some(0));
        assert_eq!(
            capture_ms("2000-03-01T00:00:00").unwrap() - capture_ms("2000-02-28T00:00:00").unwrap(),
            2 * 86_400_000
        );
        assert_eq!(capture_ms("lixo"), None);
    }

    #[test]
    fn stems_and_sequence_numbers() {
        assert_eq!(stem_key("/F/DSC_0012.NEF"), stem_key("/f/dsc_0012.jpg"));
        assert_eq!(file_seq("IMG_0123.CR3"), Some(123));
        assert_eq!(file_seq("casamento.jpg"), None);
    }
}
