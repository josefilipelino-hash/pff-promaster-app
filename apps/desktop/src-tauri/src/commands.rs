use crate::state::AppState;
use momento_core_types::{AssetId, CoreError, ErrorPayload};
use momento_database::assets::{PhotoDetails, PhotoRow};
use momento_database::library::RecentProject;
use momento_project_management::{ImportRequest, NewProject, ProjectInfo};
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
