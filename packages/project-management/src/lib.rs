//! # momento-project-management
//!
//! Camada de serviço usada pela aplicação desktop (e pelos testes de integração):
//!
//! - [`paths`]: estrutura de uma pasta de projeto `<nome>.momento/`;
//! - [`create_project`] / [`ProjectSession::open`]: criação e abertura;
//! - [`thumbnails::ThumbnailService`]: miniaturas em segundo plano com prioridade;
//! - [`analysis::AnalysisService`]: análise local (métricas, rostos, embeddings) e seleção;
//! - [`previews`]: previews e resolução total a pedido;
//! - [`ProjectSession`]: projeto aberto — catálogo, importações e eventos.

pub mod analysis;
pub mod cache;
pub mod codecs;
pub mod organize;
pub mod paths;
pub mod previews;
mod session;
pub mod thumbnails;

pub use paths::ProjectPaths;
pub use session::*;
