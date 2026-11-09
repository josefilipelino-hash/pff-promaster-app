//! Codificação dos ficheiros de saída. Cada formato recebe os píxeis **já convertidos**
//! para o espaço de saída (valores 0–1 codificados com a curva do espaço; lineares no EXR
//! e nos TIFF de vírgula flutuante), o perfil ICC e os metadados.

mod avif;
mod jp2;
mod jxl;
mod simple;

use crate::metadata::OutMeta;
use crate::recipe::{BitDepth, Compression, Format, Recipe};
use momento_color_management::{ColorSpace, Curve, Primaries};
use momento_core_types::{CoreError, CoreResult};

/// Imagem final no espaço de saída.
pub struct Pixels<'a> {
    pub w: u32,
    pub h: u32,
    pub px: &'a [[f32; 3]],
}

/// Contexto de codificação.
pub struct Ctx<'a> {
    pub recipe: &'a Recipe,
    pub space: ColorSpace,
    pub icc: &'a [u8],
    pub definition: Option<(Primaries, Curve)>,
    pub meta: &'a OutMeta,
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<String>,
}

pub fn err(fmt: &str, e: impl std::fmt::Display) -> CoreError {
    CoreError::Other(format!("codificação {fmt}: {e}"))
}

/// Ruído triangular determinístico (±1 LSB) para 8 bits: evita bandas em céus e fundos
/// lisos ao reduzir de vírgula flutuante para 256 níveis.
#[inline]
fn dither(i: usize, c: usize) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x9E37_79B9) ^ (c as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    let a = (h & 0xffff) as f32 / 65535.0;
    let b = (h >> 16) as f32 / 65535.0;
    a + b - 1.0
}

pub fn to_u8(p: &Pixels) -> Vec<u8> {
    let mut out = vec![0u8; p.px.len() * 3];
    use rayon::prelude::*;
    out.par_chunks_mut(3).enumerate().for_each(|(i, o)| {
        for (c, b) in o.iter_mut().enumerate() {
            let v = p.px[i][c].clamp(0.0, 1.0) * 255.0 + dither(i, c) * 0.5;
            *b = v.round().clamp(0.0, 255.0) as u8;
        }
    });
    out
}

/// Inteiros de `bits` bits (10, 12, 16), sem ruído.
pub fn to_uint(p: &Pixels, bits: u32) -> Vec<u16> {
    let max = ((1u32 << bits) - 1) as f32;
    p.px.iter().flat_map(|q| q.map(|v| (v.clamp(0.0, 1.0) * max).round() as u16)).collect()
}

pub fn to_f32(p: &Pixels) -> Vec<f32> {
    p.px.iter().flat_map(|q| *q).collect()
}

/// Codifica para os bytes do ficheiro final.
pub fn encode(p: &Pixels, ctx: &Ctx) -> CoreResult<Vec<u8>> {
    let r = ctx.recipe;
    match r.format {
        Format::Jpeg => simple::jpeg(p, ctx, r.quality),
        Format::Png => simple::png(p, ctx),
        Format::Tiff => simple::tiff(p, ctx, false),
        Format::BigTiff => simple::tiff(p, ctx, true),
        Format::Webp => simple::webp(p, ctx),
        Format::Psd => simple::psd(p, ctx),
        Format::Pdf => simple::pdf(p, ctx),
        Format::Exr => simple::exr(p, ctx),
        Format::Avif => avif::encode(p, ctx),
        Format::JpegXl => jxl::encode(p, ctx),
        Format::Jpeg2000 => jp2::encode(p, ctx),
        Format::Heif | Format::Heic | Format::Dng => {
            Err(CoreError::UnsupportedFormat(format!("{:?}", r.format)))
        }
    }
}

/// Profundidade efetiva (depois da normalização da receita).
pub fn depth_of(r: &Recipe) -> BitDepth {
    r.depth
}

pub fn lossless(r: &Recipe) -> bool {
    matches!(r.compression, Compression::Lossless)
}
