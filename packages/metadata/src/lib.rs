//! # momento-metadata
//!
//! Leitura de EXIF a partir de JPEG, TIFF, PNG (`eXIf`), HEIF e WebP com `kamadak-exif`
//! (Rust puro, BSD-2). A ausência ou corrupção de EXIF **nunca** é um erro de importação:
//! devolve-se um resumo vazio.
//!
//! Fase 1: campos necessários à importação e ao painel de informação. IPTC/XMP: Fase 2.

use exif::{Exif, In, Tag, Value};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExifSummary {
    /// Orientação EXIF 1–8 (1 quando ausente ou inválida).
    pub orientation: u8,
    /// Hora local de captura `AAAA-MM-DDTHH:MM:SS[.fff]` (sem fuso).
    pub capture_time: Option<String>,
    /// Fuso horário da captura, ex.: `+01:00`, quando a câmara o grava.
    pub capture_offset: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub camera_serial: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    /// Segundos.
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
    /// Milímetros.
    pub focal_length: Option<f64>,
    pub focal_length_35mm: Option<u32>,
}

impl Default for ExifSummary {
    fn default() -> Self {
        ExifSummary {
            orientation: 1,
            capture_time: None,
            capture_offset: None,
            camera_make: None,
            camera_model: None,
            camera_serial: None,
            lens: None,
            iso: None,
            exposure_time: None,
            f_number: None,
            focal_length: None,
            focal_length_35mm: None,
        }
    }
}

/// Lê o EXIF de um ficheiro completo em memória.
pub fn read_exif(bytes: &[u8]) -> ExifSummary {
    match exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        Ok(e) => summarize(&e),
        Err(_) => ExifSummary::default(),
    }
}

fn field(e: &Exif, tag: Tag) -> Option<&exif::Field> {
    e.get_field(tag, In::PRIMARY)
}

fn text(e: &Exif, tag: Tag) -> Option<String> {
    let f = field(e, tag)?;
    let s = match &f.value {
        Value::Ascii(parts) => parts
            .iter()
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect::<Vec<_>>()
            .join(" "),
        Value::Undefined(bytes, _) => String::from_utf8_lossy(bytes).into_owned(),
        _ => return None,
    };
    let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
    (!s.is_empty()).then_some(s)
}

fn uint(e: &Exif, tag: Tag) -> Option<u32> {
    field(e, tag)?.value.get_uint(0)
}

