//! Shell Tauri. Este crate só faz a ponte entre a interface e o núcleo: não contém
//! lógica de domínio. Cada comando converte erros para `ErrorPayload` (chave de tradução).

use serde::Serialize;

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

/// Informação de diagnóstico mostrada no ecrã inicial.
#[tauri::command]
fn app_info() -> AppInfo {
    app_info_impl()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a aplicação");
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
