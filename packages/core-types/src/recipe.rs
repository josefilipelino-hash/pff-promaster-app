//! Receita de edição não destrutiva (esquema).
//!
//! A receita é a **fonte de verdade** de uma edição: os originais nunca são alterados e
//! as previews/exportações são sempre derivadas de `original + receita`.
//!
//! Regras de evolução do esquema (ver ARCHITECTURE.md §12):
//! - `v` é incrementado quando o significado de um campo muda;
//! - campos novos são opcionais e têm valor neutro por omissão;
//! - campos desconhecidos (de versões futuras) são preservados em `extra` e voltam a
//!   ser gravados, para que uma versão antiga da aplicação não destrua edições novas.
//!
//! Na Fase 0 só existe o envelope. Os blocos de ajustes concretos entram na Fase 4.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RECIPE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditRecipe {
    /// Versão do esquema da receita.
    pub v: u32,
    /// Blocos de ajustes por nome (`"light"`, `"color"`, …). Vazio = original.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub blocks: BTreeMap<String, serde_json::Value>,
    /// Campos desconhecidos preservados (compatibilidade com versões futuras).
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for EditRecipe {
    fn default() -> Self {
        Self { v: RECIPE_SCHEMA_VERSION, blocks: BTreeMap::new(), extra: BTreeMap::new() }
    }
}

impl EditRecipe {
    /// `true` se a receita não altera a imagem.
    pub fn is_identity(&self) -> bool {
        self.blocks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_recipe_is_identity() {
        let r = EditRecipe::default();
        assert!(r.is_identity());
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"v":1}"#);
    }

    #[test]
    fn unknown_future_fields_survive_roundtrip() {
        let json = r#"{"v":1,"blocks":{"light":{"exposure":0.5}},"futureField":{"a":1}}"#;
        let r: EditRecipe = serde_json::from_str(json).unwrap();
        assert!(r.extra.contains_key("futureField"));
        let back: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(back["futureField"]["a"], 1);
        assert_eq!(back["blocks"]["light"]["exposure"], 0.5);
    }
}