fn rational(e: &Exif, tag: Tag) -> Option<f64> {
    let v = match &field(e, tag)?.value {
        Value::Rational(v) => v.first().map(|r| r.to_f64()),
        Value::SRational(v) => v.first().map(|r| r.to_f64()),
        other => other.get_uint(0).map(|u| u as f64),
    }?;
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Converte `AAAA:MM:DD HH:MM:SS` em `AAAA-MM-DDTHH:MM:SS`, validando os campos.
pub fn normalize_exif_datetime(s: &str, sub_sec: Option<&str>) -> Option<String> {
    let b = s.trim().as_bytes();
    if b.len() < 19 {
        return None;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    let ok = digits(0..4)
        && digits(5..7)
        && digits(8..10)
        && digits(11..13)
        && digits(14..16)
        && digits(17..19)
        && (b[4] == b':' || b[4] == b'-')
        && (b[7] == b':' || b[7] == b'-')
        && (b[10] == b' ' || b[10] == b'T')
        && b[13] == b':'
        && b[16] == b':';
    if !ok || &b[0..4] == b"0000" {
        return None;
    }
    let s = std::str::from_utf8(&b[..19]).ok()?;
    let mut out = format!("{}-{}-{}T{}", &s[0..4], &s[5..7], &s[8..10], &s[11..19]);
    if let Some(ss) =
        sub_sec.map(str::trim).filter(|x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit()))
    {
        out.push('.');
        out.push_str(&ss[..ss.len().min(3)]);
    }
    Some(out)
}

fn summarize(e: &Exif) -> ExifSummary {
    let orientation = uint(e, Tag::Orientation).filter(|o| (1..=8).contains(o)).unwrap_or(1) as u8;

    let candidates = [
        (Tag::DateTimeOriginal, Tag::SubSecTimeOriginal, Tag::OffsetTimeOriginal),
        (Tag::DateTimeDigitized, Tag::SubSecTimeDigitized, Tag::OffsetTimeDigitized),
        (Tag::DateTime, Tag::SubSecTime, Tag::OffsetTime),
    ];
    let (capture_time, capture_offset) = candidates
        .iter()
        .find_map(|(dt, ss, off)| {
            let t = normalize_exif_datetime(&text(e, *dt)?, text(e, *ss).as_deref())?;
            Some((Some(t), text(e, *off)))
        })
        .unwrap_or((None, None));

    let lens = text(e, Tag::LensModel).or_else(|| text(e, Tag::LensSpecification));
    let iso = uint(e, Tag::PhotographicSensitivity).filter(|v| *v > 0);

    ExifSummary {
        orientation,
        capture_time,
        capture_offset,
        camera_make: text(e, Tag::Make),
        camera_model: text(e, Tag::Model),
        camera_serial: text(e, Tag::BodySerialNumber),
        lens,
        iso,
        exposure_time: rational(e, Tag::ExposureTime),
        f_number: rational(e, Tag::FNumber),
        focal_length: rational(e, Tag::FocalLength),
        focal_length_35mm: uint(e, Tag::FocalLengthIn35mmFilm).filter(|v| *v > 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use momento_test_fixtures::*;

    #[test]
    fn reads_all_fields_from_jpeg() {
        let spec = spec_for(3, 6);
        let img = test_image(64, 48, 3, 6);
        let s = read_exif(&jpeg_bytes(&img, Some(&spec), 80));
        assert_eq!(s.orientation, 6);
        assert_eq!(s.camera_make.as_deref(), Some("FUJIFILM"));
        assert_eq!(s.camera_model.as_deref(), Some("X-T5"));
        assert_eq!(s.lens.as_deref(), Some("XF56mmF1.2 R WR"));
        assert_eq!(s.camera_serial.as_deref(), Some("SN00003"));
        assert_eq!(s.capture_time.as_deref(), Some("2026-06-15T14:00:21.03"));
        assert_eq!(s.capture_offset.as_deref(), Some("+01:00"));
        assert_eq!(s.iso, Some(3200));
        assert_eq!(s.exposure_time, Some(1.0 / 1000.0));
        assert_eq!(s.f_number, Some(1.4));
        assert_eq!(s.focal_length, Some(24.0));
        assert_eq!(s.focal_length_35mm, Some(24));
    }

    #[test]
    fn reads_png_exif_chunk() {
        let img = test_image(32, 32, 1, 1);
        let s = read_exif(&png_bytes(&img, Some(&spec_for(0, 1))));
        assert_eq!(s.camera_model.as_deref(), Some("EOS R5"));
        assert_eq!(s.iso, Some(100));
    }

    #[test]
    fn missing_or_corrupt_exif_is_not_an_error() {
        let img = test_image(32, 32, 1, 1);
        assert_eq!(read_exif(&jpeg_bytes(&img, None, 80)), ExifSummary::default());
        assert_eq!(read_exif(&tiff_bytes(&img)).orientation, 1);
        assert_eq!(read_exif(b"lixo"), ExifSummary::default());
        assert_eq!(read_exif(&[]), ExifSummary::default());
    }

    #[test]
    fn invalid_orientation_falls_back_to_1() {
        let spec = ExifSpec { orientation: Some(42), ..Default::default() };
        let s = read_exif(&jpeg_bytes(&test_image(16, 16, 0, 1), Some(&spec), 80));
        assert_eq!(s.orientation, 1);
    }

    #[test]
    fn datetime_normalization() {
        assert_eq!(
            normalize_exif_datetime("2026:06:15 14:03:22", None).as_deref(),
            Some("2026-06-15T14:03:22")
        );
        assert_eq!(
            normalize_exif_datetime("2026:06:15 14:03:22", Some("1234")).as_deref(),
            Some("2026-06-15T14:03:22.123")
        );
        assert_eq!(normalize_exif_datetime("0000:00:00 00:00:00", None), None);
        assert_eq!(normalize_exif_datetime("    :  :     :  :  ", None), None);
        assert_eq!(normalize_exif_datetime("2026:06", None), None);
    }
}
