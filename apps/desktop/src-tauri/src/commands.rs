use crate::state::AppState;
use momento_core_types::{AssetId, CoreError, ErrorPayload};
use momento_database::analysis::{AnalysisDetails, GroupRow, RecommendationBrief};
use momento_database::assets::{PhotoDetails, PhotoRow};
use momento_database::clock::{CameraOffset, CameraRow};
use momento_database::library::RecentProject;
use momento_database::moments::{AssetMoment, MomentRow};
use momento_database::people::{FaceRef, PersonRow};
use momento_project_management::analysis::SelectionSummary;
use momento_project_management::auto_edit::{AutoEditProgress, AutoEditResult};
use momento_project_management::consistency::{Criterion, LookGroup};
use momento_project_management::local_edit::MaskRasterInfo;
use momento_project_management::preset_store::{ImportResult, PresetStore};
use momento_project_management::{
    AnalysisStatus, ImportRequest, NewProject, ProjectInfo, SelectionSettings,
};
use momento_project_management::{AutoOptions, FaceRegions};
use momento_project_management::{Component, MatchOptions, Preset};
use momento_project_management::{
    DevelopSettings, EditView, OffsetSuggestion, OrganizeSummary, RenderQuality,
};
use serde::Serialize;
use std::path::Path;
use tauri::State;

type CmdResult<T> = Result<T, ErrorPayload>;

fn err(e: CoreError) -> ErrorPayload {
    tracing::warn!(erro = %e, "comando falhou");
    ErrorPayload::from(&e)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub app_version: String,
    pub core_version: String,
    pub catalog_schema_version: u32,
    pub os: String,
    pub arch: String,
}

pub fn app_info_impl() -> AppInfo {
    AppInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        core_version: momento_core_types::CORE_VERSION.to_string(),
        catalog_schema_version: momento_database::SCHEMA_VERSION,
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    app_info_impl()
}

#[tauri::command]
pub fn event_types() -> Vec<&'static str> {
    momento_database::project::EVENT_TYPES.to_vec()
}

/// Projeto aberto + token para os URLs de imagens.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedProject {
    #[serde(flatten)]
    pub info: ProjectInfo,
    pub image_token: u64,
}

#[tauri::command]
pub fn list_recent_projects(state: State<'_, AppState>) -> CmdResult<Vec<RecentProject>> {
    state.library().recent(50).map_err(err)
}

#[tauri::command]
pub fn forget_recent_project(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    state.library().forget(&path).map_err(err)
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    input: NewProject,
) -> CmdResult<OpenedProject> {
    let paths = momento_project_management::create_project(&input).map_err(err)?;
    tracing::info!(projeto = %paths.root.display(), "projeto criado");
    let (session, token) = state.open(&paths.root).map_err(err)?;
    Ok(OpenedProject { info: session.info().map_err(err)?, image_token: token })
}

#[tauri::command]
pub async fn open_project(state: State<'_, AppState>, path: String) -> CmdResult<OpenedProject> {
    let (session, token) = state.open(Path::new(&path)).map_err(err)?;
    Ok(OpenedProject { info: session.info().map_err(err)?, image_token: token })
}

#[tauri::command]
pub async fn close_project(state: State<'_, AppState>) -> CmdResult<()> {
    state.close_project();
    Ok(())
}

#[tauri::command]
pub fn current_project(state: State<'_, AppState>) -> CmdResult<Option<OpenedProject>> {
    let Ok(s) = state.session() else { return Ok(None) };
    let token = state.token().unwrap_or(0);
    Ok(Some(OpenedProject { info: s.info().map_err(err)?, image_token: token }))
}

#[tauri::command]
pub async fn list_photos(state: State<'_, AppState>) -> CmdResult<Vec<PhotoRow>> {
    state.session().and_then(|s| s.list_photos()).map_err(err)
}

#[tauri::command]
pub fn photo_details(state: State<'_, AppState>, id: AssetId) -> CmdResult<Option<PhotoDetails>> {
    state.session().and_then(|s| s.photo_details(id)).map_err(err)
}

#[tauri::command]
pub fn set_stars(state: State<'_, AppState>, ids: Vec<AssetId>, stars: u8) -> CmdResult<usize> {
    state.session().and_then(|s| s.set_stars(&ids, stars)).map_err(err)
}

#[tauri::command]
pub fn set_flag(state: State<'_, AppState>, ids: Vec<AssetId>, flag: i8) -> CmdResult<usize> {
    state.session().and_then(|s| s.set_flag(&ids, flag)).map_err(err)
}

#[tauri::command]
pub fn set_color(
    state: State<'_, AppState>,
    ids: Vec<AssetId>,
    color: Option<String>,
) -> CmdResult<usize> {
    state.session().and_then(|s| s.set_color(&ids, color.as_deref())).map_err(err)
}

#[tauri::command]
pub fn start_import(
    state: State<'_, AppState>,
    request: ImportRequest,
    safety: Option<momento_project_management::ImportSafety>,
) -> CmdResult<i64> {
    let s = state.session().map_err(err)?;
    s.start_import_with(&request, &safety.unwrap_or_default()).map_err(err)
}

#[tauri::command]
pub fn cancel_import(state: State<'_, AppState>) -> CmdResult<()> {
    state.session().map(|s| s.cancel_import()).map_err(err)
}

#[tauri::command]
pub fn prioritize_thumbnails(state: State<'_, AppState>, ids: Vec<AssetId>) -> CmdResult<()> {
    state.session().map(|s| s.prioritize_thumbnails(ids)).map_err(err)
}

