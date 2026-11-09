use crate::state::AppState;
use momento_core_types::{AssetId, CoreError, ErrorPayload};
use momento_database::analysis::{AnalysisDetails, GroupRow, RecommendationBrief};
use momento_database::assets::{PhotoDetails, PhotoRow};
use momento_database::clock::{CameraOffset, CameraRow};
use momento_database::library::RecentProject;
use momento_database::moments::{AssetMoment, MomentRow};
use momento_database::people::{FaceRef, PersonRow};
use momento_project_management::analysis::SelectionSummary;
use momento_project_management::{
    AnalysisStatus, ImportRequest, NewProject, ProjectInfo, SelectionSettings,
};
use momento_project_management::{OffsetSuggestion, OrganizeSummary};
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
