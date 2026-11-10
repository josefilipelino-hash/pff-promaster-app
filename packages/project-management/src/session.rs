//! Projeto aberto: catálogo, miniaturas, importações e eventos para a interface.

use crate::analysis::{AnalysisProgress, AnalysisService, SelectionSummary};
pub use crate::organize::OrganizeSummary;
use crate::paths::{project_folder_name, ProjectPaths};
use crate::thumbnails::{ThumbBatch, ThumbnailService};
pub use momento_ai_analysis::clock::OffsetSuggestion;
pub use momento_ai_analysis::selection::{Mode as SelectionMode, Settings as SelectionSettings};
use momento_core_types::recipe::EditRecipe;
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_database::analysis::{AnalysisCounts, AnalysisDetails, GroupRow, RecommendationBrief};
use momento_database::assets::{PhotoDetails, PhotoRow};
use momento_database::clock::{CameraOffset, CameraRow};
use momento_database::edits::EditState;
use momento_database::import::{ImportMode, ItemIssue, SessionCounts};
use momento_database::library::RecentProject;
use momento_database::moments::{AssetMoment, MomentRow};
use momento_database::people::{FaceRef, PersonRow};
use momento_database::project::ProjectRecord;
use momento_database::Catalog;
use momento_image_codecs::CodecRegistry;
pub use momento_image_processing::develop::auto::{
    AiDenoise, AutoOptions, AutoReport, Intensity as AutoIntensity,
};
pub use momento_image_processing::develop::consistency::{MatchOptions, MatchReport};
pub use momento_image_processing::develop::FaceRegions;
pub use momento_image_processing::develop::{
    Quality as RenderQuality, Settings as DevelopSettings,
};
use momento_import_engine::{ImportControl, ImportEvent, ImportOptions, ImportProgress};
pub use momento_presets::{apply as apply_preset_settings, Component, Preset};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Intervalo por omissão das cópias de segurança automáticas (minutos; 0 = desligadas).
pub const DEFAULT_BACKUP_MINUTES: u32 = 60;
const BACKUP_MINUTES_KEY: &str = "safety.backup_minutes";
/// Verificação da disponibilidade dos originais (discos ligados/desligados).
const AVAILABILITY_EVERY: std::time::Duration = std::time::Duration::from_secs(30);
/// Checkpoint do WAL (as alterações passam para o ficheiro principal do catálogo).
const CHECKPOINT_EVERY: std::time::Duration = std::time::Duration::from_secs(300);

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
    fn analysis(&self, _p: &AnalysisProgress) {}
    /// Edições gravadas (as miniaturas destas chaves mudaram).
    fn edits_changed(&self, _keys: &[i64]) {}
    /// Fila da edição automática.
    fn auto_edit(&self, _p: &crate::auto_edit::AutoEditProgress) {}
    /// AI Denoise de uma fotografia no editor.
    fn ai_denoise(&self, _p: &crate::auto_edit::AiDenoiseProgress) {}
    /// Fila de exportação.
    fn export_progress(&self, _p: &crate::export::ExportProgress) {}
    /// A disponibilidade dos originais mudou (disco desligado/ligado, ficheiro em falta).
    fn availability(&self, _s: &AvailabilitySummary) {}
}

/// Estado de edição + ajustes já interpretados (a interface trabalha com `settings`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditView {
    #[serde(flatten)]
    pub state: EditState,
    pub settings: DevelopSettings,
}

/// Estado da análise para a interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisStatus {
    pub running: bool,
    pub counts: AnalysisCounts,
    /// Modelos de IA locais encontrados (sem eles, só métricas técnicas).
    pub models_available: bool,
    pub settings: SelectionSettings,
    pub summary: SelectionSummary,
}

/// Pastas extra onde procurar os modelos (ex.: recursos da aplicação instalada).
static MODEL_PATHS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();

pub fn set_model_search_paths(paths: Vec<PathBuf>) {
    let _ = MODEL_PATHS.set(paths);
}

/// Pastas de LUT da aplicação (registadas pela aplicação desktop) — além de `<projeto>/luts`.
static LUT_PATHS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();

pub fn set_lut_search_paths(paths: Vec<PathBuf>) {
    for p in &paths {
        momento_image_processing::develop::lut::add_search_path(p.clone());
    }
    let _ = LUT_PATHS.set(paths);
}

pub(crate) fn lut_dirs() -> Vec<PathBuf> {
    LUT_PATHS.get().cloned().unwrap_or_default()
}

pub(crate) fn models_dir() -> Option<PathBuf> {
    momento_face_analysis::find_models_dir(MODEL_PATHS.get().map(Vec::as_slice).unwrap_or(&[]))
}

const AUTO_ANALYSIS_KEY: &str = "analysis_auto";

/// Segurança da importação (Fase 13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImportSafety {
    /// Reler cada cópia e comparar o checksum BLAKE3.
    pub verify: bool,
    /// Segunda cópia num disco de segurança.
    pub backup_dir: Option<String>,
}

