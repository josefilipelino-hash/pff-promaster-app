//! Adaptador ImageIO (macOS): HEIC, HEIF, AVIF e JPEG 2000 com os codecs do sistema.
//!
//! Estado: compilado para macOS; **não validado em execução** nesta fase (sem macOS no
//! ambiente de desenvolvimento). Ver SUPPORTED_FORMATS.md.

use crate::detect::FileFormat;
use crate::registry::{CodecCapabilities, DecodeKind, DecodedImage, ImageCodec, ImageProbe};
use image::RgbImage;
use momento_core_types::{CoreError, CoreResult};
use objc2_core_foundation::{
    kCFBooleanFalse, kCFBooleanTrue, CFBoolean, CFData, CFDictionary, CFNumber, CFRetained,
    CFString, CFType, CGPoint, CGRect, CGSize,
};
use objc2_core_graphics::{
    kCGColorSpaceSRGB, CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
};
use objc2_image_io::{
    kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceCreateThumbnailWithTransform,
    kCGImageSourceThumbnailMaxPixelSize, CGImageSource,
};

pub struct ImageIoCodec;

fn source(data: &[u8]) -> CoreResult<CFRetained<CGImageSource>> {
    let cf = CFData::from_bytes(data);
    // SAFETY: CFData válido; sem opções.
    unsafe { CGImageSource::with_data(&cf, None) }
        .ok_or_else(|| CoreError::Corrupt("ImageIO não reconhece o ficheiro".into()))
}

fn image(src: &CGImageSource, max_edge: Option<u32>) -> CoreResult<CFRetained<CGImage>> {
    let img = match max_edge {
        Some(max) => {
            let t: &CFBoolean =
                unsafe { kCFBooleanTrue }.ok_or_else(|| CoreError::Other("CF".into()))?;
            let f: &CFBoolean =
                unsafe { kCFBooleanFalse }.ok_or_else(|| CoreError::Other("CF".into()))?;
            let n = CFNumber::new_i32(max as i32);
            let keys: [&CFString; 3] = unsafe {
                [
                    kCGImageSourceThumbnailMaxPixelSize,
                    kCGImageSourceCreateThumbnailFromImageAlways,
                    kCGImageSourceCreateThumbnailWithTransform,
                ]
            };
            let values: [&CFType; 3] = [n.as_ref(), t.as_ref(), f.as_ref()];
            let opts = CFDictionary::from_slices(&keys, &values);
            // SAFETY: dicionário com chaves CFString e valores CFType, como exigido pela API.
            unsafe { src.thumbnail_at_index(0, Some(opts.as_opaque())) }
        }
        // SAFETY: índice 0; sem opções.
        None => unsafe { src.image_at_index(0, None) },
    };
    img.ok_or_else(|| CoreError::Corrupt("ImageIO não conseguiu descodificar a imagem".into()))
}

fn to_rgb(img: &CGImage) -> CoreResult<RgbImage> {
    let w = CGImage::width(Some(img));
    let h = CGImage::height(Some(img));
    crate::raster::check_size(w as u32, h as u32)?;
    let mut buf = vec![0u8; w * h * 4];
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))
        .ok_or_else(|| CoreError::Other("sRGB".into()))?;
    // SAFETY: `buf` tem w*h*4 bytes e vive até ao fim da função; RGBA 8 bits pré-multiplicado.
    let ctx = unsafe {
        CGBitmapContextCreate(
            buf.as_mut_ptr().cast(),
            w,
            h,
            8,
            w * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }
    .ok_or_else(|| CoreError::Other("CGBitmapContextCreate falhou".into()))?;
    let rect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize { width: w as f64, height: h as f64 },
    };
    CGContext::draw_image(Some(&ctx), rect, Some(img));
    drop(ctx);
    // Composição sobre branco (valores pré-multiplicados: c + (255 − a)).
    let rgb: Vec<u8> = buf
        .chunks_exact(4)
        .flat_map(|p| {
            let k = 255 - p[3];
            [p[0].saturating_add(k), p[1].saturating_add(k), p[2].saturating_add(k)]
        })
        .collect();
    RgbImage::from_raw(w as u32, h as u32, rgb)
        .ok_or_else(|| CoreError::Corrupt("buffer ImageIO inconsistente".into()))
}

impl ImageCodec for ImageIoCodec {
    fn id(&self) -> &'static str {
        "imageio"
    }
    fn priority(&self) -> i32 {
        50
    }
    fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
        let ok = matches!(
            format,
            FileFormat::Heic | FileFormat::Heif | FileFormat::Avif | FileFormat::Jpeg2000
        );
        CodecCapabilities { probe: ok, decode: ok, encode: false }
    }
    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        let src = source(data)?;
        // A criação da CGImage é preguiçosa: obter dimensões não descodifica os píxeis.
        let img = image(&src, None)?;
        Ok(ImageProbe {
            format,
            width: CGImage::width(Some(&img)) as u32,
            height: CGImage::height(Some(&img)) as u32,
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
        let src = source(data)?;
        let full = image(&src, None)?;
        let (sw, sh) = (CGImage::width(Some(&full)) as u32, CGImage::height(Some(&full)) as u32);
        let img = match min_short_edge {
            Some(m) if m < sw.min(sh) => {
                let max_edge = (m as u64 * sw.max(sh) as u64 / sw.min(sh).max(1) as u64) as u32;
                image(&src, Some(max_edge))?
            }
            _ => full,
        };
        Ok(DecodedImage {
            image: to_rgb(&img)?,
            icc_profile: None,
            source_width: sw,
            source_height: sh,
            kind: DecodeKind::Full,
        })
    }
}
