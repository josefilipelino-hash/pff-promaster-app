use momento_core_types::{CoreError, CoreResult};
use momento_database::library::Library;
use momento_import_engine::ImportProgress;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::{ImportSummary, ProjectSession, SessionEvents};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tauri::{AppHandle, Emitter};

pub static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Eventos enviados à interface.
pub const EV_IMPORT_PROGRESS: &str = "import-progress";
pub const EV_IMPORT_FINISHED: &str = "import-finished";
pub const EV_THUMBNAILS: &str = "thumbnails";
pub const EV_ANALYSIS: &str = "analysis-progress";

struct Emitter2 {
    app: AppHandle,
    library: Arc<Mutex<Library>>,
    project_path: String,
}

impl SessionEvents for Emitter2 {
    fn import_progress(&self, p: &ImportProgress) {
        let _ = self.app.emit(EV_IMPORT_PROGRESS, p);
    }
    fn import_finished(&self, s: &ImportSummary) {
        // Atualiza o número de fotografias na lista de projetos recentes.
        if let Ok(lib) = self.library.lock() {
            if let Ok(c) = momento_database::Catalog::open_existing(
                &Path::new(&self.project_path).join("catalog.db"),
            )
            .and_then(|c| c.photo_count())
            {
                let _ = lib.update_photo_count(&self.project_path, c);
            }
        }
        let _ = self.app.emit(EV_IMPORT_FINISHED, s);
    }
    fn thumbnails(&self, b: &ThumbBatch) {
        let _ = self.app.emit(EV_THUMBNAILS, b);
    }
    fn analysis(&self, p: &momento_project_management::analysis::AnalysisProgress) {
        let _ = self.app.emit(EV_ANALYSIS, p);
    }
}

pub struct OpenProject {
    pub session: Arc<ProjectSession>,
    /// Identificador desta abertura, incluído nos URLs de imagens para que a cache do
    /// WebView nunca mostre miniaturas de outro projeto com o mesmo ID.
    pub token: u64,
}

pub struct AppState {
    pub library: Arc<Mutex<Library>>,
    pub project: RwLock<Option<OpenProject>>,
    next_token: AtomicU64,
    app: AppHandle,
}

impl AppState {
    pub fn new(library_path: &Path, app: AppHandle) -> CoreResult<Self> {
        Ok(AppState {
            library: Arc::new(Mutex::new(Library::open(library_path)?)),
            project: RwLock::new(None),
            next_token: AtomicU64::new(1),
            app,
        })
    }

    pub fn library(&self) -> std::sync::MutexGuard<'_, Library> {
        self.library.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn session(&self) -> CoreResult<Arc<ProjectSession>> {
        self.project
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|p| p.session.clone())
            .ok_or_else(|| CoreError::Other("nenhum projeto aberto".into()))
    }

    pub fn token(&self) -> Option<u64> {
        self.project.read().unwrap_or_else(|e| e.into_inner()).as_ref().map(|p| p.token)
    }

    pub fn open(&self, path: &Path) -> CoreResult<(Arc<ProjectSession>, u64)> {
        self.close_project();
        let events = Arc::new(Emitter2 {
            app: self.app.clone(),
            library: self.library.clone(),
            project_path: momento_project_management::ProjectPaths::from_user_path(path)
                .root
                .to_string_lossy()
                .into_owned(),
        });
        let session = ProjectSession::open(path, events)?;
        self.library().touch(&session.recent_entry()?)?;
        let token = self.next_token.fetch_add(1, Ordering::SeqCst);
        *self.project.write().unwrap_or_else(|e| e.into_inner()) =
            Some(OpenProject { session: session.clone(), token });
        tracing::info!(projeto = %session.paths.root.display(), "projeto aberto");
        Ok((session, token))
    }

    pub fn close_project(&self) {
        let taken = self.project.write().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(p) = taken {
            if let Ok(entry) = p.session.recent_entry() {
                let _ = self.library().update_photo_count(&entry.path, entry.photo_count);
            }
            p.session.close();
            tracing::info!(projeto = %p.session.paths.root.display(), "projeto fechado");
        }
    }
}
