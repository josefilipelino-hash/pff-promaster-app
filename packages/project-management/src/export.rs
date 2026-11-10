//! Fila de exportação (Fase 9).
//!
//! Cada fotografia é revelada **uma vez** a partir do original (RAW ou outro formato, em
//! resolução total, com AI Denoise se a receita o tiver) e dessa imagem saem todas as
//! receitas do pedido (multi-exportação). Nunca se usa uma preview.
//!
//! Progresso: ficheiros feitos/restantes, bytes escritos, tamanho total estimado (corrigido
//! com os ficheiros reais), espaço livre no destino, tempo restante e erros. Sem espaço
//! suficiente, a exportação pausa sozinha (e retoma quando o utilizador pedir).

use crate::session::ProjectSession;
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_export_engine::naming::{self, Vars};
use momento_export_engine::{PhotoMeta, Prepared, Recipe};
use momento_image_processing::develop::{
    self, geometry::native_size, Quality, RenderOptions, Settings,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Pedido da interface.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportRequest {
    /// Chaves das fotografias (versões principais ou cópias virtuais).
    pub keys: Vec<i64>,
    pub recipes: Vec<Recipe>,
    /// Pasta de destino (as receitas acrescentam as suas subpastas).
    pub destination: String,
    /// Nome mostrado na fila (ex.: "Exportar casamento").
    pub name: String,
    /// Nomes (traduzidos) dos momentos, para o campo {momento}.
    pub moment_names: HashMap<i64, String>,
    /// Geração de uma galeria (definida pela sessão, nunca pela interface).
    #[serde(skip)]
    pub gallery: Option<Box<crate::gallery::GalleryJob>>,
    /// Exportação de um álbum (definida pela sessão).
    #[serde(skip)]
    pub album: Option<Box<crate::album::AlbumExportJob>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub recipe: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeEstimate {
    pub name: String,
    pub files: u64,
    pub bytes: u64,
}

/// Estimativa antes de exportar.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEstimate {
    pub photos: u64,
    pub files: u64,
    pub bytes: u64,
    pub free_bytes: Option<u64>,
    pub enough_space: bool,
    pub per_recipe: Vec<RecipeEstimate>,
    /// Ajustes automáticos às receitas (formato sem essa profundidade, etc.).
    pub notes: Vec<Note>,
    /// Receitas que não podem ser usadas (ex.: perfil ICC em falta).
    pub errors: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportError {
    pub file: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Queued,
    Running,
    Paused,
    Done,
    Cancelled,
    Failed,
}

/// Estado de um trabalho da fila (enviado à interface).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub id: u64,
    pub name: String,
    pub destination: String,
    pub state: JobState,
    /// `noSpace` quando pausou por falta de espaço.
    pub paused_reason: Option<String>,
    pub photos_total: u64,
    pub photos_done: u64,
    pub files_total: u64,
    pub files_done: u64,
    pub files_failed: u64,
    pub files_skipped: u64,
    pub bytes_written: u64,
    pub bytes_estimated: u64,
    pub free_bytes: Option<u64>,
    pub elapsed_seconds: f64,
    pub eta_seconds: Option<f64>,
    pub current: Option<String>,
    pub errors: Vec<ExportError>,
    pub notes: Vec<Note>,
    /// Pastas criadas/usadas (para "Mostrar na pasta").
    pub folders: Vec<String>,
    /// Galeria: `index.html` gerado.
    pub index: Option<String>,
}

struct Job {
    id: u64,
    req: ExportRequest,
    paused: AtomicBool,
    cancelled: AtomicBool,
    progress: Mutex<ExportProgress>,
    /// Registo no catálogo para retomar depois de uma interrupção (Fase 13).
    journal: Option<String>,
    /// Parada pelo fecho do projeto (o registo fica para retomar).
    suspended: AtomicBool,
}

/// Exportação interrompida (encerramento, falha) que pode ser retomada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingExport {
    pub id: String,
    pub name: String,
    pub destination: String,
    pub done: u64,
    pub total: u64,
}

