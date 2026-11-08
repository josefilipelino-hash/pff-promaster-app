//! Adaptador RAW para o registo de codecs.

use crate::file::{Bitmap, EmbeddedPreview, RawFile};
use image::RgbImage;
use momento_core_types::{CoreError, CoreResult};
use momento_image_codecs::{
    CodecCapabilities, CodecMetadata, CodecRegistry, DecodeKind, DecodedImage, FileFormat,
    FormatFamily, ImageCodec, ImageProbe, SupportLevel,
};

/// Lado maior a partir do qual a preview embutida da câmara é usada para ver no ecrã
/// (modo “ajustar”), em vez de revelar o RAW. A revelação fica para o zoom a 100 %.
pub const EMBEDDED_GOOD_ENOUGH_LONG_EDGE: u32 = 1600;
/// Pedidos até este lado menor (previews de ecrã) aceitam a preview “boa o suficiente”.
const SCREEN_PREVIEW_MAX_SHORT_EDGE: u32 = 1800;

fn preview_acceptable(w: u32, h: u32, min_short_edge: u32) -> bool {
    w.min(h) >= min_short_edge
        || (w.max(h) >= EMBEDDED_GOOD_ENOUGH_LONG_EDGE
            && min_short_edge <= SCREEN_PREVIEW_MAX_SHORT_EDGE)
}

/// Adaptador RAW (LibRaw). As previews JPEG/JPEG XL embutidas são descodificadas pelos
/// adaptadores de imagem do registo `inner` (JPEG com redução DCT, JPEG XL, …).
pub struct RawCodec {
    inner: CodecRegistry,
}

impl RawCodec {
    pub fn new(inner: CodecRegistry) -> Self {
        RawCodec { inner }
    }

    fn decode_preview(
        &self,
        p: EmbeddedPreview,
        min_short_edge: Option<u32>,
    ) -> CoreResult<(RgbImage, Option<Vec<u8>>)> {
        match p {
            EmbeddedPreview::Jpeg(b) => {
                let d = self.inner.decode(&b, FileFormat::Jpeg, min_short_edge)?;
                Ok((d.image, d.icc_profile))
            }
            EmbeddedPreview::JpegXl(b) => {
                let d = self.inner.decode(&b, FileFormat::JpegXl, min_short_edge)?;
                Ok((d.image, d.icc_profile))
            }
            EmbeddedPreview::Bitmap(bm) => Ok((bitmap_to_rgb(bm)?, None)),
        }
    }
}

fn bitmap_to_rgb(b: Bitmap) -> CoreResult<RgbImage> {
    RgbImage::from_raw(b.width, b.height, b.rgb)
        .ok_or_else(|| CoreError::Corrupt("bitmap com tamanho inconsistente".into()))
}

/// Algumas câmaras (ex.: Nikon D70, Kodak DCS) gravam a preview embutida **já rodada**.
/// Se a proporção da preview (retrato/paisagem) não coincidir com a do sensor numa
/// orientação de 90°, desfaz-se a rotação, para que o pipeline comum aplique a orientação
/// EXIF uma única vez, como para qualquer outra imagem.
pub(crate) fn normalize_preview_rotation(
    img: RgbImage,
    sensor_w: u32,
    sensor_h: u32,
    orientation: u8,
) -> RgbImage {
    let rotated_90 = matches!(orientation, 5..=8);
    let sensor_landscape = sensor_w >= sensor_h;
    let preview_landscape = img.width() >= img.height();
    if !rotated_90 || sensor_w == sensor_h || sensor_landscape == preview_landscape {
        return img;
    }
    match orientation {
        6 => image::imageops::rotate270(&img), // inverso de rotate90
        8 => image::imageops::rotate90(&img),  // inverso de rotate270
        5 => image::imageops::flip_horizontal(&image::imageops::rotate90(&img)),
        7 => image::imageops::flip_horizontal(&image::imageops::rotate270(&img)),
        _ => img,
    }
}

fn partial_note(info: &crate::RawInfo) -> String {
    format!(
        "Só a preview embutida pode ser mostrada: a compressão dos dados do sensor deste ficheiro não é suportada pela LibRaw (descodificador {}).",
        if info.decoder.is_empty() { "desconhecido" } else { &info.decoder }
    )
}

