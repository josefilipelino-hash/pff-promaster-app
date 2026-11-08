//! # momento-test-fixtures
//!
//! Gera imagens **sintéticas** com metadados EXIF reais (escritos segundo a especificação
//! EXIF 2.32 / TIFF 6.0), para testar importação, leitura de metadados, orientação e
//! desempenho sem usar fotografias de pessoas.

use image::{ImageEncoder, Rgb, RgbImage};
use std::io::Write;
use std::path::Path;

/// Metadados a escrever.
#[derive(Debug, Clone, Default)]
pub struct ExifSpec {
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub lens: Option<String>,
    pub orientation: Option<u16>,
    /// `AAAA:MM:DD HH:MM:SS`
    pub date_time_original: Option<String>,
    pub sub_sec: Option<String>,
    /// `+01:00`
    pub offset_time: Option<String>,
    /// (numerador, denominador)
    pub exposure_time: Option<(u32, u32)>,
    pub f_number: Option<(u32, u32)>,
    pub iso: Option<u16>,
    pub focal_length: Option<(u32, u32)>,
    pub focal_length_35mm: Option<u16>,
}

const ASCII: u16 = 2;
const SHORT: u16 = 3;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;

struct Entry {
    tag: u16,
    typ: u16,
    count: u32,
    data: Vec<u8>,
}

fn ascii(tag: u16, s: &str) -> Entry {
    let mut data = s.as_bytes().to_vec();
    data.push(0);
    Entry { tag, typ: ASCII, count: data.len() as u32, data }
}
fn short(tag: u16, v: u16) -> Entry {
    Entry { tag, typ: SHORT, count: 1, data: v.to_le_bytes().to_vec() }
}
fn long(tag: u16, v: u32) -> Entry {
    Entry { tag, typ: LONG, count: 1, data: v.to_le_bytes().to_vec() }
}
fn rational(tag: u16, (n, d): (u32, u32)) -> Entry {
    let mut data = n.to_le_bytes().to_vec();
    data.extend_from_slice(&d.to_le_bytes());
    Entry { tag, typ: RATIONAL, count: 1, data }
}

/// Serializa um IFD que começa em `start` (offset relativo ao início do TIFF).
fn ifd(mut entries: Vec<Entry>, start: u32) -> Vec<u8> {
    entries.sort_by_key(|e| e.tag);
    let header_len = 2 + 12 * entries.len() as u32 + 4;
    let mut head = (entries.len() as u16).to_le_bytes().to_vec();
    let mut data_area = Vec::new();
    for e in &entries {
        head.extend_from_slice(&e.tag.to_le_bytes());
        head.extend_from_slice(&e.typ.to_le_bytes());
        head.extend_from_slice(&e.count.to_le_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            head.extend_from_slice(&v);
        } else {
            let off = start + header_len + data_area.len() as u32;
            head.extend_from_slice(&off.to_le_bytes());
            data_area.extend_from_slice(&e.data);
            if data_area.len() % 2 == 1 {
                data_area.push(0);
            }
        }
    }
    head.extend_from_slice(&0u32.to_le_bytes()); // sem IFD seguinte
    head.extend_from_slice(&data_area);
    head
}