#[tauri::command]
pub fn save_ui_state(state: State<'_, AppState>, json: String) -> CmdResult<()> {
    state.session().and_then(|s| s.save_ui_state(&json)).map_err(err)
}

// ------------------------------------------------------------------ seleção assistida

#[tauri::command]
pub fn start_analysis(state: State<'_, AppState>, reanalyze: bool) -> CmdResult<u64> {
    state.session().and_then(|s| s.start_analysis(reanalyze)).map_err(err)
}

#[tauri::command]
pub fn cancel_analysis(state: State<'_, AppState>) -> CmdResult<()> {
    state.session().map(|s| s.cancel_analysis()).map_err(err)
}

#[tauri::command]
pub fn analysis_status(state: State<'_, AppState>) -> CmdResult<AnalysisStatus> {
    state.session().and_then(|s| s.analysis_status()).map_err(err)
}

#[tauri::command]
pub async fn set_selection_settings(
    state: State<'_, AppState>,
    settings: SelectionSettings,
) -> CmdResult<SelectionSummary> {
    state.session().and_then(|s| s.set_selection_settings(settings)).map_err(err)
}

#[tauri::command]
pub async fn recompute_selection(state: State<'_, AppState>) -> CmdResult<SelectionSummary> {
    state.session().and_then(|s| s.recompute_selection()).map_err(err)
}

#[tauri::command]
pub async fn list_recommendations(
    state: State<'_, AppState>,
) -> CmdResult<Vec<RecommendationBrief>> {
    state.session().and_then(|s| s.list_recommendations()).map_err(err)
}

#[tauri::command]
pub async fn list_groups(state: State<'_, AppState>) -> CmdResult<Vec<GroupRow>> {
    state.session().and_then(|s| s.list_groups()).map_err(err)
}

#[tauri::command]
pub fn analysis_details(state: State<'_, AppState>, id: AssetId) -> CmdResult<AnalysisDetails> {
    state.session().and_then(|s| s.analysis_details(id)).map_err(err)
}

#[tauri::command]
pub fn accept_recommendations(state: State<'_, AppState>, ids: Vec<AssetId>) -> CmdResult<usize> {
    state.session().and_then(|s| s.accept_recommendations(&ids)).map_err(err)
}

#[tauri::command]
pub fn accept_group(state: State<'_, AppState>, group: i64) -> CmdResult<usize> {
    state.session().and_then(|s| s.accept_group(group)).map_err(err)
}

// ------------------------------------------------------------------ organização (Fase 4)

#[tauri::command]
pub async fn rebuild_organization(state: State<'_, AppState>) -> CmdResult<OrganizeSummary> {
    state.session().map(|s| s.rebuild()).map_err(err)
}

#[tauri::command]
pub async fn list_persons(state: State<'_, AppState>) -> CmdResult<Vec<PersonRow>> {
    state.session().and_then(|s| s.list_persons()).map_err(err)
}

#[tauri::command]
pub async fn person_faces(
    state: State<'_, AppState>,
    person: i64,
    limit: u32,
) -> CmdResult<Vec<FaceRef>> {
    state.session().and_then(|s| s.person_faces(person, limit)).map_err(err)
}

#[tauri::command]
pub async fn photo_persons(state: State<'_, AppState>) -> CmdResult<Vec<(AssetId, i64)>> {
    state.session().and_then(|s| s.photo_persons()).map_err(err)
}

#[tauri::command]
pub async fn update_person(
    state: State<'_, AppState>,
    id: i64,
    name: Option<String>,
    role: Option<String>,
    hidden: bool,
) -> CmdResult<()> {
    state
        .session()
        .and_then(|s| s.update_person(id, name.as_deref(), role.as_deref(), hidden))
        .map_err(err)
}

#[tauri::command]
pub async fn merge_persons(state: State<'_, AppState>, from: i64, into: i64) -> CmdResult<()> {
    state.session().and_then(|s| s.merge_persons(from, into)).map_err(err)
}

#[tauri::command]
pub async fn exclude_faces(state: State<'_, AppState>, faces: Vec<i64>) -> CmdResult<usize> {
    state.session().and_then(|s| s.exclude_faces(&faces)).map_err(err)
}

#[tauri::command]
pub async fn recluster_people(state: State<'_, AppState>) -> CmdResult<u64> {
    state.session().and_then(|s| s.recluster_people()).map_err(err)
}

#[tauri::command]
pub async fn clear_people(state: State<'_, AppState>) -> CmdResult<()> {
    state.session().and_then(|s| s.clear_people()).map_err(err)
}

/// Erro da interface (exceção JavaScript, promessa rejeitada sem tratamento, falha de um
/// componente React) → registo da aplicação (anonimizado pelo escritor dos registos).
#[tauri::command]
pub fn log_ui_error(kind: String, message: String, stack: Option<String>) {
    let cut = |s: &str, n: usize| s.chars().take(n).collect::<String>();
    tracing::error!(
        tipo = %cut(&kind, 40),
        pilha = %cut(stack.as_deref().unwrap_or(""), 2000),
        "erro na interface: {}",
        cut(&message, 500)
    );
}

/// Classificações da seleção assistida para o Lightroom (`.xmp` ao lado dos RAW).
#[tauri::command]
pub async fn export_lightroom_ratings(
    state: State<'_, AppState>,
    options: momento_project_management::lightroom::LightroomOptions,
) -> CmdResult<momento_project_management::lightroom::LightroomReport> {
    state.session().and_then(|s| s.export_lightroom_ratings(&options)).map_err(err)
}

