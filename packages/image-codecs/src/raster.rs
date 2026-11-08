//! Adaptador para JPEG, PNG, TIFF/BigTIFF, WebP, BMP, GIF, ICO, PNM/PAM, TGA, OpenEXR e
//! Radiance HDR.
//!
//! - JPEG: `jpeg-decoder` com redução no domínio DCT (1/2, 1/4, 1/8), muito mais rápida
//!   do que descodificar a resolução total para gerar miniaturas.
//! - PNG e TIFF: `image` (inclui 16 bits, convertidos para 8 bits nesta fase).
//!
//! Limites de segurança: imagens acima de [`MAX_PIXELS`] são recusadas antes de alocar
//! memória, para que um ficheiro malicioso não esgote a RAM.

use crate::detect::FileFormat;
use crate::registry::{
    CodecCapabilities, CodecMetadata, DecodeKind, DecodedImage, ImageCodec, ImageProbe,
    SupportLevel,
};
use image::{DynamicImage, ImageDecoder, RgbImage};
use momento_core_types::{CoreError, CoreResult};
use std::io::Cursor;

/// 200 megapíxeis (ex.: 20 000 × 10 000). Acima disto, recusar.
pub const MAX_PIXELS: u64 = 200_000_000;

pub struct RasterCodec;

pub(crate) fn corrupt(e: impl std::fmt::Display) -> CoreError {
    CoreError::Corrupt(e.to_string())
}

pub(crate) fn check_size(w: u32, h: u32) -> CoreResult<()> {
    if w == 0 || h == 0 {
        return Err(CoreError::Corrupt("dimensões nulas".into()));
    }
    if w as u64 * h as u64 > MAX_PIXELS {
        return Err(CoreError::UnsupportedFormat(format!("imagem demasiado grande: {w}×{h}")));
    }
    Ok(())
}

fn limits() -> image::Limits {
    let mut l = image::Limits::default();
    l.max_alloc = Some(MAX_PIXELS * 16); // RGBA f32 no pior caso
    l
}

/// Formatos tratados pelo crate `image` e o respetivo identificador.
fn image_format(f: FileFormat) -> Option<image::ImageFormat> {
    use image::ImageFormat as I;
    Some(match f {
        FileFormat::Png => I::Png,
        FileFormat::Tiff | FileFormat::BigTiff => I::Tiff,
        FileFormat::WebP => I::WebP,
        FileFormat::Bmp => I::Bmp,
        FileFormat::Gif => I::Gif,
        FileFormat::Ico => I::Ico,
        FileFormat::Pnm | FileFormat::Pam => I::Pnm,
        FileFormat::Tga => I::Tga,
        FileFormat::RadianceHdr => I::Hdr,
        FileFormat::OpenExr => I::OpenExr,
        _ => return None,
    })
}

fn reader(data: &[u8], f: image::ImageFormat) -> image::ImageReader<Cursor<&[u8]>> {
    let mut r = image::ImageReader::with_format(Cursor::new(data), f);
    r.limits(limits());
    r
}

