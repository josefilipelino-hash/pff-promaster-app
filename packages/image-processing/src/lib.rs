//! # momento-image-processing
//!
//! Fase 1: geração de miniaturas e previews a partir dos originais — redução de escala,
//! orientação EXIF e codificação JPEG com o perfil ICC original preservado (o WebView
//! faz a gestão de cor das imagens com perfil embutido).
//!
//! Fase 5: [`develop`] — motor de revelação não destrutivo em `f32` (Rec. 2020 linear).

pub mod develop;
pub mod thumbnail;

pub use thumbnail::{
    apply_orientation, encode_jpeg, render_preview, render_preview_and_thumb, write_atomic,
    PreviewSpec,
};
