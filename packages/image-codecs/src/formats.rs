//! Adaptadores Rust puros para formatos adicionais (Fase 2):
//! JPEG XL (`jxl-oxide`), PSD/PSB — imagem composta (leitor próprio), JPEG 2000
//! (`jpeg2k` com o backend Rust `openjp2`).
//! Os formatos do crate `image` (WebP, BMP, GIF, OpenEXR, HDR, ICO, PNM, TGA, BigTIFF)
//! estão em `raster.rs`.

use crate::detect::FileFormat;
use crate::raster::{check_size, corrupt, flatten_to_rgb8, MAX_PIXELS};
use crate::registry::{
    CodecCapabilities, CodecMetadata, DecodeKind, DecodedImage, ImageCodec, ImageProbe,
    SupportLevel,
};
use image::{DynamicImage, ImageDecoder, RgbImage};
use momento_core_types::{CoreError, CoreResult};
use std::io::Cursor;

// ---------------------------------------------------------------- JPEG XL
pub struct JxlCodec;

impl ImageCodec for JxlCodec {
    fn id(&self) -> &'static str {
        "jxl-oxide"
    }
    fn priority(&self) -> i32 {
        10
    }
    fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
        let ok = format == FileFormat::JpegXl;
        CodecCapabilities { probe: ok, decode: ok, encode: false }
    }
    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        let d = jxl_oxide::integration::JxlDecoder::new(Cursor::new(data)).map_err(corrupt)?;
        let (w, h) = d.dimensions();
        let ct = d.color_type();
        Ok(ImageProbe {
            format,
            width: w,
            height: h,
            bits_per_sample: (ct.bits_per_pixel() / ct.channel_count() as u16) as u8,
            has_alpha: ct.has_alpha(),
        })
    }
    fn decode(
        &self,
        data: &[u8],
        _format: FileFormat,
        _min: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        let mut d = jxl_oxide::integration::JxlDecoder::new(Cursor::new(data)).map_err(corrupt)?;
        let (w, h) = d.dimensions();
        check_size(w, h)?;
        let icc = d.icc_profile().ok().flatten();
        let img = DynamicImage::from_decoder(d).map_err(corrupt)?;
        Ok(DecodedImage {
            image: flatten_to_rgb8(img),
            icc_profile: icc,
            source_width: w,
            source_height: h,
            kind: DecodeKind::Full,
        })
    }
    fn metadata(&self, data: &[u8], _format: FileFormat) -> Option<CodecMetadata> {
        let mut d = jxl_oxide::integration::JxlDecoder::new(Cursor::new(data)).ok()?;
        let icc = d.icc_profile().ok().flatten();
        Some(assumed_color(icc.as_deref()))
    }
}

// ---------------------------------------------------------------- PSD / PSB
/// PSD e PSB: imagem composta, lida pelo leitor próprio `psd_reader`.
pub struct PsdCodec;

impl ImageCodec for PsdCodec {
    fn id(&self) -> &'static str {
        "psd"
    }
    fn priority(&self) -> i32 {
        10
    }
    fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
        let ok = matches!(format, FileFormat::Psd | FileFormat::Psb);
        CodecCapabilities { probe: ok, decode: ok, encode: false }
    }
    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        let h = crate::psd_reader::validate(data)?;
        Ok(ImageProbe {
            format,
            width: h.width,
            height: h.height,
            bits_per_sample: h.depth as u8,
            has_alpha: false,
        })
    }
    fn decode(
        &self,
        data: &[u8],
        _format: FileFormat,
        _min: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        let h = crate::psd_reader::header(data)?;
        check_size(h.width, h.height)?;
        let (h, rgb) = crate::psd_reader::composite_rgb(data)?;
        let image = RgbImage::from_raw(h.width, h.height, rgb)
            .ok_or_else(|| corrupt("PSD com dados inconsistentes"))?;
        Ok(DecodedImage {
            image,
            icc_profile: None,
            source_width: h.width,
            source_height: h.height,
            kind: DecodeKind::Full,
        })
    }
    fn metadata(&self, data: &[u8], _format: FileFormat) -> Option<CodecMetadata> {
        use crate::psd_reader::ColorMode as M;
        let h = crate::psd_reader::header(data).ok()?;
        let (color, support) = match h.mode {
            M::Rgb => ("RGB".to_string(), SupportLevel::Full),
            M::Grayscale => ("Escala de cinzentos".to_string(), SupportLevel::Full),
            M::Bitmap => ("Bitmap (1 bit)".to_string(), SupportLevel::Full),
            M::Cmyk => (
                "CMYK".to_string(),
                SupportLevel::Partial("PSD em CMYK: conversão simplificada para RGB (sem perfil de cor); as cores podem não ser exatas.".into()),
            ),
            other => (format!("{other:?}"), SupportLevel::Partial(format!("PSD em modo {other:?}: pré-visualização aproximada."))),
        };
        Some(CodecMetadata {
            color_space: Some(color),
            support: Some(support),
            detail: Some(format!(
                "{} · imagem composta (camadas não editáveis)",
                if h.psb { "PSB" } else { "PSD" }
            )),
            ..Default::default()
        })
    }
}