/// Bloco TIFF com IFD0 + sub-IFD EXIF (little-endian).
pub fn exif_tiff(spec: &ExifSpec) -> Vec<u8> {
    let mut ifd0 = Vec::new();
    if let Some(v) = &spec.make {
        ifd0.push(ascii(0x010F, v));
    }
    if let Some(v) = &spec.model {
        ifd0.push(ascii(0x0110, v));
    }
    if let Some(v) = spec.orientation {
        ifd0.push(short(0x0112, v));
    }
    let mut exif = Vec::new();
    if let Some(v) = spec.exposure_time {
        exif.push(rational(0x829A, v));
    }
    if let Some(v) = spec.f_number {
        exif.push(rational(0x829D, v));
    }
    if let Some(v) = spec.iso {
        exif.push(short(0x8827, v));
    }
    if let Some(v) = &spec.date_time_original {
        exif.push(ascii(0x9003, v));
    }
    if let Some(v) = &spec.offset_time {
        exif.push(ascii(0x9011, v));
    }
    if let Some(v) = spec.focal_length {
        exif.push(rational(0x920A, v));
    }
    if let Some(v) = &spec.sub_sec {
        exif.push(ascii(0x9291, v));
    }
    if let Some(v) = spec.focal_length_35mm {
        exif.push(short(0xA405, v));
    }
    if let Some(v) = &spec.serial {
        exif.push(ascii(0xA431, v));
    }
    if let Some(v) = &spec.lens {
        exif.push(ascii(0xA434, v));
    }
    let has_exif = !exif.is_empty();
    if has_exif {
        ifd0.push(long(0x8769, 0)); // valor provisório
    }
    let ifd0_bytes_len = ifd(ifd0_clone(&ifd0), 8).len() as u32;
    if has_exif {
        let last = ifd0.iter_mut().find(|e| e.tag == 0x8769).unwrap();
        last.data = (8 + ifd0_bytes_len).to_le_bytes().to_vec();
    }
    let mut out = b"II*\0".to_vec();
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&ifd(ifd0, 8));
    if has_exif {
        out.extend_from_slice(&ifd(exif, 8 + ifd0_bytes_len));
    }
    out
}

fn ifd0_clone(v: &[Entry]) -> Vec<Entry> {
    v.iter()
        .map(|e| Entry { tag: e.tag, typ: e.typ, count: e.count, data: e.data.clone() })
        .collect()
}

/// Imagem de teste: gradiente com cor dependente de `seed` e uma barra vermelha no
/// **topo visual** e um quadrado azul no canto superior esquerdo visual, para verificar a
/// orientação. Os píxeis são gravados “rodados” de acordo com `orientation`, como uma
/// câmara faria, para que a imagem só fique correta depois de aplicada a orientação.
pub fn test_image(width: u32, height: u32, seed: u32, orientation: u16) -> RgbImage {
    let hue = (seed.wrapping_mul(47) % 360) as f32;
    let (r0, g0, b0) = hsv(hue, 0.55, 0.85);
    let mut img = RgbImage::from_fn(width, height, |x, y| {
        let t = (x + y) as f32 / (width + height) as f32;
        let k = 0.55 + 0.45 * t;
        Rgb([(r0 * k) as u8, (g0 * k) as u8, (b0 * k) as u8])
    });
    let bar = (height / 12).max(1);
    for y in 0..bar {
        for x in 0..width {
            img.put_pixel(x, y, Rgb([220, 20, 20]));
        }
    }
    let sq = (width.min(height) / 6).max(1);
    for y in bar..(bar + sq).min(height) {
        for x in 0..sq.min(width) {
            img.put_pixel(x, y, Rgb([20, 40, 220]));
        }
    }
    // Inverso da orientação: o que a câmara grava para que, aplicando a tag, se veja `img`.
    use image::imageops::*;
    match orientation {
        2 => flip_horizontal(&img),
        3 => rotate180(&img),
        4 => flip_vertical(&img),
        5 => flip_horizontal(&rotate90(&img)),
        6 => rotate270(&img),
        7 => flip_horizontal(&rotate270(&img)),
        8 => rotate90(&img),
        _ => img,
    }
}

fn hsv(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    ((r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0)
}

/// JPEG com segmento APP1/EXIF inserido logo após o SOI.
pub fn jpeg_bytes(img: &RgbImage, exif: Option<&ExifSpec>, quality: u8) -> Vec<u8> {
    let mut jpg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpg, quality)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .expect("codificação JPEG");
    let Some(spec) = exif else { return jpg };
    let tiff = exif_tiff(spec);
    let mut out = Vec::with_capacity(jpg.len() + tiff.len() + 10);
    out.extend_from_slice(&jpg[..2]); // SOI
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(&tiff);
    out.extend_from_slice(&jpg[2..]);
    out
}

