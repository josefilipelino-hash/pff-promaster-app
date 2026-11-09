//! Testes de qualidade: cada formato é descodificado por um leitor independente e
//! comparado com a imagem esperada (PSNR, diferença máxima, perfil ICC, metadados).

use super::*;
use crate::codecs::{to_u8, to_uint, Pixels};
use momento_color_management::OutputColor;

/// Cena 320×240 em Rec. 2020 linear: rampa, manchas de cor (algumas fora de sRGB),
/// textura fina e um céu liso (sensível a bandas).
fn scene() -> LinearImage {
    let (w, h) = (320u32, 240u32);
    let mut px = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let p = if v < 0.25 {
                [u, u, u].map(|c| c * c)
            } else if v < 0.5 {
                match (x / 40) % 8 {
                    0 => [0.6, 0.05, 0.03],
                    1 => [0.05, 0.5, 0.06],
                    2 => [0.04, 0.06, 0.55],
                    3 => [0.7, 0.6, 0.05],
                    4 => [0.35, 0.22, 0.16],
                    5 => [0.05, 0.4, 0.45],
                    6 => [0.8, 0.02, 0.4],
                    _ => [0.18, 0.18, 0.18],
                }
            } else if v < 0.75 {
                let t = if (x / 2 + y / 2) % 2 == 0 { 0.32 } else { 0.12 };
                [t, t * 0.9, t * 0.8]
            } else {
                [0.10 + 0.05 * u, 0.20 + 0.06 * u, 0.45 + 0.05 * u]
            };
            px.push(p);
        }
    }
    LinearImage { width: w, height: h, px }
}

fn photo() -> PhotoMeta {
    PhotoMeta {
        file_stem: "IMG_0001".into(),
        capture_time: Some("2026-06-15T17:42:05".into()),
        make: Some("Canon".into()),
        model: Some("EOS R5".into()),
        iso: Some(400),
        exposure_time: Some(0.004),
        f_number: Some(2.8),
        gps: Some((38.79, -9.39)),
        ..Default::default()
    }
}

fn recipe(format: Format, depth: BitDepth, compression: Compression) -> Recipe {
    Recipe {
        format,
        depth,
        compression,
        quality: 92,
        metadata: Metadata {
            copyright: "© 2026 Estúdio".into(),
            creator: "José Lino".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Valores esperados (codificados, 0–1) no espaço de saída.
fn expected(r: &Recipe) -> Vec<[f32; 3]> {
    let img = scene();
    let p = Prepared::new(r).unwrap();
    let mut px = img.px.clone();
    p.color.convert(&mut px);
    px
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let mse: f64 =
        a.iter().zip(b).map(|(x, y)| (*x as f64 - *y as f64).powi(2)).sum::<f64>() / a.len() as f64;
    10.0 * (255.0f64.powi(2) / mse.max(1e-12)).log10()
}

fn max_diff8(a: &[u8], b: &[u8]) -> i32 {
    a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0)
}

fn produce(r: &Recipe) -> Produced {
    Prepared::new(r).unwrap().produce(&scene(), &photo()).unwrap()
}

fn exp8(r: &Recipe) -> Vec<u8> {
    let e = expected(r);
    to_u8(&Pixels { w: 320, h: 240, px: &e })
}

#[test]
fn jpeg_quality_icc_and_metadata() {
    let r = recipe(Format::Jpeg, BitDepth::B8, Compression::Lossy);
    let out = produce(&r);
    let dec = image::load_from_memory(&out.bytes).unwrap().to_rgb8();
    assert_eq!(dec.dimensions(), (320, 240));
    let p = psnr(dec.as_raw(), &exp8(&r));
    assert!(p > 38.0, "PSNR JPEG q92 {p:.1} dB");
    // ICC e EXIF lidos por leitores independentes.
    let mut d = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(&out.bytes)).unwrap();
    use image::ImageDecoder;
    let icc = d.icc_profile().unwrap().unwrap();
    assert_eq!(momento_color_management::inspect_icc(&icc).unwrap(), "sRGB IEC61966-2.1");
    let ex =
        exif::Reader::new().read_from_container(&mut std::io::Cursor::new(&out.bytes)).unwrap();
    assert!(ex.get_field(exif::Tag::Model, exif::In::PRIMARY).is_some());
    assert!(
        ex.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY).is_none(),
        "GPS removido por omissão"
    );
    assert!(String::from_utf8_lossy(&out.bytes).contains("© 2026 Estúdio"), "XMP com copyright");
    // Menor qualidade → ficheiro menor.
    let small = produce(&Recipe { quality: 60, ..r.clone() });
    assert!(small.bytes.len() < out.bytes.len());
}

