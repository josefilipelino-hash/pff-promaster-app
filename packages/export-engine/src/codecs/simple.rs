//! JPEG, PNG, TIFF/BigTIFF, WebP, PSD, PDF e OpenEXR.

use super::{err, to_f32, to_u8, to_uint, Ctx, Pixels};
use crate::metadata::ascii_fold;
use crate::recipe::{BitDepth, Compression};
use momento_core_types::{CoreError, CoreResult};
use std::io::Cursor;

const XMP_NS: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

pub fn jpeg(p: &Pixels, ctx: &Ctx, quality: u8) -> CoreResult<Vec<u8>> {
    if p.w > 65_535 || p.h > 65_535 {
        return Err(CoreError::Other("JPEG: máximo de 65 535 píxeis por lado".into()));
    }
    let data = to_u8(p);
    let mut out = Vec::new();
    let mut enc = jpeg_encoder::Encoder::new(&mut out, quality);
    // 4:4:4 em qualidade alta (sem perda de cor em contornos finos), 4:2:0 no resto.
    enc.set_sampling_factor(if quality >= 90 {
        jpeg_encoder::SamplingFactor::R_4_4_4
    } else {
        jpeg_encoder::SamplingFactor::R_4_2_0
    });
    enc.set_optimized_huffman_tables(true);
    let ppi = ctx.meta.ppi.round().clamp(1.0, 65_535.0) as u16;
    enc.set_density(jpeg_encoder::PixelDensity {
        density: (ppi, ppi),
        unit: jpeg_encoder::PixelDensityUnit::Inches,
    });
    if let Some(e) = &ctx.exif {
        enc.add_exif_metadata(e).map_err(|e| err("JPEG", e))?;
    }
    if let Some(x) = &ctx.xmp {
        let mut seg = XMP_NS.to_vec();
        seg.extend(x.as_bytes());
        enc.add_app_segment(1, seg).map_err(|e| err("JPEG", e))?;
    }
    enc.add_icc_profile(ctx.icc).map_err(|e| err("JPEG", e))?;
    enc.encode(&data, p.w as u16, p.h as u16, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| err("JPEG", e))?;
    Ok(out)
}

pub fn png(p: &Pixels, ctx: &Ctx) -> CoreResult<Vec<u8>> {
    let sixteen = ctx.recipe.depth == BitDepth::B16;
    let data: Vec<u8> = if sixteen {
        to_uint(p, 16).iter().flat_map(|v| v.to_be_bytes()).collect()
    } else {
        to_u8(p)
    };
    let mut info = png::Info::with_size(p.w, p.h);
    info.color_type = png::ColorType::Rgb;
    info.bit_depth = if sixteen { png::BitDepth::Sixteen } else { png::BitDepth::Eight };
    info.icc_profile = Some(std::borrow::Cow::Borrowed(ctx.icc));
    let ppm = (ctx.meta.ppi / 0.0254).round() as u32;
    info.pixel_dims = Some(png::PixelDimensions { xppu: ppm, yppu: ppm, unit: png::Unit::Meter });
    if let Some(e) = &ctx.exif {
        info.exif_metadata = Some(std::borrow::Cow::Borrowed(e));
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::with_info(&mut out, info).map_err(|e| err("PNG", e))?;
        enc.set_compression(match ctx.recipe.compression {
            Compression::Best => png::Compression::High,
            _ => png::Compression::Fast,
        });
        if let Some(x) = &ctx.xmp {
            enc.add_itxt_chunk("XML:com.adobe.xmp".into(), x.clone()).map_err(|e| err("PNG", e))?;
        }
        let mut w = enc.write_header().map_err(|e| err("PNG", e))?;
        w.write_image_data(&data).map_err(|e| err("PNG", e))?;
        w.finish().map_err(|e| err("PNG", e))?;
    }
    Ok(out)
}

fn tiff_tags<W: std::io::Write + std::io::Seek, K: tiff::encoder::TiffKind>(
    d: &mut tiff::encoder::DirectoryEncoder<W, K>,
    ctx: &Ctx,
) -> tiff::TiffResult<()> {
    use tiff::tags::Tag;
    d.write_tag(Tag::Software, "Momento Studio")?;
    d.write_tag(Tag::Unknown(34675), ctx.icc)?;
    let m = ctx.meta;
    if !m.creator.is_empty() {
        d.write_tag(Tag::Artist, ascii_fold(&m.creator).as_str())?;
    }
    if !m.copyright.is_empty() {
        d.write_tag(Tag::Copyright, ascii_fold(&m.copyright).as_str())?;
    }
    if let Some(c) = &m.camera {
        if let Some(v) = &c.make {
            d.write_tag(Tag::Make, ascii_fold(v).as_str())?;
        }
        if let Some(v) = &c.model {
            d.write_tag(Tag::Model, ascii_fold(v).as_str())?;
        }
        if let Some(t) = &c.capture_time {
            if t.len() >= 19 {
                let s = format!("{}:{}:{} {}", &t[0..4], &t[5..7], &t[8..10], &t[11..19]);
                d.write_tag(Tag::DateTime, s.as_str())?;
            }
        }
    }
    if let Some(x) = &ctx.xmp {
        d.write_tag(Tag::Unknown(700), x.as_bytes())?;
    }
    Ok(())
}

