//! Adaptadores que usam os codecs do sistema operativo.
//!
//! - macOS: ImageIO (HEIC/HEIF/AVIF e outros), sem custos de patentes HEVC;
//! - Windows: Windows Imaging Component (WIC) — HEIC requer as extensões “HEIF Image
//!   Extensions” + “HEVC Video Extensions” e AVIF a “AV1 Video Extension” da Microsoft Store;
//! - outras plataformas: nenhum (HEIC/AVIF ficam como “codec em falta”).

use crate::registry::CodecRegistry;

#[cfg(target_os = "macos")]
mod imageio;
#[cfg(target_os = "windows")]
mod wic;

/// Regista os adaptadores do sistema disponíveis nesta plataforma.
#[allow(unused_variables)]
pub fn register(r: &mut CodecRegistry) {
    #[cfg(target_os = "macos")]
    r.register(std::sync::Arc::new(imageio::ImageIoCodec));
    #[cfg(target_os = "windows")]
    r.register(std::sync::Arc::new(wic::WicCodec));
}

/// Nome do adaptador de sistema (para diagnóstico), se existir.
pub fn system_codec_name() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("ImageIO")
    } else if cfg!(target_os = "windows") {
        Some("WIC")
    } else {
        None
    }
}
