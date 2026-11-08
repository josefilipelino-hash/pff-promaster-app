//! # momento-image-codecs
//!
//! Arquitetura de codecs baseada em adaptadores (ARCHITECTURE.md §10):
//!
//! - [`detect`]: identifica o formato **real** pelos primeiros bytes do ficheiro; a
//!   extensão serve apenas para desambiguar dentro de famílias com o mesmo contentor
//!   (por exemplo NEF/ARW/DNG, que são todos TIFF).
//! - [`ImageCodec`]: contrato que cada adaptador implementa.
//! - [`CodecRegistry`]: escolhe o adaptador com maior prioridade para um formato e
//!   permite adicionar formatos sem alterar o núcleo.
//! - [`raster::RasterCodec`]: adaptador da Fase 1 para JPEG, PNG e TIFF.

pub mod detect;
pub mod raster;
pub mod registry;

pub use detect::{detect, Confidence, Detection, FileFormat, FormatFamily};
pub use registry::{CodecCapabilities, CodecRegistry, DecodedImage, ImageCodec, ImageProbe};

/// Registo com os adaptadores disponíveis nesta compilação.
pub fn default_registry() -> CodecRegistry {
    let mut r = CodecRegistry::new();
    r.register(std::sync::Arc::new(raster::RasterCodec));
    r
}

/// Identificador estável de formato guardado no catálogo (`asset.format`).
pub fn format_id(f: FileFormat) -> &'static str {
    use FileFormat::*;
    match f {
        Jpeg => "jpeg",
        Png => "png",
        Tiff => "tiff",
        BigTiff => "bigtiff",
        Gif => "gif",
        Bmp => "bmp",
        WebP => "webp",
        Heic => "heic",
        Heif => "heif",
        Avif => "avif",
        JpegXl => "jxl",
        Jpeg2000 => "jp2",
        Psd => "psd",
        Psb => "psb",
        OpenExr => "exr",
        RadianceHdr => "hdr",
        Ico => "ico",
        Pnm => "pnm",
        Pam => "pam",
        Pcx => "pcx",
        Tga => "tga",
        Svg => "svg",
        Pdf => "pdf",
        CanonCr2 => "cr2",
        CanonCr3 => "cr3",
        CanonCrw => "crw",
        FujiRaf => "raf",
        OlympusOrf => "orf",
        PanasonicRw2 => "rw2",
        MinoltaMrw => "mrw",
        SigmaX3f => "x3f",
        TiffBasedRaw => "raw-tiff",
    }
}

/// Inverso de [`format_id`] para os formatos com adaptador.
pub fn format_from_id(id: &str) -> Option<FileFormat> {
    Some(match id {
        "jpeg" => FileFormat::Jpeg,
        "png" => FileFormat::Png,
        "tiff" => FileFormat::Tiff,
        _ => return None,
    })
}
