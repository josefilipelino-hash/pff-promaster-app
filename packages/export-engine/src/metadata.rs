//! Metadados de saída: EXIF (estrutura TIFF, big-endian) e pacote XMP, construídos a
//! partir dos dados do catálogo — nunca copiados às cegas do original, para que a remoção
//! do GPS (ou de tudo) seja garantida.

use crate::recipe::{Metadata, MetadataMode};
use serde::{Deserialize, Serialize};

/// Dados de uma fotografia (do catálogo).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhotoMeta {
    /// Nome do ficheiro original, sem extensão.
    pub file_stem: String,
    /// "AAAA-MM-DDTHH:MM:SS" (hora local da captura).
    pub capture_time: Option<String>,
    pub capture_offset: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
    pub focal_length: Option<f64>,
    pub gps: Option<(f64, f64)>,
    pub rating: Option<u8>,
    pub project: String,
    pub client: String,
    pub moment: String,
}

/// O que efetivamente vai para o ficheiro, depois de aplicar o modo e a remoção do GPS.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OutMeta {
    pub creator: String,
    pub copyright: String,
    pub contact: String,
    pub camera: Option<PhotoMeta>,
    pub gps: Option<(f64, f64)>,
    pub width: u32,
    pub height: u32,
    pub ppi: f32,
    /// Espaço sRGB (EXIF ColorSpace = 1) ou outro (0xFFFF).
    pub srgb: bool,
}

impl OutMeta {
    pub fn new(m: &Metadata, photo: &PhotoMeta, w: u32, h: u32, ppi: f32, srgb: bool) -> Self {
        let none = m.mode == MetadataMode::None;
        OutMeta {
            creator: if none { String::new() } else { m.creator.trim().to_string() },
            copyright: if none { String::new() } else { m.copyright.trim().to_string() },
            contact: if none { String::new() } else { m.contact.trim().to_string() },
            camera: (m.mode == MetadataMode::All).then(|| photo.clone()),
            gps: if m.mode == MetadataMode::All && !m.remove_gps { photo.gps } else { None },
            width: w,
            height: h,
            ppi,
            srgb,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.creator.is_empty()
            && self.copyright.is_empty()
            && self.contact.is_empty()
            && self.camera.is_none()
    }
}

const SOFTWARE: &str = "Momento Studio";

/// Tipos TIFF.
const ASCII: u16 = 2;
const SHORT: u16 = 3;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;
const BYTE: u16 = 1;
const UNDEFINED: u16 = 7;

struct Entry {
    tag: u16,
    typ: u16,
    count: u32,
    data: Vec<u8>,
}

fn ascii(tag: u16, s: &str) -> Entry {
    let mut data: Vec<u8> = s.bytes().map(|b| if b.is_ascii() { b } else { b'?' }).collect();
    data.push(0);
    Entry { tag, typ: ASCII, count: data.len() as u32, data }
}

/// Campos ASCII (EXIF, TIFF): acentos retirados e "©" → "(c)". A norma EXIF define
/// estes campos como ASCII; o texto Unicode completo vai no XMP.
pub fn ascii_fold(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '©' => out.push_str("(c)"),
            '®' => out.push_str("(R)"),
            c if c.is_ascii() && c != '\0' => out.push(c),
            c => {
                const FROM: &str = "áàâãäåçéèêëíìîïñóòôõöúùûüýÁÀÂÃÄÅÇÉÈÊËÍÌÎÏÑÓÒÔÕÖÚÙÛÜÝ";
                const TO: &str = "aaaaaaceeeeiiiinooooouuuuyAAAAAACEEEEIIIINOOOOOUUUUY";
                match FROM.chars().position(|f| f == c) {
                    Some(i) => out.push(TO.as_bytes()[i] as char),
                    None => out.push('?'),
                }
            }
        }
    }
    out
}

fn text(tag: u16, s: &str) -> Entry {
    ascii(tag, &ascii_fold(s))
}

