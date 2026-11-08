//! Projeto aberto: catálogo, miniaturas, importações e eventos para a interface.

use crate::paths::{project_folder_name, ProjectPaths};
use crate::thumbnails::{ThumbBatch, ThumbnailService};
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_database::assets::{PhotoDetails, PhotoRow};
use momento_database::import::{ImportMode, ItemIssue, SessionCounts};
use momento_database::library::RecentProject;
use momento_database::project::ProjectRecord;
use momento_database::Catalog;
use momento_image_codecs::CodecRegistry;
use momento_import_engine::{ImportControl, ImportEvent, ImportOptions, ImportProgress};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Número de cópias de segurança do catálogo mantidas.
const BACKUPS_KEPT: usize = 10;
/// Intervalo mínimo entre cópias de segurança automáticas (segundos).
const BACKUP_INTERVAL: i64 = 3600;

/// Dados do formulário “Novo projeto”.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProject {
    pub name: String,
    pub client_name: String,
    pub event_type: String,
    pub event_date: Option<String>,
    pub location: String,
    /// Pasta dos originais (destino das cópias no modo “copiar”). Opcional.
    pub originals_dir: Option<String>,
    /// Pasta onde é criada a pasta do projeto `<nome>.momento`.
    pub work_dir: String,
}

/// Informação do projeto aberto enviada à interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub path: String,
    #[serde(flatten)]
    pub record: ProjectRecord,
    pub photo_count: u64,
    pub ui_state: Option<String>,
    pub import_running: bool,
}

/// Relatório final de uma importação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub session_id: i64,
    pub counts: SessionCounts,
    pub issues: Vec<ItemIssue>,
    pub cancelled: bool,
    pub suspended: bool,
    pub error: Option<String>,
}

/// Receptor de eventos (implementado pela aplicação desktop com `app.emit`).
pub trait SessionEvents: Send + Sync {
    fn import_progress(&self, p: &ImportProgress);
    fn import_finished(&self, s: &ImportSummary);
    fn thumbnails(&self, b: &ThumbBatch);
}

/// Pedido de importação vindo da interface.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub paths: Vec<String>,
    pub mode: ImportMode,
    pub recursive: bool,
}

fn threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

/// Cria a pasta do projeto e o catálogo. Não abre o projeto.
pub fn create_project(input: &NewProject) -> CoreResult<ProjectPaths> {
    let work = Path::new(input.work_dir.trim());
    if input.work_dir.trim().is_empty() || !work.is_dir() {
        return Err(CoreError::Other(format!(
            "a pasta de trabalho não existe: {}",
            input.work_dir
        )));
    }
    let originals = input.originals_dir.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(o) = originals {
        std::fs::create_dir_all(o).map_err(|e| {
            CoreError::Other(format!("não foi possível criar a pasta dos originais {o}: {e}"))
        })?;
    }
    let folder = project_folder_name(input.event_date.as_deref(), &input.client_name, &input.name);
    let paths = ProjectPaths::new(work.join(folder));
    if paths.root.exists() {
        return Err(CoreError::Other(format!("já existe um projeto em {}", paths.root.display())));
    }
    std::fs::create_dir_all(paths.cache())?;
    std::fs::create_dir_all(paths.backups())?;
    let record = ProjectRecord {
        name: input.name.trim().to_string(),
        client_name: input.client_name.trim().to_string(),
        event_type: input.event_type.clone(),
        event_date: input.event_date.clone().filter(|d| !d.is_empty()),
        location: input.location.trim().to_string(),
        originals_dir: originals.map(String::from),
        status: "created".into(),
        notes: String::new(),
        created_at: 0,
        updated_at: 0,
    };
    let result = Catalog::open(&paths.catalog()).and_then(|c| c.create_project(&record));
    if let Err(e) = result {
        // Limpeza apenas do que acabámos de criar (pasta nova e vazia de fotografias).
        let _ = std::fs::remove_dir_all(&paths.root);
        return Err(e);
    }
    Ok(paths)
}

/// Cópia de segurança do catálogo, se a última tiver mais de uma hora; mantém as últimas 10.
fn backup_catalog(catalog: &Catalog, paths: &ProjectPaths) -> CoreResult<Option<PathBuf>> {
    let dir = paths.backups();
    std::fs::create_dir_all(&dir)?;
    let mut existing: Vec<(i64, PathBuf)> = std::fs::read_dir(&dir)?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ts = name.strip_prefix("catalog-")?.strip_suffix(".db")?.parse::<i64>().ok()?;
            Some((ts, e.path()))
        })
        .collect();
    existing.sort();
    let now = momento_database::now_unix();
    if existing.last().is_some_and(|(ts, _)| now - ts < BACKUP_INTERVAL) {
        return Ok(None);
    }
    let dest = dir.join(format!("catalog-{now}.db"));
    catalog.backup_to(&dest)?;
    existing.push((now, dest.clone()));
    while existing.len() > BACKUPS_KEPT {
        let (_, old) = existing.remove(0);
        let _ = std::fs::remove_file(old);
    }
    Ok(Some(dest))
}