#[test]
fn png_8_and_16_bit_are_exact() {
    let r8 = recipe(Format::Png, BitDepth::B8, Compression::Fast);
    let out = produce(&r8);
    let dec = image::load_from_memory(&out.bytes).unwrap();
    assert_eq!(dec.color(), image::ColorType::Rgb8);
    assert_eq!(max_diff8(dec.to_rgb8().as_raw(), &exp8(&r8)), 0);
    let r16 = Recipe {
        color: ColorSpace::DisplayP3,
        ..recipe(Format::Png, BitDepth::B16, Compression::Best)
    };
    let out = produce(&r16);
    let dec = image::load_from_memory(&out.bytes).unwrap();
    assert_eq!(dec.color(), image::ColorType::Rgb16);
    let e = expected(&r16);
    let want = to_uint(&Pixels { w: 320, h: 240, px: &e }, 16);
    assert_eq!(dec.to_rgb16().as_raw(), &want, "16 bits sem perdas");
    let dec = png::Decoder::new(std::io::Cursor::new(&out.bytes)).read_info().unwrap();
    let icc = dec.info().icc_profile.as_ref().unwrap();
    assert_eq!(momento_color_management::inspect_icc(icc).unwrap(), "Display P3");
    assert!(dec.info().pixel_dims.is_some(), "PPI gravado");
}

#[test]
fn tiff_bigtiff_all_depths_and_compressions() {
    use tiff::decoder::{Decoder, DecodingResult};
    for big in [false, true] {
        for depth in [BitDepth::B8, BitDepth::B16, BitDepth::F32] {
            for comp in [Compression::None, Compression::Lzw, Compression::Zip] {
                let r = Recipe {
                    color: ColorSpace::AdobeRgb,
                    ..recipe(if big { Format::BigTiff } else { Format::Tiff }, depth, comp)
                };
                let out = produce(&r);
                if big {
                    assert_eq!(&out.bytes[2..4], &[43, 0], "cabeçalho BigTIFF");
                }
                let mut d = Decoder::new(std::io::Cursor::new(&out.bytes)).unwrap();
                assert_eq!(d.dimensions().unwrap(), (320, 240));
                let icc = d.get_tag_u8_vec(tiff::tags::Tag::Unknown(34675)).unwrap();
                assert!(momento_color_management::inspect_icc(&icc)
                    .unwrap()
                    .starts_with("Adobe RGB"));
                let e = expected(&r);
                let p = Pixels { w: 320, h: 240, px: &e };
                match (d.read_image().unwrap(), depth) {
                    (DecodingResult::U8(v), BitDepth::B8) => assert_eq!(v, to_u8(&p)),
                    (DecodingResult::U16(v), BitDepth::B16) => assert_eq!(v, to_uint(&p, 16)),
                    (DecodingResult::F32(v), BitDepth::F32) => {
                        let want: Vec<f32> = e.iter().flatten().copied().collect();
                        assert_eq!(v, want, "float exato (linear)");
                    }
                    (_, d) => panic!("tipo inesperado para {d:?}"),
                }
            }
        }
    }
}