/// Temperatura (K) do balanço da câmara de um RAW, para mostrar a temperatura em Kelvin.
#[tauri::command]
pub async fn as_shot_kelvin(state: State<'_, AppState>, key: i64) -> CmdResult<Option<f32>> {
    state.session().and_then(|s| s.as_shot_kelvin(key)).map_err(err)
}

/// Privacidade/espaço: apaga a cache regenerável do projeto aberto.
#[tauri::command]
pub async fn clear_cache(
    state: State<'_, AppState>,
) -> CmdResult<momento_project_management::cache::Cleared> {
    state.session().and_then(|s| s.clear_cache()).map_err(err)
}

/// Privacidade: fecha o projeto (se for o aberto), move a pasta do projeto para o Lixo e
/// retira-o dos recentes. Os originais nunca são tocados (ver `safety::delete_project`).
#[tauri::command]
pub async fn delete_project(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let target = Path::new(&path);
    let is_open = state.session().ok().is_some_and(|s| {
        s.paths.root == target || s.paths.root.canonicalize().ok() == target.canonicalize().ok()
    });
    if is_open {
        state.close_project();
    }
    momento_project_management::safety::delete_project(target).map_err(err)?;
    let _ = state.library().forget(&path);
    Ok(())
}

#[tauri::command]
pub async fn list_moments(state: State<'_, AppState>) -> CmdResult<Vec<MomentRow>> {
    state.session().and_then(|s| s.list_moments()).map_err(err)
}

#[tauri::command]
pub async fn asset_moments(state: State<'_, AppState>) -> CmdResult<Vec<AssetMoment>> {
    state.session().and_then(|s| s.asset_moments()).map_err(err)
}

#[tauri::command]
pub async fn create_moment(
    state: State<'_, AppState>,
    name: String,
    after: Option<i64>,
) -> CmdResult<i64> {
    state.session().and_then(|s| s.create_moment(&name, after)).map_err(err)
}

#[tauri::command]
pub async fn rename_moment(state: State<'_, AppState>, id: i64, name: String) -> CmdResult<()> {
    state.session().and_then(|s| s.rename_moment(id, &name)).map_err(err)
}

#[tauri::command]
pub async fn delete_moment(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    state.session().and_then(|s| s.delete_moment(id)).map_err(err)
}

#[tauri::command]
pub async fn assign_moment(
    state: State<'_, AppState>,
    ids: Vec<AssetId>,
    moment: Option<i64>,
) -> CmdResult<usize> {
    state.session().and_then(|s| s.assign_moment(&ids, moment)).map_err(err)
}

#[tauri::command]
pub async fn reset_moments(state: State<'_, AppState>, ids: Vec<AssetId>) -> CmdResult<usize> {
    state.session().and_then(|s| s.reset_moments(&ids)).map_err(err)
}

#[tauri::command]
pub async fn reclassify_moments(state: State<'_, AppState>) -> CmdResult<(u64, u64)> {
    state.session().and_then(|s| s.reclassify_moments()).map_err(err)
}

#[tauri::command]
pub async fn list_cameras(state: State<'_, AppState>) -> CmdResult<Vec<CameraRow>> {
    state.session().and_then(|s| s.list_cameras()).map_err(err)
}

#[tauri::command]
pub async fn suggest_offsets(
    state: State<'_, AppState>,
    reference: String,
) -> CmdResult<Vec<OffsetSuggestion>> {
    state.session().and_then(|s| s.suggest_offsets(&reference)).map_err(err)
}

#[tauri::command]
pub async fn set_camera_offsets(
    state: State<'_, AppState>,
    offsets: Vec<CameraOffset>,
) -> CmdResult<OrganizeSummary> {
    state.session().and_then(|s| s.set_camera_offsets(&offsets)).map_err(err)
}

// ------------------------------------------------------------------ edição (Fase 5)

#[tauri::command]
pub async fn edit_view(state: State<'_, AppState>, key: i64) -> CmdResult<EditView> {
    state.session().and_then(|s| s.edit_view(key)).map_err(err)
}

