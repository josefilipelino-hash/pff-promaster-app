//! # momento-core-types
//!
//! Tipos partilhados por todos os módulos do núcleo. Este crate não pode depender de
//! nenhum outro crate do workspace, para evitar ciclos.

pub mod ids;
pub mod recipe;

pub use ids::*;

use serde::{Deserialize, Serialize};

/// Versão do núcleo, exposta à interface e gravada nos catálogos.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Erro comum do núcleo. Cada módulo pode ter erros próprios, mas converte-os para
/// este tipo na fronteira com a aplicação (comandos Tauri).
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("erro de entrada/saída: {0}")]
    Io(#[from] std::io::Error),
    #[error("erro de base de dados: {0}")]
    Database(String),
    #[error("formato não suportado: {0}")]
    UnsupportedFormat(String),
    #[error("ficheiro inválido ou corrompido: {0}")]
    Corrupt(String),
    #[error("versão do catálogo ({found}) mais recente do que a suportada ({supported})")]
    CatalogTooNew { found: u32, supported: u32 },
    #[error("{0}")]
    Other(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

/// Erro serializável enviado para a interface. A interface traduz `code`; `detail` é
/// apenas para diagnóstico e nunca contém conteúdo de imagens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorPayload {
    pub code: String,
    pub detail: String,
}

impl From<&CoreError> for ErrorPayload {
    fn from(e: &CoreError) -> Self {
        let code = match e {
            CoreError::Io(_) => "error.io",
            CoreError::Database(_) => "error.database",
            CoreError::UnsupportedFormat(_) => "error.unsupportedFormat",
            CoreError::Corrupt(_) => "error.corrupt",
            CoreError::CatalogTooNew { .. } => "error.catalogTooNew",
            CoreError::Other(_) => "error.other",
        };
        ErrorPayload { code: code.into(), detail: e.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_payload_uses_translation_key() {
        let e = CoreError::UnsupportedFormat("xyz".into());
        let p = ErrorPayload::from(&e);
        assert_eq!(p.code, "error.unsupportedFormat");
        assert!(p.detail.contains("xyz"));
    }
}
