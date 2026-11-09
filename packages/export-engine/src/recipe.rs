//! Receita de exportação: tudo o que define um ficheiro de saída. Serializada em JSON
//! (biblioteca global e planos de exportação).

use momento_color_management::{ColorSpace, Intent};
use serde::{Deserialize, Serialize};

/// Formatos de saída. HEIF/HEIC existem na lista para explicar porque não estão
/// disponíveis (ver [`Format::caps`]); nunca produzem ficheiros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Jpeg,
    JpegXl,
    Jpeg2000,
    Png,
    Tiff,
    BigTiff,
    Webp,
    Avif,
    Heif,
    Heic,
    Exr,
    Dng,
    Pdf,
    Psd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BitDepth {
    #[serde(rename = "8")]
    B8,
    #[serde(rename = "10")]
    B10,
    #[serde(rename = "12")]
    B12,
    #[serde(rename = "16")]
    B16,
    #[serde(rename = "16f")]
    F16,
    #[serde(rename = "32f")]
    F32,
}

impl BitDepth {
    pub fn is_float(self) -> bool {
        matches!(self, BitDepth::F16 | BitDepth::F32)
    }
    pub fn bits(self) -> u32 {
        match self {
            BitDepth::B8 => 8,
            BitDepth::B10 => 10,
            BitDepth::B12 => 12,
            BitDepth::B16 | BitDepth::F16 => 16,
            BitDepth::F32 => 32,
        }
    }
}

/// Compressão (as opções válidas dependem do formato).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Compression {
    None,
    /// PNG: compressão rápida.
    Fast,
    /// PNG: máxima.
    Best,
    Lzw,
    Zip,
    Piz,
    /// PSD: RLE (PackBits).
    Rle,
    Lossless,
    Lossy,
}

/// Espaços de cor aceites por um formato.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpaceSupport {
    /// Qualquer espaço, com o perfil ICC embutido (incl. ICC personalizado).
    Icc,
    /// Só os espaços normalizados (sinalizados sem ICC: JPEG XL, EXR).
    Builtin,
    /// Só os que têm código CICP (AVIF): sRGB, Display P3, Rec. 709, Rec. 2020.
    Cicp,
}

/// O que um formato permite — a interface só mostra opções que funcionam.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Caps {
    pub format: Format,
    pub available: bool,
    /// Chave i18n da razão quando indisponível.
    pub reason: Option<&'static str>,
    pub extension: &'static str,
    pub depths: Vec<BitDepth>,
    pub compressions: Vec<Compression>,
    /// Qualidade (1–100) aplicável (com perdas).
    pub quality: bool,
    pub spaces: SpaceSupport,
    /// Metadados EXIF/XMP embutidos.
    pub metadata: bool,
    /// PPI gravado no ficheiro.
    pub resolution: bool,
    /// Dados sempre lineares (EXR).
    pub linear: bool,
}

impl Format {
    pub const ALL: [Format; 14] = [
        Format::Jpeg,
        Format::JpegXl,
        Format::Jpeg2000,
        Format::Png,
        Format::Tiff,
        Format::BigTiff,
        Format::Webp,
        Format::Avif,
        Format::Heif,
        Format::Heic,
        Format::Exr,
        Format::Dng,
        Format::Pdf,
        Format::Psd,
    ];

