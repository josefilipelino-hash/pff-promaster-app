//! Previews a pedido para o visualizador.
//!
//! - `preview`: 2560 px, gerada uma vez e guardada em cache;
//! - `full`: resolução total. Para JPEG e PNG serve-se o **próprio original** (o WebView
//!   descodifica-o e aplica a orientação EXIF e o perfil ICC); para TIFF, que nem todos os
//!   WebViews mostram, gera-se um JPEG de resolução total em cache.

use crate::paths::{CacheLevel, ProjectPaths};
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_database::Catalog;
use momento_image_codecs::{format_from_id, CodecRegistry, FileFormat};
use momento_image_processing::{render_preview, write_atomic, PreviewSpec};
use std::path::PathBuf;

/// Ficheiro a servir e o respetivo tipo MIME.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Servable {
    pub path: PathBuf,
    pub mime: &'static str,
}

fn mime(f: FileFormat) -> &'static str {
    match f {
        FileFormat::Png => "image/png",
        _ => "image/jpeg",
    }
}

pub fn ensure(
    paths: &ProjectPaths,
    registry: &CodecRegistry,
    id: AssetId,
    level: CacheLevel,
) -> CoreResult<Servable> {
    let cached = paths.cached(level, id);
    if level == CacheLevel::Thumb {
        return if cached.is_file() {
            Ok(Servable { path: cached, mime: "image/jpeg" })
        } else {
            Err(CoreError::Other("miniatura ainda não gerada".into()))
        };
    }
    let catalog = Catalog::open_existing(&paths.catalog())?;
    let src = catalog
        .asset_source(id)?
        .ok_or_else(|| CoreError::Other(format!("fotografia {id} inexistente")))?;
    let format = format_from_id(&src.format)
        .ok_or_else(|| CoreError::UnsupportedFormat(src.format.clone()))?;
    if level == CacheLevel::Full && matches!(format, FileFormat::Jpeg | FileFormat::Png) {
        if !std::path::Path::new(&src.path).is_file() {
            return Err(CoreError::Other(format!("original não encontrado: {}", src.path)));
        }
        return Ok(Servable { path: PathBuf::from(src.path), mime: mime(format) });
    }
    if cached.is_file() {
        return Ok(Servable { path: cached, mime: "image/jpeg" });
    }
    let data = std::fs::read(&src.path)
        .map_err(|e| CoreError::Other(format!("original não encontrado: {} ({e})", src.path)))?;
    let spec = if level == CacheLevel::Full { PreviewSpec::FULL } else { PreviewSpec::PREVIEW };
    let jpg = render_preview(registry, &data, format, src.orientation, spec)?;
    write_atomic(&cached, &jpg)?;
    Ok(Servable { path: cached, mime: "image/jpeg" })
}
