//! # momento-project-management
//!
//! Camada de serviço usada pela aplicação desktop (e pelos testes de integração):
//!
//! - [`paths`]: estrutura de uma pasta de projeto `<nome>.momento/`;
//! - [`create_project`] / [`ProjectSession::open`]: criação e abertura;
//! - [`thumbnails::ThumbnailService`]: miniaturas em segundo plano com prioridade;
//! - [`analysis::AnalysisService`]: análise local (métricas, rostos, embeddings) e seleção;
//! - [`previews`]: previews e resolução total a pedido;
//! - [`auto_edit`]: edição automática por fotografia e fila em segundo plano (Fase 6);
//! - [`ProjectSession`]: projeto aberto — catálogo, importações e eventos.

pub mod album;
pub mod analysis;
pub mod auto_edit;
pub mod availability;
pub mod cache;
pub mod codecs;
pub mod consistency;
pub mod edit;
pub mod export;
pub mod export_store;
pub mod gallery;
pub mod lensprofile;
pub mod lightroom;
pub mod local_edit;
pub mod organize;
pub mod originals;
pub mod paths;
pub mod pool;
pub mod preset_store;
pub mod previews;
pub mod safety;
mod session;
pub mod thumbnails;

pub use paths::ProjectPaths;
pub use session::*;