fn tiff_write<K: tiff::encoder::TiffKind>(
    mut enc: tiff::encoder::TiffEncoder<&mut Cursor<Vec<u8>>, K>,
    p: &Pixels,
    ctx: &Ctx,
) -> tiff::TiffResult<()> {
    use tiff::encoder::{colortype, Rational};
    use tiff::tags::ResolutionUnit;
    let res = Rational { n: (ctx.meta.ppi * 100.0).round() as u32, d: 100 };
    macro_rules! write {
        ($ct:ty, $data:expr) => {{
            let mut img = enc.new_image::<$ct>(p.w, p.h)?;
            img.resolution(ResolutionUnit::Inch, res);
            tiff_tags(img.encoder(), ctx)?;
            img.write_data(&$data)?;
        }};
    }
    match ctx.recipe.depth {
        BitDepth::B16 => write!(colortype::RGB16, to_uint(p, 16)),
        BitDepth::F32 => write!(colortype::RGB32Float, to_f32(p)),
        _ => write!(colortype::RGB8, to_u8(p)),
    }
    Ok(())
}

pub fn tiff(p: &Pixels, ctx: &Ctx, big: bool) -> CoreResult<Vec<u8>> {
    use tiff::encoder::{compression::DeflateLevel, Compression as TC, Predictor, TiffEncoder};
    let comp = match ctx.recipe.compression {
        Compression::Lzw => TC::Lzw,
        Compression::Zip => TC::Deflate(DeflateLevel::Balanced),
        _ => TC::Uncompressed,
    };
    // Preditor horizontal: melhora muito LZW/ZIP em inteiros (não existe para float aqui).
    let pred = if comp != TC::Uncompressed && !ctx.recipe.depth.is_float() {
        Predictor::Horizontal
    } else {
        Predictor::None
    };
    let mut cur = Cursor::new(Vec::new());
    let name = if big { "BigTIFF" } else { "TIFF" };
    if big {
        let enc = TiffEncoder::new_big(&mut cur).map_err(|e| err(name, e))?;
        tiff_write(enc.with_compression(comp).with_predictor(pred), p, ctx)
    } else {
        let enc = TiffEncoder::new(&mut cur).map_err(|e| err(name, e))?;
        tiff_write(enc.with_compression(comp).with_predictor(pred), p, ctx)
    }
    .map_err(|e| err(name, e))?;
    Ok(cur.into_inner())
}

fn riff_chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
    out.extend(id);
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
}

pub fn webp(p: &Pixels, ctx: &Ctx) -> CoreResult<Vec<u8>> {
    if p.w > 16_383 || p.h > 16_383 {
        return Err(CoreError::Other("WebP: máximo de 16 383 píxeis por lado".into()));
    }
    let data = to_u8(p);
    let enc = webp::Encoder::from_rgb(&data, p.w, p.h);
    let mem = if ctx.recipe.compression == Compression::Lossless {
        enc.encode_lossless()
    } else {
        enc.encode(ctx.recipe.quality as f32)
    };
    let simple: &[u8] = &mem;
    if simple.len() < 20 || &simple[0..4] != b"RIFF" || &simple[8..12] != b"WEBP" {
        return Err(err("WebP", "saída inesperada do libwebp"));
    }
    // Contentor estendido (VP8X): perfil ICC, EXIF e XMP.
    let image = &simple[12..];
    let mut flags = 0x20u8; // ICC
    if ctx.exif.is_some() {
        flags |= 0x08;
    }
    if ctx.xmp.is_some() {
        flags |= 0x04;
    }
    let mut vp8x = vec![flags, 0, 0, 0];
    vp8x.extend(&(p.w - 1).to_le_bytes()[0..3]);
    vp8x.extend(&(p.h - 1).to_le_bytes()[0..3]);
    let mut body = b"WEBP".to_vec();
    riff_chunk(&mut body, b"VP8X", &vp8x);
    riff_chunk(&mut body, b"ICCP", ctx.icc);
    body.extend(image);
    if let Some(e) = &ctx.exif {
        riff_chunk(&mut body, b"EXIF", e);
    }
    if let Some(x) = &ctx.xmp {
        riff_chunk(&mut body, b"XMP ", x.as_bytes());
    }
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    Ok(out)
}