impl ImageCodec for RawCodec {
    fn id(&self) -> &'static str {
        "libraw"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
        let ok = format.family() == FormatFamily::Raw;
        CodecCapabilities { probe: ok, decode: ok, encode: false }
    }

    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        let raw = RawFile::open(data)?;
        let i = raw.info();
        if i.width == 0 || i.height == 0 {
            return Err(CoreError::Corrupt("RAW sem dimensões".into()));
        }
        Ok(ImageProbe {
            format,
            width: i.width,
            height: i.height,
            bits_per_sample: i.bits,
            has_alpha: false,
        })
    }

    fn decode(
        &self,
        data: &[u8],
        _format: FileFormat,
        min_short_edge: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        let mut raw = RawFile::open(data)?;
        let info = raw.info().clone();
        let (sw, sh) = (info.width, info.height);
        let mk = |image: RgbImage, icc, kind| DecodedImage {
            image,
            icc_profile: icc,
            source_width: sw,
            source_height: sh,
            kind,
        };

        // 1) Preview embutida: suficiente para miniaturas e previews quando é grande o bastante.
        let want_preview = min_short_edge.is_some() || !info.sensor_supported;
        if want_preview {
            if let Some((_, _, pw, ph)) = info.preview {
                let big_enough = match min_short_edge {
                    Some(min) => preview_acceptable(pw, ph, min) || (pw == 0 && ph == 0),
                    None => true,
                };
                if big_enough || !info.sensor_supported {
                    match raw.embedded_preview() {
                        Ok(Some(p)) => {
                            let (img, icc) = self.decode_preview(p, min_short_edge)?;
                            let img = normalize_preview_rotation(img, sw, sh, info.orientation);
                            let (iw, ih) = img.dimensions();
                            if !info.sensor_supported
                                || min_short_edge.is_none_or(|m| preview_acceptable(iw, ih, m))
                            {
                                return Ok(mk(img, icc, DecodeKind::EmbeddedPreview));
                            }
                        }
                        Ok(None) => {}
                        Err(e) if !info.sensor_supported => return Err(e),
                        Err(_) => {} // tenta revelar
                    }
                }
            }
            if !info.sensor_supported {
                return Err(CoreError::UnsupportedFormat(partial_note(&info)));
            }
        }

        // 2) Revelação: meia resolução se chegar para o pedido, senão completa.
        let half = min_short_edge.is_some_and(|m| sw.min(sh) / 2 >= m);
        let bm = raw.develop(half)?;
        let kind = if half { DecodeKind::HalfSize } else { DecodeKind::Full };
        Ok(mk(bitmap_to_rgb(bm)?, None, kind))
    }

    fn metadata(&self, data: &[u8], _format: FileFormat) -> Option<CodecMetadata> {
        let raw = RawFile::open(data).ok()?;
        let i = raw.info();
        let make = i.make.clone();
        let support = if i.sensor_supported {
            SupportLevel::Full
        } else if i.preview.is_some() {
            SupportLevel::Partial(partial_note(i))
        } else {
            SupportLevel::Partial(format!(
                "Formato reconhecido mas sem dados de imagem descodificáveis (descodificador {}).",
                i.decoder
            ))
        };
        let color_space = if i.is_foveon {
            "Câmara (sensor Foveon)".to_string()
        } else if i.color_description.is_empty() {
            "Câmara".to_string()
        } else {
            format!("Câmara ({}, {} cores)", i.color_description, i.colors)
        };
        Some(CodecMetadata {
            orientation: Some(i.orientation),
            capture_time: i.capture_time.clone(),
            camera_make: (!make.is_empty()).then_some(make),
            camera_model: (!i.model.is_empty()).then(|| i.model.clone()),
            lens: i.lens.clone(),
            iso: i.iso,
            exposure_time: i.shutter,
            f_number: i.aperture,
            focal_length: i.focal_length,
            focal_length_35mm: i.focal_length_35mm,
            color_space: Some(color_space),
            embedded_preview: i.preview.map(|(_, _, w, h)| (w, h)),
            detail: Some(format!(
                "LibRaw {} · descodificador {} · {} bits{}",
                crate::libraw_version(),
                i.decoder,
                i.bits,
                if i.is_dng { " · DNG" } else { "" }
            )),
            support: Some(support),
        })
    }
}

#[cfg(test)]
mod tests;
