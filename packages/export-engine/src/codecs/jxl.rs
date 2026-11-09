//! JPEG XL **sem perdas** (codificador zune-jpegxl, 8/16 bits).
//!
//! O zune-jpegxl escreve sempre "sRGB" no cabeçalho. Como o cabeçalho da imagem acaba
//! alinhado ao byte, reescrevemo-lo com a codificação de cor certa (primárias, branco e
//! curva enumerados ou personalizados — sem ICC) e mantemos o resto do fluxo intacto. O
//! resultado vai num contentor ISOBMFF com caixas Exif e XMP.

use super::{err, to_u8, to_uint, Ctx, Pixels};
use crate::recipe::BitDepth;
use momento_color_management::{Curve, Primaries, D65, P3, REC2020, REC709};
use momento_core_types::{CoreError, CoreResult};

/// Escritor de bits LSB-primeiro (como o JPEG XL).
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn put(&mut self, nbits: u32, v: u64) {
        self.acc |= v << self.n;
        self.n += nbits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    fn pad(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
    /// Enum = U32(Val(0), Val(1), BitsOffset(4, 2), BitsOffset(6, 18)).
    fn enm(&mut self, v: u32) {
        match v {
            0 => self.put(2, 0),
            1 => self.put(2, 1),
            2..=17 => {
                self.put(2, 2);
                self.put(4, (v - 2) as u64);
            }
            _ => {
                self.put(2, 3);
                self.put(6, (v - 18) as u64);
            }
        }
    }
    /// Coordenada xy: U32(Bits(19), BitsOffset(19, 524288), BitsOffset(20, 1048576),
    /// BitsOffset(21, 2097152)) de PackSigned(round(v × 10⁶)).
    fn xy(&mut self, v: f64) {
        let i = (v * 1e6).round() as i64;
        let p = if i >= 0 { 2 * i } else { -2 * i - 1 } as u64;
        if p < 524_288 {
            self.put(2, 0);
            self.put(19, p);
        } else if p < 1_048_576 {
            self.put(2, 1);
            self.put(19, p - 524_288);
        } else if p < 2_097_152 {
            self.put(2, 2);
            self.put(20, p - 1_048_576);
        } else {
            self.put(2, 3);
            self.put(21, p - 2_097_152);
        }
    }
}

/// Codificação de cor do cabeçalho; `None` = predefinição (sRGB).
#[derive(Clone, Copy)]
struct Color {
    p: Primaries,
    curve: Curve,
}

fn size(b: &mut Bits, s: u32) {
    let v = (s - 1) as u64;
    if v < 1 << 9 {
        b.put(2, 0);
        b.put(9, v);
    } else if v < 1 << 13 {
        b.put(2, 1);
        b.put(13, v);
    } else if v < 1 << 18 {
        b.put(2, 2);
        b.put(18, v);
    } else {
        b.put(2, 3);
        b.put(30, v);
    }
}

/// Assinatura + SizeHeader + ImageMetadata, exatamente como o zune-jpegxl (RGB sem
/// alfa), com a codificação de cor pedida.
fn header(w: u32, h: u32, sixteen: bool, color: Option<Color>) -> Vec<u8> {
    let mut b = Bits::default();
    b.put(16, 0x0AFF);
    b.put(1, 0); // não "small"
    size(&mut b, h);
    b.put(3, 0); // sem razão de aspeto
    size(&mut b, w);
    b.put(1, 0); // metadados não predefinidos
    b.put(1, 0); // sem campos extra
    b.put(1, 0); // inteiros
    if sixteen {
        b.put(2, 3);
        b.put(6, 15);
    } else {
        b.put(2, 0);
    }
    b.put(1, if sixteen { 0 } else { 1 }); // buffer de 16 bits suficiente (≤ 14 bits)
    b.put(2, 0); // sem canais extra
    b.put(1, 0); // não XYB
    match color {
        None => b.put(1, 1),
        Some(c) => {
            b.put(1, 0); // all_default
            b.put(1, 0); // want_icc
            b.enm(0); // RGB
                      // Branco.
            if c.p.white == D65 {
                b.enm(1);
            } else {
                b.enm(2);
                b.xy(c.p.white.0);
                b.xy(c.p.white.1);
            }
            // Primárias.
            let same = |a: &Primaries| a.r == c.p.r && a.g == c.p.g && a.b == c.p.b;
            if same(&REC709) {
                b.enm(1);
            } else if same(&REC2020) {
                b.enm(9);
            } else if same(&P3) {
                b.enm(11);
            } else {
                b.enm(2);
                for xy in [c.p.r, c.p.g, c.p.b] {
                    b.xy(xy.0);
                    b.xy(xy.1);
                }
            }
            // Curva.
            match c.curve {
                Curve::Gamma(g) => {
                    b.put(1, 1);
                    b.put(24, (1e7 / g).round() as u64);
                }
                other => {
                    b.put(1, 0);
                    b.enm(match other {
                        Curve::Bt709 => 1,
                        Curve::Linear => 8,
                        _ => 13,
                    });
                }
            }
            b.enm(1); // intento relativo
        }
    }
    b.put(2, 0); // sem extensões
    b.put(1, 1); // transformações predefinidas
    b.pad()
}

fn bn_box(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend(((data.len() + 8) as u32).to_be_bytes());
    out.extend(kind);
    out.extend(data);
}

pub fn encode(p: &Pixels, c: &Ctx) -> CoreResult<Vec<u8>> {
    use zune_core::bit_depth::BitDepth as ZD;
    use zune_core::colorspace::ColorSpace as ZC;
    use zune_core::options::EncoderOptions;
    if p.w < 2 || p.h < 2 {
        return Err(CoreError::Other("JPEG XL: mínimo de 2×2 píxeis".into()));
    }
    let sixteen = c.recipe.depth == BitDepth::B16;
    let data: Vec<u8> = if sixteen {
        to_uint(p, 16).iter().flat_map(|v| v.to_ne_bytes()).collect()
    } else {
        to_u8(p)
    };
    let opts = EncoderOptions::new(
        p.w as usize,
        p.h as usize,
        ZC::RGB,
        if sixteen { ZD::Sixteen } else { ZD::Eight },
    );
    let mut stream = Vec::new();
    zune_jpegxl::JxlSimpleEncoder::new(&data, opts)
        .encode(&mut stream)
        .map_err(|e| err("JPEG XL", format!("{e:?}")))?;
    let default = header(p.w, p.h, sixteen, None);
    if stream.len() < default.len() || stream[..default.len()] != default[..] {
        return Err(err("JPEG XL", "cabeçalho inesperado do codificador"));
    }
    let (prim, curve) = c
        .definition
        .ok_or_else(|| CoreError::UnsupportedFormat("JPEG XL: perfil ICC personalizado".into()))?;
    let srgb = prim.r == REC709.r && prim.white == D65 && curve == Curve::Srgb;
    let mut code = if srgb {
        default.clone()
    } else {
        header(p.w, p.h, sixteen, Some(Color { p: prim, curve }))
    };
    code.extend(&stream[default.len()..]);
    // Contentor: assinatura, ftyp, Exif, XMP, codestream.
    let mut out = vec![0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A];
    bn_box(&mut out, b"ftyp", b"jxl \0\0\0\0jxl ");
    if let Some(e) = &c.exif {
        let mut d = vec![0, 0, 0, 0];
        d.extend(e);
        bn_box(&mut out, b"Exif", &d);
    }
    if let Some(x) = &c.xmp {
        bn_box(&mut out, b"xml ", x.as_bytes());
    }
    bn_box(&mut out, b"jxlc", &code);
    Ok(out)
}