const JOURNAL_JOB: &str = "export_job:";
const JOURNAL_NEXT: &str = "export_next:";

#[derive(Default)]
pub struct ExportQueue {
    jobs: Mutex<Vec<Arc<Job>>>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    next_id: AtomicU64,
}

/// Espaço livre no volume de `p` (sobe até uma pasta existente).
pub fn free_space(p: &Path) -> Option<u64> {
    let mut d = p.to_path_buf();
    loop {
        if d.exists() {
            return fs4::available_space(&d).ok();
        }
        if !d.pop() {
            return None;
        }
    }
}

/// Margem de segurança: nunca encher o disco até ao fim.
const SPACE_MARGIN: u64 = 64 * 1024 * 1024;

impl ExportQueue {
    pub fn list(&self) -> Vec<ExportProgress> {
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|j| j.progress.lock().unwrap_or_else(|e| e.into_inner()).clone())
            .collect()
    }

    fn get(&self, id: u64) -> Option<Arc<Job>> {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|j| j.id == id).cloned()
    }

    pub fn running(&self) -> bool {
        self.list()
            .iter()
            .any(|p| matches!(p.state, JobState::Queued | JobState::Running | JobState::Paused))
    }

    /// Fecho do projeto: para entre ficheiros; o registo fica para retomar.
    pub fn cancel_all(&self) {
        for j in self.jobs.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            j.suspended.store(true, Ordering::SeqCst);
            j.cancelled.store(true, Ordering::SeqCst);
        }
        if let Some(h) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = h.join();
        }
    }
}

impl ProjectSession {
    fn photo_dims(&self, keys: &[i64]) -> CoreResult<Vec<(i64, u32, u32)>> {
        let rows: HashMap<i64, (u32, u32)> =
            self.list_photos()?.into_iter().map(|r| (r.id.0, (r.width, r.height))).collect();
        let cat = self.open_catalog()?;
        let mut out = Vec::new();
        for &k in keys {
            let Some(&(w, h)) = rows.get(&k) else { continue };
            let (variant, _) = cat.resolve_key(k)?;
            let s = Settings::from_recipe(&cat.recipe_of(variant)?);
            let (cw, ch) = native_size(w as usize, h as usize, &s.geometry);
            out.push((k, cw, ch));
        }
        Ok(out)
    }

    /// Estimativa: ficheiros, tamanho total, espaço livre e ajustes às receitas.
    pub fn estimate_export(&self, req: &ExportRequest) -> CoreResult<ExportEstimate> {
        let dims = self.photo_dims(&req.keys)?;
        let mut est = ExportEstimate {
            photos: dims.len() as u64,
            files: 0,
            bytes: 0,
            free_bytes: free_space(Path::new(&req.destination)),
            enough_space: true,
            per_recipe: Vec::new(),
            notes: Vec::new(),
            errors: Vec::new(),
        };
        for r in &req.recipes {
            match Prepared::new(r) {
                Ok(p) => {
                    let bytes: u64 = dims.iter().map(|&(_, w, h)| p.estimate(w, h)).sum();
                    est.notes.extend(
                        p.notes
                            .iter()
                            .map(|n| Note { recipe: r.name.clone(), code: n.to_string() }),
                    );
                    est.per_recipe.push(RecipeEstimate {
                        name: r.name.clone(),
                        files: dims.len() as u64,
                        bytes,
                    });
                    est.files += dims.len() as u64;
                    est.bytes += bytes;
                }
                Err(e) => est.errors.push(Note { recipe: r.name.clone(), code: e.to_string() }),
            }
        }
        est.enough_space = est.free_bytes.is_none_or(|f| f >= est.bytes + SPACE_MARGIN);
        Ok(est)
    }