/// Frame binário (ArrayBuffer na interface): u32 largura, u32 altura, histograma 3×256 u32,
/// RGBA. `quality`: "fast" durante a interação, "final" depois de parar.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn render_edit(
    state: State<'_, AppState>,
    key: i64,
    settings: DevelopSettings,
    max_width: u32,
    max_height: u32,
    quality: String,
    uncropped: bool,
    overlay: Option<usize>,
) -> CmdResult<tauri::ipc::Response> {
    let q = if quality == "fast" { RenderQuality::Fast } else { RenderQuality::Final };
    let bytes = state
        .session()
        .and_then(|s| {
            s.render_edit_overlay(key, &settings, max_width, max_height, q, uncropped, overlay)
        })
        .map_err(err)?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn commit_edit(
    state: State<'_, AppState>,
    key: i64,
    label: String,
    settings: DevelopSettings,
) -> CmdResult<EditView> {
    state.session().and_then(|s| s.commit_edit(key, &label, &settings)).map_err(err)
}

#[tauri::command]
pub async fn undo_edit(state: State<'_, AppState>, key: i64) -> CmdResult<EditView> {
    state.session().and_then(|s| s.undo_edit(key)).map_err(err)
}

#[tauri::command]
pub async fn redo_edit(state: State<'_, AppState>, key: i64) -> CmdResult<EditView> {
    state.session().and_then(|s| s.redo_edit(key)).map_err(err)
}

#[tauri::command]
pub async fn goto_history(state: State<'_, AppState>, key: i64, seq: i64) -> CmdResult<EditView> {
    state.session().and_then(|s| s.goto_history(key, seq)).map_err(err)
}

#[tauri::command]
pub async fn reset_edit(state: State<'_, AppState>, key: i64) -> CmdResult<EditView> {
    state.session().and_then(|s| s.reset_edit(key)).map_err(err)
}

#[tauri::command]
pub async fn create_snapshot(
    state: State<'_, AppState>,
    key: i64,
    name: String,
) -> CmdResult<EditView> {
    state.session().and_then(|s| s.create_snapshot(key, &name)).map_err(err)
}

#[tauri::command]
pub async fn apply_snapshot(state: State<'_, AppState>, key: i64, id: i64) -> CmdResult<EditView> {
    state.session().and_then(|s| s.apply_snapshot(key, id)).map_err(err)
}

#[tauri::command]
pub async fn delete_snapshot(state: State<'_, AppState>, key: i64, id: i64) -> CmdResult<EditView> {
    state.session().and_then(|s| s.delete_snapshot(key, id)).map_err(err)
}

#[tauri::command]
pub async fn create_virtual_copy(state: State<'_, AppState>, key: i64) -> CmdResult<i64> {
    state.session().and_then(|s| s.create_virtual_copy(key)).map_err(err)
}

#[tauri::command]
pub async fn delete_virtual_copy(state: State<'_, AppState>, key: i64) -> CmdResult<()> {
    state.session().and_then(|s| s.delete_virtual_copy(key)).map_err(err)
}

#[tauri::command]
pub async fn paste_settings(
    state: State<'_, AppState>,
    keys: Vec<i64>,
    settings: DevelopSettings,
    blocks: Vec<String>,
) -> CmdResult<usize> {
    state.session().and_then(|s| s.paste_settings(&keys, &settings, &blocks)).map_err(err)
}

#[tauri::command]
pub async fn pick_white_balance(
    state: State<'_, AppState>,
    key: i64,
    settings: DevelopSettings,
    u: f32,
    v: f32,
) -> CmdResult<Option<(f32, f32)>> {
    state.session().and_then(|s| s.pick_white_balance(key, &settings, u, v)).map_err(err)
}

#[tauri::command]
pub async fn auto_straighten(
    state: State<'_, AppState>,
    key: i64,
    settings: DevelopSettings,
) -> CmdResult<Option<f32>> {
    state.session().and_then(|s| s.auto_straighten(key, &settings)).map_err(err)
}

#[tauri::command]
pub async fn apply_aspect(
    state: State<'_, AppState>,
    keys: Vec<i64>,
    aspect: String,
) -> CmdResult<usize> {
    state.session().and_then(|s| s.apply_aspect(&keys, &aspect)).map_err(err)
}

#[tauri::command]
pub async fn upright(
    state: State<'_, AppState>,
    key: i64,
    settings: DevelopSettings,
    mode: String,
) -> CmdResult<Option<momento_project_management::UprightOutcome>> {
    state.session().and_then(|s| s.upright(key, &settings, &mode)).map_err(err)
}

// ---------------------------------------------------------------- edição automática (Fase 6)

#[tauri::command]
pub async fn auto_edit_photo(
    state: State<'_, AppState>,
    key: i64,
    options: AutoOptions,
) -> CmdResult<AutoEditResult> {
    state.session().and_then(|s| s.auto_edit_photo(key, &options)).map_err(err)
}

#[tauri::command]
pub async fn start_auto_edit(
    state: State<'_, AppState>,
    keys: Vec<i64>,
    options: AutoOptions,
) -> CmdResult<u64> {
    state.session().and_then(|s| s.start_auto_edit(keys, options)).map_err(err)
}

#[tauri::command]
pub async fn auto_edit_status(state: State<'_, AppState>) -> CmdResult<AutoEditProgress> {
    state.session().map(|s| s.auto_edit_status()).map_err(err)
}

#[tauri::command]
pub async fn pause_auto_edit(state: State<'_, AppState>) -> CmdResult<()> {
    state.session().map(|s| s.pause_auto_edit()).map_err(err)
}

#[tauri::command]
pub async fn resume_auto_edit(state: State<'_, AppState>) -> CmdResult<()> {
    state.session().map(|s| s.resume_auto_edit()).map_err(err)
}

#[tauri::command]
pub async fn cancel_auto_edit(state: State<'_, AppState>) -> CmdResult<()> {
    state.session().map(|s| s.cancel_auto_edit()).map_err(err)
}

#[tauri::command]
pub async fn ai_denoise_available(state: State<'_, AppState>) -> CmdResult<bool> {
    state.session().map(|s| s.ai_denoise_available()).map_err(err)
}

// ---------------------------------------------------------------- presets e consistência (Fase 7)

fn with_store<T>(
    state: &AppState,
    f: impl FnOnce(&PresetStore) -> momento_core_types::CoreResult<T>,
) -> CmdResult<T> {
    let lib = state.library();
    let store = PresetStore { lib: &lib, lut_dir: state.lut_dir.clone() };
    f(&store).map_err(err)
}

#[tauri::command]
pub async fn list_presets(state: State<'_, AppState>) -> CmdResult<Vec<Preset>> {
    with_store(&state, |s| s.list())
}

#[tauri::command]
pub async fn create_preset(
    state: State<'_, AppState>,
    name: String,
    group: String,
    components: Vec<Component>,
    settings: DevelopSettings,
) -> CmdResult<Preset> {
    with_store(&state, |s| s.create(&name, &group, &components, &settings))
}

#[tauri::command]
pub async fn rename_preset(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CmdResult<Preset> {
    with_store(&state, |s| s.rename(&id, &name))
}

#[tauri::command]
pub async fn move_preset(
    state: State<'_, AppState>,
    id: String,
    group: String,
) -> CmdResult<Preset> {
    with_store(&state, |s| s.move_to(&id, &group, None))
}

#[tauri::command]
pub async fn reorder_presets(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<()> {
    with_store(&state, |s| s.reorder(&ids))
}

#[tauri::command]
pub async fn duplicate_preset(
    state: State<'_, AppState>,
    id: String,
    suffix: String,
) -> CmdResult<Preset> {
    with_store(&state, |s| s.duplicate(&id, &suffix))
}

#[tauri::command]
pub async fn delete_preset(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    with_store(&state, |s| s.delete(&id))
}

#[tauri::command]
pub async fn import_preset_file(
    state: State<'_, AppState>,
    path: String,
) -> CmdResult<ImportResult> {
    with_store(&state, |s| s.import(Path::new(&path)))
}

#[tauri::command]
pub async fn export_presets(
    state: State<'_, AppState>,
    ids: Vec<String>,
    path: String,
) -> CmdResult<usize> {
    with_store(&state, |s| s.export(&ids, Path::new(&path)))
}

/// Ajustes que resultariam de aplicar um preset (pré-visualização sem gravar).
#[tauri::command]
pub async fn preset_preview_settings(
    state: State<'_, AppState>,
    settings: DevelopSettings,
    preset_id: String,
    amount: f32,
) -> CmdResult<DevelopSettings> {
    let p = with_store(&state, |s| s.get(&preset_id))?;
    Ok(momento_project_management::apply_preset_settings(
        &settings,
        &p.settings,
        &p.components,
        amount,
    ))
}

/// Aplica um preset a fotografias (uma, seleção, momento ou casamento completo).
#[tauri::command]
pub async fn apply_preset(
    state: State<'_, AppState>,
    keys: Vec<i64>,
    preset_id: String,
    amount: f32,
) -> CmdResult<usize> {
    let p = with_store(&state, |s| s.get(&preset_id))?;
    state
        .session()
        .and_then(|s| s.apply_preset(&keys, &p.name, &p.settings, &p.components, amount))
        .map_err(err)
}

#[tauri::command]
pub async fn measure_consistency(state: State<'_, AppState>) -> CmdResult<usize> {
    state.session().and_then(|s| s.measure_consistency()).map_err(err)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupsResult {
    pub groups: Vec<LookGroup>,
    pub missing: usize,
}

#[tauri::command]
pub async fn consistency_groups(
    state: State<'_, AppState>,
    criterion: Criterion,
) -> CmdResult<GroupsResult> {
    state
        .session()
        .and_then(|s| s.consistency_groups(criterion))
        .map(|(groups, missing)| GroupsResult { groups, missing })
        .map_err(err)
}

#[tauri::command]
pub async fn start_sync(
    state: State<'_, AppState>,
    reference: i64,
    keys: Vec<i64>,
    options: MatchOptions,
) -> CmdResult<u64> {
    state.session().and_then(|s| s.start_sync(reference, keys, options)).map_err(err)
}

// ---------------------------------------------------------------- edição localizada (Fase 8)

#[tauri::command]
pub async fn create_mask_raster(
    state: State<'_, AppState>,
    key: i64,
    kind: String,
    point: Option<[f32; 2]>,
) -> CmdResult<MaskRasterInfo> {
    state.session().and_then(|s| s.create_mask_raster(key, &kind, point)).map_err(err)
}

#[tauri::command]
pub async fn detect_portrait(state: State<'_, AppState>, key: i64) -> CmdResult<Vec<FaceRegions>> {
    state.session().and_then(|s| s.detect_portrait(key)).map_err(err)
}

#[tauri::command]
pub async fn map_points(
    state: State<'_, AppState>,
    key: i64,
    settings: DevelopSettings,
    points: Vec<[f32; 2]>,
    to_source: bool,
    uncropped: bool,
) -> CmdResult<Vec<Option<[f32; 2]>>> {
    state
        .session()
        .and_then(|s| s.map_points(key, &settings, &points, to_source, uncropped))
        .map_err(err)
}

#[tauri::command]
pub async fn suggest_heal_source(
    state: State<'_, AppState>,
    key: i64,
    x: f32,
    y: f32,
    r: f32,
) -> CmdResult<[f32; 2]> {
    state.session().and_then(|s| s.suggest_heal_source(key, x, y, r)).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_versions() {
        let i = app_info_impl();
        assert!(!i.core_version.is_empty());
        assert_eq!(i.catalog_schema_version, momento_database::SCHEMA_VERSION);
        let json = serde_json::to_value(&i).unwrap();
        assert!(json.get("catalogSchemaVersion").is_some(), "camelCase para a interface");
    }
}

// ---------------------------------------------------------------- exportação (Fase 9)

use momento_export_engine::presets::Plan as ExportPlan;
use momento_export_engine::{Caps as ExportCaps, Format as ExportFormat, Recipe as ExportRecipe};
use momento_project_management::export::{ExportEstimate, ExportProgress, ExportRequest};
use momento_project_management::export_store::ExportStore;

fn with_export_store<T>(
    state: &AppState,
    f: impl FnOnce(&ExportStore) -> momento_core_types::CoreResult<T>,
) -> CmdResult<T> {
    let lib = state.library();
    f(&ExportStore { lib: &lib }).map_err(err)
}

/// O que cada formato permite (a interface só mostra opções que funcionam).
#[tauri::command]
pub async fn export_capabilities() -> CmdResult<Vec<ExportCaps>> {
    Ok(ExportFormat::ALL.iter().map(|f| f.caps()).collect())
}

#[tauri::command]
pub async fn export_recipes(state: State<'_, AppState>) -> CmdResult<Vec<ExportRecipe>> {
    with_export_store(&state, |s| s.recipes())
}

#[tauri::command]
pub async fn save_export_recipe(
    state: State<'_, AppState>,
    recipe: ExportRecipe,
) -> CmdResult<ExportRecipe> {
    with_export_store(&state, |s| s.save_recipe(&recipe))
}

#[tauri::command]
pub async fn delete_export_preset(state: State<'_, AppState>, id: String) -> CmdResult<bool> {
    with_export_store(&state, |s| s.delete(&id))
}

#[tauri::command]
pub async fn export_plans(state: State<'_, AppState>) -> CmdResult<Vec<ExportPlan>> {
    with_export_store(&state, |s| s.plans())
}

#[tauri::command]
pub async fn save_export_plan(
    state: State<'_, AppState>,
    plan: ExportPlan,
) -> CmdResult<ExportPlan> {
    with_export_store(&state, |s| s.save_plan(&plan))
}

/// Valida um perfil ICC (personalizado/laboratório) e devolve a sua descrição.
#[tauri::command]
pub async fn inspect_icc_profile(path: String) -> CmdResult<String> {
    let data = std::fs::read(&path).map_err(|e| err(momento_core_types::CoreError::Io(e)))?;
    momento_color_management::inspect_icc(&data).map_err(err)
}

#[tauri::command]
pub async fn estimate_export(
    state: State<'_, AppState>,
    request: ExportRequest,
) -> CmdResult<ExportEstimate> {
    let s = state.session().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || s.estimate_export(&request))
        .await
        .map_err(|e| err(momento_core_types::CoreError::Other(e.to_string())))?
        .map_err(err)
}

#[tauri::command]
pub async fn start_export(state: State<'_, AppState>, request: ExportRequest) -> CmdResult<u64> {
    state.session().and_then(|s| s.start_export(request)).map_err(err)
}

#[tauri::command]
pub async fn export_jobs(state: State<'_, AppState>) -> CmdResult<Vec<ExportProgress>> {
    state.session().map(|s| s.export_jobs()).map_err(err)
}

#[tauri::command]
pub async fn pause_export(state: State<'_, AppState>, id: u64, paused: bool) -> CmdResult<bool> {
    state.session().map(|s| s.pause_export(id, paused)).map_err(err)
}

#[tauri::command]
pub async fn cancel_export(state: State<'_, AppState>, id: u64) -> CmdResult<bool> {
    state.session().map(|s| s.cancel_export(id)).map_err(err)
}

#[tauri::command]
pub async fn clear_finished_exports(state: State<'_, AppState>) -> CmdResult<()> {
    state.session().map(|s| s.clear_finished_exports()).map_err(err)
}

/// Abre uma pasta de exportação no gestor de ficheiros do sistema.
#[tauri::command]
pub async fn reveal_folder(path: String) -> CmdResult<()> {
    let p = std::path::PathBuf::from(&path);
    if !p.is_dir() {
        return Err(err(momento_core_types::CoreError::Other(format!(
            "pasta inexistente: {path}"
        ))));
    }
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let cmd = "xdg-open";
    std::process::Command::new(cmd)
        .arg(&p)
        .spawn()
        .map(|_| ())
        .map_err(|e| err(momento_core_types::CoreError::Io(e)))
}

// ---------------------------------------------------------------- galeria (Fase 10)

use momento_project_management::gallery::{FavoritesImport, GalleryConfig, GalleryPreview};

#[tauri::command]
pub async fn gallery_config(state: State<'_, AppState>) -> CmdResult<GalleryConfig> {
    state.session().and_then(|s| s.gallery_config()).map_err(err)
}

#[tauri::command]
pub async fn save_gallery_config(
    state: State<'_, AppState>,
    config: GalleryConfig,
) -> CmdResult<()> {
    state.session().and_then(|s| s.save_gallery_config(&config)).map_err(err)
}

#[tauri::command]
pub async fn gallery_preview(
    state: State<'_, AppState>,
    config: GalleryConfig,
    moment_names: std::collections::HashMap<i64, String>,
) -> CmdResult<GalleryPreview> {
    let s = state.session().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || s.gallery_preview(&config, &moment_names))
        .await
        .map_err(|e| err(momento_core_types::CoreError::Other(e.to_string())))?
        .map_err(err)
}

#[tauri::command]
pub async fn start_gallery(
    state: State<'_, AppState>,
    config: GalleryConfig,
    moment_names: std::collections::HashMap<i64, String>,
    replace: bool,
) -> CmdResult<u64> {
    state.session().and_then(|s| s.start_gallery(config, moment_names, replace)).map_err(err)
}

#[tauri::command]
pub async fn set_gallery_hidden(
    state: State<'_, AppState>,
    keys: Vec<i64>,
    hidden: bool,
) -> CmdResult<GalleryConfig> {
    state.session().and_then(|s| s.set_gallery_hidden(&keys, hidden)).map_err(err)
}

#[tauri::command]
pub async fn import_client_favorites(
    state: State<'_, AppState>,
    text: String,
) -> CmdResult<FavoritesImport> {
    state.session().and_then(|s| s.import_client_favorites(&text)).map_err(err)
}

/// Abre a galeria gerada no navegador predefinido (só `index.html` de uma pasta de galeria
/// criada pelo Momento).
#[tauri::command]
pub async fn open_gallery(path: String) -> CmdResult<()> {
    let p = std::path::PathBuf::from(&path);
    // Só abre o que o Momento gerou: a galeria (pasta marcada) ou o PDF de um álbum.
    let gallery = p.file_name().is_some_and(|n| n == "index.html")
        && p.parent().is_some_and(|d| d.join(momento_gallery::site::MARKER).is_file());
    let album_pdf = p.is_file()
        && p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with("-impressao.pdf") || n.ends_with("-prova.pdf"));
    if !gallery && !album_pdf {
        return Err(err(momento_core_types::CoreError::Other(
            "não é uma galeria nem um álbum do Momento".into(),
        )));
    }
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&p).spawn().map(|_| ()).map_err(|e| err(momento_core_types::CoreError::Io(e)))
}