struct RunningImport {
    control: Arc<ImportControl>,
    handle: JoinHandle<()>,
}

/// Projeto aberto.
pub struct ProjectSession {
    pub paths: ProjectPaths,
    /// Ligação usada pelos comandos da interface (leituras e classificações).
    catalog: Mutex<Catalog>,
    registry: Arc<CodecRegistry>,
    thumbs: Mutex<Option<ThumbnailService>>,
    events: Arc<dyn SessionEvents>,
    import: Mutex<Option<RunningImport>>,
}

impl ProjectSession {
    /// Abre um projeto: migra o catálogo, faz cópia de segurança, inicia as miniaturas em
    /// falta e retoma uma importação interrompida.
    pub fn open(user_path: &Path, events: Arc<dyn SessionEvents>) -> CoreResult<Arc<Self>> {
        let paths = ProjectPaths::from_user_path(user_path);
        if !paths.catalog().is_file() {
            return Err(CoreError::Other(format!(
                "não é uma pasta de projeto: {}",
                paths.root.display()
            )));
        }
        let catalog = Catalog::open(&paths.catalog())?;
        if catalog.project()?.is_none() {
            return Err(CoreError::Other("catálogo sem projeto".into()));
        }
        if let Err(e) = backup_catalog(&catalog, &paths) {
            tracing::warn!(erro = %e, "cópia de segurança do catálogo falhou");
        }
        {
            let p = paths.clone();
            std::thread::spawn(move || crate::cache::prune_project(&p));
        }
        let registry = Arc::new(crate::codecs::full_registry());
        let ev = events.clone();
        let thumbs = ThumbnailService::start(
            paths.clone(),
            registry.clone(),
            threads().saturating_sub(1).max(2),
            Arc::new(move |b| ev.thumbnails(&b)),
        )?;
        thumbs.enqueue(catalog.pending_thumbnail_ids()?);
        let unfinished = catalog.unfinished_import_session()?;

        let session = Arc::new(ProjectSession {
            paths,
            catalog: Mutex::new(catalog),
            registry,
            thumbs: Mutex::new(Some(thumbs)),
            events,
            import: Mutex::new(None),
        });
        if let Some(s) = unfinished {
            tracing::info!(sessao = s.id.0, "a retomar importação interrompida");
            session.spawn_import(s.id, s.mode, s.recursive)?;
        }
        Ok(session)
    }

    fn catalog(&self) -> std::sync::MutexGuard<'_, Catalog> {
        self.catalog.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn registry(&self) -> &CodecRegistry {
        &self.registry
    }

    pub fn info(&self) -> CoreResult<ProjectInfo> {
        let c = self.catalog();
        Ok(ProjectInfo {
            path: self.paths.root.to_string_lossy().into_owned(),
            record: c.project()?.ok_or_else(|| CoreError::Other("catálogo sem projeto".into()))?,
            photo_count: c.photo_count()?,
            ui_state: c.load_ui_state()?,
            import_running: self.import_running(),
        })
    }

    pub fn recent_entry(&self) -> CoreResult<RecentProject> {
        let info = self.info()?;
        Ok(RecentProject {
            path: info.path,
            name: info.record.name,
            client_name: info.record.client_name,
            event_type: info.record.event_type,
            event_date: info.record.event_date,
            location: info.record.location,
            photo_count: info.photo_count,
            last_opened: 0,
            available: true,
        })
    }

    pub fn list_photos(&self) -> CoreResult<Vec<PhotoRow>> {
        self.catalog().list_photos()
    }

    pub fn photo_details(&self, id: AssetId) -> CoreResult<Option<PhotoDetails>> {
        self.catalog().photo_details(id)
    }

    pub fn set_stars(&self, ids: &[AssetId], stars: u8) -> CoreResult<usize> {
        self.catalog().set_stars(ids, stars)
    }

    pub fn set_flag(&self, ids: &[AssetId], flag: i8) -> CoreResult<usize> {
        self.catalog().set_flag(ids, flag)
    }

    pub fn set_color(&self, ids: &[AssetId], color: Option<&str>) -> CoreResult<usize> {
        self.catalog().set_color(ids, color)
    }

    pub fn save_ui_state(&self, json: &str) -> CoreResult<()> {
        self.catalog().save_ui_state(json)
    }