    /// Junta um pedido à fila e devolve o seu número.
    pub fn start_export(self: &Arc<Self>, req: ExportRequest) -> CoreResult<u64> {
        if req.keys.is_empty() && req.album.is_none() {
            return Err(CoreError::Other("nenhuma fotografia selecionada".into()));
        }
        if req.recipes.is_empty() && req.album.is_none() {
            return Err(CoreError::Other("nenhuma receita escolhida".into()));
        }
        if req.destination.trim().is_empty() {
            return Err(CoreError::Other("escolha a pasta de destino".into()));
        }
        let q = &self.exports;
        let id = q.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let files = match &req.album {
            Some(a) => a.album.spreads.len() as u64,
            None => (req.keys.len() * req.recipes.len()) as u64,
        };
        let job = Arc::new(Job {
            id,
            progress: Mutex::new(ExportProgress {
                id,
                name: req.name.clone(),
                destination: req.destination.clone(),
                state: JobState::Queued,
                paused_reason: None,
                photos_total: if req.album.is_some() { files } else { req.keys.len() as u64 },
                photos_done: 0,
                files_total: files,
                files_done: 0,
                files_failed: 0,
                files_skipped: 0,
                bytes_written: 0,
                bytes_estimated: 0,
                free_bytes: free_space(Path::new(&req.destination)),
                elapsed_seconds: 0.0,
                eta_seconds: None,
                current: None,
                errors: Vec::new(),
                notes: Vec::new(),
                folders: Vec::new(),
                index: None,
            }),
            journal: self.write_journal(id, &req),
            req,
            paused: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            suspended: AtomicBool::new(false),
        });
        q.jobs.lock().unwrap_or_else(|e| e.into_inner()).push(job.clone());
        self.emit_export(&job);
        let mut w = q.worker.lock().unwrap_or_else(|e| e.into_inner());
        if w.as_ref().is_none_or(|h| h.is_finished()) {
            let s = self.clone();
            *w = Some(std::thread::spawn(move || s.export_worker()));
        }
        Ok(id)
    }

    pub fn export_jobs(&self) -> Vec<ExportProgress> {
        self.exports.list()
    }

    // ------------------------------------------------ exportações interrompidas (Fase 13)

    fn write_journal(&self, id: u64, req: &ExportRequest) -> Option<String> {
        // Galerias e álbuns têm partes definidas pela sessão (não serializáveis): voltam a
        // ser geradas por inteiro; o resto é retomado.
        if req.gallery.is_some() || req.album.is_some() {
            return None;
        }
        let jid = format!("{}-{id}", momento_database::now_unix());
        let json = serde_json::to_string(req).ok()?;
        let cat = self.catalog();
        cat.save_value(&format!("{JOURNAL_JOB}{jid}"), &json).ok()?;
        cat.save_value(&format!("{JOURNAL_NEXT}{jid}"), "0").ok()?;
        Some(jid)
    }

    fn drop_journal(&self, jid: Option<&str>) {
        if let Some(j) = jid {
            let cat = self.catalog();
            let _ = cat.delete_value(&format!("{JOURNAL_JOB}{j}"));
            let _ = cat.delete_value(&format!("{JOURNAL_NEXT}{j}"));
        }
    }