impl Default for ImportSafety {
    fn default() -> Self {
        ImportSafety { verify: true, backup_dir: None }
    }
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
    pub(crate) events: Arc<dyn SessionEvents>,
    import: Mutex<Option<RunningImport>>,
    analysis: Mutex<Option<AnalysisService>>,
    /// Serializa o início da análise (clique do utilizador vs retoma automática no fim da
    /// importação — antes podiam começar duas e uma era descartada a meio).
    analysis_start: Mutex<()>,
    /// O projeto está a fechar: nada novo começa em segundo plano (Fase 15).
    closing: std::sync::atomic::AtomicBool,
    pub(crate) edits: crate::edit::EditService,
    /// Modelo do AI Denoise (carregado na primeira utilização).
    pub(crate) denoiser: std::sync::OnceLock<Option<Arc<momento_face_analysis::Denoiser>>>,
    /// AI Denoise pedido pelo editor (só o mais recente) e se há um cálculo em curso.
    #[allow(clippy::type_complexity)]
    pub(crate) ai_next:
        Mutex<Option<(i64, crate::edit::AiKey, Arc<momento_image_processing::develop::Source>)>>,
    pub(crate) ai_busy: std::sync::atomic::AtomicBool,
    /// Cálculos de AI Denoise em curso (para não repetir o mesmo em paralelo).
    pub(crate) ai_inflight: Mutex<std::collections::HashSet<crate::edit::AiKey>>,
    pub(crate) auto_job: Mutex<Option<crate::auto_edit::AutoEditJob>>,
    /// Fila de exportação.
    pub(crate) exports: crate::export::ExportQueue,
    /// Ligações ao catálogo reutilizadas pelo trabalho fora da ligação principal.
    pool: Arc<crate::pool::CatalogPool>,
    /// Ordem das previews a gerar (mais recente primeiro).
    view_gate: momento_task_queue::foreground::LifoGate,
    /// Uso exclusivo do projeto (libertado no fecho limpo).
    lock: Mutex<Option<crate::safety::SessionLock>>,
    /// O que aconteceu ao abrir (sessão interrompida, restauro…).
    open_report: crate::safety::OpenReport,
    /// Manutenção em segundo plano: disponibilidade, checkpoints, cópias de segurança.
    maintenance: Mutex<Option<JoinHandle<()>>>,
    stop_maintenance: Arc<(Mutex<bool>, std::sync::Condvar)>,
}

/// Cópia de segurança automática se a última tiver mais de `minutes` minutos.
fn backup_if_due(catalog: &Catalog, paths: &ProjectPaths, minutes: u32) -> CoreResult<bool> {
    if minutes == 0 {
        return Ok(false);
    }
    let last = crate::safety::list_backups(paths)
        .into_iter()
        .find(|b| b.kind == "auto")
        .map(|b| b.created);
    if last.is_some_and(|t| momento_database::now_unix() - t < minutes as i64 * 60) {
        return Ok(false);
    }
    crate::safety::create_backup(catalog, paths, "auto").map(|_| true)
}

