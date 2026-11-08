//! Adaptador WIC (Windows Imaging Component): HEIC, HEIF e AVIF com os codecs do sistema.
//!
//! O Windows só descodifica HEIC com as extensões “HEIF Image Extensions” e “HEVC Video
//! Extensions”, e AVIF com a “AV1 Video Extension” (Microsoft Store). Sem elas, o WIC
//! devolve `WINCODEC_ERR_COMPONENTNOTFOUND`, convertido aqui em “codec em falta” com a
//! indicação da extensão a instalar.
//!
//! Estado: compilado para Windows; **não validado em execução** nesta fase.

use crate::detect::FileFormat;
use crate::registry::{
    CodecCapabilities, DecodeKind, DecodedImage, ImageCodec, ImageProbe, MISSING_CODEC,
};
use image::RgbImage;
use momento_core_types::{CoreError, CoreResult};
use windows::core::{Interface, GUID};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat24bppRGB, IWICBitmapSource, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

pub struct WicCodec;

/// HRESULT de componente WIC inexistente (codec não instalado).
const WINCODEC_ERR_COMPONENTNOTFOUND: i32 = 0x8898_2F50_u32 as i32;

fn extension_hint(format: FileFormat) -> &'static str {
    match format {
        FileFormat::Avif => "instale a “AV1 Video Extension” da Microsoft Store",
        _ => "instale as extensões “HEIF Image Extensions” e “HEVC Video Extensions” da Microsoft Store",
    }
}

fn wic_err(e: windows::core::Error, format: FileFormat) -> CoreError {
    if e.code().0 == WINCODEC_ERR_COMPONENTNOTFOUND {
        CoreError::UnsupportedFormat(format!(
            "{MISSING_CODEC}: o Windows não tem descodificador para {format:?} — {}",
            extension_hint(format)
        ))
    } else {
        CoreError::Corrupt(format!("WIC: {e}"))
    }
}

struct Frame {
    factory: IWICImagingFactory,
    source: IWICBitmapSource,
    width: u32,
    height: u32,
}

fn open(data: &[u8], format: FileFormat) -> CoreResult<Frame> {
    // SAFETY: inicialização COM por thread (idempotente); objetos COM libertados no Drop.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| wic_err(e, format))?;
        let stream = factory.CreateStream().map_err(|e| wic_err(e, format))?;
        stream.InitializeFromMemory(data).map_err(|e| wic_err(e, format))?;
        let decoder = factory
            .CreateDecoderFromStream(
                &stream,
                std::ptr::null::<GUID>(),
                WICDecodeMetadataCacheOnDemand,
            )
            .map_err(|e| wic_err(e, format))?;
        let frame = decoder.GetFrame(0).map_err(|e| wic_err(e, format))?;
        let (mut w, mut h) = (0u32, 0u32);
        frame.GetSize(&mut w, &mut h).map_err(|e| wic_err(e, format))?;
        let source: IWICBitmapSource = frame.cast().map_err(|e| wic_err(e, format))?;
        Ok(Frame { factory, source, width: w, height: h })
    }
}

impl ImageCodec for WicCodec {
    fn id(&self) -> &'static str {
        "wic"
    }
    fn priority(&self) -> i32 {
        50
    }
    fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
        let ok = matches!(format, FileFormat::Heic | FileFormat::Heif | FileFormat::Avif);
        CodecCapabilities { probe: ok, decode: ok, encode: false }
    }
    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        let f = open(data, format)?;
        Ok(ImageProbe {
            format,
            width: f.width,
            height: f.height,
            bits_per_sample: 8,
            has_alpha: false,
        })
    }
    fn decode(
        &self,
        data: &[u8],
        format: FileFormat,
        min_short_edge: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        let f = open(data, format)?;
        let (sw, sh) = (f.width, f.height);
        crate::raster::check_size(sw, sh)?;
        // SAFETY: chamadas COM com objetos válidos; `buf` tem stride*h bytes.
        unsafe {
            let (mut src, mut w, mut h) = (f.source.clone(), sw, sh);
            if let Some(m) = min_short_edge.filter(|m| *m < sw.min(sh)) {
                let scale = m as f64 / sw.min(sh) as f64;
                w = ((sw as f64 * scale).round() as u32).max(1);
                h = ((sh as f64 * scale).round() as u32).max(1);
                let scaler = f.factory.CreateBitmapScaler().map_err(|e| wic_err(e, format))?;
                scaler
                    .Initialize(&src, w, h, WICBitmapInterpolationModeFant)
                    .map_err(|e| wic_err(e, format))?;
                src = scaler.cast().map_err(|e| wic_err(e, format))?;
            }
            let conv = f.factory.CreateFormatConverter().map_err(|e| wic_err(e, format))?;
            conv.Initialize(
                &src,
                &GUID_WICPixelFormat24bppRGB,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|e| wic_err(e, format))?;
            let stride = w * 3;
            let mut buf = vec![0u8; stride as usize * h as usize];
            conv.CopyPixels(std::ptr::null(), stride, &mut buf).map_err(|e| wic_err(e, format))?;
            let image = RgbImage::from_raw(w, h, buf)
                .ok_or_else(|| CoreError::Corrupt("buffer WIC inconsistente".into()))?;
            Ok(DecodedImage {
                image,
                icc_profile: None,
                source_width: sw,
                source_height: sh,
                kind: DecodeKind::Full,
            })
        }
    }
}