    fn with_thumbs(&self, f: impl FnOnce(&ThumbnailService)) {
        if let Some(t) = self.thumbs.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f(t);
        }
    }

    pub fn prioritize_thumbnails(&self, ids: Vec<AssetId>) {
        self.with_thumbs(|t| t.prioritize(ids));
    }

    pub fn enqueue_thumbnails(&self, ids: Vec<AssetId>) {
        self.with_thumbs(|t| t.enqueue(ids));
    }

    pub fn thumbnails_remaining(&self) -> usize {
        let mut n = 0;
        self.with_thumbs(|t| n = t.remaining());
        n
    }

    pub fn cache_stats(&self) -> crate::cache::CacheStats {
        crate::cache::stats(&self.paths)
    }

    pub fn import_running(&self) -> bool {
        self.import
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|r| !r.handle.is_finished())
    }

    /// Inicia uma importação em segundo plano. Só uma de cada vez.
    pub fn start_import(self: &Arc<Self>, req: &ImportRequest) -> CoreResult<i64> {
        if self.import_running() {
            return Err(CoreError::Other("já existe uma importação em curso".into()));
        }
        if req.paths.is_empty() {
            return Err(CoreError::Other("nenhum ficheiro indicado".into()));
        }
        let paths: Vec<PathBuf> = req.paths.iter().map(PathBuf::from).collect();
        let options = self.import_options(req.mode, req.recursive)?;
        let (session, _) =
            momento_import_engine::create_session(&mut self.catalog(), &paths, &options)?;
        self.catalog().set_project_status("importing")?;
        self.spawn_import(session, req.mode, req.recursive)?;
        Ok(session.0)
    }

    fn import_options(&self, mode: ImportMode, recursive: bool) -> CoreResult<ImportOptions> {
        let originals = self.catalog().project()?.and_then(|p| p.originals_dir).map(PathBuf::from);
        if mode == ImportMode::Copy && originals.is_none() {
            return Err(CoreError::Other(
                "defina a pasta dos originais para copiar fotografias".into(),
            ));
        }
        Ok(ImportOptions {
            mode,
            recursive,
            originals_dir: originals,
            project_dir: Some(self.paths.root.clone()),
            threads: (threads() / 2).max(2),
        })
    }

    fn spawn_import(
        self: &Arc<Self>,
        session: momento_core_types::ImportSessionId,
        mode: ImportMode,
        recursive: bool,
    ) -> CoreResult<()> {
        let options = self.import_options(mode, recursive)?;
        let control = Arc::new(ImportControl::default());
        let (me, ctl) = (self.clone(), control.clone());
        let handle = std::thread::Builder::new()
            .name("import".into())
            .spawn(move || {
                let result = momento_import_engine::run_session(
                    &me.paths.catalog(),
                    session,
                    &options,
                    &me.registry,
                    &ctl,
                    &|ev| match ev {
                        ImportEvent::Progress(p) => me.events.import_progress(&p),
                        ImportEvent::AssetsAdded(ids) => me.enqueue_thumbnails(ids),
                    },
                );
                let (counts, error) = match result {
                    Ok(c) => (c, None),
                    Err(e) => {
                        tracing::error!(erro = %e, "importação falhou");
                        (
                            me.catalog().import_session_counts(session).unwrap_or_default(),
                            Some(e.to_string()),
                        )
                    }
                };
                let suspended = counts.pending > 0 && !ctl.is_cancelled();
                let _ =
                    me.catalog().set_project_status(if counts.imported > 0 || counts.total > 0 {
                        "imported"
                    } else {
                        "created"
                    });
                let issues = me.catalog().import_session_issues(session, 500).unwrap_or_default();
                me.events.import_finished(&ImportSummary {
                    session_id: session.0,
                    counts,
                    issues,
                    cancelled: ctl.is_cancelled(),
                    suspended,
                    error,
                });
            })
            .map_err(CoreError::Io)?;
        *self.import.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(RunningImport { control, handle });
        Ok(())
    }

    pub fn cancel_import(&self) {
        if let Some(r) = self.import.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            r.control.cancel();
        }
    }

    /// Espera pelo fim da importação em curso (testes e fecho).
    pub fn wait_import(&self) {
        let taken = self.import.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(r) = taken {
            let _ = r.handle.join();
        }
    }

    /// Fecha o projeto: suspende a importação (retomada na próxima abertura) e para as
    /// miniaturas (as pendentes ficam registadas no catálogo).
    pub fn close(&self) {
        if let Some(r) = self.import.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            r.control.suspend();
        }
        self.wait_import();
        let t = self.thumbs.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(t) = t {
            t.shutdown();
        }
    }
}

impl Drop for ProjectSession {
    fn drop(&mut self) {
        if let Some(r) = self.import.get_mut().unwrap_or_else(|e| e.into_inner()).as_ref() {
            r.control.suspend();
        }
    }
}
