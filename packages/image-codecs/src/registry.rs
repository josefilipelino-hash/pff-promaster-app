//! Contrato dos adaptadores de codecs e registo com prioridades.

use crate::detect::FileFormat;
use momento_core_types::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// O que um adaptador sabe fazer para um formato.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodecCapabilities {
    pub probe: bool,
    pub decode: bool,
    pub encode: bool,
}

/// Informação obtida sem descodificar os píxeis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageProbe {
    pub format: FileFormat,
    /// Dimensões gravadas (antes de aplicar a orientação EXIF).
    pub width: u32,
    pub height: u32,
    pub bits_per_sample: u8,
    pub has_alpha: bool,
}

/// Origem dos píxeis devolvidos por `decode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DecodeKind {
    /// Imagem completa (resolução total ou redução exata da imagem principal).
    Full,
    /// Revelação RAW a meia resolução.
    HalfSize,
    /// Preview JPEG/bitmap embutida no ficheiro (RAW, PSD, …).
    EmbeddedPreview,
}

/// Nível de suporte de um ficheiro concreto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "level", content = "note")]
pub enum SupportLevel {
    /// Todos os dados de imagem podem ser descodificados.
    Full,
    /// Só parte é utilizável (ex.: apenas a preview embutida de um RAW cujo formato de
    /// compressão do sensor não é suportado). A nota explica o motivo.
    Partial(String),
}

/// Metadados obtidos pelo próprio adaptador (ex.: LibRaw para RAW). Os campos `None`
/// são completados pelo leitor EXIF genérico.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodecMetadata {
    pub orientation: Option<u8>,
    pub capture_time: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
    pub focal_length: Option<f64>,
    pub focal_length_35mm: Option<u32>,
    /// Descrição do espaço/perfil de cor (ex.: “sRGB IEC61966-2.1”, “Câmara RGBG”).
    pub color_space: Option<String>,
    /// Dimensões da maior preview embutida, se existir.
    pub embedded_preview: Option<(u32, u32)>,
    /// Informação técnica (ex.: descodificador LibRaw usado).
    pub detail: Option<String>,
    pub support: Option<SupportLevel>,
}

/// Imagem descodificada em RGB de 8 bits (miniaturas e previews).
/// O pipeline de revelação em `f32` chega na Fase 4.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub image: image::RgbImage,
    /// Perfil ICC embutido no ficheiro, para ser preservado nas previews.
    pub icc_profile: Option<Vec<u8>>,
    /// Dimensões originais (antes de qualquer redução).
    pub source_width: u32,
    pub source_height: u32,
    pub kind: DecodeKind,
}

/// Contrato de um adaptador. As implementações têm de ser `Send + Sync`, porque são
/// chamadas a partir de várias threads em simultâneo.
pub trait ImageCodec: Send + Sync {
    /// Identificador estável, ex.: `"raster"`, `"libraw"`, `"imageio"`.
    fn id(&self) -> &'static str;
    /// Prioridade: o registo escolhe o maior valor entre os adaptadores disponíveis.
    fn priority(&self) -> i32 {
        0
    }
    fn capabilities(&self, format: FileFormat) -> CodecCapabilities;
    /// Lê dimensões e propriedades básicas sem descodificar os píxeis.
    fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe>;
    /// Descodifica com o lado menor do resultado ≥ `min_short_edge` quando o formato
    /// permite redução barata (JPEG: 1/2, 1/4, 1/8); `None` = resolução total.
    fn decode(
        &self,
        data: &[u8],
        format: FileFormat,
        min_short_edge: Option<u32>,
    ) -> CoreResult<DecodedImage>;
    /// Metadados próprios do formato. Por omissão, nenhum (usa-se o leitor EXIF genérico).
    fn metadata(&self, _data: &[u8], _format: FileFormat) -> Option<CodecMetadata> {
        None
    }
}

/// Prefixo das mensagens de erro quando falta o descodificador para um formato reconhecido.
pub const MISSING_CODEC: &str = "MISSING_CODEC";

fn missing_codec(format: FileFormat) -> CoreError {
    CoreError::UnsupportedFormat(format!(
        "{MISSING_CODEC}: sem descodificador para {format:?} nesta plataforma"
    ))
}

/// `true` se o erro indica que falta um descodificador (formato reconhecido).
pub fn is_missing_codec(e: &CoreError) -> bool {
    matches!(e, CoreError::UnsupportedFormat(m) if m.starts_with(MISSING_CODEC))
}

/// Executa uma operação de um adaptador convertendo pânicos em erro “corrompido”: um
/// ficheiro malformado nunca pode terminar a importação nem a aplicação.
fn guarded<T>(codec: &str, f: impl FnOnce() -> CoreResult<T>) -> CoreResult<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => Err(CoreError::Corrupt(format!(
            "falha interna do descodificador {codec} (ficheiro malformado)"
        ))),
    }
}

#[derive(Default, Clone)]
pub struct CodecRegistry {
    codecs: Vec<Arc<dyn ImageCodec>>,
}