// ---------------------------------------------------------------- álbuns (Fase 11)

use momento_album::auto::AutoOptions as AlbumAutoOptions;
use momento_album::export::ExportKind as AlbumExportKind;
use momento_album::preflight::{Issue as AlbumIssue, PhotoInfo as AlbumPhotoInfo};
use momento_album::{Album, AlbumSpec, LabPreset};
use momento_project_management::album::{AlbumSummary, AutoReport, LabStore};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumFormat {
    name: &'static str,
    width_cm: f32,
    height_cm: f32,
}

#[tauri::command]
pub async fn album_formats() -> CmdResult<Vec<AlbumFormat>> {
    Ok(momento_album::FORMATS
        .iter()
        .map(|(n, w, h)| AlbumFormat { name: n, width_cm: *w, height_cm: *h })
        .collect())
}

#[tauri::command]
pub async fn album_templates(count: usize) -> CmdResult<Vec<String>> {
    Ok(momento_album::layouts::templates_for(count))
}

#[tauri::command]
pub async fn list_albums(state: State<'_, AppState>) -> CmdResult<Vec<AlbumSummary>> {
    state.session().and_then(|s| s.list_albums()).map_err(err)
}

#[tauri::command]
pub async fn get_album(state: State<'_, AppState>, id: String) -> CmdResult<Album> {
    state.session().and_then(|s| s.get_album(&id)).map_err(err)
}