    /// Exportações que ficaram a meio (encerramento inesperado ou fecho do projeto).
    pub fn pending_exports(&self) -> CoreResult<Vec<PendingExport>> {
        let running: HashSet<String> = self
            .exports
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|j| !j.cancelled.load(Ordering::SeqCst))
            .filter_map(|j| j.journal.clone())
            .collect();
        let cat = self.catalog();
        let mut out = Vec::new();
        for (k, v) in cat.values_with_prefix(JOURNAL_JOB)? {
            let jid = k.trim_start_matches(JOURNAL_JOB).to_string();
            if running.contains(&jid) {
                continue;
            }
            let Ok(req) = serde_json::from_str::<ExportRequest>(&v) else { continue };
            let done = cat
                .load_value(&format!("{JOURNAL_NEXT}{jid}"))?
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0);
            out.push(PendingExport {
                id: jid,
                name: req.name,
                destination: req.destination,
                done,
                total: req.keys.len() as u64,
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Retoma uma exportação interrompida com as fotografias que faltavam.
    pub fn resume_export(self: &Arc<Self>, jid: &str) -> CoreResult<u64> {
        let (json, next) = {
            let cat = self.catalog();
            let json = cat
                .load_value(&format!("{JOURNAL_JOB}{jid}"))?
                .ok_or_else(|| CoreError::Other("exportação interrompida não encontrada".into()))?;
            let next = cat
                .load_value(&format!("{JOURNAL_NEXT}{jid}"))?
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0);
            (json, next)
        };
        let mut req: ExportRequest =
            serde_json::from_str(&json).map_err(|e| CoreError::Other(e.to_string()))?;
        // Temporários de escritas interrompidas no destino (só o padrão da aplicação).
        crate::safety::remove_temp_files(Path::new(&req.destination), 4);
        req.keys = req.keys.split_off(next.min(req.keys.len()));
        if req.keys.is_empty() {
            self.drop_journal(Some(jid));
            return Err(CoreError::Other("esta exportação já estava completa".into()));
        }
        let id = self.start_export(req)?;
        self.drop_journal(Some(jid));
        Ok(id)
    }

    pub fn discard_export(&self, jid: &str) {
        self.drop_journal(Some(jid));
    }

    pub fn pause_export(&self, id: u64, paused: bool) -> bool {
        let Some(j) = self.exports.get(id) else { return false };
        j.paused.store(paused, Ordering::SeqCst);
        {
            let mut p = j.progress.lock().unwrap_or_else(|e| e.into_inner());
            if matches!(p.state, JobState::Running | JobState::Paused) {
                p.state = if paused { JobState::Paused } else { JobState::Running };
                if !paused {
                    p.paused_reason = None;
                }
            }
        }
        self.emit_export(&j);
        true
    }

    pub fn cancel_export(&self, id: u64) -> bool {
        let Some(j) = self.exports.get(id) else { return false };
        j.cancelled.store(true, Ordering::SeqCst);
        // Cancelada pelo utilizador: não é para retomar.
        self.drop_journal(j.journal.as_deref());
        {
            let mut p = j.progress.lock().unwrap_or_else(|e| e.into_inner());
            if p.state == JobState::Queued {
                p.state = JobState::Cancelled;
            }
        }
        self.emit_export(&j);
        true
    }

    /// Remove da lista os trabalhos terminados.
    pub fn clear_finished_exports(&self) {
        self.exports.jobs.lock().unwrap_or_else(|e| e.into_inner()).retain(|j| {
            let s = j.progress.lock().unwrap_or_else(|e| e.into_inner()).state;
            matches!(s, JobState::Queued | JobState::Running | JobState::Paused)
        });
    }

    fn emit_export(&self, j: &Job) {
        let p = j.progress.lock().unwrap_or_else(|e| e.into_inner()).clone();
        self.events.export_progress(&p);
    }

    fn export_worker(self: Arc<Self>) {
        loop {
            let next = self
                .exports
                .jobs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .find(|j| {
                    j.progress.lock().unwrap_or_else(|e| e.into_inner()).state == JobState::Queued
                })
                .cloned();
            let Some(job) = next else { return };
            if let Err(e) = self.run_export(&job) {
                let mut p = job.progress.lock().unwrap_or_else(|e| e.into_inner());
                p.state = JobState::Failed;
                p.errors.push(ExportError { file: String::new(), message: e.to_string() });
                drop(p);
                self.emit_export(&job);
            }
        }
    }

    fn photo_meta(
        &self,
        asset: AssetId,
        stars: u8,
        project: &(String, String),
        moment: String,
    ) -> CoreResult<PhotoMeta> {
        let d = self
            .photo_details(asset)?
            .ok_or_else(|| CoreError::Other(format!("fotografia {asset} inexistente")))?;
        let stem = Path::new(&d.file_name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(PhotoMeta {
            file_stem: stem,
            capture_time: d.corrected_time.or(d.capture_time),
            capture_offset: d.capture_offset,
            make: d.camera_make,
            model: d.camera_model,
            serial: d.camera_serial,
            lens: d.lens,
            iso: d.iso,
            exposure_time: d.exposure_time,
            f_number: d.f_number,
            focal_length: d.focal_length,
            gps: d.gps_lat.zip(d.gps_lon),
            rating: Some(stars),
            project: project.0.clone(),
            client: project.1.clone(),
            moment,
        })
    }

    fn run_export(self: &Arc<Self>, job: &Arc<Job>) -> CoreResult<()> {
        let started = Instant::now();
        let req = &job.req;
        let update = |f: &mut dyn FnMut(&mut ExportProgress)| {
            let mut p = job.progress.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut p);
            p.elapsed_seconds = started.elapsed().as_secs_f64();
        };
        let paused = job.paused.load(Ordering::SeqCst);
        update(&mut |p| p.state = if paused { JobState::Paused } else { JobState::Running });
        self.emit_export(job);
        if let Some(a) = &req.album {
            // Álbum: um ficheiro por spread (ou página) ou um PDF, a partir dos originais.
            let res =
                self.run_album_export(a, Path::new(&req.destination), &|done, total, bytes| {
                    while job.paused.load(Ordering::SeqCst) && !job.cancelled.load(Ordering::SeqCst)
                    {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                    let elapsed = started.elapsed().as_secs_f64();
                    update(&mut |p| {
                        p.files_done = done as u64;
                        p.photos_done = done as u64;
                        if bytes > 0 {
                            p.bytes_written = bytes;
                            p.bytes_estimated = bytes;
                        }
                        p.current = (done < total).then(|| format!("spread {}", done + 1));
                        p.eta_seconds =
                            (done > 0).then(|| elapsed / done as f64 * (total - done) as f64);
                    });
                    self.emit_export(job);
                    !job.cancelled.load(Ordering::SeqCst)
                });
            update(&mut |p| {
                p.current = None;
                p.free_bytes = free_space(Path::new(&req.destination));
                match &res {
                    Ok(files) => {
                        p.state = JobState::Done;
                        p.folders = vec![req.destination.clone()];
                        p.index = files.iter().find(|f| f.ends_with(".pdf")).cloned();
                    }
                    Err(e) if job.cancelled.load(Ordering::SeqCst) => {
                        let _ = e;
                        p.state = JobState::Cancelled;
                    }
                    Err(e) => {
                        p.state = JobState::Failed;
                        p.errors.push(ExportError { file: String::new(), message: e.to_string() });
                    }
                }
            });
            self.emit_export(job);
            return Ok(());
        }
        // Receitas preparadas uma vez (perfil ICC, marca de água, normalização).
        let mut prepared = Vec::new();
        for r in &req.recipes {
            let p = Prepared::new(r)
                .map_err(|e| CoreError::Other(format!("receita \"{}\": {e}", r.name)))?;
            update(&mut |pr| {
                pr.notes.extend(
                    p.notes.iter().map(|n| Note { recipe: r.name.clone(), code: n.to_string() }),
                )
            });
            prepared.push(p);
        }
        let info = self.info()?;
        let project = (info.record.name.clone(), info.record.client_name.clone());
        let rows: HashMap<i64, momento_database::assets::PhotoRow> =
            self.list_photos()?.into_iter().map(|r| (r.id.0, r)).collect();
        let cat = self.open_catalog()?;
        let moments: HashMap<AssetId, i64> = cat
            .asset_moments()?
            .into_iter()
            .filter_map(|m| m.moment_id.map(|id| (m.asset, id)))
            .collect();
        let moment_kind: HashMap<i64, String> = cat
            .list_moments()?
            .into_iter()
            .map(|m| (m.id, m.name.or(m.kind).unwrap_or_default()))
            .collect();
        let protected: HashSet<String> =
            cat.known_paths()?.iter().map(|p| naming::path_key(Path::new(p))).collect();
        // Estimativa inicial por receita e fotografia; corrigida com os ficheiros reais.
        let dims: HashMap<i64, (u32, u32)> =
            self.photo_dims(&req.keys)?.into_iter().map(|(k, w, h)| (k, (w, h))).collect();
        let est: Vec<Vec<u64>> = prepared
            .iter()
            .map(|p| {
                req.keys
                    .iter()
                    .map(|k| dims.get(k).map(|&(w, h)| p.estimate(w, h)).unwrap_or(0))
                    .collect()
            })
            .collect();
        let mut ratio = vec![(0u64, 0u64); prepared.len()]; // (real, estimado) já escritos
        let total_est: u64 = est.iter().flatten().sum();
        update(&mut |p| p.bytes_estimated = total_est);
        let dest = PathBuf::from(&req.destination);
        let mut reserved = HashSet::new();
        let mut folders: HashSet<String> = HashSet::new();
        // Ficheiros escritos: (chave, receita, caminho, largura, altura).
        let mut outputs: Vec<(i64, usize, PathBuf, u32, u32)> = Vec::new();
        // Pipeline em duas fases (Fase 12, medido com `perf`: a thread da exportação fazia
        // em série a descodificação e a codificação, com a CPU a 215 % de 400 %): uma thread
        // prepara (descodifica + revela) a fotografia seguinte enquanto esta codifica e grava
        // a atual. O canal tem capacidade 1: no máximo uma fotografia adiantada em memória.
        let load = |cat: &momento_database::Catalog,
                    key: i64,
                    row: &momento_database::assets::PhotoRow|
         -> CoreResult<_> {
            let (variant, asset) = cat.resolve_key(key)?;
            let settings = Settings::from_recipe(&cat.recipe_of(variant)?);
            // Sempre o original em resolução total (+ AI Denoise da receita).
            let (src, _) = self.source_for(cat, asset, &settings, true, true, key)?;
            // Resolução nativa do recorte (nunca ampliada).
            let (nw, nh) = native_size(src.width(), src.height(), &settings.geometry);
            let lin = develop::render_linear(
                &src,
                &settings,
                &RenderOptions {
                    max_width: nw,
                    max_height: nh,
                    quality: Quality::Final,
                    uncropped: false,
                },
            );
            let moment = moments
                .get(&asset)
                .map(|id| {
                    req.moment_names
                        .get(id)
                        .cloned()
                        .or_else(|| moment_kind.get(id).cloned())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let meta = self.photo_meta(asset, row.stars, &project, moment)?;
            Ok((lin, meta))
        };
        std::thread::scope(|sc| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<(
                usize,
                CoreResult<(develop::LinearImage, PhotoMeta)>,
            )>(1);
            let (load, rows_ref) = (&load, &rows);
            sc.spawn(move || {
                let rows = rows_ref;
                let cat = match self.open_catalog() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send((usize::MAX, Err(e)));
                        return;
                    }
                };
                for (pi, &key) in req.keys.iter().enumerate() {
                    if job.cancelled.load(Ordering::SeqCst) {
                        break;
                    }
                    let Some(row) = rows.get(&key) else { continue };
                    // A fotografia aberta pelo utilizador passa à frente.
                    momento_task_queue::foreground::yield_to_foreground();
                    if tx.send((pi, load(&cat, key, row))).is_err() {
                        break;
                    }
                }
            });
            for (pi, &key) in req.keys.iter().enumerate() {
                if job.cancelled.load(Ordering::SeqCst) {
                    break;
                }
                let Some(row) = rows.get(&key) else {
                    update(&mut |p| {
                        p.files_failed += prepared.len() as u64;
                        p.errors.push(ExportError {
                            file: key.to_string(),
                            message: "fotografia inexistente".into(),
                        });
                    });
                    continue;
                };
                update(&mut |p| p.current = Some(row.file_name.clone()));
                self.emit_export(job);
                let photo = match rx.recv() {
                    Ok((i, r)) if i == pi => r,
                    Ok((_, Err(e))) => Err(e),
                    Ok(_) => Err(CoreError::Other("ordem da exportação".into())),
                    Err(_) => break,
                };
                let (lin, meta) = match photo {
                    Ok(v) => v,
                    Err(e) => {
                        update(&mut |p| {
                            p.files_failed += prepared.len() as u64;
                            p.photos_done += 1;
                            p.errors.push(ExportError {
                                file: row.file_name.clone(),
                                message: e.to_string(),
                            });
                        });
                        self.emit_export(job);
                        continue;
                    }
                };
                let (lw, lh) = (lin.width, lin.height);
                let mut lin = Some(lin);
                for (ri, prep) in prepared.iter().enumerate() {
                    // Pausa (pedida ou por falta de espaço) e cancelamento entre ficheiros.
                    if job.paused.load(Ordering::SeqCst) {
                        update(&mut |p| p.state = JobState::Paused);
                        self.emit_export(job);
                        while job.paused.load(Ordering::SeqCst)
                            && !job.cancelled.load(Ordering::SeqCst)
                        {
                            std::thread::sleep(std::time::Duration::from_millis(200));
                        }
                        update(&mut |p| {
                            p.state = JobState::Running;
                            p.paused_reason = None;
                        });
                        self.emit_export(job);
                    }
                    if job.cancelled.load(Ordering::SeqCst) {
                        break;
                    }
                    let r = &prep.recipe;
                    let (ow, oh) = prep.output_size(lw, lh);
                    let date = meta.capture_time.clone().unwrap_or_default();
                    let vars = Vars {
                        stem: meta.file_stem.clone(),
                        seq: r.naming.seq_start + pi as u32,
                        date: date.get(0..10).unwrap_or("").to_string(),
                        time: date.get(11..19).unwrap_or("").replace(':', ""),
                        project: project.0.clone(),
                        client: project.1.clone(),
                        recipe: r.name.clone(),
                        moment: meta.moment.clone(),
                        width: ow,
                        height: oh,
                        token: req
                            .gallery
                            .as_ref()
                            .and_then(|g| g.tokens.get(&key).cloned())
                            .unwrap_or_default(),
                    };
                    let dir = dest.join(naming::subfolder(&r.naming, &vars));
                    let name = naming::file_name(&r.naming, &vars, prep.extension());
                    let Some(path) =
                        naming::resolve(&dir, &name, r.naming.conflict, &mut reserved, &protected)
                    else {
                        update(&mut |p| p.files_skipped += 1);
                        continue;
                    };
                    // A última receita fica com a imagem (sem a copiar).
                    let produced = if ri + 1 == prepared.len() {
                        match lin.take() {
                            Some(l) => prep.produce_owned(l, &meta),
                            None => Err(CoreError::Other("imagem já usada".into())),
                        }
                    } else {
                        match lin.as_ref() {
                            Some(l) => prep.produce(l, &meta),
                            None => Err(CoreError::Other("imagem já usada".into())),
                        }
                    };
                    let result = produced.and_then(|out| {
                        // Espaço: pausa em vez de falhar a meio de um ficheiro.
                        loop {
                            let free = free_space(&dir);
                            update(&mut |p| p.free_bytes = free);
                            if free.is_none_or(|f| f >= out.bytes.len() as u64 + SPACE_MARGIN)
                                || job.cancelled.load(Ordering::SeqCst)
                            {
                                break;
                            }
                            job.paused.store(true, Ordering::SeqCst);
                            update(&mut |p| {
                                p.state = JobState::Paused;
                                p.paused_reason = Some("noSpace".into());
                            });
                            self.emit_export(job);
                            while job.paused.load(Ordering::SeqCst)
                                && !job.cancelled.load(Ordering::SeqCst)
                            {
                                std::thread::sleep(std::time::Duration::from_millis(300));
                            }
                        }
                        if job.cancelled.load(Ordering::SeqCst) {
                            return Err(CoreError::Other("cancelada".into()));
                        }
                        momento_image_processing::write_atomic(&path, &out.bytes)?;
                        Ok((out.bytes.len() as u64, out.width, out.height))
                    });
                    match result {
                        Ok((n, ow, oh)) => {
                            outputs.push((key, ri, path.clone(), ow, oh));
                            ratio[ri].0 += n;
                            ratio[ri].1 += est[ri][pi];
                            folders.insert(dir.to_string_lossy().into_owned());
                            update(&mut |p| {
                                p.files_done += 1;
                                p.bytes_written += n;
                            });
                        }
                        Err(e) if job.cancelled.load(Ordering::SeqCst) => {
                            tracing::info!(erro = %e, "exportação cancelada");
                            break;
                        }
                        Err(e) => update(&mut |p| {
                            p.files_failed += 1;
                            p.errors.push(ExportError {
                                file: path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                                message: e.to_string(),
                            });
                        }),
                    }
                    // Progresso a cada ficheiro (não só por fotografia).
                    self.emit_export(job);
                }
                // Estimativa corrigida: o que falta × (real ÷ estimado) de cada receita.
                let remaining: u64 = (0..prepared.len())
                    .map(|ri| {
                        let left: u64 = est[ri][pi + 1..].iter().sum();
                        let (real, guess) = ratio[ri];
                        if guess > 0 {
                            (left as f64 * real as f64 / guess as f64) as u64
                        } else {
                            left
                        }
                    })
                    .sum();
                let elapsed = started.elapsed().as_secs_f64();
                if let Some(j) = &job.journal {
                    // Fotografia completa: uma retoma começa na seguinte.
                    let _ = cat.save_value(&format!("{JOURNAL_NEXT}{j}"), &(pi + 1).to_string());
                }
                update(&mut |p| {
                    p.photos_done += 1;
                    p.bytes_estimated = p.bytes_written + remaining;
                    let left = p.photos_total - p.photos_done;
                    p.eta_seconds = Some(elapsed / p.photos_done as f64 * left as f64);
                    p.folders = folders.iter().cloned().collect();
                });
                self.emit_export(job);
            }
            drop(rx);
        });
        // Galeria: montar o site com os ficheiros escritos.
        let mut failed = false;
        if let Some(g) = req.gallery.as_ref().filter(|_| !job.cancelled.load(Ordering::SeqCst)) {
            update(&mut |p| p.current = Some("galeria".into()));
            self.emit_export(job);
            match self.finish_gallery(g, &outputs) {
                Ok(index) => update(&mut |p| p.index = Some(index.clone())),
                Err(e) => {
                    failed = true;
                    update(&mut |p| {
                        p.errors.push(ExportError { file: String::new(), message: e.to_string() })
                    });
                }
            }
        }
        update(&mut |p| {
            p.current = None;
            p.eta_seconds = Some(0.0);
            p.folders = {
                let mut f: Vec<String> = folders.iter().cloned().collect();
                f.sort();
                f
            };
            p.state = if job.cancelled.load(Ordering::SeqCst) {
                JobState::Cancelled
            } else if failed {
                JobState::Failed
            } else {
                JobState::Done
            };
            p.free_bytes = free_space(&dest);
        });
        // O registo fica só se o projeto foi fechado a meio (para retomar).
        if !job.suspended.load(Ordering::SeqCst) {
            self.drop_journal(job.journal.as_deref());
        }
        self.emit_export(job);
        Ok(())
    }
}