#[test]
fn webp_lossless_exact_and_lossy_good() {
    let r = recipe(Format::Webp, BitDepth::B8, Compression::Lossless);
    let out = produce(&r);
    assert_eq!(&out.bytes[12..16], b"VP8X", "contentor estendido (ICC/EXIF/XMP)");
    let dec = image::load_from_memory(&out.bytes).unwrap().to_rgb8();
    assert_eq!(max_diff8(dec.as_raw(), &exp8(&r)), 0);
    let r = recipe(Format::Webp, BitDepth::B8, Compression::Lossy);
    let out = produce(&r);
    let dec = image::load_from_memory(&out.bytes).unwrap().to_rgb8();
    let p = psnr(dec.as_raw(), &exp8(&r));
    assert!(p > 33.0, "PSNR WebP q92 {p:.1}");
    use image::ImageDecoder;
    let mut d = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(&out.bytes)).unwrap();
    assert!(d.icc_profile().unwrap().is_some());
}

#[test]
fn jpeg_xl_lossless_with_color_encoding() {
    for (space, depth) in [
        (ColorSpace::Srgb, BitDepth::B8),
        (ColorSpace::DisplayP3, BitDepth::B16),
        (ColorSpace::AdobeRgb, BitDepth::B8),
        (ColorSpace::ProPhoto, BitDepth::B16),
        (ColorSpace::Rec2020, BitDepth::B16),
    ] {
        let r = Recipe { color: space, ..recipe(Format::JpegXl, depth, Compression::Lossless) };
        let out = produce(&r);
        let img = jxl_oxide::JxlImage::builder().read(std::io::Cursor::new(&out.bytes)).unwrap();
        let hdr = &img.image_header().metadata;
        let enc = format!("{:?}", hdr.colour_encoding);
        match space {
            ColorSpace::DisplayP3 => assert!(enc.contains("P3"), "{enc}"),
            ColorSpace::Rec2020 => assert!(enc.contains("Bt2100"), "{enc}"),
            ColorSpace::AdobeRgb | ColorSpace::ProPhoto => assert!(enc.contains("Gamma"), "{enc}"),
            _ => {}
        }
        let render = img.render_frame(0).unwrap();
        let fb = render.image_all_channels();
        let e = expected(&r);
        let max = if depth == BitDepth::B8 { 255.0 } else { 65535.0 };
        let want: Vec<f32> = if depth == BitDepth::B8 {
            to_u8(&Pixels { w: 320, h: 240, px: &e }).iter().map(|v| *v as f32).collect()
        } else {
            to_uint(&Pixels { w: 320, h: 240, px: &e }, 16).iter().map(|v| *v as f32).collect()
        };
        let worst =
            fb.buf().iter().zip(&want).map(|(a, b)| (a * max - b).abs()).fold(0.0f32, f32::max);
        assert!(worst < 0.51, "{space:?} sem perdas (dif. {worst})");
    }
}

