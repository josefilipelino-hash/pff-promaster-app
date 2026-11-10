//! # momento-gallery
//!
//! Galerias web estáticas para os clientes (Fase 10): uma pasta com `index.html`, CSS,
//! JavaScript e fontes próprias (sem bibliotecas nem serviços externos), imagens com nomes
//! aleatórios e ZIP para download. Funciona aberta diretamente do disco ou em qualquer
//! alojamento estático.
//!
//! - [`settings`]: o que o fotógrafo escolhe (textos, capa, hero, logótipo, tema, …);
//! - [`chapters`]: capítulos a partir dos momentos (Preparação, Cerimónia, …);
//! - [`crypto`]: PIN/palavra-passe (AES-GCM + PBKDF2, decifrado no navegador);
//! - [`site`]: montagem do site e dos ZIP;
//! - [`publish`]: arquitetura dos fornecedores futuros (S3, R2, B2, SFTP).

pub mod chapters;
pub mod crypto;
pub mod publish;
pub mod settings;
pub mod site;

pub use settings::*;

#[cfg(test)]
mod tests;