#[tauri::command]
pub async fn create_album(
    state: State<'_, AppState>,
    name: String,
    spec: AlbumSpec,
) -> CmdResult<Album> {
    state.session().and_then(|s| s.create_album(&name, spec)).map_err(err)
}

#[tauri::command]
pub async fn save_album(state: State<'_, AppState>, album: Album) -> CmdResult<()> {
    state.session().and_then(|s| s.save_album(&album)).map_err(err)
}

#[tauri::command]
pub async fn delete_album(state: State<'_, AppState>, id: String) -> CmdResult<bool> {
    state.session().and_then(|s| s.delete_album(&id)).map_err(err)
}

#[tauri::command]
pub async fn auto_album(
    state: State<'_, AppState>,
    id: String,
    options: AlbumAutoOptions,
) -> CmdResult<AutoReport> {
    let s = state.session().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || s.auto_album(&id, options))
        .await
        .map_err(|e| err(momento_core_types::CoreError::Other(e.to_string())))?
        .map_err(err)
}

#[tauri::command]
pub async fn apply_album_layout(
    state: State<'_, AppState>,
    album: Album,
    spread: usize,
    template: String,
    margin: f32,
    spacing: f32,
    extra: Vec<i64>,
) -> CmdResult<Album> {
    state
        .session()
        .and_then(|s| s.apply_album_layout(&album, spread, &template, margin, spacing, &extra))
        .map_err(err)
}

