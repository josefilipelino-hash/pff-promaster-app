//! Registo completo de codecs da aplicação.
//!
//! | Adaptador | Formatos | Plataformas |
//! |---|---|---|
//! | `raster` (crate `image`, `jpeg-decoder`) | JPEG, PNG, TIFF/BigTIFF, WebP, BMP, GIF, ICO, PNM/PAM, TGA, OpenEXR, HDR | todas |
//! | `jxl-oxide` | JPEG XL | todas |
//! | `psd` | PSD (imagem composta) | todas |
//! | `openjp2` | JPEG 2000 | todas |
//! | `libraw` (LibRaw 0.22.3) | RAW de câmara e DNG | todas |
//! | `imageio` | HEIC, HEIF, AVIF (e outros) | macOS |
//! | `wic` | HEIC, HEIF, AVIF (com extensões da Microsoft Store) | Windows |

use momento_image_codecs::{default_registry, CodecRegistry};
use momento_raw_engine::RawCodec;
use std::sync::Arc;

pub fn full_registry() -> CodecRegistry {
    let mut r = default_registry();
    // A preview embutida dos RAW (JPEG, JPEG XL, bitmap) é descodificada pelos adaptadores puros.
    r.register(Arc::new(RawCodec::new(default_registry())));
    momento_image_codecs::platform::register(&mut r);
    r
}
