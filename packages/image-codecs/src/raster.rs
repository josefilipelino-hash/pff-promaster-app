//! Adaptador para JPEG, PNG e TIFF (Fase 1).
//!
//! - JPEG: `jpeg-decoder` com redução no domínio DCT (1/2, 1/4, 1/8), muito mais rápida
//!   do que descodificar a resolução total para gerar miniaturas.
//! - PNG e TIFF: `image` (inclui 16 bits, convertidos para 8 bits nesta fase).
//!
//! Limites de segurança: imagens acima de [`MAX_PIXELS`] são recusadas antes de alocar
//! memória, para que um ficheiro malicioso não esgote a RAM.

use crate::detect::FileFormat;
use crate::registry::{CodecCapabilities, DecodedImage, ImageCodec, ImageProbe};
use image::{DynamicImage, ImageDecoder, RgbImage};
use momento_core_types::{CoreError, CoreResult};
use std::io::Cursor;

/// 200 megapíxeis (ex.: 20 000 × 10 000). Acima disto, recusar.
pub const MAX_PIXELS: u64 = 200_000_000;

pub struct RasterCodec;

fn corrupt(e: impl std::fmt::Display) -> CoreError {
    CoreError::Corrupt(e.to_string())
}

fn check_size(w: u32, h: u32) -> CoreResult<()> {
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
    l.max_alloc = Some(MAX_PIXELS * 8); // RGBA de 16 bits no pior caso
    l
}

impl ImageCodec for RasterCodec {
    fn id(&self) -> &'static str {
        "raster"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
        let ok = matches!(format, FileFormat::Jpeg | FileFormat::Png | FileFormat::Tiff);
        CodecCapabilities { probe: ok, decode: ok, encode: false }
    }

    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        match format {
            FileFormat::Jpeg => {
                let mut d = jpeg_decoder::Decoder::new(Cursor::new(data));
                d.read_info().map_err(corrupt)?;
                let info = d.info().ok_or_else(|| corrupt("cabeçalho JPEG sem informação"))?;
                Ok(ImageProbe {
                    format,
                    width: info.width as u32,
                    height: info.height as u32,
                    bits_per_sample: if info.pixel_format == jpeg_decoder::PixelFormat::L16 {
                        16
                    } else {
                        8
                    },
                    has_alpha: false,
                })
            }
            FileFormat::Png => probe_with(
                image::codecs::png::PngDecoder::new(Cursor::new(data)).map_err(corrupt)?,
                format,
            ),
            FileFormat::Tiff => probe_with(
                image::codecs::tiff::TiffDecoder::new(Cursor::new(data)).map_err(corrupt)?,
                format,
            ),
            other => Err(CoreError::UnsupportedFormat(format!("{other:?}"))),
        }
    }

    fn decode(
        &self,
        data: &[u8],
        format: FileFormat,
        min_short_edge: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        match format {
            FileFormat::Jpeg => decode_jpeg(data, min_short_edge),
            FileFormat::Png => decode_with(
                image::codecs::png::PngDecoder::new(Cursor::new(data)).map_err(corrupt)?,
            ),
            FileFormat::Tiff => decode_with(
                image::codecs::tiff::TiffDecoder::new(Cursor::new(data)).map_err(corrupt)?,
            ),
            other => Err(CoreError::UnsupportedFormat(format!("{other:?}"))),
        }
    }
}

fn probe_with(d: impl ImageDecoder, format: FileFormat) -> CoreResult<ImageProbe> {
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

fn decode_with(mut d: impl ImageDecoder) -> CoreResult<DecodedImage> {
    let (w, h) = d.dimensions();
    check_size(w, h)?;
    d.set_limits(limits()).map_err(corrupt)?;
    let icc = d.icc_profile().ok().flatten();
    let img = DynamicImage::from_decoder(d).map_err(corrupt)?;
    let rgb = flatten_to_rgb8(img);
    Ok(DecodedImage { image: rgb, icc_profile: icc, source_width: w, source_height: h })
}

/// Converte para RGB 8 bits; transparência composta sobre branco.
fn flatten_to_rgb8(img: DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u32;
        let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
        image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])])
    })
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
    Ok(DecodedImage { image, icc_profile: d.icc_profile(), source_width: sw, source_height: sh })
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
