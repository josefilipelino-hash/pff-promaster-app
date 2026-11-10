//! Previews a pedido para o visualizador.
//!
//! - `preview`: 2560 px, gerada uma vez e guardada em cache;
//! - `full`: resolução total. Para JPEG e PNG serve-se o **próprio original** (o WebView
//!   descodifica-o e aplica a orientação EXIF e o perfil ICC); para os restantes formatos
//!   (RAW, TIFF, HEIC, …) gera-se um JPEG de resolução total em cache — no caso dos RAW, é
//!   aqui que acontece a revelação completa, só quando o utilizador faz zoom a 100 %.

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
            return Err(crate::availability::unavailable(&src.path));
        }
        return Ok(Servable { path: PathBuf::from(src.path), mime: mime(format) });
    }
    if cached.is_file() {
        crate::cache::touch(&cached);
        return Ok(Servable { path: cached, mime: "image/jpeg" });
    }
    let data = crate::availability::read_original(&src.path)?;
    let spec = if level == CacheLevel::Full { PreviewSpec::FULL } else { PreviewSpec::PREVIEW };
    let jpg = render_preview(registry, &data, format, src.orientation, spec)?;
    write_atomic(&cached, &jpg)?;
    maybe_prune(paths);
    Ok(Servable { path: cached, mime: "image/jpeg" })
}

/// Verifica os limites da cache a cada 64 previews novas (em segundo plano).
fn maybe_prune(paths: &ProjectPaths) {
    static WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 64 == 63 {
        let p = paths.clone();
        std::thread::spawn(move || crate::cache::prune_project(&p));
    }
}