/// PNG com bloco `eXIf` (PNG 1.5 / extensão de 2017) antes do IDAT.
pub fn png_bytes(img: &RgbImage, exif: Option<&ExifSpec>) -> Vec<u8> {
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .expect("codificação PNG");
    let Some(spec) = exif else { return png };
    let data = exif_tiff(spec);
    // Inserir após o IHDR (8 bytes de assinatura + 25 bytes do bloco IHDR).
    let split = 8 + 25;
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    let mut body = b"eXIf".to_vec();
    body.extend_from_slice(&data);
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    let mut out = png[..split].to_vec();
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[split..]);
    out
}

/// TIFF (sem EXIF; a leitura deve funcionar na ausência de metadados).
pub fn tiff_bytes(img: &RgbImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::tiff::TiffEncoder::new(&mut out)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .expect("codificação TIFF");
    out.into_inner()
}

/// TIFF de 16 bits por canal.
pub fn tiff16_bytes(width: u32, height: u32) -> Vec<u8> {
    let img = image::ImageBuffer::<Rgb<u16>, _>::from_fn(width, height, |x, y| {
        Rgb([(x * 65535 / width.max(1)) as u16, (y * 65535 / height.max(1)) as u16, 30000])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::tiff::TiffEncoder::new(&mut out)
        .write_image(bytemuck_u16(img.as_raw()), width, height, image::ExtendedColorType::Rgb16)
        .expect("codificação TIFF 16");
    out.into_inner()
}

fn bytemuck_u16(v: &[u16]) -> &[u8] {
    // SAFETY: u16 não tem padding; alinhamento de u8 é 1; tamanho em bytes = len * 2.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut f = std::fs::File::create(path)?;
    f.write_all(bytes)
}

/// Câmaras fictícias usadas nos conjuntos de teste.
pub const CAMERAS: &[(&str, &str, &str)] = &[
    ("Canon", "EOS R5", "RF24-70mm F2.8 L IS USM"),
    ("NIKON CORPORATION", "NIKON Z 6_2", "NIKKOR Z 85mm f/1.8 S"),
    ("SONY", "ILCE-7M4", "FE 35mm F1.4 GM"),
    ("FUJIFILM", "X-T5", "XF56mmF1.2 R WR"),
];

/// Especificação EXIF determinística para a fotografia `i` de um conjunto.
pub fn spec_for(i: u32, orientation: u16) -> ExifSpec {
    let (make, model, lens) = CAMERAS[(i as usize) % CAMERAS.len()];
    let secs = 14 * 3600 + i * 7;
    ExifSpec {
        make: Some(make.into()),
        model: Some(model.into()),
        serial: Some(format!("SN{:05}", i % CAMERAS.len() as u32)),
        lens: Some(lens.into()),
        orientation: Some(orientation),
        date_time_original: Some(format!(
            "2026:06:15 {:02}:{:02}:{:02}",
            secs / 3600 % 24,
            secs / 60 % 60,
            secs % 60
        )),
        sub_sec: Some(format!("{:02}", i % 100)),
        offset_time: Some("+01:00".into()),
        exposure_time: Some([(1, 250), (1, 125), (1, 60), (1, 1000), (1, 2)][(i % 5) as usize]),
        f_number: Some([(28, 10), (18, 10), (40, 10), (14, 10)][(i % 4) as usize]),
        iso: Some([100, 400, 800, 3200, 6400][(i % 5) as usize]),
        focal_length: Some([(35, 1), (50, 1), (85, 1), (24, 1), (70, 1)][(i % 5) as usize]),
        focal_length_35mm: Some([35, 50, 85, 24, 70][(i % 5) as usize]),
    }
}