    pub fn caps(self) -> Caps {
        use BitDepth::*;
        use Compression as C;
        let base = |ext: &'static str| Caps {
            format: self,
            available: true,
            reason: None,
            extension: ext,
            depths: vec![B8],
            compressions: vec![],
            quality: false,
            spaces: SpaceSupport::Icc,
            metadata: true,
            resolution: true,
            linear: false,
        };
        match self {
            Format::Jpeg => Caps { quality: true, ..base("jpg") },
            Format::JpegXl => Caps {
                depths: vec![B8, B16],
                compressions: vec![C::Lossless],
                spaces: SpaceSupport::Builtin,
                resolution: false,
                ..base("jxl")
            },
            Format::Jpeg2000 => Caps {
                depths: vec![B8, B12, B16],
                compressions: vec![C::Lossless, C::Lossy],
                quality: true,
                metadata: false,
                resolution: false,
                ..base("jp2")
            },
            Format::Png => {
                Caps { depths: vec![B8, B16], compressions: vec![C::Fast, C::Best], ..base("png") }
            }
            Format::Tiff | Format::BigTiff => Caps {
                depths: vec![B8, B16, F32],
                compressions: vec![C::None, C::Lzw, C::Zip],
                ..base("tif")
            },
            Format::Webp => Caps {
                compressions: vec![C::Lossy, C::Lossless],
                quality: true,
                resolution: false,
                ..base("webp")
            },
            Format::Avif => Caps {
                depths: vec![B8, B10, B12],
                quality: true,
                spaces: SpaceSupport::Cicp,
                resolution: false,
                ..base("avif")
            },
            Format::Heif | Format::Heic => Caps {
                available: false,
                reason: Some("export.unavailable.hevc"),
                depths: vec![],
                ..base(if self == Format::Heif { "heif" } else { "heic" })
            },
            Format::Exr => Caps {
                depths: vec![F16, F32],
                compressions: vec![C::None, C::Zip, C::Piz],
                spaces: SpaceSupport::Builtin,
                resolution: false,
                linear: true,
                ..base("exr")
            },
            Format::Dng => Caps {
                available: false,
                reason: Some("export.unavailable.dng"),
                depths: vec![],
                ..base("dng")
            },
            Format::Pdf => Caps { quality: true, ..base("pdf") },
            Format::Psd => {
                Caps { depths: vec![B8, B16], compressions: vec![C::None, C::Rle], ..base("psd") }
            }
        }
    }
}

/// Unidade das dimensões.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Unit {
    #[default]
    Px,
    Cm,
    In,
}

/// Como redimensionar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SizeMode {
    /// Resolução total (depois do recorte da edição).
    #[default]
    Original,
    LongEdge,
    ShortEdge,
    Width,
    Height,
    /// Cabe dentro de largura × altura (sem cortar).
    Fit,
    /// Preenche largura × altura exatas, cortando ao centro (ex.: Instagram 4:5).
    Fill,
    /// Megapíxeis.
    Megapixels,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Size {
    pub mode: SizeMode,
    pub unit: Unit,
    /// Lado/largura (na unidade) ou megapíxeis.
    pub a: f32,
    /// Altura (Fit/Fill).
    pub b: f32,
    /// Nunca ampliar acima da resolução da fotografia.
    pub no_enlarge: bool,
    /// Píxeis por polegada (gravado no ficheiro; converte cm/in em píxeis).
    pub ppi: f32,
    /// Fill: rodar a moldura para a orientação da fotografia (4:5 ↔ 5:4).
    pub match_orientation: bool,
}