impl ImageCodec for RasterCodec {
    fn id(&self) -> &'static str {
        "raster"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
        let ok = format == FileFormat::Jpeg || image_format(format).is_some();
        CodecCapabilities { probe: ok, decode: ok, encode: false }
    }

    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        if format == FileFormat::Jpeg {
            let mut d = jpeg_decoder::Decoder::new(Cursor::new(data));
            d.read_info().map_err(corrupt)?;
            let info = d.info().ok_or_else(|| corrupt("cabeçalho JPEG sem informação"))?;
            return Ok(ImageProbe {
                format,
                width: info.width as u32,
                height: info.height as u32,
                bits_per_sample: if info.pixel_format == jpeg_decoder::PixelFormat::L16 {
                    16
                } else {
                    8
                },
                has_alpha: false,
            });
        }
        let f = image_format(format)
            .ok_or_else(|| CoreError::UnsupportedFormat(format!("{format:?}")))?;
        let d = reader(data, f).into_decoder().map_err(corrupt)?;
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
        format: FileFormat,
        min_short_edge: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        if format == FileFormat::Jpeg {
            return decode_jpeg(data, min_short_edge);
        }
        let f = image_format(format)
            .ok_or_else(|| CoreError::UnsupportedFormat(format!("{format:?}")))?;
        let mut d = reader(data, f).into_decoder().map_err(corrupt)?;
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

    fn metadata(&self, data: &[u8], format: FileFormat) -> Option<CodecMetadata> {
        let mut cmyk = false;
        let icc = if format == FileFormat::Jpeg {
            let mut d = jpeg_decoder::Decoder::new(Cursor::new(data));
            d.read_info().ok()?;
            cmyk = d.info().is_some_and(|i| i.pixel_format == jpeg_decoder::PixelFormat::CMYK32);
            d.icc_profile()
        } else {
            let mut d = reader(data, image_format(format)?).into_decoder().ok()?;
            d.icc_profile().ok().flatten()
        };
        let color_space = match (&icc, format) {
            (Some(p), _) => {
                crate::icc::icc_description(p).unwrap_or_else(|| "Perfil ICC embutido".into())
            }
            (None, FileFormat::OpenExr | FileFormat::RadianceHdr) => "Linear (HDR)".into(),
            (None, _) => "sRGB (sem perfil embutido — assumido)".into(),
        };
        let support = match format {
            FileFormat::OpenExr | FileFormat::RadianceHdr => SupportLevel::Partial(
                "Imagem HDR: a pré-visualização converte para sRGB e corta os valores acima do branco (sem mapeamento de tons).".into(),
            ),
            FileFormat::Gif => SupportLevel::Partial("GIF: só o primeiro fotograma é mostrado.".into()),
            FileFormat::Jpeg if cmyk => SupportLevel::Partial(
                "JPEG em CMYK: conversão simplificada para RGB (sem gestão de cor); as cores podem não ser exatas.".into(),
            ),
            _ => SupportLevel::Full,
        };
        Some(CodecMetadata {
            color_space: Some(color_space),
            support: Some(support),
            ..Default::default()
        })
    }
}

fn linear_to_srgb8(v: f32) -> u8 {
    let v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}

/// Converte para RGB 8 bits; transparência composta sobre branco; dados em vírgula
/// flutuante (EXR/HDR, lineares) convertidos para sRGB.
pub(crate) fn flatten_to_rgb8(img: DynamicImage) -> RgbImage {
    match img {
        DynamicImage::ImageRgb32F(f) => RgbImage::from_fn(f.width(), f.height(), |x, y| {
            let p = f.get_pixel(x, y).0;
            image::Rgb([linear_to_srgb8(p[0]), linear_to_srgb8(p[1]), linear_to_srgb8(p[2])])
        }),
        DynamicImage::ImageRgba32F(f) => RgbImage::from_fn(f.width(), f.height(), |x, y| {
            let p = f.get_pixel(x, y).0;
            let a = if p[3].is_finite() { p[3].clamp(0.0, 1.0) } else { 1.0 };
            let mix = |c: f32| linear_to_srgb8(c * a + (1.0 - a));
            image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])])
        }),
        img if !img.color().has_alpha() => img.to_rgb8(),
        img => {
            let rgba = img.to_rgba8();
            RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
                let p = rgba.get_pixel(x, y).0;
                let a = p[3] as u32;
                let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
                image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])])
            })
        }
    }
}

