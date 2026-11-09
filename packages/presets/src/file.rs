//! Formato próprio `.momentopreset`: JSON legível com um ou mais presets e as LUT que
//! usam (texto `.cube` incluído), para que o ficheiro funcione noutro computador.

use crate::Preset;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT: &str = "momento-preset";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "momentopreset";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetFile {
    pub format: String,
    pub version: u32,
    pub presets: Vec<Preset>,
    /// Identificador da LUT → conteúdo `.cube`.
    #[serde(default)]
    pub luts: BTreeMap<String, String>,
}

/// Serializa presets (os internos exportam-se como presets normais).
pub fn export(presets: &[Preset], luts: BTreeMap<String, String>) -> String {
    let presets = presets.iter().cloned().map(|mut p| {
        p.builtin = false;
        p
    });
    serde_json::to_string_pretty(&PresetFile {
        format: FORMAT.into(),
        version: VERSION,
        presets: presets.collect(),
        luts,
    })
    .expect("serializável")
}

/// Lê um ficheiro `.momentopreset`. Recusa outros formatos e versões mais recentes.
pub fn import(text: &str) -> Result<PresetFile, String> {
    let f: PresetFile =
        serde_json::from_str(text).map_err(|e| format!("ficheiro inválido: {e}"))?;
    if f.format != FORMAT {
        return Err(format!("formato desconhecido: {}", f.format));
    }
    if f.version > VERSION {
        return Err(format!("criado por uma versão mais recente (formato {})", f.version));
    }
    if f.presets.is_empty() {
        return Err("o ficheiro não tem presets".into());
    }
    for (id, cube) in &f.luts {
        momento_image_processing::develop::lut::parse_cube(cube)
            .map_err(|e| format!("LUT {id}: {e}"))?;
    }
    Ok(f)
}