impl Default for Size {
    fn default() -> Self {
        Size {
            mode: SizeMode::Original,
            unit: Unit::Px,
            a: 2048.0,
            b: 2048.0,
            no_enlarge: true,
            ppi: 300.0,
            match_orientation: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SharpenTarget {
    #[default]
    None,
    Screen,
    Matte,
    Glossy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SharpenAmount {
    Low,
    #[default]
    Standard,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Sharpen {
    pub target: SharpenTarget,
    pub amount: SharpenAmount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataMode {
    /// Câmara, objetiva, exposição, data, autor e copyright (GPS conforme `remove_gps`).
    #[default]
    All,
    /// Só autor, copyright e contacto.
    Copyright,
    /// Nenhum metadado (o perfil de cor mantém-se: não é metadado).
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Metadata {
    pub mode: MetadataMode,
    pub remove_gps: bool,
    pub copyright: String,
    pub creator: String,
    /// Contacto (email/sítio) — XMP.
    pub contact: String,
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            mode: MetadataMode::All,
            remove_gps: true,
            copyright: String::new(),
            creator: String::new(),
            contact: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    #[default]
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WatermarkKind {
    #[default]
    Text,
    Image,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Watermark {
    pub enabled: bool,
    pub kind: WatermarkKind,
    pub text: String,
    /// Imagem PNG (com transparência) ou JPEG.
    pub image_path: String,
    /// Fonte TTF/OTF opcional (vazio = Inter, incluída).
    pub font_path: String,
    /// Cor do texto (sRGB #rrggbb).
    pub color: String,
    /// Tamanho: % do lado menor (altura do texto / largura da imagem).
    pub size: f32,
    /// 0–100.
    pub opacity: f32,
    pub anchor: Anchor,
    /// Margem: % do lado menor.
    pub margin: f32,
}

impl Default for Watermark {
    fn default() -> Self {
        Watermark {
            enabled: false,
            kind: WatermarkKind::Text,
            text: "© ".into(),
            image_path: String::new(),
            font_path: String::new(),
            color: "#ffffff".into(),
            size: 4.0,
            opacity: 60.0,
            anchor: Anchor::BottomRight,
            margin: 3.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Conflict {
    /// Acrescenta -2, -3… ao nome.
    #[default]
    Rename,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Naming {
    /// Modelo: {nome} {seq} {data} {hora} {projeto} {cliente} {receita} {momento} {largura} {altura}
    pub template: String,
    pub prefix: String,
    pub suffix: String,
    pub seq_start: u32,
    pub seq_digits: u32,
    /// Subpasta dentro do destino (pode ter os mesmos campos; "/" separa níveis).
    pub subfolder: String,
    pub conflict: Conflict,
}

impl Default for Naming {
    fn default() -> Self {
        Naming {
            template: "{nome}".into(),
            prefix: String::new(),
            suffix: String::new(),
            seq_start: 1,
            seq_digits: 4,
            subfolder: String::new(),
            conflict: Conflict::Rename,
        }
    }
}

/// Receita completa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub format: Format,
    pub depth: BitDepth,
    pub quality: u8,
    pub compression: Compression,
    pub color: ColorSpace,
    /// Caminho do perfil ICC (personalizado/laboratório) quando `color = icc`.
    pub icc_path: String,
    pub intent: Intent,
    pub size: Size,
    pub sharpen: Sharpen,
    pub metadata: Metadata,
    pub watermark: Watermark,
    pub naming: Naming,
}

impl Default for Recipe {
    fn default() -> Self {
        Recipe {
            id: String::new(),
            name: String::new(),
            format: Format::Jpeg,
            depth: BitDepth::B8,
            quality: 90,
            compression: Compression::Lossy,
            color: ColorSpace::Srgb,
            icc_path: String::new(),
            intent: Intent::Perceptual,
            size: Size::default(),
            sharpen: Sharpen::default(),
            metadata: Metadata::default(),
            watermark: Watermark::default(),
            naming: Naming::default(),
        }
    }
}

impl Recipe {
    /// Corrige combinações impossíveis para o formato (profundidade, compressão, espaço) e
    /// devolve a lista de ajustes feitos (chaves i18n) — nunca falha em silêncio.
    pub fn normalized(&self) -> (Recipe, Vec<&'static str>) {
        let mut r = self.clone();
        let mut notes = Vec::new();
        let caps = r.format.caps();
        if !caps.depths.is_empty() && !caps.depths.contains(&r.depth) {
            r.depth = *caps.depths.last().filter(|_| r.depth.bits() > 8).unwrap_or(&caps.depths[0]);
            notes.push("export.note.depth");
        }
        if caps.compressions.is_empty() {
            r.compression = if caps.quality { Compression::Lossy } else { Compression::None };
        } else if !caps.compressions.contains(&r.compression) {
            r.compression = caps.compressions[if caps.compressions.len() > 1 { 1 } else { 0 }];
            notes.push("export.note.compression");
        }
        let space_ok = match caps.spaces {
            SpaceSupport::Icc => true,
            SpaceSupport::Builtin => r.color != ColorSpace::Icc,
            SpaceSupport::Cicp => matches!(
                r.color,
                ColorSpace::Srgb | ColorSpace::DisplayP3 | ColorSpace::Rec709 | ColorSpace::Rec2020
            ),
        };
        if !space_ok {
            r.color = ColorSpace::Srgb;
            notes.push("export.note.space");
        }
        if r.format == Format::Jpeg2000
            && r.compression == Compression::Lossless
            && r.depth != BitDepth::B8
        {
            // Ver codecs::jp2: sem perdas a 12/16 bits não é lido exatamente pelo OpenJPEG.
            r.depth = BitDepth::B8;
            notes.push("export.note.jp2Lossless");
        }
        r.quality = r.quality.clamp(1, 100);
        r.size.ppi = r.size.ppi.clamp(1.0, 4800.0);
        (r, notes)
    }
}