/// Previews geradas em simultâneo. Medido (navegação rápida, 16 fotografias de 24 MP a
/// cada 40 ms, 4 execuções): sem limite, a última esperava 844 ms; com 1/2/3 permissões,
/// 575/634/562 ms (e 5,2/2,7/2,0 s para todas). 3 sobrepõe as fases em série
/// (descodificar/codificar) sem pôr a fotografia vista atrás das antigas.
const VIEW_PERMITS: usize = 3;

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
        // Uso exclusivo; a sessão anterior fechou bem?
        let (lock, unclean) = crate::safety::SessionLock::acquire(&paths)?;
        let mut report =
            crate::safety::OpenReport { previous_unclean: unclean, ..Default::default() };
        // Validação (completa depois de uma interrupção) e, se preciso, restauro da versão
        // válida mais recente; o catálogo danificado fica guardado.
        crate::safety::validate_or_recover(&paths, unclean, &mut report)?;
        let catalog = Catalog::open(&paths.catalog())?;
        let Some(record) = catalog.project()? else {
            return Err(CoreError::Other("catálogo sem projeto".into()));
        };
        if unclean {
            tracing::warn!(
                "a sessão anterior não fechou o projeto; catálogo validado por completo"
            );
            let mut n = crate::safety::remove_temp_files(&paths.cache(), 3);
            if let Some(o) = record.originals_dir.as_deref() {
                n += crate::safety::remove_temp_files(Path::new(o), 4);
            }
            report.temp_files_removed = n;
            let _ = catalog.log_event(
                "sessao_interrompida",
                &format!(
                    "validação completa em {} ms; {n} ficheiros temporários removidos",
                    report.validation_ms
                ),
            );
        }
        if let Some(ts) = report.restored_from {
            let _ = catalog.log_event(
                "catalogo_restaurado",
                &format!("versão {ts}; danificado guardado em backups/"),
            );
        }
        if let Err(e) = backup_if_due(&catalog, &paths, 60) {
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

        momento_image_processing::develop::lut::add_search_path(paths.root.join("luts"));
        momento_image_processing::develop::local::add_raster_path(paths.root.join("masks"));
        let paths_catalog = paths.catalog();
        let session = Arc::new(ProjectSession {
            paths,
            catalog: Mutex::new(catalog),
            registry,
            thumbs: Mutex::new(Some(thumbs)),
            events,
            import: Mutex::new(None),
            analysis: Mutex::new(None),
            analysis_start: Mutex::new(()),
            closing: std::sync::atomic::AtomicBool::new(false),
            edits: crate::edit::EditService::new(),
            denoiser: std::sync::OnceLock::new(),
            ai_next: Mutex::new(None),
            ai_busy: std::sync::atomic::AtomicBool::new(false),
            ai_inflight: Mutex::new(std::collections::HashSet::new()),
            auto_job: Mutex::new(None),
            exports: Default::default(),
            pool: crate::pool::CatalogPool::new(paths_catalog),
            lock: Mutex::new(Some(lock)),
            open_report: report,
            maintenance: Mutex::new(None),
            stop_maintenance: Arc::new((Mutex::new(false), std::sync::Condvar::new())),
            view_gate: momento_task_queue::foreground::LifoGate::new(VIEW_PERMITS),
        });
        session.start_maintenance();
        if let Some(s) = unfinished {
            tracing::info!(sessao = s.id.0, "a retomar importação interrompida");
            session.spawn_import(s.id, s.mode, s.recursive)?;
        } else {
            // Retoma uma análise interrompida (só se o utilizador já a tinha iniciado).
            session.resume_analysis_if_auto();
        }
        Ok(session)
    }

    pub(crate) fn catalog(&self) -> std::sync::MutexGuard<'_, Catalog> {
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

    // ------------------------------------------------------------------ edição (Fase 5)

    fn view(&self, key: i64) -> CoreResult<EditView> {
        let state = self.catalog().edit_state(key)?;
        let settings = DevelopSettings::from_recipe(&state.recipe);
        Ok(EditView { state, settings })
    }

    pub fn edit_view(&self, key: i64) -> CoreResult<EditView> {
        self.view(key)
    }

    /// Render para a interface (frame binário: dimensões, histograma, RGBA). Não grava nada.
    pub fn render_edit(
        self: &Arc<Self>,
        key: i64,
        settings: &DevelopSettings,
        max_w: u32,
        max_h: u32,
        quality: RenderQuality,
        uncropped: bool,
    ) -> CoreResult<Vec<u8>> {
        let _fg = momento_task_queue::foreground::Interactive::begin();
        let (_, asset) = self.catalog().resolve_key(key)?;
        // AI Denoise: se ainda não estiver calculado, render sem ele e cálculo em segundo
        // plano (evento `ai_denoise` quando estiver pronto).
        let (src, _) =
            self.source_for(&*self.open_catalog()?, asset, settings, false, false, key)?;
        Ok(self.edits.render_frame(&src, settings, max_w, max_h, quality, uncropped))
    }

    /// Render com a máscara `overlay` (índice em `settings.masks`) mostrada a vermelho.
    #[allow(clippy::too_many_arguments)]
    pub fn render_edit_overlay(
        self: &Arc<Self>,
        key: i64,
        settings: &DevelopSettings,
        max_w: u32,
        max_h: u32,
        quality: RenderQuality,
        uncropped: bool,
        overlay: Option<usize>,
    ) -> CoreResult<Vec<u8>> {
        let (_, asset) = self.catalog().resolve_key(key)?;
        let (src, _) =
            self.source_for(&*self.open_catalog()?, asset, settings, false, false, key)?;
        Ok(self
            .edits
            .render_frame_overlay(&src, settings, max_w, max_h, quality, uncropped, overlay))
    }

    /// Ligação própria para operações longas (não bloqueia a do resto da interface).
    pub(crate) fn open_catalog(&self) -> CoreResult<crate::pool::PooledCatalog> {
        self.pool.get()
    }

    pub(crate) fn after_edit(self: &Arc<Self>, keys: Vec<i64>) {
        // Sidecars e miniaturas editadas em segundo plano.
        let me = self.clone();
        std::thread::spawn(move || {
            let Ok(cat) = me.open_catalog() else { return };
            for &k in &keys {
                if let Err(e) = me.refresh_edited(&cat, k) {
                    tracing::warn!(erro = %e, "miniatura editada ou sidecar não gerados");
                }
            }
            me.events.edits_changed(&keys);
        });
    }

    /// Grava os ajustes (nova entrada no histórico com a etiqueta indicada).
    pub fn commit_edit(
        self: &Arc<Self>,
        key: i64,
        label: &str,
        settings: &DevelopSettings,
    ) -> CoreResult<EditView> {
        {
            let mut c = self.catalog();
            let (variant, _) = c.resolve_key(key)?;
            let base = c.recipe_of(variant)?;
            let recipe = settings.to_recipe(&base);
            if recipe != base {
                c.commit_edit(variant, label, &recipe)?;
            }
        }
        self.after_edit(vec![key]);
        self.view(key)
    }

    pub fn undo_edit(self: &Arc<Self>, key: i64) -> CoreResult<EditView> {
        let (variant, _) = self.catalog().resolve_key(key)?;
        self.catalog().undo(variant)?;
        self.after_edit(vec![key]);
        self.view(key)
    }

    pub fn redo_edit(self: &Arc<Self>, key: i64) -> CoreResult<EditView> {
        let (variant, _) = self.catalog().resolve_key(key)?;
        self.catalog().redo(variant)?;
        self.after_edit(vec![key]);
        self.view(key)
    }

    pub fn goto_history(self: &Arc<Self>, key: i64, seq: i64) -> CoreResult<EditView> {
        let (variant, _) = self.catalog().resolve_key(key)?;
        self.catalog().goto_history(variant, seq)?;
        self.after_edit(vec![key]);
        self.view(key)
    }

    /// Repor: volta ao original (fica no histórico; pode ser desfeito).
    pub fn reset_edit(self: &Arc<Self>, key: i64) -> CoreResult<EditView> {
        self.commit_edit(key, "reset", &DevelopSettings::default())
    }

    pub fn create_snapshot(&self, key: i64, name: &str) -> CoreResult<EditView> {
        let (variant, _) = self.catalog().resolve_key(key)?;
        self.catalog().create_snapshot(variant, name)?;
        self.view(key)
    }

    pub fn apply_snapshot(self: &Arc<Self>, key: i64, id: i64) -> CoreResult<EditView> {
        let (variant, _) = self.catalog().resolve_key(key)?;
        let (sv, _, recipe) = self.catalog().snapshot_recipe(id)?;
        if sv != variant {
            return Err(CoreError::Other("o instantâneo pertence a outra fotografia".into()));
        }
        self.catalog().commit_edit(variant, "snapshot", &recipe)?;
        self.after_edit(vec![key]);
        self.view(key)
    }

    pub fn delete_snapshot(&self, key: i64, id: i64) -> CoreResult<EditView> {
        self.catalog().delete_snapshot(id)?;
        self.view(key)
    }

    pub fn create_virtual_copy(self: &Arc<Self>, key: i64) -> CoreResult<i64> {
        let k = self.catalog().create_virtual_copy(key)?;
        self.after_edit(vec![k]);
        Ok(k)
    }

    pub fn delete_virtual_copy(&self, key: i64) -> CoreResult<()> {
        self.catalog().delete_virtual_copy(key)
    }

    /// Colar/sincronizar: copia os blocos escolhidos de `settings` para cada fotografia.
    pub fn paste_settings(
        self: &Arc<Self>,
        keys: &[i64],
        settings: &DevelopSettings,
        blocks: &[String],
    ) -> CoreResult<usize> {
        let from = settings.to_recipe(&EditRecipe::default());
        let mut n = 0;
        {
            let mut c = self.catalog();
            for &k in keys {
                let (variant, _) = c.resolve_key(k)?;
                let base = c.recipe_of(variant)?;
                let merged = momento_image_processing::develop::merge_blocks(&base, &from, blocks);
                if merged != base {
                    c.commit_edit(variant, "paste", &merged)?;
                    n += 1;
                }
            }
        }
        self.after_edit(keys.to_vec());
        Ok(n)
    }

    /// Seletor de balanço de brancos: (temperatura, matiz) que neutralizam o ponto (u, v).
    pub fn pick_white_balance(
        &self,
        key: i64,
        settings: &DevelopSettings,
        u: f32,
        v: f32,
    ) -> CoreResult<Option<(f32, f32)>> {
        let (_, asset) = self.catalog().resolve_key(key)?;
        let src = self.edits.source(&*self.open_catalog()?, &self.registry, asset)?;
        Ok(momento_image_processing::develop::sample_for_white_balance(&src, settings, u, v)
            .map(momento_image_processing::develop::color::neutralize))
    }

    pub fn auto_straighten(&self, key: i64, settings: &DevelopSettings) -> CoreResult<Option<f32>> {
        let (_, asset) = self.catalog().resolve_key(key)?;
        let src = self.edits.source(&*self.open_catalog()?, &self.registry, asset)?;
        Ok(momento_image_processing::develop::auto_straighten(&src, settings))
    }

    /// Ficheiro a servir à interface para uma chave (`thumb`/`preview`/`full`): a versão
    /// editada quando há ajustes, senão os ficheiros normais do asset.
    pub fn serve_image(
        self: &Arc<Self>,
        key: i64,
        level: crate::paths::CacheLevel,
    ) -> CoreResult<crate::previews::Servable> {
        // A preview/1:1 da fotografia aberta passa à frente do segundo plano.
        let _fg = (level != crate::paths::CacheLevel::Thumb)
            .then(momento_task_queue::foreground::Interactive::begin);
        let cat = self.open_catalog()?;
        let (variant, asset) = cat.resolve_key(key)?;
        let recipe = cat.recipe_of(variant)?;
        // Já em cache: sai logo, sem esperar por gerações em curso.
        let cached = recipe.is_identity() && self.paths.cached(level, asset).is_file();
        // Gerar: no máximo VIEW_PERMITS de cada vez, o pedido mais recente primeiro (a
        // fotografia em que o utilizador parou ao navegar depressa).
        let _turn =
            (level != crate::paths::CacheLevel::Thumb && !cached).then(|| self.view_gate.acquire());
        if recipe.is_identity() {
            return crate::previews::ensure(&self.paths, &self.registry, asset, level);
        }
        let name = match level {
            crate::paths::CacheLevel::Thumb => "thumbs",
            crate::paths::CacheLevel::Preview => "previews",
            crate::paths::CacheLevel::Full => "full",
        };
        let settings = DevelopSettings::from_recipe(&recipe);
        let src =
            |edge: Option<u32>| -> CoreResult<Arc<momento_image_processing::develop::Source>> {
                self.source_for(&cat, asset, &settings, edge.is_none(), true, key).map(|s| s.0)
            };
        let path = crate::edit::ensure_edited(&self.paths, &src, variant, &recipe, name)?;
        Ok(crate::previews::Servable { path, mime: "image/jpeg" })
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
        self.start_import_with(req, &ImportSafety::default())
    }

    /// Importação com verificação opcional e cópia para um disco de segurança (Fase 13).
    pub fn start_import_with(
        self: &Arc<Self>,
        req: &ImportRequest,
        safety: &ImportSafety,
    ) -> CoreResult<i64> {
        if let Some(b) = safety.backup_dir.as_deref() {
            let b = Path::new(b);
            // A cópia de segurança tem de ser OUTRA pasta (idealmente outro disco): na mesma
            // pasta dos originais não protege nada (Fase 15).
            let originals =
                self.catalog().project()?.and_then(|p| p.originals_dir).map(PathBuf::from);
            let same = |x: &Path, y: &Path| {
                let c = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
                c(x) == c(y) || c(x).starts_with(c(y)) || c(y).starts_with(c(x))
            };
            if originals.as_deref().is_some_and(|o| same(o, b)) || same(&self.paths.root, b) {
                return Err(CoreError::Other(
                    "o disco de segurança não pode ser a pasta dos originais nem a do projeto"
                        .into(),
                ));
            }
            if !b.is_dir() {
                return Err(CoreError::Unavailable(
                    b.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                ));
            }
            if b.starts_with(&self.paths.root) {
                return Err(CoreError::Other(
                    "a cópia de segurança não pode ficar dentro do projeto".into(),
                ));
            }
        }
        if self.import_running() {
            return Err(CoreError::Other("já existe uma importação em curso".into()));
        }
        if req.paths.is_empty() {
            return Err(CoreError::Other("nenhum ficheiro indicado".into()));
        }
        let paths: Vec<PathBuf> = req.paths.iter().map(PathBuf::from).collect();
        let options = self.import_options(req.mode, req.recursive, Some(safety))?;
        let (session, _) =
            momento_import_engine::create_session(&mut self.catalog(), &paths, &options)?;
        // As mesmas opções numa retoma depois de uma interrupção.
        self.catalog().save_setting(
            &format!("import_safety:{}", session.0),
            &serde_json::to_string(safety).unwrap_or_default(),
        )?;
        self.catalog().set_project_status("importing")?;
        self.spawn_import(session, req.mode, req.recursive)?;
        Ok(session.0)
    }

    fn import_options(
        &self,
        mode: ImportMode,
        recursive: bool,
        safety: Option<&ImportSafety>,
    ) -> CoreResult<ImportOptions> {
        let safety = safety.cloned().unwrap_or_default();
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
            verify: safety.verify,
            backup_dir: safety.backup_dir.map(PathBuf::from),
        })
    }

    fn spawn_import(
        self: &Arc<Self>,
        session: momento_core_types::ImportSessionId,
        mode: ImportMode,
        recursive: bool,
    ) -> CoreResult<()> {
        let saved: Option<ImportSafety> = self
            .catalog()
            .load_setting(&format!("import_safety:{}", session.0))?
            .and_then(|j| serde_json::from_str(&j).ok());
        let options = self.import_options(mode, recursive, saved.as_ref())?;
        // Retoma: temporários de cópias interrompidas também no disco de segurança (a
        // pasta dos originais é limpa na abertura). Só o padrão de nomes da aplicação.
        if let Some(b) = &options.backup_dir {
            crate::safety::remove_temp_files(b, 4);
        }
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
                me.resume_analysis_if_auto();
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

    // ------------------------------------------------------------------ análise

    pub fn analysis_running(&self) -> bool {
        self.analysis
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|a| !a.is_finished())
    }

    /// Analisa as fotografias por analisar (ou todas, com `reanalyze`). Não bloqueia.
    pub fn start_analysis(&self, reanalyze: bool) -> CoreResult<u64> {
        let _gate = self.analysis_start.lock().unwrap_or_else(|e| e.into_inner());
        if self.closing.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(CoreError::Other("o projeto está a fechar".into()));
        }
        if self.analysis_running() {
            return Err(CoreError::Other("já existe uma análise em curso".into()));
        }
        if reanalyze {
            self.catalog().clear_analysis()?;
        }
        self.catalog().save_setting(AUTO_ANALYSIS_KEY, "1")?;
        let ids = self.catalog().pending_analysis_ids()?;
        let n = ids.len() as u64;
        let ev = self.events.clone();
        let threads = (threads() / 2).max(1);
        let svc = crate::analysis::run(
            self.paths.clone(),
            self.registry.clone(),
            models_dir(),
            ids,
            threads,
            Arc::new(move |p| ev.analysis(&p)),
        )?;
        *self.analysis.lock().unwrap_or_else(|e| e.into_inner()) = Some(svc);
        Ok(n)
    }

    fn resume_analysis_if_auto(&self) {
        let auto =
            self.catalog().load_setting(AUTO_ANALYSIS_KEY).ok().flatten().is_some_and(|v| v == "1");
        let pending = self.catalog().analysis_counts().map(|c| c.pending).unwrap_or(0);
        if auto && pending > 0 && !self.analysis_running() {
            if let Err(e) = self.start_analysis(false) {
                tracing::warn!(erro = %e, "não foi possível retomar a análise");
            }
        }
    }

    /// Para a análise. O que já foi analisado fica gravado; a seleção é recalculada.
    pub fn cancel_analysis(&self) {
        let _ = self.catalog().save_setting(AUTO_ANALYSIS_KEY, "0");
        if let Some(a) = self.analysis.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            a.cancel();
        }
    }

    /// Espera pelo fim da análise em curso (testes).
    pub fn wait_analysis(&self) {
        let taken = self.analysis.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(a) = taken {
            a.wait();
        }
    }

    fn selection_summary(&self) -> CoreResult<SelectionSummary> {
        let recs = self.catalog().list_recommendations()?;
        let groups = self.catalog().list_groups()?;
        let mut s = SelectionSummary {
            groups: groups.len() as u64,
            sequences: groups.iter().filter(|g| g.kind == "sequence").count() as u64,
            ..Default::default()
        };
        for r in &recs {
            match r.label.as_str() {
                "best" => s.best += 1,
                "alternative" => s.alternative += 1,
                "review" => s.review += 1,
                "reject" => s.reject += 1,
                _ => s.duplicate += 1,
            }
        }
        Ok(s)
    }

    pub fn analysis_status(&self) -> CoreResult<AnalysisStatus> {
        let counts = self.catalog().analysis_counts()?;
        let settings = crate::analysis::load_settings(&self.catalog());
        Ok(AnalysisStatus {
            running: self.analysis_running(),
            counts,
            models_available: models_dir().is_some(),
            settings,
            summary: self.selection_summary()?,
        })
    }

    /// Muda o modo/objetivo e recalcula as recomendações (rápido: não reanalisa).
    pub fn set_selection_settings(
        &self,
        settings: SelectionSettings,
    ) -> CoreResult<SelectionSummary> {
        let mut c = self.catalog();
        crate::analysis::save_settings(&c, &settings)?;
        crate::analysis::recompute_selection(&mut c)
    }

    /// Recalcula com o estado atual (ex.: depois de o utilizador marcar favoritas).
    pub fn recompute_selection(&self) -> CoreResult<SelectionSummary> {
        crate::analysis::recompute_selection(&mut self.catalog())
    }

    pub fn list_recommendations(&self) -> CoreResult<Vec<RecommendationBrief>> {
        self.catalog().list_recommendations()
    }

    pub fn list_groups(&self) -> CoreResult<Vec<GroupRow>> {
        self.catalog().list_groups()
    }

    pub fn analysis_details(&self, id: AssetId) -> CoreResult<AnalysisDetails> {
        self.catalog().analysis_details(id)
    }

    /// “Aceitar sugestões”: favorita nas MELHOR, rejeitada nas REJEITAR SUGERIDO — só em
    /// fotografias sem decisão do utilizador. Reversível; nada é apagado. Alternativas,
    /// REVER e duplicadas ficam como estão.
    pub fn accept_recommendations(&self, ids: &[AssetId]) -> CoreResult<usize> {
        let mut c = self.catalog();
        let labels = c.labels_for(ids)?;
        let picks: Vec<AssetId> = labels.iter().filter(|l| l.1 == "best").map(|l| l.0).collect();
        let rejects: Vec<AssetId> =
            labels.iter().filter(|l| l.1 == "reject").map(|l| l.0).collect();
        c.accept_recommendations(&picks, &rejects)
    }

    pub fn accept_group(&self, group: i64) -> CoreResult<usize> {
        let ids: Vec<AssetId> =
            self.catalog().group_labels(group)?.into_iter().map(|l| l.0).collect();
        self.accept_recommendations(&ids)
    }

    // ------------------------------------------------------------------ organização (Fase 4)

    /// Recalcula seleção, pessoas e momentos a partir do catálogo (sem reanalisar).
    pub fn rebuild(&self) -> OrganizeSummary {
        crate::organize::rebuild(&mut self.catalog())
    }

    pub fn list_persons(&self) -> CoreResult<Vec<PersonRow>> {
        self.catalog().list_persons()
    }

    pub fn person_faces(&self, person: i64, limit: u32) -> CoreResult<Vec<FaceRef>> {
        self.catalog().person_faces(person, limit)
    }

    /// Pares (fotografia, pessoa) para pesquisar “noiva + mãe”, “noivos juntos”…
    pub fn photo_persons(&self) -> CoreResult<Vec<(AssetId, i64)>> {
        self.catalog().photo_persons()
    }

    /// Identificar uma pessoa (nome, papel, ocultar). Os momentos são reavaliados (os
    /// papéis noiva/noivo ajudam a distinguir preparações, sessão do casal…).
    pub fn update_person(
        &self,
        id: i64,
        name: Option<&str>,
        role: Option<&str>,
        hidden: bool,
    ) -> CoreResult<()> {
        let mut c = self.catalog();
        c.update_person(id, name, role, hidden)?;
        crate::organize::refresh_moments(&mut c).map(|_| ())
    }

    pub fn merge_persons(&self, from: i64, into: i64) -> CoreResult<()> {
        let mut c = self.catalog();
        c.merge_persons(from, into)?;
        crate::organize::refresh_moments(&mut c).map(|_| ())
    }

    /// “Não é esta pessoa”.
    pub fn exclude_faces(&self, faces: &[i64]) -> CoreResult<usize> {
        self.catalog().exclude_faces(faces)
    }

    /// Atribui rostos a uma pessoa existente ou a uma nova (`person = None`).
    pub fn assign_faces(&self, faces: &[i64], person: Option<i64>) -> CoreResult<i64> {
        let c = self.catalog();
        let p = match person {
            Some(p) => p,
            None => c.create_person(None, None)?,
        };
        c.assign_faces(faces, p)?;
        Ok(p)
    }

    /// Reagrupa rostos (ex.: depois de juntar/excluir vários).
    pub fn recluster_people(&self) -> CoreResult<u64> {
        let mut c = self.catalog();
        let n = crate::organize::recluster_people(&mut c)?;
        crate::organize::refresh_moments(&mut c)?;
        Ok(n)
    }

    /// Apaga todos os dados de pessoas (identidades incluídas) deste projeto.
    pub fn clear_people(&self) -> CoreResult<()> {
        self.catalog().clear_people()
    }

    pub fn list_moments(&self) -> CoreResult<Vec<MomentRow>> {
        self.catalog().list_moments()
    }

    pub fn asset_moments(&self) -> CoreResult<Vec<AssetMoment>> {
        self.catalog().asset_moments()
    }

    pub fn create_moment(&self, name: &str, after: Option<i64>) -> CoreResult<i64> {
        self.catalog().create_moment(name, after)
    }

    pub fn rename_moment(&self, id: i64, name: &str) -> CoreResult<()> {
        self.catalog().rename_moment(id, name)
    }

    pub fn delete_moment(&self, id: i64) -> CoreResult<()> {
        let mut c = self.catalog();
        c.delete_moment(id)?;
        crate::organize::refresh_moments(&mut c).map(|_| ())
    }

    /// Arrastar fotografias para um momento (`None` = “Por classificar”). As fotografias
    /// parecidas sem decisão do utilizador são reavaliadas com este exemplo.
    pub fn assign_moment(&self, ids: &[AssetId], moment: Option<i64>) -> CoreResult<usize> {
        let mut c = self.catalog();
        let n = c.assign_moment(ids, moment)?;
        crate::organize::refresh_moments(&mut c)?;
        Ok(n)
    }

    /// Devolve fotografias à classificação automática.
    pub fn reset_moments(&self, ids: &[AssetId]) -> CoreResult<usize> {
        let mut c = self.catalog();
        let n = c.clear_user_moments(ids)?;
        crate::organize::refresh_moments(&mut c)?;
        Ok(n)
    }

    pub fn reclassify_moments(&self) -> CoreResult<(u64, u64)> {
        crate::organize::refresh_moments(&mut self.catalog())
    }

    pub fn list_cameras(&self) -> CoreResult<Vec<CameraRow>> {
        self.catalog().list_cameras()
    }

    pub fn suggest_offsets(&self, reference: &str) -> CoreResult<Vec<OffsetSuggestion>> {
        crate::organize::suggest_offsets(&self.catalog(), reference)
    }

    /// Grava as correções de hora e reconstrói a cronologia: ordem das fotografias,
    /// sequências/recomendações e momentos.
    pub fn set_camera_offsets(&self, offsets: &[CameraOffset]) -> CoreResult<OrganizeSummary> {
        let mut c = self.catalog();
        c.set_camera_offsets(offsets)?;
        let mut s = OrganizeSummary {
            selection: crate::analysis::recompute_selection(&mut c)?,
            ..Default::default()
        };
        let (cl, un) = crate::organize::refresh_moments(&mut c)?;
        s.classified = cl;
        s.unclassified = un;
        Ok(s)
    }

    /// Fecha o projeto: suspende a importação (retomada na próxima abertura) e para as
    /// miniaturas (as pendentes ficam registadas no catálogo).
    pub fn close(&self) {
        // Nada novo começa a partir daqui (ex.: a importação, ao terminar, retomava a
        // análise automática — que ficava a escrever no catálogo depois do fecho).
        self.closing.store(true, std::sync::atomic::Ordering::SeqCst);
        // Exportação: para entre ficheiros (os ficheiros completos ficam; nunca meio escritos).
        self.exports.cancel_all();
        // Edição automática: para (o que já foi editado fica; o resto não é tocado).
        self.cancel_auto_edit();
        self.wait_auto_edit();
        let a = self.analysis.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(a) = a {
            a.cancel();
            a.wait();
        }
        if let Some(r) = self.import.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            r.control.suspend();
        }
        self.wait_import();
        // Uma análise que tenha começado entretanto (antes do sinal acima) também para.
        let a = self.analysis.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(a) = a {
            a.cancel();
            a.wait();
        }
        let t = self.thumbs.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(t) = t {
            t.shutdown();
        }
        self.stop_maintenance();
        // Fecho limpo: tudo no ficheiro principal, uma versão recente e o bloqueio libertado.
        {
            let cat = self.catalog();
            if let Err(e) = cat.checkpoint() {
                tracing::warn!(erro = %e, "checkpoint no fecho falhou");
            }
            if let Err(e) = backup_if_due(&cat, &self.paths, 15) {
                tracing::warn!(erro = %e, "cópia de segurança no fecho falhou");
            }
        }
        if let Some(l) = self.lock.lock().unwrap_or_else(|e| e.into_inner()).take() {
            l.release();
        }
    }

    // ---------------------------------------------------------------- segurança (Fase 13)

    pub fn open_report(&self) -> crate::safety::OpenReport {
        self.open_report.clone()
    }

    pub fn backup_minutes(&self) -> u32 {
        self.catalog()
            .load_setting(BACKUP_MINUTES_KEY)
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_BACKUP_MINUTES)
    }

    pub fn set_backup_minutes(&self, minutes: u32) -> CoreResult<()> {
        self.catalog().save_setting(BACKUP_MINUTES_KEY, &minutes.to_string())
    }

    pub fn list_backups(&self) -> Vec<crate::safety::BackupInfo> {
        crate::safety::list_backups(&self.paths)
    }

    /// Versão manual (nunca apagada automaticamente).
    pub fn create_backup(&self) -> CoreResult<crate::safety::BackupInfo> {
        let b = crate::safety::create_backup(&self.catalog(), &self.paths, "manual")?;
        let _ = self.catalog().log_event("copia_manual", &b.created.to_string());
        Ok(b)
    }

    pub fn export_catalog(&self, dest: &Path) -> CoreResult<()> {
        crate::safety::export_catalog(&self.catalog(), dest)?;
        let _ = self.catalog().log_event("catalogo_exportado", "");
        Ok(())
    }

    /// Validação completa agora (pode demorar em catálogos grandes).
    pub fn validate_catalog(&self) -> CoreResult<momento_database::safety::Validation> {
        let v = self.catalog().validate(true)?;
        let _ = self.catalog().log_event("validacao", if v.ok() { "ok" } else { "problemas" });
        Ok(v)
    }

    pub fn safety_events(&self, limit: u32) -> CoreResult<Vec<(i64, String, String)>> {
        self.catalog().recent_events(limit)
    }

    /// Verifica agora os originais: offline/em falta/reconectados. As miniaturas que
    /// ficaram por fazer com o disco desligado são geradas ao reconectar.
    pub fn check_availability(&self) -> CoreResult<AvailabilitySummary> {
        let originals = self.open_catalog()?.originals()?;
        let now = crate::availability::scan(&originals);
        let before: std::collections::HashMap<AssetId, momento_database::safety::Availability> =
            originals.iter().map(|o| (o.id, o.status)).collect();
        let changed: Vec<(AssetId, momento_database::safety::Availability)> =
            now.iter().filter(|(id, a)| before.get(id) != Some(a)).cloned().collect();
        let reconnected = changed
            .iter()
            .filter(|(_, a)| *a == momento_database::safety::Availability::Ok)
            .count() as u64;
        if !changed.is_empty() {
            self.catalog().set_availability(&changed)?;
            if reconnected > 0 {
                let pending = self.catalog().pending_thumbnail_ids()?;
                self.enqueue_thumbnails(pending);
            }
        }
        let (ok, offline, missing) = self.catalog().availability_counts()?;
        let s = AvailabilitySummary {
            ok,
            offline,
            missing,
            changed: changed.len() as u64,
            reconnected,
        };
        if !changed.is_empty() {
            tracing::info!(
                offline,
                em_falta = missing,
                reconectadas = reconnected,
                "disponibilidade dos originais mudou"
            );
            self.events.availability(&s);
        }
        Ok(s)
    }

    fn start_maintenance(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let stop = self.stop_maintenance.clone();
        let h = std::thread::Builder::new().name("manutencao".into()).spawn(move || {
            let mut last_check = std::time::Instant::now() - AVAILABILITY_EVERY;
            let mut last_checkpoint = std::time::Instant::now();
            loop {
                {
                    let (m, cv) = &*stop;
                    let g = m.lock().unwrap_or_else(|e| e.into_inner());
                    let (g, _) = cv
                        .wait_timeout_while(g, std::time::Duration::from_secs(5), |s| !*s)
                        .unwrap_or_else(|e| e.into_inner());
                    if *g {
                        return;
                    }
                }
                let Some(s) = weak.upgrade() else { return };
                if last_check.elapsed() >= AVAILABILITY_EVERY {
                    last_check = std::time::Instant::now();
                    if let Err(e) = s.check_availability() {
                        tracing::warn!(erro = %e, "verificação dos originais falhou");
                    }
                }
                if last_checkpoint.elapsed() >= CHECKPOINT_EVERY {
                    last_checkpoint = std::time::Instant::now();
                    if let Ok(c) = s.open_catalog() {
                        let _ = c.checkpoint();
                    }
                    let minutes = s.backup_minutes();
                    if let Ok(c) = s.open_catalog() {
                        if let Err(e) = backup_if_due(&c, &s.paths, minutes) {
                            tracing::warn!(erro = %e, "cópia de segurança automática falhou");
                        }
                    }
                }
            }
        });
        if let Ok(h) = h {
            *self.maintenance.lock().unwrap_or_else(|e| e.into_inner()) = Some(h);
        }
    }

    fn stop_maintenance(&self) {
        {
            let (m, cv) = &*self.stop_maintenance;
            *m.lock().unwrap_or_else(|e| e.into_inner()) = true;
            cv.notify_all();
        }
        if let Some(h) = self.maintenance.lock().unwrap_or_else(|e| e.into_inner()).take() {
            if h.thread().id() != std::thread::current().id() {
                let _ = h.join();
            }
        }
    }
}

/// Resultado de uma verificação dos originais.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilitySummary {
    pub ok: u64,
    pub offline: u64,
    pub missing: u64,
    pub changed: u64,
    pub reconnected: u64,
}

impl Drop for ProjectSession {
    fn drop(&mut self) {
        if let Some(r) = self.import.get_mut().unwrap_or_else(|e| e.into_inner()).as_ref() {
            r.control.suspend();
        }
    }
}