/// O descodificador openjp2 (usado só para verificar) liberta um ponteiro nulo ao terminar,
/// o que as verificações de debug do Rust detetam: este teste corre em release.
#[test]
#[cfg_attr(debug_assertions, ignore = "openjp2: verificação de debug no descodificador")]
fn jpeg2000_lossless_and_lossy() {
    // Sem perdas: 8 bits exato.
    let r = recipe(Format::Jpeg2000, BitDepth::B8, Compression::Lossless);
    let out = produce(&r);
    let img = jpeg2k::Image::from_bytes(&out.bytes).unwrap();
    assert!(img.has_icc_profile(), "ICC na caixa colr");
    let comps = img.components();
    assert_eq!(comps.len(), 3);
    let want = exp8(&r);
    for (c, comp) in comps.iter().enumerate() {
        assert_eq!(comp.precision(), 8);
        assert!(
            comp.data().iter().enumerate().all(|(i, v)| *v == want[i * 3 + c] as i32),
            "8 bits exato"
        );
    }
    // Sem perdas a 12/16 bits: ajustado para 8 bits, com nota (ver codecs::jp2).
    let (n, notes) = recipe(Format::Jpeg2000, BitDepth::B16, Compression::Lossless).normalized();
    assert_eq!((n.depth, notes), (BitDepth::B8, vec!["export.note.jp2Lossless"]));
    // Com perdas a 12 e 16 bits: precisão e PSNR altos.
    for depth in [BitDepth::B12, BitDepth::B16] {
        let r = recipe(Format::Jpeg2000, depth, Compression::Lossy);
        let out = produce(&r);
        let img = jpeg2k::Image::from_bytes(&out.bytes).unwrap();
        let comps = img.components();
        assert_eq!(comps[0].precision(), depth.bits());
        let max = ((1u32 << depth.bits()) - 1) as f32;
        let got: Vec<u8> = (0..320 * 240)
            .flat_map(|i| (0..3).map(move |c| (i, c)))
            .map(|(i, c)| {
                ((comps[c].data()[i] as f32 / max) * 255.0).round().clamp(0.0, 255.0) as u8
            })
            .collect();
        let p = psnr(&got, &exp8(&r));
        assert!(p > 35.0, "{depth:?} PSNR {p:.1}");
    }
    let r = recipe(Format::Jpeg2000, BitDepth::B8, Compression::Lossy);
    let lossy = produce(&r);
    let lossless = produce(&recipe(Format::Jpeg2000, BitDepth::B8, Compression::Lossless));
    assert!(lossy.bytes.len() < lossless.bytes.len());
    let img = jpeg2k::Image::from_bytes(&lossy.bytes).unwrap();
    let comps = img.components();
    let got: Vec<u8> = (0..320 * 240)
        .flat_map(|i| (0..3).map(move |c| (i, c)))
        .map(|(i, c)| comps[c].data()[i].clamp(0, 255) as u8)
        .collect();
    let p = psnr(&got, &exp8(&r));
    assert!(p > 35.0, "PSNR JP2 {p:.1}");
}

#[test]
fn openexr_half_and_float_linear() {
    for depth in [BitDepth::F16, BitDepth::F32] {
        for comp in [Compression::None, Compression::Zip, Compression::Piz] {
            let r = Recipe { color: ColorSpace::Rec2020, ..recipe(Format::Exr, depth, comp) };
            let out = produce(&r);
            let img = read_first_rgba_layer_from_file_like(&out.bytes);
            let e = expected(&r);
            for (i, q) in img.iter().enumerate().step_by(97) {
                for c in 0..3 {
                    let tol = if depth == BitDepth::F16 { 2e-3 * e[i][c].max(0.01) } else { 0.0 };
                    assert!(
                        (q[c] - e[i][c]).abs() <= tol,
                        "{depth:?} {comp:?} {q:?} vs {:?}",
                        e[i]
                    );
                }
            }
            let meta =
                exr::meta::MetaData::read_from_buffered(std::io::Cursor::new(&out.bytes), false)
                    .unwrap();
            let ch = meta.headers[0].shared_attributes.chromaticities.unwrap();
            assert!((ch.red.0 - 0.708).abs() < 1e-4, "primárias Rec. 2020");
        }
    }

    fn read_first_rgba_layer_from_file_like(b: &[u8]) -> Vec<[f32; 3]> {
        use exr::prelude::*;
        let img = read()
            .no_deep_data()
            .largest_resolution_level()
            .rgba_channels(
                |res, _| vec![[0.0f32; 3]; res.width() * res.height()],
                |v: &mut Vec<[f32; 3]>, pos, (r, g, b, _a): (f32, f32, f32, f32)| {
                    let w = 320;
                    v[pos.y() * w + pos.x()] = [r, g, b];
                },
            )
            .first_valid_layer()
            .all_attributes()
            .from_buffered(std::io::Cursor::new(b))
            .unwrap();
        img.layer_data.channel_data.pixels
    }
}