#[tauri::command]
pub async fn album_preflight(
    state: State<'_, AppState>,
    album: Album,
) -> CmdResult<Vec<AlbumIssue>> {
    let s = state.session().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || s.album_preflight(&album))
        .await
        .map_err(|e| err(momento_core_types::CoreError::Other(e.to_string())))?
        .map_err(err)
}

#[tauri::command]
pub async fn album_photo_infos(state: State<'_, AppState>) -> CmdResult<Vec<AlbumPhotoInfo>> {
    let s = state.session().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || {
        s.album_photo_infos().map(|m| m.into_values().collect())
    })
    .await
    .map_err(|e| err(momento_core_types::CoreError::Other(e.to_string())))?
    .map_err(err)
}

#[tauri::command]
pub async fn start_album_export(
    state: State<'_, AppState>,
    id: String,
    kind: AlbumExportKind,
    destination: String,
) -> CmdResult<u64> {
    state.session().and_then(|s| s.start_album_export(&id, kind, &destination)).map_err(err)
}

#[tauri::command]
pub async fn lab_presets(state: State<'_, AppState>) -> CmdResult<Vec<LabPreset>> {
    let lib = state.library();
    LabStore { lib: &lib }.list().map_err(err)
}

#[tauri::command]
pub async fn save_lab_preset(
    state: State<'_, AppState>,
    preset: LabPreset,
) -> CmdResult<LabPreset> {
    let lib = state.library();
    LabStore { lib: &lib }.save(&preset).map_err(err)
}

#[tauri::command]
pub async fn delete_lab_preset(state: State<'_, AppState>, id: String) -> CmdResult<bool> {
    let lib = state.library();
    LabStore { lib: &lib }.delete(&id).map_err(err)
}

// ---------- Diagnóstico de desempenho (Fase 12) ----------

static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Instante do arranque do processo (para medir o tempo de lançamento).
pub fn mark_process_start() {
    PROCESS_START.get_or_init(std::time::Instant::now);
}

/// Modo de diagnóstico (`MOMENTO_PERF=1`): a interface mede FPS e tempos e regista-os.
#[tauri::command]
pub fn perf_mode() -> bool {
    std::env::var("MOMENTO_PERF").is_ok_and(|v| v == "1")
}

