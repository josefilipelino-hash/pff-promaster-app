//! Contrato dos adaptadores de codecs e registo com prioridades.

use crate::detect::FileFormat;
use momento_core_types::CoreResult;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// O que um adaptador sabe fazer para um formato.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodecCapabilities {
    pub probe: bool,
    pub embedded_preview: bool,
    pub decode_full: bool,
    pub encode: bool,
    pub metadata: bool,
}

/// Informação obtida sem descodificar os píxeis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageProbe {
    pub format: FileFormat,
    pub width: u32,
    pub height: u32,
    pub bits_per_sample: u8,
    pub has_alpha: bool,
    /// Orientação EXIF (1–8).
    pub orientation: u8,
    pub has_icc_profile: bool,
}

/// Contrato de um adaptador. As implementações têm de ser `Send + Sync`, porque são
/// chamadas a partir de várias threads de trabalho em simultâneo.
///
/// Os métodos de descodificação de píxeis (`decode_preview`, `decode_full`) entram na
/// Fase 1 juntamente com o tipo de buffer de imagem do `image-processing`.
pub trait ImageCodec: Send + Sync {
    /// Identificador estável, ex.: `"image-rs"`, `"libraw"`, `"imageio"`.
    fn id(&self) -> &'static str;
    /// Prioridade: o registo escolhe o maior valor entre os adaptadores disponíveis.
    fn priority(&self) -> i32 {
        0
    }
    /// Capacidades para um formato (todas `false` = não suportado).
    fn capabilities(&self, format: FileFormat) -> CodecCapabilities;
    /// Lê dimensões e propriedades básicas sem descodificar os píxeis.
    fn probe(&self, path: &Path, format: FileFormat) -> CoreResult<ImageProbe>;
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
    /// O chamador tenta o seguinte se o primeiro falhar (ex.: ImageIO → libheif).
    pub fn candidates(
        &self,
        format: FileFormat,
        need: impl Fn(&CodecCapabilities) -> bool,
    ) -> Vec<Arc<dyn ImageCodec>> {
        self.codecs.iter().filter(|c| need(&c.capabilities(format))).cloned().collect()
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.codecs.iter().map(|c| c.id()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use momento_core_types::CoreError;

    struct Fake {
        id: &'static str,
        prio: i32,
        formats: &'static [FileFormat],
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
            CodecCapabilities { probe: ok, ..Default::default() }
        }
        fn probe(&self, _: &Path, _: FileFormat) -> CoreResult<ImageProbe> {
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
        }));
        r.register(Arc::new(Fake { id: "imageio", prio: 50, formats: &[FileFormat::Heic] }));
        r.register(Arc::new(Fake { id: "image-rs", prio: 20, formats: &[FileFormat::Jpeg] }));

        let heic: Vec<_> =
            r.candidates(FileFormat::Heic, |c| c.probe).iter().map(|c| c.id()).collect();
        assert_eq!(heic, ["imageio", "libheif"]);
        assert!(r.candidates(FileFormat::Psd, |c| c.probe).is_empty());
    }
}