#[test]
fn psd_raw_and_rle() {
    for comp in [Compression::None, Compression::Rle] {
        let r = recipe(Format::Psd, BitDepth::B8, comp);
        let out = produce(&r);
        let (hdr, rgb) = momento_image_codecs::psd_reader::composite_rgb(&out.bytes).unwrap();
        assert_eq!((hdr.width, hdr.height), (320, 240));
        assert_eq!(max_diff8(&rgb, &exp8(&r)), 0, "{comp:?}");
    }
    let r = recipe(Format::Psd, BitDepth::B16, Compression::None);
    let out = produce(&r);
    let (hdr, rgb) = momento_image_codecs::psd_reader::composite_rgb(&out.bytes).unwrap();
    assert_eq!(hdr.depth, 16);
    assert!(max_diff8(&rgb, &exp8(&r)) <= 1);
}

#[test]
fn pdf_structure_is_valid() {
    let r = Recipe {
        size: Size { ppi: 300.0, ..Default::default() },
        ..recipe(Format::Pdf, BitDepth::B8, Compression::Lossy)
    };
    let out = produce(&r);
    let b = &out.bytes;
    let find = |pat: &[u8]| b.windows(pat.len()).rposition(|w| w == pat);
    assert!(b.starts_with(b"%PDF-1.6") && b.ends_with(b"%%EOF\n"));
    // Tabela xref: cada deslocamento (em bytes) aponta para "N 0 obj".
    let sx = find(b"startxref\n").unwrap() + 10;
    let tail = String::from_utf8_lossy(&b[sx..]);
    let off: usize = tail.lines().next().unwrap().trim().parse().unwrap();
    assert!(b[off..].starts_with(b"xref"));
    let table = String::from_utf8_lossy(&b[off..sx]);
    let mut n = 0;
    for line in table.lines().skip(3).take_while(|l| l.ends_with(" n ")) {
        n += 1;
        let o: usize = line[..10].parse().unwrap();
        assert!(b[o..].starts_with(format!("{n} 0 obj").as_bytes()), "objeto {n}");
    }
    assert!(n >= 7);
    let s = String::from_utf8_lossy(b);
    // Página com 320/300 polegadas = 76,8 pt.
    assert!(s.contains("/MediaBox [0 0 76.800 57.600]"));
    assert!(s.contains("/ICCBased") && s.contains("/DCTDecode"));
}

#[test]
fn avif_container_and_color_signalling() {
    for (space, depth) in [
        (ColorSpace::Srgb, BitDepth::B8),
        (ColorSpace::DisplayP3, BitDepth::B10),
        (ColorSpace::Rec2020, BitDepth::B12),
    ] {
        let r = Recipe { color: space, ..recipe(Format::Avif, depth, Compression::Lossy) };
        let out = produce(&r);
        assert_eq!(&out.bytes[4..12], b"ftypavif");
        let b = &out.bytes;
        let i = b.windows(4).position(|w| w == b"colr").expect("caixa colr");
        assert_eq!(&b[i + 4..i + 8], b"nclx");
        let prim = u16::from_be_bytes([b[i + 8], b[i + 9]]);
        assert_eq!(
            prim,
            match space {
                ColorSpace::Srgb => 1,
                ColorSpace::DisplayP3 => 12,
                _ => 9,
            }
        );
        let j = b.windows(4).position(|w| w == b"pixi").expect("caixa pixi");
        assert_eq!(b[j + 9], depth.bits() as u8, "bits por componente");
    }
    // Adobe RGB não tem código CICP: a receita é corrigida para sRGB (com aviso).
    let (n, notes) = Recipe {
        color: ColorSpace::AdobeRgb,
        ..recipe(Format::Avif, BitDepth::B8, Compression::Lossy)
    }
    .normalized();
    assert_eq!(n.color, ColorSpace::Srgb);
    assert_eq!(notes, ["export.note.space"]);
}