/// Regista uma medição da interface no log (só em modo de diagnóstico; sem dados pessoais).
#[tauri::command]
pub fn perf_log(event: String, data: serde_json::Value) {
    if !perf_mode() {
        return;
    }
    let since = PROCESS_START.get().map(|t| t.elapsed().as_millis()).unwrap_or(0);
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))?
                .split_whitespace()
                .nth(1)?
                .parse::<u64>()
                .ok()
        })
        .map(|kb| kb / 1024);
    tracing::info!(target: "perf", evento = %event, desde_arranque_ms = since as u64, rss_mb = ?rss, dados = %data, "medição");
}

// ---------------------------------------------------------------- segurança (Fase 13)

use momento_project_management::export::PendingExport;
use momento_project_management::originals::{RelinkReport, RemoveReport};
use momento_project_management::safety::{BackupInfo, OpenReport};
use momento_project_management::AvailabilitySummary;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyStatus {
    report: OpenReport,
    availability: AvailabilitySummary,
    backups: Vec<BackupInfo>,
    backup_minutes: u32,
    /// (quando, tipo, detalhe), mais recentes primeiro.
    events: Vec<(i64, String, String)>,
    pending_exports: Vec<PendingExport>,
}

#[tauri::command]
pub async fn safety_status(state: State<'_, AppState>) -> CmdResult<SafetyStatus> {
    let s = state.session().map_err(err)?;
    let availability = s.check_availability().map_err(err)?;
    Ok(SafetyStatus {
        report: s.open_report(),
        availability,
        backups: s.list_backups(),
        backup_minutes: s.backup_minutes(),
        events: s.safety_events(30).map_err(err)?,
        pending_exports: s.pending_exports().map_err(err)?,
    })
}

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>) -> CmdResult<BackupInfo> {
    state.session().and_then(|s| s.create_backup()).map_err(err)
}

#[tauri::command]
pub fn set_backup_minutes(state: State<'_, AppState>, minutes: u32) -> CmdResult<()> {
    state.session().and_then(|s| s.set_backup_minutes(minutes)).map_err(err)
}

#[tauri::command]
pub async fn validate_catalog(
    state: State<'_, AppState>,
) -> CmdResult<momento_database::safety::Validation> {
    state.session().and_then(|s| s.validate_catalog()).map_err(err)
}

#[tauri::command]
pub async fn export_catalog(state: State<'_, AppState>, dest: String) -> CmdResult<()> {
    state.session().and_then(|s| s.export_catalog(Path::new(&dest))).map_err(err)
}

/// Substitui o catálogo (versão ou catálogo importado): fecha, substitui e reabre.
async fn replace_and_reopen(state: &AppState, file: &str) -> CmdResult<OpenedProject> {
    let s = state.session().map_err(err)?;
    let root = s.paths.root.clone();
    let paths = s.paths.clone();
    drop(s);
    state.close_project();
    let replaced = momento_project_management::safety::replace_catalog(&paths, Path::new(file));
    // Reabre sempre (mesmo que a substituição tenha sido recusada).
    let (session, token) = state.open(&root).map_err(err)?;
    replaced.map_err(err)?;
    tracing::info!("catálogo substituído (restauro/importação)");
    Ok(OpenedProject { info: session.info().map_err(err)?, image_token: token })
}

#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, file: String) -> CmdResult<OpenedProject> {
    replace_and_reopen(&state, &file).await
}

#[tauri::command]
pub async fn import_catalog(state: State<'_, AppState>, file: String) -> CmdResult<OpenedProject> {
    replace_and_reopen(&state, &file).await
}

#[tauri::command]
pub async fn check_availability(state: State<'_, AppState>) -> CmdResult<AvailabilitySummary> {
    state.session().and_then(|s| s.check_availability()).map_err(err)
}

#[tauri::command]
pub async fn remove_from_catalog(
    state: State<'_, AppState>,
    keys: Vec<i64>,
    confirmed: bool,
) -> CmdResult<RemoveReport> {
    state.session().and_then(|s| s.remove_from_catalog(&keys, confirmed)).map_err(err)
}

#[tauri::command]
pub async fn move_to_trash(
    state: State<'_, AppState>,
    keys: Vec<i64>,
    confirmed: bool,
) -> CmdResult<RemoveReport> {
    state.session().and_then(|s| s.move_to_trash(&keys, confirmed)).map_err(err)
}

/// Mostra o original no gestor de ficheiros (selecionado, quando o sistema o permite).
#[tauri::command]
pub async fn reveal_original(state: State<'_, AppState>, key: i64) -> CmdResult<()> {
    let p = state.session().and_then(|s| s.original_path(key)).map_err(err)?;
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(&p).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let r = std::process::Command::new("xdg-open").arg(p.parent().unwrap_or(&p)).spawn();
    r.map(|_| ()).map_err(|e| err(momento_core_types::CoreError::Io(e)))
}

#[tauri::command]
pub async fn relink_file(
    state: State<'_, AppState>,
    key: i64,
    file: String,
) -> CmdResult<RelinkReport> {
    state.session().and_then(|s| s.relink_file(key, Path::new(&file))).map_err(err)
}

#[tauri::command]
pub async fn relink_folder(state: State<'_, AppState>, folder: String) -> CmdResult<RelinkReport> {
    state.session().and_then(|s| s.relink_folder(Path::new(&folder))).map_err(err)
}

#[tauri::command]
pub fn resume_export(state: State<'_, AppState>, id: String) -> CmdResult<u64> {
    state.session().and_then(|s| s.resume_export(&id)).map_err(err)
}

#[tauri::command]
pub fn discard_export(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.session().map(|s| s.discard_export(&id)).map_err(err)
}