fn short(tag: u16, v: u16) -> Entry {
    Entry { tag, typ: SHORT, count: 1, data: v.to_be_bytes().to_vec() }
}

fn long(tag: u16, v: u32) -> Entry {
    Entry { tag, typ: LONG, count: 1, data: v.to_be_bytes().to_vec() }
}

fn rationals(tag: u16, v: &[(u32, u32)]) -> Entry {
    let mut data = Vec::new();
    for (n, d) in v {
        data.extend(n.to_be_bytes());
        data.extend(d.to_be_bytes());
    }
    Entry { tag, typ: RATIONAL, count: v.len() as u32, data }
}

fn rational(v: f64, den: u32) -> (u32, u32) {
    ((v * den as f64).round().max(0.0) as u32, den)
}

/// Escreve um IFD em `out` (offsets absolutos a partir do início do TIFF).
fn write_ifd(out: &mut Vec<u8>, mut entries: Vec<Entry>, next: u32) {
    entries.sort_by_key(|e| e.tag);
    let start = out.len();
    let table = 2 + entries.len() * 12 + 4;
    let mut extra: Vec<u8> = Vec::new();
    out.extend((entries.len() as u16).to_be_bytes());
    for e in &entries {
        out.extend(e.tag.to_be_bytes());
        out.extend(e.typ.to_be_bytes());
        out.extend(e.count.to_be_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            out.extend(v);
        } else {
            let off = (start + table + extra.len()) as u32;
            out.extend(off.to_be_bytes());
            extra.extend(&e.data);
            if extra.len() % 2 == 1 {
                extra.push(0);
            }
        }
    }
    out.extend(next.to_be_bytes());
    out.extend(extra);
}

fn ifd_size(entries: &[Entry]) -> usize {
    2 + entries.len() * 12
        + 4
        + entries
            .iter()
            .filter(|e| e.data.len() > 4)
            .map(|e| e.data.len().div_ceil(2) * 2)
            .sum::<usize>()
}

/// "AAAA-MM-DDTHH:MM:SS" → "AAAA:MM:DD HH:MM:SS".
fn exif_date(s: &str) -> Option<String> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    Some(format!("{}:{}:{} {}", &s[0..4], &s[5..7], &s[8..10], &s[11..19]))
}

fn dms(v: f64) -> [(u32, u32); 3] {
    let v = v.abs();
    let d = v.floor();
    let m = ((v - d) * 60.0).floor();
    let s = ((v - d) * 60.0 - m) * 60.0;
    [(d as u32, 1), (m as u32, 1), rational(s, 10000)]
}

