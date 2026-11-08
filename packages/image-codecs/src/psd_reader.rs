//! Leitor próprio da **imagem composta** de ficheiros PSD (v1) e PSB (v2).
//!
//! O Photoshop grava, no fim do ficheiro, uma cópia já composta de todas as camadas
//! (secção “Image Data”). Ler só essa secção é rápido e robusto: não é preciso interpretar
//! camadas, máscaras nem efeitos. Formato segundo a especificação pública da Adobe
//! (“Adobe Photoshop File Formats Specification”).
//!
//! Suportado: 8 e 16 bits; RGB, escala de cinzentos, CMYK (conversão simplificada) e
//! bitmap de 1 bit; compressão RAW e RLE (PackBits). Não suportado: ZIP, Lab, indexado.

use momento_core_types::{CoreError, CoreResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Bitmap,
    Grayscale,
    Indexed,
    Rgb,
    Cmyk,
    Multichannel,
    Duotone,
    Lab,
    Other(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsdHeader {
    pub psb: bool,
    pub channels: u16,
    pub width: u32,
    pub height: u32,
    pub depth: u16,
    pub mode: ColorMode,
}

struct R<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> CoreResult<&'a [u8]> {
        let end = self
            .p
            .checked_add(n)
            .filter(|e| *e <= self.b.len())
            .ok_or_else(|| bad("fim inesperado do ficheiro"))?;
        let s = &self.b[self.p..end];
        self.p = end;
        Ok(s)
    }
    fn u16(&mut self) -> CoreResult<u16> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> CoreResult<u32> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self) -> CoreResult<u64> {
        let s = self.take(8)?;
        Ok(u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }
    fn skip_section(&mut self, long: bool) -> CoreResult<()> {
        let len = if long { self.u64()? as usize } else { self.u32()? as usize };
        self.take(len).map(|_| ())
    }
}

fn bad(m: &str) -> CoreError {
    CoreError::Corrupt(format!("PSD: {m}"))
}

pub fn header(data: &[u8]) -> CoreResult<PsdHeader> {
    let mut r = R { b: data, p: 0 };
    if r.take(4)? != b"8BPS" {
        return Err(bad("assinatura inválida"));
    }
    let version = r.u16()?;
    if version != 1 && version != 2 {
        return Err(bad("versão desconhecida"));
    }
    r.take(6)?;
    let channels = r.u16()?;
    let height = r.u32()?;
    let width = r.u32()?;
    let depth = r.u16()?;
    let mode = match r.u16()? {
        0 => ColorMode::Bitmap,
        1 => ColorMode::Grayscale,
        2 => ColorMode::Indexed,
        3 => ColorMode::Rgb,
        4 => ColorMode::Cmyk,
        7 => ColorMode::Multichannel,
        8 => ColorMode::Duotone,
        9 => ColorMode::Lab,
        o => ColorMode::Other(o),
    };
    let max = if version == 1 { 30_000 } else { 300_000 };
    if channels == 0
        || channels > 56
        || width == 0
        || height == 0
        || width > max
        || height > max
        || ![1, 8, 16, 32].contains(&depth)
    {
        return Err(bad("cabeçalho com valores inválidos"));
    }
    Ok(PsdHeader { psb: version == 2, channels, width, height, depth, mode })
}

/// Verifica a estrutura (secções e tamanho dos dados da imagem composta) sem descodificar
/// os píxeis. Deteta ficheiros truncados ou malformados no momento da importação.
pub fn validate(data: &[u8]) -> CoreResult<PsdHeader> {
    let h = header(data)?;
    let mut r = R { b: data, p: 26 };
    r.skip_section(false)?;
    r.skip_section(false)?;
    r.skip_section(h.psb)?;
    let compression = r.u16()?;
    let (w, ht) = (h.width as usize, h.height as usize);
    let row_bytes = match h.depth {
        1 => w.div_ceil(8),
        d => w * (d as usize / 8),
    };
    let channels = h.channels as usize;
    match compression {
        0 => {
            let need = row_bytes
                .checked_mul(ht)
                .and_then(|p| p.checked_mul(channels.min(4)))
                .ok_or_else(|| bad("imagem demasiado grande"))?;
            r.take(need)?;
        }
        1 => {
            let mut total = 0usize;
            for _ in 0..ht * channels {
                total += if h.psb { r.u32()? as usize } else { r.u16()? as usize };
            }
            r.take(total)?;
        }
        2 | 3 => {
            return Err(CoreError::UnsupportedFormat("PSD com compressão ZIP não suportado".into()))
        }
        _ => return Err(bad("compressão desconhecida")),
    }
    Ok(h)
}

