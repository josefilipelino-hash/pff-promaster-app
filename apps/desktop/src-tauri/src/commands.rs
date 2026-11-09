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
pub fn start_import(state: State<'_, AppState>, request: ImportRequest) -> CmdResult<i64> {
    let s = state.session().map_err(err)?;
    s.start_import(&request).map_err(err)
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
