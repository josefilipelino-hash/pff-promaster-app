//! JPEG 2000 (JP2) com o OpenJPEG em Rust (crate openjp2): sem perdas (5/3 reversível)
//! ou com perdas (9/7, qualidade → PSNR alvo); 8, 12 ou 16 bits; perfil ICC na caixa
//! `colr`. Os metadados EXIF/XMP não são escritos (o openjp2 não tem caixas para eles).
//!
//! Sem perdas só a 8 bits: a 12/16 bits o OpenJPEG de referência (C) não reproduz
//! exatamente o que este porte escreve (verificado com `scripts/validate-exports.py`); a
//! receita é ajustada com uma nota (`Recipe::normalized`).

use super::{err, to_u8, to_uint, Ctx, Pixels};
use crate::recipe::{BitDepth, Compression};
use momento_core_types::CoreResult;
use openjp2::{
    opj_cparameters_t, opj_image, opj_image_comptparm, Codec, Stream, CODEC_FORMAT, COLOR_SPACE,
};

pub fn encode(p: &Pixels, c: &Ctx) -> CoreResult<Vec<u8>> {
    let bits: u32 = match c.recipe.depth {
        BitDepth::B12 => 12,
        BitDepth::B16 => 16,
        _ => 8,
    };
    let (w, h) = (p.w, p.h);
    let comp =
        opj_image_comptparm { dx: 1, dy: 1, w, h, x0: 0, y0: 0, prec: bits, bpp: bits, sgnd: 0 };
    let mut image = opj_image::create(&[comp; 3], COLOR_SPACE::OPJ_CLRSPC_SRGB)
        .ok_or_else(|| err("JPEG 2000", "imagem"))?;
    image.x0 = 0;
    image.y0 = 0;
    image.x1 = w;
    image.y1 = h;
    let data: Vec<u16> =
        if bits == 8 { to_u8(p).into_iter().map(u16::from).collect() } else { to_uint(p, bits) };
    if let Some(planes) = image.comps_data_mut_iter() {
        for (ch, plane) in planes.enumerate() {
            for (i, v) in plane.iter_mut().enumerate() {
                *v = data[i * 3 + ch] as i32;
            }
        }
    }
    let mut params = opj_cparameters_t::default();
    unsafe { openjp2::openjpeg::opj_set_default_encoder_parameters(&mut params) };
    params.tcp_numlayers = 1;
    params.tcp_mct = 1;
    // Níveis de resolução: até 6, limitados pelo lado menor.
    params.numresolution = (1 + (w.min(h) as f32).log2().floor() as i32 - 5).clamp(1, 6);
    if c.recipe.compression == Compression::Lossless {
        params.irreversible = 0;
        params.tcp_rates[0] = 0.0;
        params.cp_disto_alloc = 1;
    } else {
        params.irreversible = 1;
        // Qualidade 1–100 → PSNR alvo 30–60 dB.
        params.tcp_distoratio[0] = 30.0 + c.recipe.quality as f32 * 0.3;
        params.cp_fixed_quality = 1;
    }
    let mut codec = Codec::new_encoder(CODEC_FORMAT::OPJ_CODEC_JP2)
        .ok_or_else(|| err("JPEG 2000", "codificador"))?;
    if codec.setup_encoder(&mut params, &mut image) == 0 {
        return Err(err("JPEG 2000", "parâmetros"));
    }
    // O openjp2 só escreve para ficheiro: temporário privado, lido e apagado.
    let tmp = tempfile::Builder::new().suffix(".jp2").tempfile()?;
    {
        let mut stream = Stream::new_file(tmp.path(), 1 << 20, false)?;
        let ok = codec.start_compress(&mut image, &mut stream) != 0
            && codec.encode(&mut stream) != 0
            && codec.end_compress(&mut stream) != 0;
        if !ok {
            return Err(err("JPEG 2000", "compressão"));
        }
    }
    drop(codec);
    // O openjp2 (porte em Rust) não copia o perfil ICC para a caixa `colr`: substituímos
    // a caixa enumerada (sRGB) por uma com o perfil (método 2, ICC restrito).
    with_icc(&std::fs::read(tmp.path())?, c.icc)
}

fn boxes(b: &[u8]) -> CoreResult<Vec<(&[u8], &[u8])>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 8 <= b.len() {
        let len = u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
        let len = if len == 0 { b.len() - i } else { len };
        if len < 8 || i + len > b.len() {
            return Err(err("JPEG 2000", "caixa inválida"));
        }
        out.push((&b[i + 4..i + 8], &b[i + 8..i + len]));
        i += len;
    }
    Ok(out)
}

fn put_box(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
    out.extend(((data.len() + 8) as u32).to_be_bytes());
    out.extend(kind);
    out.extend(data);
}

fn with_icc(jp2: &[u8], icc: &[u8]) -> CoreResult<Vec<u8>> {
    let mut out = Vec::with_capacity(jp2.len() + icc.len());
    for (kind, data) in boxes(jp2)? {
        if kind == b"jp2h" {
            let mut h = Vec::new();
            for (k, d) in boxes(data)? {
                if k == b"colr" {
                    let mut colr = vec![2u8, 0, 0];
                    colr.extend(icc);
                    put_box(&mut h, b"colr", &colr);
                } else {
                    put_box(&mut h, k, d);
                }
            }
            put_box(&mut out, b"jp2h", &h);
        } else {
            put_box(&mut out, kind, data);
        }
    }
    Ok(out)
}