/// Descomprime PackBits para exatamente `out_len` bytes.
fn packbits(src: &[u8], out: &mut Vec<u8>, out_len: usize) -> CoreResult<()> {
    let start = out.len();
    let mut i = 0;
    while out.len() - start < out_len {
        let n = *src.get(i).ok_or_else(|| bad("RLE truncado"))? as i8;
        i += 1;
        if n >= 0 {
            let c = n as usize + 1;
            out.extend_from_slice(src.get(i..i + c).ok_or_else(|| bad("RLE truncado"))?);
            i += c;
        } else if n != -128 {
            let v = *src.get(i).ok_or_else(|| bad("RLE truncado"))?;
            i += 1;
            out.extend(std::iter::repeat_n(v, (1 - n as isize) as usize));
        }
    }
    out.truncate(start + out_len);
    Ok(())
}

/// Imagem composta em RGB 8 bits (transparência ignorada: o composto já é opaco).
pub fn composite_rgb(data: &[u8]) -> CoreResult<(PsdHeader, Vec<u8>)> {
    let h = header(data)?;
    let mut r = R { b: data, p: 26 };
    r.skip_section(false)?; // Color Mode Data
    r.skip_section(false)?; // Image Resources
    r.skip_section(h.psb)?; // Layer and Mask Information (8 bytes de tamanho no PSB)
    let compression = r.u16()?;
    let (w, ht) = (h.width as usize, h.height as usize);
    let row_bytes = match h.depth {
        1 => w.div_ceil(8),
        d => w * (d as usize / 8),
    };
    let plane = row_bytes.checked_mul(ht).ok_or_else(|| bad("imagem demasiado grande"))?;
    let needed_channels = match h.mode {
        ColorMode::Rgb => 3,
        ColorMode::Cmyk => 4,
        ColorMode::Grayscale | ColorMode::Bitmap | ColorMode::Duotone | ColorMode::Multichannel => {
            1
        }
        _ => {
            return Err(CoreError::UnsupportedFormat(format!(
                "PSD em modo {:?} não suportado",
                h.mode
            )))
        }
    }
    .min(h.channels as usize);
    let mut planes = Vec::with_capacity(plane * needed_channels);
    match compression {
        0 => planes.extend_from_slice(r.take(plane * needed_channels)?),
        1 => {
            let counts_len = if h.psb { 4 } else { 2 };
            let total_rows = ht * h.channels as usize;
            let mut counts = Vec::with_capacity(total_rows);
            for _ in 0..total_rows {
                counts.push(if h.psb { r.u32()? as usize } else { r.u16()? as usize });
            }
            let _ = counts_len;
            for ch in 0..needed_channels {
                for row in 0..ht {
                    let c = counts[ch * ht + row];
                    packbits(r.take(c)?, &mut planes, row_bytes)?;
                }
            }
        }
        2 | 3 => {
            return Err(CoreError::UnsupportedFormat("PSD com compressão ZIP não suportado".into()))
        }
        _ => return Err(bad("compressão desconhecida")),
    }
    let sample = |ch: usize, x: usize, y: usize| -> u8 {
        let off = ch * plane + y * row_bytes;
        match h.depth {
            1 => {
                let byte = planes[off + x / 8];
                if byte & (0x80 >> (x % 8)) != 0 {
                    0
                } else {
                    255
                } // 1 = preto
            }
            8 => planes[off + x],
            16 => planes[off + x * 2], // byte mais significativo
            _ => {
                let i = off + x * 4;
                let f =
                    f32::from_be_bytes([planes[i], planes[i + 1], planes[i + 2], planes[i + 3]]);
                (f.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as u8
            }
        }
    };
    let mut rgb = Vec::with_capacity(w * ht * 3);
    for y in 0..ht {
        for x in 0..w {
            match (h.mode, needed_channels) {
                (ColorMode::Rgb, 3) => {
                    rgb.extend_from_slice(&[sample(0, x, y), sample(1, x, y), sample(2, x, y)])
                }
                (ColorMode::Cmyk, 4) => {
                    // Valores PSD em CMYK estão invertidos (255 = sem tinta).
                    let k = sample(3, x, y) as u32;
                    let f = |c: u8| (c as u32 * k / 255) as u8;
                    rgb.extend_from_slice(&[
                        f(sample(0, x, y)),
                        f(sample(1, x, y)),
                        f(sample(2, x, y)),
                    ]);
                }
                _ => {
                    let v = sample(0, x, y);
                    rgb.extend_from_slice(&[v, v, v]);
                }
            }
        }
    }
    Ok((h, rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constrói um PSD mínimo (RGB 8 bits, 3×2) com compressão RAW ou RLE.
    pub fn tiny_psd(rle: bool, psb: bool) -> Vec<u8> {
        let (w, h) = (3u32, 2u32);
        let mut v = b"8BPS".to_vec();
        v.extend_from_slice(&(if psb { 2u16 } else { 1u16 }).to_be_bytes());
        v.extend_from_slice(&[0; 6]);
        v.extend_from_slice(&3u16.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&8u16.to_be_bytes());
        v.extend_from_slice(&3u16.to_be_bytes());
        v.extend_from_slice(&0u32.to_be_bytes()); // color mode data
        v.extend_from_slice(&0u32.to_be_bytes()); // image resources
        if psb {
            v.extend_from_slice(&0u64.to_be_bytes())
        } else {
            v.extend_from_slice(&0u32.to_be_bytes())
        }
        let planes: [[u8; 6]; 3] =
            [[255, 0, 0, 10, 20, 30], [0, 255, 0, 10, 20, 30], [0, 0, 255, 10, 20, 30]];
        if !rle {
            v.extend_from_slice(&0u16.to_be_bytes());
            for p in planes {
                v.extend_from_slice(&p);
            }
        } else {
            v.extend_from_slice(&1u16.to_be_bytes());
            // Cada linha (3 bytes) codificada como literal: [2, a, b, c] → 4 bytes.
            for _ in 0..(3 * h) {
                if psb {
                    v.extend_from_slice(&4u32.to_be_bytes())
                } else {
                    v.extend_from_slice(&4u16.to_be_bytes())
                }
            }
            for p in planes {
                for row in p.chunks(3) {
                    v.push(2);
                    v.extend_from_slice(row);
                }
            }
        }
        v
    }

    #[test]
    fn reads_raw_rle_and_psb_composites() {
        for (rle, psb) in [(false, false), (true, false), (true, true), (false, true)] {
            let (h, rgb) = composite_rgb(&tiny_psd(rle, psb)).unwrap();
            assert_eq!((h.width, h.height, h.psb), (3, 2, psb));
            assert_eq!(&rgb[0..9], &[255, 0, 0, 0, 255, 0, 0, 0, 255], "rle={rle} psb={psb}");
            assert_eq!(&rgb[9..12], &[10, 10, 10]);
        }
    }

    #[test]
    fn validate_detects_truncation() {
        let good = tiny_psd(true, false);
        assert!(validate(&good).is_ok());
        assert!(validate(&good[..good.len() - 1]).is_err());
        assert!(validate(&tiny_psd(false, true)).is_ok());
    }

    #[test]
    fn packbits_repeat_runs() {
        let mut out = Vec::new();
        packbits(&[0xFE, 7, 1, 1, 2], &mut out, 5).unwrap(); // 3×7 e literal [1, 2]
        assert_eq!(out, [7, 7, 7, 1, 2]);
    }

    #[test]
    fn truncated_or_garbage_never_panics() {
        let good = tiny_psd(true, false);
        for n in 0..good.len() {
            let _ = composite_rgb(&good[..n]);
        }
        let mut x = 7u32;
        for _ in 0..2000 {
            let mut v = good.clone();
            for _ in 0..4 {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let i = (x as usize) % v.len();
                v[i] = x as u8;
            }
            let _ = composite_rgb(&v);
        }
    }
}