impl CodecRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, codec: Arc<dyn ImageCodec>) {
        self.codecs.push(codec);
        // Ordem estável: prioridade decrescente, depois ordem de registo.
        self.codecs.sort_by_key(|c| std::cmp::Reverse(c.priority()));
    }

    /// Adaptadores capazes de satisfazer `need` para `format`, por ordem de preferência.
    pub fn candidates(
        &self,
        format: FileFormat,
        need: impl Fn(&CodecCapabilities) -> bool,
    ) -> Vec<Arc<dyn ImageCodec>> {
        self.codecs.iter().filter(|c| need(&c.capabilities(format))).cloned().collect()
    }

    pub fn supports_decode(&self, format: FileFormat) -> bool {
        !self.candidates(format, |c| c.decode).is_empty()
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.codecs.iter().map(|c| c.id()).collect()
    }

    /// Tenta cada adaptador por ordem; devolve o primeiro sucesso ou o último erro.
    /// Se nenhum adaptador existir para o formato nesta compilação/plataforma, devolve
    /// [`CoreError::UnsupportedFormat`] com o prefixo `MISSING_CODEC`.
    pub fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        let mut last = missing_codec(format);
        for c in self.candidates(format, |c| c.probe) {
            match guarded(c.id(), || c.probe(data, format)) {
                Ok(p) => return Ok(p),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Metadados do primeiro adaptador que os fornecer.
    pub fn metadata(&self, data: &[u8], format: FileFormat) -> Option<CodecMetadata> {
        self.candidates(format, |c| c.probe)
            .iter()
            .find_map(|c| guarded(c.id(), || Ok(c.metadata(data, format))).ok().flatten())
    }

    pub fn decode(
        &self,
        data: &[u8],
        format: FileFormat,
        min_short_edge: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        let mut last = missing_codec(format);
        for c in self.candidates(format, |c| c.decode) {
            match guarded(c.id(), || c.decode(data, format, min_short_edge)) {
                Ok(d) => return Ok(d),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        id: &'static str,
        prio: i32,
        formats: &'static [FileFormat],
        fails: bool,
    }

    impl ImageCodec for Fake {
        fn id(&self) -> &'static str {
            self.id
        }
        fn priority(&self) -> i32 {
            self.prio
        }
        fn capabilities(&self, format: FileFormat) -> CodecCapabilities {
            let ok = self.formats.contains(&format);
            CodecCapabilities { probe: ok, decode: ok, encode: false }
        }
        fn probe(&self, _: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
            if self.fails {
                return Err(CoreError::Corrupt(self.id.into()));
            }
            Ok(ImageProbe { format, width: 1, height: 1, bits_per_sample: 8, has_alpha: false })
        }
        fn decode(&self, _: &[u8], _: FileFormat, _: Option<u32>) -> CoreResult<DecodedImage> {
            Err(CoreError::Other("fake".into()))
        }
    }

    #[test]
    fn candidates_are_ordered_by_priority_and_filtered() {
        let mut r = CodecRegistry::new();
        r.register(Arc::new(Fake {
            id: "libheif",
            prio: 10,
            formats: &[FileFormat::Heic, FileFormat::Avif],
            fails: false,
        }));
        r.register(Arc::new(Fake {
            id: "imageio",
            prio: 50,
            formats: &[FileFormat::Heic],
            fails: false,
        }));
        r.register(Arc::new(Fake {
            id: "image-rs",
            prio: 20,
            formats: &[FileFormat::Jpeg],
            fails: false,
        }));

        let heic: Vec<_> =
            r.candidates(FileFormat::Heic, |c| c.probe).iter().map(|c| c.id()).collect();
        assert_eq!(heic, ["imageio", "libheif"]);
        assert!(r.candidates(FileFormat::Psd, |c| c.probe).is_empty());
        assert!(!r.supports_decode(FileFormat::Psd));
    }

    struct Panicky;
    impl ImageCodec for Panicky {
        fn id(&self) -> &'static str {
            "panicky"
        }
        fn capabilities(&self, _: FileFormat) -> CodecCapabilities {
            CodecCapabilities { probe: true, decode: true, encode: false }
        }
        fn probe(&self, _: &[u8], _: FileFormat) -> CoreResult<ImageProbe> {
            panic!("descodificador com defeito")
        }
        fn decode(&self, _: &[u8], _: FileFormat, _: Option<u32>) -> CoreResult<DecodedImage> {
            panic!("descodificador com defeito")
        }
        fn metadata(&self, _: &[u8], _: FileFormat) -> Option<CodecMetadata> {
            panic!("descodificador com defeito")
        }
    }

    #[test]
    fn panics_in_codecs_become_corrupt_errors() {
        let mut r = CodecRegistry::new();
        r.register(Arc::new(Panicky));
        assert!(matches!(r.probe(&[], FileFormat::Png), Err(CoreError::Corrupt(_))));
        assert!(matches!(r.decode(&[], FileFormat::Png, None), Err(CoreError::Corrupt(_))));
        assert!(r.metadata(&[], FileFormat::Png).is_none());
    }

    #[test]
    fn missing_codec_is_identifiable() {
        let r = CodecRegistry::new();
        let e = r.probe(&[], FileFormat::Heic).unwrap_err();
        assert!(is_missing_codec(&e));
        assert!(!is_missing_codec(&CoreError::Corrupt("x".into())));
    }

    #[test]
    fn falls_back_to_next_codec_on_error() {
        let mut r = CodecRegistry::new();
        r.register(Arc::new(Fake { id: "a", prio: 50, formats: &[FileFormat::Heic], fails: true }));
        r.register(Arc::new(Fake {
            id: "b",
            prio: 10,
            formats: &[FileFormat::Heic],
            fails: false,
        }));
        assert!(r.probe(&[], FileFormat::Heic).is_ok());
        assert!(matches!(r.probe(&[], FileFormat::Psd), Err(CoreError::UnsupportedFormat(_))));
    }
}