/// EXIF como TIFF (cabeçalho "MM"), sem o prefixo "Exif\0\0". `None` se não há nada.
pub fn exif(m: &OutMeta) -> Option<Vec<u8>> {
    if m.is_empty() {
        return None;
    }
    let mut ifd0 = vec![
        short(0x0112, 1),
        rationals(0x011A, &[rational(m.ppi as f64, 100)]),
        rationals(0x011B, &[rational(m.ppi as f64, 100)]),
        short(0x0128, 2),
        ascii(0x0131, SOFTWARE),
    ];
    if !m.creator.is_empty() {
        ifd0.push(text(0x013B, &m.creator));
    }
    if !m.copyright.is_empty() {
        ifd0.push(text(0x8298, &m.copyright));
    }
    let mut exif_ifd = vec![
        Entry { tag: 0x9000, typ: UNDEFINED, count: 4, data: b"0232".to_vec() },
        short(0xA001, if m.srgb { 1 } else { 0xFFFF }),
        long(0xA002, m.width),
        long(0xA003, m.height),
    ];
    if let Some(c) = &m.camera {
        if let Some(v) = &c.make {
            ifd0.push(text(0x010F, v));
        }
        if let Some(v) = &c.model {
            ifd0.push(text(0x0110, v));
        }
        if let Some(d) = c.capture_time.as_deref().and_then(exif_date) {
            ifd0.push(ascii(0x0132, &d));
            exif_ifd.push(ascii(0x9003, &d));
            exif_ifd.push(ascii(0x9004, &d));
        }
        if let Some(o) = &c.capture_offset {
            exif_ifd.push(ascii(0x9011, o));
        }
        if let Some(v) = c.exposure_time {
            exif_ifd.push(rationals(
                0x829A,
                &[if v < 1.0 { (1, (1.0 / v).round() as u32) } else { rational(v, 10) }],
            ));
        }
        if let Some(v) = c.f_number {
            exif_ifd.push(rationals(0x829D, &[rational(v, 10)]));
        }
        if let Some(v) = c.iso {
            exif_ifd.push(short(0x8827, v.min(65535) as u16));
        }
        if let Some(v) = c.focal_length {
            exif_ifd.push(rationals(0x920A, &[rational(v, 10)]));
        }
        if let Some(v) = &c.serial {
            exif_ifd.push(text(0xA431, v));
        }
        if let Some(v) = &c.lens {
            exif_ifd.push(text(0xA434, v));
        }
    }
    let gps_ifd = m.gps.map(|(lat, lon)| {
        vec![
            Entry { tag: 0x0000, typ: BYTE, count: 4, data: vec![2, 3, 0, 0] },
            ascii(0x0001, if lat >= 0.0 { "N" } else { "S" }),
            rationals(0x0002, &dms(lat)),
            ascii(0x0003, if lon >= 0.0 { "E" } else { "W" }),
            rationals(0x0004, &dms(lon)),
        ]
    });
    // Ponteiros: o tamanho do IFD0 não depende dos valores dos ponteiros.
    ifd0.push(long(0x8769, 0));
    if gps_ifd.is_some() {
        ifd0.push(long(0x8825, 0));
    }
    let exif_off = 8 + ifd_size(&ifd0);
    let gps_off = exif_off + ifd_size(&exif_ifd);
    for e in ifd0.iter_mut() {
        if e.tag == 0x8769 {
            e.data = (exif_off as u32).to_be_bytes().to_vec();
        } else if e.tag == 0x8825 {
            e.data = (gps_off as u32).to_be_bytes().to_vec();
        }
    }
    let mut out = b"MM\0\x2a\0\0\0\x08".to_vec();
    write_ifd(&mut out, ifd0, 0);
    debug_assert_eq!(out.len(), exif_off);
    write_ifd(&mut out, exif_ifd, 0);
    if let Some(g) = gps_ifd {
        write_ifd(&mut out, g, 0);
    }
    Some(out)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Pacote XMP (UTF-8). `None` se não há nada a escrever.
pub fn xmp(m: &OutMeta) -> Option<String> {
    if m.is_empty() {
        return None;
    }
    let mut props = format!("   <xmp:CreatorTool>{SOFTWARE}</xmp:CreatorTool>\n");
    if !m.creator.is_empty() {
        props += &format!(
            "   <dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>\n",
            esc(&m.creator)
        );
    }
    if !m.copyright.is_empty() {
        props += &format!(
            "   <dc:rights><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:rights>\n   <xmpRights:Marked>True</xmpRights:Marked>\n",
            esc(&m.copyright)
        );
    }
    if !m.contact.is_empty() {
        props += &format!(
            "   <Iptc4xmpCore:CreatorContactInfo rdf:parseType=\"Resource\"><Iptc4xmpCore:CiUrlWork>{}</Iptc4xmpCore:CiUrlWork></Iptc4xmpCore:CreatorContactInfo>\n",
            esc(&m.contact)
        );
    }
    if let Some(c) = &m.camera {
        if let Some(t) = &c.capture_time {
            props += &format!(
                "   <photoshop:DateCreated>{}{}</photoshop:DateCreated>\n",
                esc(t),
                esc(c.capture_offset.as_deref().unwrap_or(""))
            );
        }
        if let Some(r) = c.rating.filter(|r| *r > 0) {
            props += &format!("   <xmp:Rating>{}</xmp:Rating>\n", r.min(5));
        }
    }
    Some(format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description rdf:about=\"\"\n\
    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n\
    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n\
    xmlns:xmpRights=\"http://ns.adobe.com/xap/1.0/rights/\"\n\
    xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\"\n\
    xmlns:Iptc4xmpCore=\"http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/\">\n\
{props}  </rdf:Description>\n\
 </rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo() -> PhotoMeta {
        PhotoMeta {
            file_stem: "IMG_0001".into(),
            capture_time: Some("2026-06-15T17:42:05".into()),
            capture_offset: Some("+01:00".into()),
            make: Some("Canon".into()),
            model: Some("EOS R5".into()),
            lens: Some("RF 50mm F1.2".into()),
            iso: Some(400),
            exposure_time: Some(1.0 / 250.0),
            f_number: Some(2.8),
            focal_length: Some(50.0),
            gps: Some((38.7967, -9.3906)),
            rating: Some(4),
            ..Default::default()
        }
    }

    fn meta(mode: MetadataMode, remove_gps: bool) -> OutMeta {
        let m = Metadata {
            mode,
            remove_gps,
            copyright: "© 2026 Estúdio Lino".into(),
            creator: "José Lino".into(),
            contact: "https://example.com".into(),
        };
        OutMeta::new(&m, &photo(), 2048, 1365, 300.0, true)
    }

    /// O EXIF que escrevemos é lido corretamente por um leitor independente (kamadak-exif).
    #[test]
    fn exif_is_readable_and_gps_follows_the_option() {
        let read = |b: &[u8]| exif::Reader::new().read_raw(b.to_vec()).unwrap();
        let with = read(&exif(&meta(MetadataMode::All, false)).unwrap());
        let f = |e: &exif::Exif, t: exif::Tag| {
            e.get_field(t, exif::In::PRIMARY).map(|f| f.display_value().to_string())
        };
        assert_eq!(f(&with, exif::Tag::Model).as_deref(), Some("\"EOS R5\""));
        assert_eq!(f(&with, exif::Tag::PhotographicSensitivity).as_deref(), Some("400"));
        assert_eq!(f(&with, exif::Tag::ExposureTime).as_deref(), Some("1/250"));
        assert_eq!(f(&with, exif::Tag::DateTimeOriginal).as_deref(), Some("2026-06-15 17:42:05"));
        assert!(f(&with, exif::Tag::GPSLatitude).is_some());
        assert_eq!(f(&with, exif::Tag::GPSLongitudeRef).as_deref(), Some("W"));
        assert_eq!(
            f(&with, exif::Tag::Copyright).as_deref(),
            Some("\"(c) 2026 Estudio Lino\""),
            "EXIF em ASCII"
        );

        let no_gps = read(&exif(&meta(MetadataMode::All, true)).unwrap());
        assert!(f(&no_gps, exif::Tag::GPSLatitude).is_none(), "GPS removido");
        assert!(f(&no_gps, exif::Tag::Model).is_some());

        let copy = read(&exif(&meta(MetadataMode::Copyright, false)).unwrap());
        assert!(f(&copy, exif::Tag::Model).is_none(), "só copyright: sem câmara");
        assert!(f(&copy, exif::Tag::GPSLatitude).is_none(), "só copyright: sem GPS");
        assert!(f(&copy, exif::Tag::Artist).is_some());

        assert!(exif(&meta(MetadataMode::None, false)).is_none());
        assert!(xmp(&meta(MetadataMode::None, false)).is_none());
    }

    #[test]
    fn xmp_escapes_and_contains_rights() {
        let mut m = meta(MetadataMode::All, true);
        m.creator = "Ana & <Pedro>".into();
        let x = xmp(&m).unwrap();
        assert!(x.contains("Ana &amp; &lt;Pedro&gt;"));
        assert!(x.contains("© 2026 Estúdio Lino"));
        assert!(x.contains("<xmp:Rating>4</xmp:Rating>"));
        assert!(!x.contains("GPS"));
    }
}