/// PackBits (RLE do Photoshop).
fn packbits(row: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < row.len() {
        let mut run = 1;
        while i + run < row.len() && run < 128 && row[i + run] == row[i] {
            run += 1;
        }
        if run >= 2 {
            out.push((1i32 - run as i32) as i8 as u8);
            out.push(row[i]);
            i += run;
        } else {
            let start = i;
            let mut n = 0;
            while i < row.len() && n < 128 {
                if i + 1 < row.len() && row[i + 1] == row[i] {
                    break;
                }
                i += 1;
                n += 1;
            }
            if n == 0 {
                i += 1;
                n = 1;
            }
            out.push((n - 1) as u8);
            out.extend(&row[start..start + n]);
        }
    }
}

fn psd_resource(out: &mut Vec<u8>, id: u16, data: &[u8]) {
    out.extend(b"8BIM");
    out.extend(id.to_be_bytes());
    out.extend([0, 0]); // nome vazio (string Pascal, par)
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
}

/// PSD achatado (uma imagem composta, sem camadas).
pub fn psd(p: &Pixels, ctx: &Ctx) -> CoreResult<Vec<u8>> {
    if p.w > 30_000 || p.h > 30_000 {
        return Err(CoreError::Other("PSD: máximo de 30 000 píxeis por lado".into()));
    }
    let sixteen = ctx.recipe.depth == BitDepth::B16;
    let mut out = b"8BPS".to_vec();
    out.extend(1u16.to_be_bytes());
    out.extend([0u8; 6]);
    out.extend(3u16.to_be_bytes());
    out.extend(p.h.to_be_bytes());
    out.extend(p.w.to_be_bytes());
    out.extend((if sixteen { 16u16 } else { 8 }).to_be_bytes());
    out.extend(3u16.to_be_bytes()); // RGB
    out.extend(0u32.to_be_bytes()); // dados do modo de cor
    let mut res = Vec::new();
    let fixed = ((ctx.meta.ppi as f64) * 65536.0).round() as u32;
    let mut ri = Vec::new();
    ri.extend(fixed.to_be_bytes());
    ri.extend(1u16.to_be_bytes());
    ri.extend(1u16.to_be_bytes());
    ri.extend(fixed.to_be_bytes());
    ri.extend(1u16.to_be_bytes());
    ri.extend(1u16.to_be_bytes());
    psd_resource(&mut res, 1005, &ri);
    psd_resource(&mut res, 1039, ctx.icc);
    if let Some(x) = &ctx.xmp {
        psd_resource(&mut res, 1060, x.as_bytes());
    }
    out.extend((res.len() as u32).to_be_bytes());
    out.extend(res);
    out.extend(0u32.to_be_bytes()); // camadas e máscaras: nenhuma
    let (w, h) = (p.w as usize, p.h as usize);
    let plane = |c: usize| -> Vec<u8> {
        if sixteen {
            to_uint(p, 16).iter().skip(c).step_by(3).flat_map(|v| v.to_be_bytes()).collect()
        } else {
            to_u8(p).iter().skip(c).step_by(3).copied().collect()
        }
    };
    let planes: Vec<Vec<u8>> = (0..3).map(plane).collect();
    let row_bytes = w * if sixteen { 2 } else { 1 };
    if ctx.recipe.compression == Compression::Rle {
        out.extend(1u16.to_be_bytes());
        let mut counts = Vec::new();
        let mut data = Vec::new();
        for pl in &planes {
            for y in 0..h {
                let before = data.len();
                packbits(&pl[y * row_bytes..(y + 1) * row_bytes], &mut data);
                counts.extend(((data.len() - before) as u16).to_be_bytes());
            }
        }
        out.extend(counts);
        out.extend(data);
    } else {
        out.extend(0u16.to_be_bytes());
        for pl in planes {
            out.extend(pl);
        }
    }
    Ok(out)
}

fn pdf_text(s: &str) -> String {
    if s.is_ascii() {
        let e = s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
        format!("({e})")
    } else {
        let mut hex = String::from("<FEFF");
        for u in s.encode_utf16() {
            hex += &format!("{u:04X}");
        }
        hex + ">"
    }
}

