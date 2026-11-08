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
            let state = AppState::new(&data_dir.join("library.db"), app.handle().clone())
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            app.manage(state);
            tracing::info!(versao = momento_core_types::CORE_VERSION, dados = %data_dir.display(), "aplicação iniciada");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
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
