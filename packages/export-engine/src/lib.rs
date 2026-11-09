//! # momento-export-engine
//!
//! Motor de exportação (Fase 9). Recebe a imagem final da revelação em Rec. 2020 linear
//! (sempre a partir do original + receita, nunca de uma preview) e produz o ficheiro:
//! dimensões e recorte, nitidez de saída, marca de água, conversão de cor (perfil ICC),
//! profundidade, codificação e metadados. Os nomes e as pastas também são decididos aqui.

pub mod codecs;
pub mod metadata;
pub mod naming;
pub mod presets;
pub mod process;
pub mod recipe;

pub use metadata::PhotoMeta;
pub use recipe::*;

use momento_color_management::{ColorSpace, OutputColor};
use momento_core_types::{CoreError, CoreResult};
use momento_image_processing::develop::LinearImage;

/// Receita preparada para uma exportação: perfil de cor, marca de água e notas da
/// normalização (carregados uma vez, usados em todas as fotografias).
pub struct Prepared {
    pub recipe: Recipe,
    pub notes: Vec<&'static str>,
    color: OutputColor,
    mark: Option<process::Mark>,
}

impl Prepared {
    pub fn new(r: &Recipe) -> CoreResult<Self> {
        let (recipe, notes) = r.normalized();
        let caps = recipe.format.caps();
        if !caps.available {
            return Err(CoreError::UnsupportedFormat(format!(
                "{:?}: {}",
                recipe.format,
                caps.reason.unwrap_or("")
            )));
        }
        let color = if recipe.color == ColorSpace::Icc {
            let icc = std::fs::read(&recipe.icc_path)
                .map_err(|e| CoreError::Other(format!("perfil ICC {}: {e}", recipe.icc_path)))?;
            OutputColor::custom(&icc, recipe.intent)?
        } else {
            OutputColor::builtin(recipe.color, caps.linear || recipe.depth.is_float())?
        };
        let mark = process::prepare_mark(&recipe.watermark)?;
        Ok(Prepared { recipe, notes, color, mark })
    }

    pub fn extension(&self) -> &'static str {
        self.recipe.format.caps().extension
    }

    pub fn color_description(&self) -> &str {
        &self.color.description
    }

    /// Dimensões finais para uma imagem revelada `w`×`h`.
    pub fn output_size(&self, w: u32, h: u32) -> (u32, u32) {
        let p = process::plan(&self.recipe.size, w, h);
        (p.w, p.h)
    }

    /// Produz o ficheiro (bytes) a partir da imagem revelada em resolução total.
    pub fn produce(&self, img: &LinearImage, photo: &PhotoMeta) -> CoreResult<Produced> {
        let r = &self.recipe;
        let plan = process::plan(&r.size, img.width, img.height);
        let mut px = process::resample(&img.px, img.width as usize, img.height as usize, &plan);
        let (w, h) = (plan.w as usize, plan.h as usize);
        process::sharpen(&mut px, w, h, r);
        if let Some(m) = &self.mark {
            process::watermark(&mut px, w, h, &r.watermark, m);
        }
        self.color.convert(&mut px);
        let caps = r.format.caps();
        let meta = metadata::OutMeta::new(
            &r.metadata,
            photo,
            plan.w,
            plan.h,
            r.size.ppi,
            self.color.space == ColorSpace::Srgb,
        );
        let (exif, xmp) = if caps.metadata {
            (metadata::exif(&meta), metadata::xmp(&meta))
        } else {
            (None, None)
        };
        let ctx = codecs::Ctx {
            recipe: r,
            space: self.color.space,
            icc: self.color.icc(),
            definition: self.color.definition(),
            meta: &meta,
            exif,
            xmp,
        };
        let bytes = codecs::encode(&codecs::Pixels { w: plan.w, h: plan.h, px: &px }, &ctx)?;
        Ok(Produced { bytes, width: plan.w, height: plan.h })
    }

    /// Tamanho estimado (bytes) de um ficheiro `w`×`h` desta receita, antes de exportar.
    pub fn estimate(&self, w: u32, h: u32) -> u64 {
        let (ow, oh) = self.output_size(w, h);
        (estimate_bpp(&self.recipe) * ow as f64 * oh as f64) as u64 + 16 * 1024
    }
}

pub struct Produced {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

fn jpeg_bpp(q: u8) -> f64 {
    // Bytes por píxel medidos em fotografias típicas (interpolação linear).
    const T: [(f64, f64); 7] = [
        (50.0, 0.09),
        (60.0, 0.12),
        (75.0, 0.17),
        (80.0, 0.2),
        (85.0, 0.25),
        (90.0, 0.32),
        (95.0, 0.45),
    ];
    let q = q as f64;
    if q >= 95.0 {
        return 0.45 + (q - 95.0) / 5.0 * 0.45;
    }
    if q <= 50.0 {
        return 0.09 * q / 50.0 + 0.02;
    }
    let i = T.iter().position(|t| t.0 >= q).unwrap_or(T.len() - 1).max(1);
    let (a, b) = (T[i - 1], T[i]);
    a.1 + (b.1 - a.1) * (q - a.0) / (b.0 - a.0)
}

/// Bytes por píxel esperados (estimativa inicial; a fila corrige-a com os ficheiros reais).
pub fn estimate_bpp(r: &Recipe) -> f64 {
    let d = r.depth;
    let wide = |b8: f64, b16: f64, f32_: f64| match d {
        BitDepth::B8 => b8,
        BitDepth::F32 => f32_,
        _ => b16,
    };
    match r.format {
        Format::Jpeg | Format::Pdf => jpeg_bpp(r.quality),
        Format::Webp if r.compression == Compression::Lossless => 1.8,
        Format::Webp => jpeg_bpp(r.quality) * 0.7,
        Format::Avif => jpeg_bpp(r.quality) * 0.5 * if d == BitDepth::B8 { 1.0 } else { 1.2 },
        Format::Png => wide(2.2, 4.5, 4.5),
        Format::JpegXl => wide(1.6, 3.5, 3.5),
        Format::Jpeg2000 if r.compression == Compression::Lossless => match d {
            BitDepth::B8 => 1.8,
            BitDepth::B12 => 2.6,
            _ => 3.4,
        },
        Format::Jpeg2000 => jpeg_bpp(r.quality),
        Format::Tiff | Format::BigTiff => {
            let raw = wide(3.0, 6.0, 12.0);
            if r.compression == Compression::None {
                raw
            } else {
                raw * 0.7
            }
        }
        Format::Exr => {
            let raw = if d == BitDepth::F16 { 6.0 } else { 12.0 };
            if r.compression == Compression::None {
                raw
            } else {
                raw * 0.6
            }
        }
        Format::Psd => {
            let raw = wide(3.0, 6.0, 6.0);
            if r.compression == Compression::Rle {
                raw * 0.85
            } else {
                raw
            }
        }
        Format::Heif | Format::Heic | Format::Dng => 6.0,
    }
}

#[cfg(test)]
mod quality_tests;
