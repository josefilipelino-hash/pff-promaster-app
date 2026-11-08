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
pub mod formats;
pub mod icc;
pub mod platform;
pub mod psd_reader;
pub mod raster;
pub mod registry;

pub use detect::{detect, Confidence, Detection, FileFormat, FormatFamily};
pub use registry::{
    is_missing_codec, CodecCapabilities, CodecMetadata, CodecRegistry, DecodeKind, DecodedImage,
    ImageCodec, ImageProbe, SupportLevel, MISSING_CODEC,
};

/// Registo com os adaptadores Rust puros (todas as plataformas). Os adaptadores de RAW
/// (LibRaw) e de sistema (ImageIO no macOS, WIC no Windows) são acrescentados pela
/// aplicação — ver `momento_project_management::codecs::full_registry`.
pub fn default_registry() -> CodecRegistry {
    let mut r = CodecRegistry::new();
    r.register(std::sync::Arc::new(raster::RasterCodec));
    r.register(std::sync::Arc::new(formats::JxlCodec));
    r.register(std::sync::Arc::new(formats::PsdCodec));
    r.register(std::sync::Arc::new(formats::Jp2Codec));
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

/// Inverso de [`format_id`].
pub fn format_from_id(id: &str) -> Option<FileFormat> {
    use FileFormat::*;
    const ALL: &[FileFormat] = &[
        Jpeg,
        Png,
        Gif,
        Bmp,
        WebP,
        Tiff,
        BigTiff,
        Heic,
        Heif,
        Avif,
        JpegXl,
        Jpeg2000,
        Psd,
        Psb,
        OpenExr,
        RadianceHdr,
        Ico,
        Pnm,
        Pam,
        Pcx,
        Tga,
        Svg,
        Pdf,
        CanonCr2,
        CanonCr3,
        CanonCrw,
        FujiRaf,
        OlympusOrf,
        PanasonicRw2,
        MinoltaMrw,
        SigmaX3f,
        TiffBasedRaw,
    ];
    ALL.iter().copied().find(|f| format_id(*f) == id)
}
