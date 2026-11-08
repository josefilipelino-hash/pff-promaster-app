//! Deteção de formato por assinatura (magic bytes).
//!
//! Lê-se no máximo [`SNIFF_LEN`] bytes. A deteção nunca entra em pânico com dados
//! truncados ou aleatórios: no pior caso devolve `None`.

use serde::{Deserialize, Serialize};

/// Número de bytes necessários para a deteção.
pub const SNIFF_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormatFamily {
    Raster,
    Raw,
    Vector,
    Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileFormat {
    Jpeg,
    Png,
    Gif,
    Bmp,
    WebP,
    Tiff,
    BigTiff,
    Heic,
    Heif,
    Avif,
    JpegXl,
    Jpeg2000,
    Psd,
    Psb,
    OpenExr,
    RadianceHdr,
    Ico,
    Pnm,
    Pam,
    Pcx,
    Tga,
    Svg,
    Pdf,
    // RAW com assinatura própria
    CanonCr2,
    CanonCr3,
    CanonCrw,
    FujiRaf,
    OlympusOrf,
    PanasonicRw2,
    MinoltaMrw,
    SigmaX3f,
    /// RAW sobre contentor TIFF sem assinatura distintiva (NEF, ARW, DNG, PEF, SRW,
    /// 3FR, ERF, DCR, …). A marca/modelo exatos são confirmados pelo `raw-engine`
    /// lendo as tags TIFF (Make, DNGVersion).
    TiffBasedRaw,
}

impl FileFormat {
    pub fn family(self) -> FormatFamily {
        use FileFormat::*;
        match self {
            CanonCr2 | CanonCr3 | CanonCrw | FujiRaf | OlympusOrf | PanasonicRw2 | MinoltaMrw
            | SigmaX3f | TiffBasedRaw => FormatFamily::Raw,
            Svg => FormatFamily::Vector,
            Pdf => FormatFamily::Document,
            _ => FormatFamily::Raster,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// Assinatura inequívoca no conteúdo.
    Signature,
    /// Contentor confirmado pelo conteúdo; subtipo indicado pela extensão.
    SignatureAndExtension,
    /// Formato sem assinatura (ex.: TGA); apenas a extensão. Validação fica para o codec.
    ExtensionOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detection {
    pub format: FileFormat,
    pub confidence: Confidence,
}

/// Extensões RAW conhecidas que usam contentor TIFF genérico.
const TIFF_RAW_EXTENSIONS: &[&str] = &[
    "nef", "nrw", "arw", "srf", "sr2", "dng", "pef", "srw", "3fr", "fff", "erf", "dcr", "kdc",
    "mos", "mef", "rwl", "iiq", "raw",
];

fn starts(b: &[u8], sig: &[u8]) -> bool {
    b.len() >= sig.len() && &b[..sig.len()] == sig
}

fn at(b: &[u8], off: usize, sig: &[u8]) -> bool {
    b.len() >= off + sig.len() && &b[off..off + sig.len()] == sig
}

/// Deteta o formato a partir dos primeiros bytes (`head`) e, opcionalmente, da extensão.
pub fn detect(head: &[u8], extension: Option<&str>) -> Option<Detection> {
    use Confidence::*;
    use FileFormat::*;
    let ext = extension.map(|e| e.trim_start_matches('.').to_ascii_lowercase());
    let sig = |format| Some(Detection { format, confidence: Signature });

    // RAW com assinatura própria (antes do TIFF genérico, porque alguns começam por "II").
    if at(head, 6, b"HEAPCCDR") {
        return sig(CanonCrw);
    }
    if starts(head, b"FUJIFILMCCD-RAW") {
        return sig(FujiRaf);
    }
    if starts(head, b"IIRO") || starts(head, b"IIRS") || starts(head, b"MMOR") {
        return sig(OlympusOrf);
    }
    if starts(head, b"IIU\0") {
        return sig(PanasonicRw2);
    }
    if starts(head, b"\0MRM") {
        return sig(MinoltaMrw);
    }
    if starts(head, b"FOVb") {
        return sig(SigmaX3f);
    }

    // TIFF e derivados.
    let tiff_le = starts(head, b"II*\0");
    let tiff_be = starts(head, b"MM\0*");
    if tiff_le || tiff_be {
        if tiff_le && at(head, 8, b"CR\x02") {
            return sig(CanonCr2);
        }
        if let Some(e) = ext.as_deref() {
            if TIFF_RAW_EXTENSIONS.contains(&e) {
                return Some(Detection { format: TiffBasedRaw, confidence: SignatureAndExtension });
            }
        }
        return sig(Tiff);
    }
    if starts(head, b"II+\0") || starts(head, b"MM\0+") {
        return sig(BigTiff);
    }

    if starts(head, &[0xFF, 0xD8, 0xFF]) {
        return sig(Jpeg);
    }
    if starts(head, b"\x89PNG\r\n\x1a\n") {
        return sig(Png);
    }
    if starts(head, b"GIF87a") || starts(head, b"GIF89a") {
        return sig(Gif);
    }
    if starts(head, b"RIFF") && at(head, 8, b"WEBP") {
        return sig(WebP);
    }
    if starts(head, b"BM") && head.len() >= 14 {
        return sig(Bmp);
    }

    // ISO Base Media File Format (HEIF, AVIF, CR3, …): caixa "ftyp" no offset 4.
    if at(head, 4, b"ftyp") && head.len() >= 12 {
        let major = &head[8..12];
        let box_len = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
        let brands_end = box_len.min(head.len());
        let compat: Vec<&[u8]> = if brands_end > 16 {
            head[16..brands_end].chunks_exact(4).collect()
        } else {
            Vec::new()
        };
        let has = |b: &[u8]| major == b || compat.contains(&b);
        if has(b"crx ") {
            return sig(CanonCr3);
        }
        if has(b"avif") || has(b"avis") {
            return sig(Avif);
        }
        if ["heic", "heix", "hevc", "hevx", "heim", "heis"].iter().any(|b| has(b.as_bytes())) {
            return sig(Heic);
        }
        if has(b"mif1") || has(b"msf1") {
            return sig(Heif);
        }
        if has(b"jxl ") {
            return sig(JpegXl);
        }
        if has(b"jp2 ") || has(b"jpx ") {
            return sig(Jpeg2000);
        }
        return None;
    }
    if starts(head, &[0x00, 0x00, 0x00, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A])
        || starts(head, &[0xFF, 0x0A])
    {
        return sig(JpegXl);
    }
    if starts(head, &[0x00, 0x00, 0x00, 0x0C, b'j', b'P', b' ', b' ', 0x0D, 0x0A, 0x87, 0x0A])
        || starts(head, &[0xFF, 0x4F, 0xFF, 0x51])
    {
        return sig(Jpeg2000);
    }

    if starts(head, b"8BPS\x00\x01") {
        return sig(Psd);
    }
    if starts(head, b"8BPS\x00\x02") {
        return sig(Psb);
    }
    if starts(head, &[0x76, 0x2F, 0x31, 0x01]) {
        return sig(OpenExr);
    }
    if starts(head, b"#?RADIANCE") || starts(head, b"#?RGBE") {
        return sig(RadianceHdr);
    }
    if starts(head, b"%PDF-") {
        return sig(Pdf);
    }
    if starts(head, &[0x00, 0x00, 0x01, 0x00]) && head.len() >= 6 && (head[4] != 0 || head[5] != 0)
    {
        return sig(Ico);
    }
    if head.len() >= 3 && head[0] == b'P' && head[2].is_ascii_whitespace() {
        match head[1] {
            b'1'..=b'6' => return sig(Pnm),
            b'7' => return sig(Pam),
            _ => {}
        }
    }
    if head.len() >= 3 && head[0] == 0x0A && head[1] <= 5 && head[2] == 1 {
        return sig(Pcx);
    }
    if looks_like_svg(head) {
        return sig(Svg);
    }

    // Sem assinatura: só a extensão.
    match ext.as_deref() {
        Some("tga") | Some("icb") | Some("vda") | Some("vst") => {
            Some(Detection { format: Tga, confidence: ExtensionOnly })
        }
        _ => None,
    }
}

fn looks_like_svg(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start_matches('\u{feff}').trim_start();
    (t.starts_with("<?xml") || t.starts_with("<svg") || t.starts_with("<!--"))
        && text.contains("<svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(head: &[u8], ext: Option<&str>) -> Option<FileFormat> {
        detect(head, ext).map(|x| x.format)
    }

    fn ftyp(major: &[u8; 4], compat: &[&[u8; 4]]) -> Vec<u8> {
        let len = 16 + 4 * compat.len();
        let mut v = (len as u32).to_be_bytes().to_vec();
        v.extend_from_slice(b"ftyp");
        v.extend_from_slice(major);
        v.extend_from_slice(&[0, 0, 0, 0]);
        for c in compat {
            v.extend_from_slice(*c);
        }
        v
    }

    #[test]
    fn common_rasters() {
        assert_eq!(d(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0], None), Some(FileFormat::Jpeg));
        assert_eq!(d(b"\x89PNG\r\n\x1a\n\0\0", None), Some(FileFormat::Png));
        assert_eq!(d(b"GIF89a\0\0", None), Some(FileFormat::Gif));
        assert_eq!(d(b"RIFF\0\0\0\0WEBPVP8 ", None), Some(FileFormat::WebP));
        assert_eq!(d(b"BM\0\0\0\0\0\0\0\0\0\0\0\0", None), Some(FileFormat::Bmp));
        assert_eq!(d(b"II*\0\x08\0\0\0", Some("tif")), Some(FileFormat::Tiff));
        assert_eq!(d(b"MM\0+\0\x08\0\0", None), Some(FileFormat::BigTiff));
        assert_eq!(d(b"8BPS\x00\x01\0\0", None), Some(FileFormat::Psd));
        assert_eq!(d(b"8BPS\x00\x02\0\0", None), Some(FileFormat::Psb));
        assert_eq!(d(&[0x76, 0x2F, 0x31, 0x01, 2, 0], None), Some(FileFormat::OpenExr));
        assert_eq!(d(b"#?RADIANCE\n", None), Some(FileFormat::RadianceHdr));
        assert_eq!(d(b"P6\n640 480\n255\n", None), Some(FileFormat::Pnm));
        assert_eq!(d(b"P7\nWIDTH 1\n", None), Some(FileFormat::Pam));
        assert_eq!(d(b"%PDF-1.7\n", None), Some(FileFormat::Pdf));
        assert_eq!(d(&[0, 0, 1, 0, 1, 0, 16, 16], None), Some(FileFormat::Ico));
        assert_eq!(d(&[0xFF, 0x0A, 0xFA], None), Some(FileFormat::JpegXl));
        assert_eq!(d(&[0xFF, 0x4F, 0xFF, 0x51], None), Some(FileFormat::Jpeg2000));
        assert_eq!(d(b"<?xml version=\"1.0\"?>\n<svg xmlns=", None), Some(FileFormat::Svg));
    }

    #[test]
    fn isobmff_brands() {
        assert_eq!(d(&ftyp(b"heic", &[b"mif1", b"heic"]), None), Some(FileFormat::Heic));
        assert_eq!(d(&ftyp(b"mif1", &[b"mif1", b"avif"]), None), Some(FileFormat::Avif));
        assert_eq!(d(&ftyp(b"mif1", &[b"mif1"]), None), Some(FileFormat::Heif));
        assert_eq!(d(&ftyp(b"crx ", &[b"crx ", b"isom"]), Some("cr3")), Some(FileFormat::CanonCr3));
        assert_eq!(d(&ftyp(b"isom", &[b"mp41"]), None), None, "vídeo MP4 não é imagem");
    }

    #[test]
    fn raw_signatures() {
        assert_eq!(d(b"II*\0\x10\0\0\0CR\x02\0", Some("cr2")), Some(FileFormat::CanonCr2));
        assert_eq!(d(b"II\x1a\0\0\0HEAPCCDR", None), Some(FileFormat::CanonCrw));
        assert_eq!(d(b"FUJIFILMCCD-RAW 0201", None), Some(FileFormat::FujiRaf));
        assert_eq!(d(b"IIRO\x08\0\0\0", None), Some(FileFormat::OlympusOrf));
        assert_eq!(d(b"IIU\0\x08\0\0\0", None), Some(FileFormat::PanasonicRw2));
        assert_eq!(d(b"\0MRM\0\0", None), Some(FileFormat::MinoltaMrw));
        assert_eq!(d(b"FOVb\0\0", None), Some(FileFormat::SigmaX3f));
    }

    #[test]
    fn tiff_based_raw_needs_signature_and_extension() {
        let det = detect(b"MM\0*\0\0\0\x08", Some("NEF")).unwrap();
        assert_eq!(det.format, FileFormat::TiffBasedRaw);
        assert_eq!(det.confidence, Confidence::SignatureAndExtension);
        assert_eq!(det.format.family(), FormatFamily::Raw);
    }

    #[test]
    fn content_wins_over_extension() {
        // PNG com extensão .jpg é detetado como PNG.
        assert_eq!(d(b"\x89PNG\r\n\x1a\n\0\0", Some("jpg")), Some(FileFormat::Png));
        // Texto com extensão .nef não é RAW.
        assert_eq!(d(b"hello world, not an image", Some("nef")), None);
    }

    #[test]
    fn extension_only_formats_are_marked() {
        let det = detect(&[0, 0, 2, 0, 0, 0], Some("tga")).unwrap();
        assert_eq!(det.confidence, Confidence::ExtensionOnly);
    }

    #[test]
    fn never_panics_on_short_or_random_input() {
        for len in 0..SNIFF_LEN {
            let zeros = vec![0u8; len];
            let _ = detect(&zeros, None);
            let ff = vec![0xFFu8; len];
            let _ = detect(&ff, Some("x"));
        }
        // Pseudoaleatório determinístico.
        let mut x: u32 = 0x1234_5678;
        for _ in 0..5000 {
            let mut buf = [0u8; SNIFF_LEN];
            for b in buf.iter_mut() {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                *b = x as u8;
            }
            let n = (x as usize) % SNIFF_LEN;
            let _ = detect(&buf[..n], None);
        }
    }
}
