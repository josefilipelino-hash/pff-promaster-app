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

/// Imagem descodificada em RGB de 8 bits (Fase 1: miniaturas e previews).
/// O pipeline de revelação em `f32` chega na Fase 4.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub image: image::RgbImage,
    /// Perfil ICC embutido no ficheiro, para ser preservado nas previews.
    pub icc_profile: Option<Vec<u8>>,
    /// Dimensões originais (antes de qualquer redução).
    pub source_width: u32,
    pub source_height: u32,
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
    pub fn probe(&self, data: &[u8], format: FileFormat) -> CoreResult<ImageProbe> {
        let mut last = CoreError::UnsupportedFormat(format!("{format:?}"));
        for c in self.candidates(format, |c| c.probe) {
            match c.probe(data, format) {
                Ok(p) => return Ok(p),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    pub fn decode(
        &self,
        data: &[u8],
        format: FileFormat,
        min_short_edge: Option<u32>,
    ) -> CoreResult<DecodedImage> {
        let mut last = CoreError::UnsupportedFormat(format!("{format:?}"));
        for c in self.candidates(format, |c| c.decode) {
            match c.decode(data, format, min_short_edge) {
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
