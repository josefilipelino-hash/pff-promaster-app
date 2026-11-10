//! Shell Tauri: ponte fina entre a interface e o núcleo. Sem lógica de domínio.
//! Cada comando converte erros em `ErrorPayload` (chave de tradução + detalhe).

mod commands;
mod protocol;
mod state;

use state::AppState;
use tauri::Manager;

pub use commands::{app_info_impl, AppInfo};

fn init_logging(app: &tauri::App) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = app.path().app_log_dir().ok().and_then(|dir| {
        std::fs::create_dir_all(&dir).ok()?;
        let (writer, guard) =
            tracing_appender::non_blocking(tracing_appender::rolling::daily(dir, "momento.log"));
        state::LOG_GUARD.set(guard).ok();
        Some(fmt::layer().with_ansi(false).with_writer(writer))
    });
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(file_layer)
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("momento", protocol::handle)
        .setup(|app| {
            init_logging(app);
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            // LUT importadas (presets) — partilhadas por todos os projetos.
            momento_project_management::set_lut_search_paths(vec![data_dir.join("luts")]);
            let state = AppState::new(&data_dir.join("library.db"), app.handle().clone())
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            app.manage(state);
            // Modelos de IA locais incluídos como recursos da aplicação instalada.
            if let Ok(res) = app.path().resource_dir() {
                momento_project_management::set_model_search_paths(vec![res.join("models")]);
            }
            tracing::info!(versao = momento_core_types::CORE_VERSION, dados = %data_dir.display(), "aplicação iniciada");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::export_capabilities,
            commands::export_recipes,
            commands::save_export_recipe,
            commands::delete_export_preset,
            commands::export_plans,
            commands::save_export_plan,
            commands::inspect_icc_profile,
            commands::estimate_export,
            commands::start_export,
            commands::export_jobs,
            commands::pause_export,
            commands::cancel_export,
            commands::clear_finished_exports,
            commands::reveal_folder,
            commands::gallery_config,
            commands::save_gallery_config,
            commands::gallery_preview,
            commands::start_gallery,
            commands::set_gallery_hidden,
            commands::import_client_favorites,
            commands::open_gallery,
            commands::album_formats,
            commands::album_templates,
            commands::list_albums,
            commands::get_album,
            commands::create_album,
            commands::save_album,
            commands::delete_album,
            commands::auto_album,
            commands::apply_album_layout,
            commands::album_preflight,
            commands::album_photo_infos,
            commands::start_album_export,
            commands::lab_presets,
            commands::save_lab_preset,
            commands::delete_lab_preset,
            commands::event_types,
            commands::list_recent_projects,
            commands::forget_recent_project,
            commands::create_project,
            commands::open_project,
            commands::close_project,
            commands::current_project,
            commands::list_photos,
            commands::photo_details,
            commands::set_stars,
            commands::set_flag,
            commands::set_color,
            commands::start_import,
            commands::cancel_import,
            commands::prioritize_thumbnails,
            commands::save_ui_state,
            commands::start_analysis,
            commands::cancel_analysis,
            commands::analysis_status,
            commands::set_selection_settings,
            commands::recompute_selection,
            commands::list_recommendations,
            commands::list_groups,
            commands::analysis_details,
            commands::accept_recommendations,
                        commands::accept_group,
            commands::rebuild_organization,
            commands::list_persons,
            commands::person_faces,
            commands::photo_persons,
            commands::update_person,
            commands::merge_persons,
            commands::exclude_faces,
            commands::recluster_people,
            commands::clear_people,
            commands::list_moments,
            commands::asset_moments,
            commands::create_moment,
            commands::rename_moment,
            commands::delete_moment,
            commands::assign_moment,
            commands::reset_moments,
            commands::reclassify_moments,
            commands::list_cameras,
            commands::suggest_offsets,
                        commands::set_camera_offsets,
            commands::edit_view,
            commands::render_edit,
            commands::commit_edit,
            commands::undo_edit,
            commands::redo_edit,
            commands::goto_history,
            commands::reset_edit,
            commands::create_snapshot,
            commands::apply_snapshot,
            commands::delete_snapshot,
            commands::create_virtual_copy,
            commands::delete_virtual_copy,
            commands::paste_settings,
            commands::pick_white_balance,
            commands::auto_straighten,
            commands::auto_edit_photo,
            commands::start_auto_edit,
            commands::auto_edit_status,
            commands::pause_auto_edit,
            commands::resume_auto_edit,
            commands::cancel_auto_edit,
            commands::ai_denoise_available,
            commands::list_presets,
            commands::create_preset,
            commands::rename_preset,
            commands::move_preset,
            commands::reorder_presets,
            commands::duplicate_preset,
            commands::delete_preset,
            commands::import_preset_file,
            commands::export_presets,
            commands::preset_preview_settings,
            commands::apply_preset,
            commands::measure_consistency,
            commands::consistency_groups,
            commands::start_sync,
            commands::create_mask_raster,
            commands::detect_portrait,
            commands::map_points,
            commands::suggest_heal_source,
        ])
        .build(tauri::generate_context!())
        .expect("falha ao iniciar a aplicação");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            // Suspende a importação (retomada na próxima abertura) e para as miniaturas.
            handle.state::<AppState>().close_project();
            tracing::info!("aplicação terminada");
        }
    });
}
