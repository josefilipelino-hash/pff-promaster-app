//! Estado da interface guardado no catálogo (filtro, tamanho das miniaturas, seleção),
//! para que o projeto reabra exatamente como foi deixado.

use crate::{db_err, Catalog};
use momento_core_types::CoreResult;
use rusqlite::OptionalExtension;

const KEY: &str = "ui_state";

impl Catalog {
    /// JSON opaco definido pela interface (máximo 64 KiB).
    pub fn save_ui_state(&self, json: &str) -> CoreResult<()> {
        if json.len() > 64 * 1024 || serde_json::from_str::<serde_json::Value>(json).is_err() {
            return Err(momento_core_types::CoreError::Other(
                "estado da interface inválido".into(),
            ));
        }
        self.connection()
            .execute(
                "INSERT INTO catalog_info(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [KEY, json],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn load_ui_state(&self) -> CoreResult<Option<String>> {
        self.connection()
            .query_row("SELECT value FROM catalog_info WHERE key = ?1", [KEY], |r| r.get(0))
            .optional()
            .map_err(db_err)
    }
}

impl Catalog {
    /// Valor guardado em `catalog_info` (ex.: definições da galeria).
    pub fn load_value(&self, key: &str) -> CoreResult<Option<String>> {
        self.connection()
            .query_row("SELECT value FROM catalog_info WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .map_err(db_err)
    }

    pub fn save_value(&self, key: &str, value: &str) -> CoreResult<()> {
        if value.len() > 4 * 1024 * 1024 {
            return Err(momento_core_types::CoreError::Other("valor demasiado grande".into()));
        }
        self.connection()
            .execute(
                "INSERT INTO catalog_info(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, value],
            )
            .map_err(db_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Catalog;

    #[test]
    fn ui_state_roundtrip_and_validation() {
        let c = Catalog::open_in_memory().unwrap();
        assert_eq!(c.load_ui_state().unwrap(), None);
        c.save_ui_state(r#"{"filter":"picks","thumbSize":220}"#).unwrap();
        c.save_ui_state(r#"{"filter":"all","thumbSize":180}"#).unwrap();
        assert_eq!(c.load_ui_state().unwrap().unwrap(), r#"{"filter":"all","thumbSize":180}"#);
        assert!(c.save_ui_state("not json").is_err());
    }
}