fn decode_jpeg(data: &[u8], min_short_edge: Option<u32>) -> CoreResult<DecodedImage> {
    use jpeg_decoder::{Decoder, PixelFormat};
    let mut d = Decoder::new(Cursor::new(data));
    d.read_info().map_err(corrupt)?;
    let info = d.info().ok_or_else(|| corrupt("cabeçalho JPEG sem informação"))?;
    let (sw, sh) = (info.width as u32, info.height as u32);
    check_size(sw, sh)?;
    if let Some(min) = min_short_edge {
        // Pedir a menor escala cujo lado menor continue ≥ `min`.
        let short = sw.min(sh).max(1);
        let factor = (min as f64 / short as f64).min(1.0);
        let rw = ((sw as f64 * factor).ceil() as u32).clamp(1, u16::MAX as u32) as u16;
        let rh = ((sh as f64 * factor).ceil() as u32).clamp(1, u16::MAX as u32) as u16;
        d.scale(rw, rh).map_err(corrupt)?;
    }
    let pixels = d.decode().map_err(corrupt)?;
    let info = d.info().ok_or_else(|| corrupt("JPEG sem informação após descodificação"))?;
    let (w, h) = (info.width as u32, info.height as u32);
    let rgb: Vec<u8> = match info.pixel_format {
        PixelFormat::RGB24 => pixels,
        PixelFormat::L8 => pixels.iter().flat_map(|&v| [v, v, v]).collect(),
        PixelFormat::L16 => pixels.chunks_exact(2).flat_map(|c| [c[0], c[0], c[0]]).collect(),
        PixelFormat::CMYK32 => pixels
            .chunks_exact(4)
            .flat_map(|c| {
                // jpeg-decoder devolve CMYK já invertido (Adobe); conversão ingénua
                // suficiente para miniaturas. Gestão de cor CMYK real: Fase 2 (LittleCMS).
                let k = c[3] as u32;
                [
                    (c[0] as u32 * k / 255) as u8,
                    (c[1] as u32 * k / 255) as u8,
                    (c[2] as u32 * k / 255) as u8,
                ]
            })
            .collect(),
    };
    let image =
        RgbImage::from_raw(w, h, rgb).ok_or_else(|| corrupt("buffer JPEG inconsistente"))?;
    Ok(DecodedImage {
        image,
        icc_profile: d.icc_profile(),
        source_width: sw,
        source_height: sh,
        kind: DecodeKind::Full,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use momento_test_fixtures::*;

    #[test]
    fn probes_and_decodes_all_three_formats() {
        let img = test_image(400, 300, 5, 1);
        let c = RasterCodec;
        for (fmt, bytes) in [
            (FileFormat::Jpeg, jpeg_bytes(&img, None, 90)),
            (FileFormat::Png, png_bytes(&img, None)),
            (FileFormat::Tiff, tiff_bytes(&img)),
        ] {
            let p = c.probe(&bytes, fmt).unwrap();
            assert_eq!((p.width, p.height), (400, 300), "{fmt:?}");
            let d = c.decode(&bytes, fmt, None).unwrap();
            assert_eq!(d.image.dimensions(), (400, 300), "{fmt:?}");
            // Barra vermelha no topo preservada.
            let px = d.image.get_pixel(200, 5).0;
            assert!(px[0] > 180 && px[1] < 80, "{fmt:?} {px:?}");
        }
    }

    #[test]
    fn jpeg_scaled_decode_respects_minimum() {
        let img = test_image(1600, 1200, 1, 1);
        let bytes = jpeg_bytes(&img, None, 85);
        let d = RasterCodec.decode(&bytes, FileFormat::Jpeg, Some(300)).unwrap();
        // 1/4 da resolução: 400×300 (o lado menor continua ≥ 300).
        assert_eq!(d.image.dimensions(), (400, 300));
        assert_eq!((d.source_width, d.source_height), (1600, 1200));
    }

    #[test]
    fn sixteen_bit_tiff_is_converted() {
        let bytes = tiff16_bytes(64, 32);
        let p = RasterCodec.probe(&bytes, FileFormat::Tiff).unwrap();
        assert_eq!(p.bits_per_sample, 16);
        let d = RasterCodec.decode(&bytes, FileFormat::Tiff, None).unwrap();
        assert_eq!(d.image.dimensions(), (64, 32));
    }

    #[test]
    fn corrupt_data_returns_error_not_panic() {
        let img = test_image(200, 100, 1, 1);
        let good = jpeg_bytes(&img, None, 85);
        let truncated = &good[..good.len() / 3];
        // O cabeçalho é válido mas os dados estão cortados: erro ou imagem parcial, nunca pânico.
        let _ = RasterCodec.decode(truncated, FileFormat::Jpeg, None);
        assert!(RasterCodec.decode(b"\xFF\xD8\xFF garbage", FileFormat::Jpeg, None).is_err());
        assert!(RasterCodec.probe(b"\x89PNG\r\n\x1a\n", FileFormat::Png).is_err());
        assert!(RasterCodec.decode(b"II*\0\0\0\0\0", FileFormat::Tiff, None).is_err());
    }

    #[test]
    fn transparent_png_is_flattened_on_white() {
        let rgba = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0]));
        let mut bytes = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut bytes),
            rgba.as_raw(),
            4,
            4,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
        let d = RasterCodec.decode(&bytes, FileFormat::Png, None).unwrap();
        assert_eq!(d.image.get_pixel(0, 0).0, [255, 255, 255]);
        assert!(RasterCodec.probe(&bytes, FileFormat::Png).unwrap().has_alpha);
    }
}