fn assumed_color(icc: Option<&[u8]>) -> CodecMetadata {
    let color = icc
        .and_then(crate::icc::icc_description)
        .map(|d| {
            // Perfis sintetizados pelo jxl-oxide (ex.: "RGB_D65_SRG_Rel_SRG") → nome legível.
            let l = d.to_ascii_lowercase();
            if l.contains("srgb") || l.contains("_srg") {
                "sRGB".to_string()
            } else {
                d
            }
        })
        .unwrap_or_else(|| "sRGB (sem perfil embutido — assumido)".into());
    CodecMetadata {
        color_space: Some(color),
        support: Some(SupportLevel::Full),
        ..Default::default()
    }
}

// ---------------------------------------------------------------- JPEG 2000
pub struct Jp2Codec;

fn j2k_to_rgb(d: jpeg2k::ImageData) -> CoreResult<RgbImage> {
    use jpeg2k::ImagePixelData as P;
    let (w, h) = (d.width, d.height);
    let img = match d.data {
        P::L8(v) => DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(w, h, v).ok_or_else(|| corrupt("JP2"))?,
        ),
        P::La8(v) => DynamicImage::ImageLumaA8(
            image::GrayAlphaImage::from_raw(w, h, v).ok_or_else(|| corrupt("JP2"))?,
        ),
        P::Rgb8(v) => {
            DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, v).ok_or_else(|| corrupt("JP2"))?)
        }
        P::Rgba8(v) => DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(w, h, v).ok_or_else(|| corrupt("JP2"))?,
        ),
        P::L16(v) => DynamicImage::ImageLuma16(
            image::ImageBuffer::from_raw(w, h, v).ok_or_else(|| corrupt("JP2"))?,
        ),
        P::La16(v) => DynamicImage::ImageLumaA16(
            image::ImageBuffer::from_raw(w, h, v).ok_or_else(|| corrupt("JP2"))?,
        ),
        P::Rgb16(v) => DynamicImage::ImageRgb16(
            image::ImageBuffer::from_raw(w, h, v).ok_or_else(|| corrupt("JP2"))?,
        ),
        P::Rgba16(v) => DynamicImage::ImageRgba16(
            image::ImageBuffer::from_raw(w, h, v).ok_or_else(|| corrupt("JP2"))?,
        ),
    };
    Ok(flatten_to_rgb8(img))
}

impl ImageCodec for Jp2Codec {
    fn id(&self) -> &'static str {
        "openjp2"
    }
    fn priority(&self) -> i32 {
        5 // o ImageIO (macOS) tem prioridade quando disponível
    }
    fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
        let ok = format == FileFormat::Jpeg2000;
        CodecCapabilities { probe: ok, decode: ok, encode: false }
    }
    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        // Descodificação ao nível de resolução mais baixo só para obter as dimensões.
        let img = jpeg2k::Image::from_bytes(data).map_err(corrupt)?;
        Ok(ImageProbe {
            format,
            width: img.width(),
            height: img.height(),
            bits_per_sample: 8,
            has_alpha: false,
        })
    }
    fn decode(
        &self,
        data: &[u8],
        _format: FileFormat,
        min_short_edge: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        let full = jpeg2k::Image::from_bytes(data).map_err(corrupt)?;
        let (w, h) = (full.width(), full.height());
        check_size(w, h)?;
        if w as u64 * h as u64 > MAX_PIXELS {
            return Err(CoreError::UnsupportedFormat("JPEG 2000 demasiado grande".into()));
        }
        // Redução por níveis de resolução (cada nível divide por 2).
        let reduce = match min_short_edge {
            Some(m) if m > 0 => {
                let mut r = 0;
                while w.min(h) >> (r + 1) >= m && r < 5 {
                    r += 1;
                }
                r
            }
            _ => 0,
        };
        let img = if reduce > 0 {
            jpeg2k::Image::from_bytes_with(data, jpeg2k::DecodeParameters::new().reduce(reduce))
                .map_err(corrupt)?
        } else {
            full
        };
        let pixels = img.get_pixels(None).map_err(corrupt)?;
        Ok(DecodedImage {
            image: j2k_to_rgb(pixels)?,
            icc_profile: None,
            source_width: w,
            source_height: h,
            kind: DecodeKind::Full,
        })
    }
    fn metadata(&self, _data: &[u8], _format: FileFormat) -> Option<CodecMetadata> {
        Some(assumed_color(None))
    }
}