/// Exportar em Display P3 e voltar a ler com o perfil embutido dá as cores originais —
/// prova que os píxeis e o ICC estão de acordo (gama larga preservada).
#[test]
fn wide_gamut_roundtrip_through_embedded_profile() {
    let r = Recipe {
        color: ColorSpace::DisplayP3,
        ..recipe(Format::Png, BitDepth::B16, Compression::Fast)
    };
    let out = produce(&r);
    let dec = png::Decoder::new(std::io::Cursor::new(&out.bytes)).read_info().unwrap();
    let icc = dec.info().icc_profile.as_ref().unwrap().to_vec();
    let img = image::load_from_memory(&out.bytes).unwrap().to_rgb16();
    let src = lcms2::Profile::new_icc(&icc).unwrap();
    let dst = momento_color_management::builtin_profile(ColorSpace::Rec2020, true).unwrap();
    let t: lcms2::Transform<[f32; 3], [f32; 3]> = lcms2::Transform::new(
        &src,
        lcms2::PixelFormat::RGB_FLT,
        &dst,
        lcms2::PixelFormat::RGB_FLT,
        lcms2::Intent::RelativeColorimetric,
    )
    .unwrap();
    let sc = scene();
    // Mancha vermelha saturada (fora de sRGB, dentro de P3).
    let (x, y) = (20u32, 90u32);
    let p = img.get_pixel(x, y).0.map(|v| v as f32 / 65535.0);
    let mut back = [p];
    t.transform_in_place(&mut back);
    let want = sc.px[(y * 320 + x) as usize];
    for c in 0..3 {
        assert!((back[0][c] - want[c]).abs() < 0.01, "{:?} vs {want:?}", back[0]);
    }
    // Em sRGB a mesma mancha é cortada (prova de que P3 guarda mais gama).
    let s = OutputColor::builtin(ColorSpace::Srgb, false).unwrap();
    let mut v = [want];
    s.convert(&mut v);
    assert!(v[0][1] == 0.0 || v[0][2] == 0.0, "fora de sRGB");
}

#[test]
fn sizes_sharpen_watermark_and_estimates() {
    let r = Recipe {
        size: Size { mode: SizeMode::Fill, a: 100.0, b: 125.0, ..Default::default() },
        sharpen: Sharpen { target: SharpenTarget::Screen, amount: SharpenAmount::High },
        watermark: Watermark {
            enabled: true,
            text: "© M".into(),
            size: 15.0,
            opacity: 100.0,
            ..Default::default()
        },
        ..recipe(Format::Jpeg, BitDepth::B8, Compression::Lossy)
    };
    let p = Prepared::new(&r).unwrap();
    let out = p.produce(&scene(), &photo()).unwrap();
    assert_eq!((out.width, out.height), (100, 125));
    let dec = image::load_from_memory(&out.bytes).unwrap().to_rgb8();
    assert_eq!(dec.dimensions(), (100, 125));
    // Estimativa na ordem de grandeza certa (a fila corrige com os ficheiros reais).
    let full = Prepared::new(&recipe(Format::Jpeg, BitDepth::B8, Compression::Lossy)).unwrap();
    let real = full.produce(&scene(), &photo()).unwrap().bytes.len() as f64;
    let est = full.estimate(320, 240) as f64;
    assert!(est / real < 4.0 && real / est < 4.0, "estimativa {est} vs real {real}");
}

#[test]
fn unavailable_formats_refuse_honestly() {
    for f in [Format::Heif, Format::Heic, Format::Dng] {
        let caps = f.caps();
        assert!(!caps.available && caps.reason.is_some());
        assert!(Prepared::new(&recipe(f, BitDepth::B8, Compression::None)).is_err());
    }
}