/// PDF de uma página com a fotografia (JPEG embutido) e o perfil ICC; o tamanho da
/// página resulta do PPI (impressão a 100 %).
pub fn pdf(p: &Pixels, ctx: &Ctx) -> CoreResult<Vec<u8>> {
    let jpg = {
        let plain = Ctx { exif: None, xmp: None, ..*ctx };
        jpeg(p, &plain, ctx.recipe.quality)?
    };
    let ppi = ctx.meta.ppi.max(1.0);
    let (pw, ph) = (p.w as f32 / ppi * 72.0, p.h as f32 / ppi * 72.0);
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let meta_obj = ctx.xmp.as_ref().map(|_| 8);
    objs.push(
        format!(
            "<< /Type /Catalog /Pages 2 0 R{} >>",
            meta_obj.map(|n| format!(" /Metadata {n} 0 R")).unwrap_or_default()
        )
        .into_bytes(),
    );
    objs.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
    objs.push(
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.3} {ph:.3}] /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>"
        )
        .into_bytes(),
    );
    let content = format!("q\n{pw:.3} 0 0 {ph:.3} 0 0 cm\n/Im0 Do\nQ\n");
    objs.push(
        [
            format!("<< /Length {} >>\nstream\n", content.len()).as_bytes(),
            content.as_bytes(),
            b"\nendstream",
        ]
        .concat(),
    );
    objs.push(
        [
            format!(
                "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace [/ICCBased 6 0 R] /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
                p.w,
                p.h,
                jpg.len()
            )
            .as_bytes(),
            &jpg,
            b"\nendstream",
        ]
        .concat(),
    );
    objs.push(
        [
            format!("<< /N 3 /Alternate /DeviceRGB /Length {} >>\nstream\n", ctx.icc.len())
                .as_bytes(),
            ctx.icc,
            b"\nendstream",
        ]
        .concat(),
    );
    let m = ctx.meta;
    let mut info = String::from("<< /Producer (Momento Studio)");
    if !m.creator.is_empty() {
        info += &format!(" /Author {}", pdf_text(&m.creator));
    }
    if !m.copyright.is_empty() {
        info += &format!(" /Subject {}", pdf_text(&m.copyright));
    }
    info += " >>";
    objs.push(info.into_bytes());
    if let Some(x) = &ctx.xmp {
        objs.push(
            [
                format!("<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n", x.len())
                    .as_bytes(),
                x.as_bytes(),
                b"\nendstream",
            ]
            .concat(),
        );
    }
    let mut out = b"%PDF-1.6\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend(o);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 7 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    Ok(out)
}

/// OpenEXR linear (meia precisão ou 32 bits), com as cromaticidades do espaço.
pub fn exr(p: &Pixels, ctx: &Ctx) -> CoreResult<Vec<u8>> {
    use exr::meta::attribute::{Chromaticities, Text};
    use exr::prelude::*;
    let (w, h) = (p.w as usize, p.h as usize);
    let mut la =
        LayerAttributes { software_name: Some(Text::from("Momento Studio")), ..Default::default() };
    let m = ctx.meta;
    if !m.copyright.is_empty() || !m.creator.is_empty() {
        // Texto ASCII: o exr guarda carateres em Latin-1 e o OpenEXR lê UTF-8.
        la.owner = Text::new_or_none(ascii_fold(format!("{} {}", m.copyright, m.creator).trim()));
    }
    if let Some(t) = m.camera.as_ref().and_then(|c| c.capture_time.clone()) {
        if t.len() >= 19 {
            la.capture_date = Text::new_or_none(format!(
                "{}:{}:{} {}",
                &t[0..4],
                &t[5..7],
                &t[8..10],
                &t[11..19]
            ));
        }
    }
    let encoding = Encoding {
        compression: match ctx.recipe.compression {
            crate::recipe::Compression::Zip => exr::compression::Compression::ZIP16,
            crate::recipe::Compression::Piz => exr::compression::Compression::PIZ,
            _ => exr::compression::Compression::Uncompressed,
        },
        blocks: Blocks::ScanLines,
        line_order: LineOrder::Increasing,
    };
    let px = p.px;
    let mut cur = Cursor::new(Vec::new());
    let chroma = ctx.definition.map(|(pr, _)| Chromaticities {
        red: Vec2(pr.r.0 as f32, pr.r.1 as f32),
        green: Vec2(pr.g.0 as f32, pr.g.1 as f32),
        blue: Vec2(pr.b.0 as f32, pr.b.1 as f32),
        white: Vec2(pr.white.0 as f32, pr.white.1 as f32),
    });
    macro_rules! write {
        ($f:expr) => {{
            let layer = Layer::new(
                (w, h),
                la,
                encoding,
                SpecificChannels::rgb(|pos: Vec2<usize>| {
                    let q = px[pos.y() * w + pos.x()];
                    ($f(q[0]), $f(q[1]), $f(q[2]))
                }),
            );
            let mut image = Image::from_layer(layer);
            image.attributes.chromaticities = chroma;
            image.write().to_buffered(&mut cur).map_err(|e| err("EXR", e))?;
        }};
    }
    if ctx.recipe.depth == BitDepth::F16 {
        write!(|v: f32| f16::from_f32(v))
    } else {
        write!(|v: f32| v)
    }
    Ok(cur.into_inner())
}
