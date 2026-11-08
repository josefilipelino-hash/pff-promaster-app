//! # momento-image-codecs
//!
//! Arquitetura de codecs baseada em adaptadores (ARCHITECTURE.md §10):
//!
//! - [`detect`]: identifica o formato **real** pelos primeiros bytes do ficheiro; a
//!   extensão serve apenas para desambiguar dentro de famílias com o mesmo contentor
//!   (por exemplo NEF/ARW/DNG, que são todos TIFF).
//! - [`ImageCodec`]: contrato que cada adaptador (image-rs, LibRaw, ImageIO, libheif…)
//!   implementa.
//! - [`CodecRegistry`]: escolhe o adaptador com maior prioridade para um formato e
//!   permite adicionar formatos sem alterar o núcleo.
//!
//! Na Fase 0 não existe nenhum adaptador registado: só o contrato e a deteção.

pub mod detect;
pub mod registry;

pub use detect::{detect, Confidence, Detection, FileFormat, FormatFamily};
pub use registry::{CodecCapabilities, CodecRegistry, ImageCodec, ImageProbe};
