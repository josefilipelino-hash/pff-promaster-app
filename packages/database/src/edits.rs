//! Edição não destrutiva: receita por variante, histórico (desfazer/refazer), instantâneos
//! e cópias virtuais. Os ficheiros originais nunca são tocados.

use crate::{db_err, now_unix, Catalog};
use momento_core_types::recipe::EditRecipe;
use momento_core_types::{
    decode_key, virtual_key, AssetId, CoreError, CoreResult, PhotoRef, VariantId,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub seq: i64,
    pub label: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRow {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}

/// Estado de edição de uma fotografia (para a interface).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditState {
    pub key: i64,
    pub asset: AssetId,
    pub variant: i64,
    pub recipe: EditRecipe,
    /// Posição atual no histórico (0 = original).
    pub position: i64,
    pub history: Vec<HistoryEntry>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub snapshots: Vec<SnapshotRow>,
}

fn parse(s: &str) -> CoreResult<EditRecipe> {
    serde_json::from_str(s).map_err(|e| CoreError::Other(format!("receita inválida: {e}")))
}

fn json(r: &EditRecipe) -> String {
    serde_json::to_string(r).expect("receita serializável")
}

impl Catalog {
    /// Variante e asset designados por uma chave da interface.
    pub fn resolve_key(&self, key: i64) -> CoreResult<(VariantId, AssetId)> {
        let c = self.connection();
        let row = match decode_key(key) {
            PhotoRef::Master(a) => c
                .query_row(
                    "SELECT id, asset_id FROM variant WHERE asset_id = ?1 AND is_master = 1",
                    [a.0],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional(),
            PhotoRef::Variant(v) => c
                .query_row("SELECT id, asset_id FROM variant WHERE id = ?1", [v.0], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional(),
        }
        .map_err(db_err)?;
        let (v, a): (i64, i64) =
            row.ok_or_else(|| CoreError::Other(format!("fotografia {key} inexistente")))?;
        Ok((VariantId(v), AssetId(a)))
    }

    pub fn recipe_of(&self, variant: VariantId) -> CoreResult<EditRecipe> {
        match self
            .connection()
            .query_row("SELECT recipe FROM edit WHERE variant_id = ?1", [variant.0], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .map_err(db_err)?
        {
            Some(s) => parse(&s),
            None => Ok(EditRecipe::default()),
        }
    }

    pub fn edit_state(&self, key: i64) -> CoreResult<EditState> {
        let (variant, asset) = self.resolve_key(key)?;
        let c = self.connection();
        let (recipe, position) = c
            .query_row(
                "SELECT recipe, position FROM edit WHERE variant_id = ?1",
                [variant.0],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(db_err)?
            .unwrap_or_else(|| (json(&EditRecipe::default()), 0));
        let mut st = c.prepare("SELECT seq, label, created_at FROM edit_history WHERE variant_id = ?1 ORDER BY seq").map_err(db_err)?;
        let history: Vec<HistoryEntry> = st
            .query_map([variant.0], |r| {
                Ok(HistoryEntry { seq: r.get(0)?, label: r.get(1)?, created_at: r.get(2)? })
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        let mut sn = c.prepare("SELECT id, name, created_at FROM snapshot WHERE variant_id = ?1 ORDER BY created_at, id").map_err(db_err)?;
        let snapshots = sn
            .query_map([variant.0], |r| {
                Ok(SnapshotRow { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)? })
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        let max = history.last().map(|h| h.seq).unwrap_or(0);
        Ok(EditState {
            key,
            asset,
            variant: variant.0,
            recipe: parse(&recipe)?,
            position,
            can_undo: position > 0,
            can_redo: position < max,
            history,
            snapshots,
        })
    }

    /// Grava uma alteração: nova entrada no histórico (as entradas “à frente”, depois de
    /// desfazer, são descartadas) e a receita atual.
    pub fn commit_edit(
        &mut self,
        variant: VariantId,
        label: &str,
        recipe: &EditRecipe,
    ) -> CoreResult<()> {
        let now = now_unix();
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        let pos: Option<i64> = tx
            .query_row("SELECT position FROM edit WHERE variant_id = ?1", [variant.0], |r| r.get(0))
            .optional()
            .map_err(db_err)?;
        let pos = match pos {
            Some(p) => p,
            None => {
                // Primeira edição: a entrada 0 é o original (receita vazia).
                tx.execute(
                    "INSERT OR IGNORE INTO edit_history(variant_id, seq, label, recipe, created_at) VALUES(?1, 0, 'original', ?2, ?3)",
                    params![variant.0, json(&EditRecipe::default()), now],
                )
                .map_err(db_err)?;
                0
            }
        };
        tx.execute(
            "DELETE FROM edit_history WHERE variant_id = ?1 AND seq > ?2",
            params![variant.0, pos],
        )
        .map_err(db_err)?;
        tx.execute(
            "INSERT INTO edit_history(variant_id, seq, label, recipe, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![variant.0, pos + 1, label, json(recipe), now],
        )
        .map_err(db_err)?;
        tx.execute(
            "INSERT INTO edit(variant_id, recipe, position, updated_at) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(variant_id) DO UPDATE SET recipe = excluded.recipe, position = excluded.position, updated_at = excluded.updated_at",
            params![variant.0, json(recipe), pos + 1, now],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    /// Vai para uma entrada do histórico (desfazer/refazer/clicar no histórico).
    pub fn goto_history(&mut self, variant: VariantId, seq: i64) -> CoreResult<EditRecipe> {
        let recipe: String = self
            .connection()
            .query_row(
                "SELECT recipe FROM edit_history WHERE variant_id = ?1 AND seq = ?2",
                params![variant.0, seq],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| CoreError::Other("entrada do histórico inexistente".into()))?;
        self.connection()
            .execute(
                "UPDATE edit SET recipe = ?1, position = ?2, updated_at = ?3 WHERE variant_id = ?4",
                params![recipe, seq, now_unix(), variant.0],
            )
            .map_err(db_err)?;
        parse(&recipe)
    }

    pub fn undo(&mut self, variant: VariantId) -> CoreResult<Option<EditRecipe>> {
        let pos: Option<i64> = self
            .connection()
            .query_row("SELECT position FROM edit WHERE variant_id = ?1", [variant.0], |r| r.get(0))
            .optional()
            .map_err(db_err)?;
        match pos {
            Some(p) if p > 0 => self.goto_history(variant, p - 1).map(Some),
            _ => Ok(None),
        }
    }

    pub fn redo(&mut self, variant: VariantId) -> CoreResult<Option<EditRecipe>> {
        let pos: Option<i64> = self
            .connection()
            .query_row("SELECT position FROM edit WHERE variant_id = ?1", [variant.0], |r| r.get(0))
            .optional()
            .map_err(db_err)?;
        let Some(p) = pos else { return Ok(None) };
        let exists: bool = self
            .connection()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM edit_history WHERE variant_id = ?1 AND seq = ?2)",
                params![variant.0, p + 1],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        if exists {
            self.goto_history(variant, p + 1).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn create_snapshot(&self, variant: VariantId, name: &str) -> CoreResult<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Other("o nome do instantâneo é obrigatório".into()));
        }
        let recipe = self.recipe_of(variant)?;
        self.connection()
            .execute(
                "INSERT INTO snapshot(variant_id, name, recipe, created_at) VALUES(?1, ?2, ?3, ?4)",
                params![variant.0, name, json(&recipe), now_unix()],
            )
            .map_err(db_err)?;
        Ok(self.connection().last_insert_rowid())
    }

    pub fn snapshot_recipe(&self, id: i64) -> CoreResult<(VariantId, String, EditRecipe)> {
        let (v, name, r): (i64, String, String) = self
            .connection()
            .query_row("SELECT variant_id, name, recipe FROM snapshot WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| CoreError::Other("instantâneo inexistente".into()))?;
        Ok((VariantId(v), name, parse(&r)?))
    }

    pub fn delete_snapshot(&self, id: i64) -> CoreResult<()> {
        self.connection().execute("DELETE FROM snapshot WHERE id = ?1", [id]).map_err(db_err)?;
        Ok(())
    }

    /// Cópia virtual: nova variante do mesmo ficheiro, com a receita atual. Devolve a chave.
    pub fn create_virtual_copy(&mut self, key: i64) -> CoreResult<i64> {
        let (variant, asset) = self.resolve_key(key)?;
        let recipe = self.recipe_of(variant)?;
        let now = now_unix();
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        let n: i64 = tx
            .query_row(
                "SELECT count(*) FROM variant WHERE asset_id = ?1 AND is_master = 0",
                [asset.0],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        tx.execute(
            "INSERT INTO variant(asset_id, name, is_master, created_at) VALUES(?1, ?2, 0, ?3)",
            params![asset.0, format!("{}", n + 1), now],
        )
        .map_err(db_err)?;
        let v = tx.last_insert_rowid();
        tx.execute("INSERT INTO rating(variant_id, updated_at) VALUES(?1, ?2)", params![v, now])
            .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        if !recipe.is_identity() {
            self.commit_edit(VariantId(v), "virtualCopy", &recipe)?;
        }
        Ok(virtual_key(VariantId(v)))
    }

    /// Apaga uma cópia virtual (a versão principal não pode ser apagada; o ficheiro nunca é tocado).
    pub fn delete_virtual_copy(&self, key: i64) -> CoreResult<()> {
        let PhotoRef::Variant(v) = decode_key(key) else {
            return Err(CoreError::Other("só cópias virtuais podem ser apagadas".into()));
        };
        let n = self
            .connection()
            .execute("DELETE FROM variant WHERE id = ?1 AND is_master = 0", [v.0])
            .map_err(db_err)?;
        if n == 0 {
            return Err(CoreError::Other("cópia virtual inexistente".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::tests::new_asset;
    use crate::assets::InsertResult;

    fn recipe(exposure: f64) -> EditRecipe {
        let mut r = EditRecipe::default();
        r.blocks.insert("light".into(), serde_json::json!({ "exposure": exposure }));
        r
    }

    #[test]
    fn history_undo_redo_branching_snapshots_and_virtual_copies() {
        let mut c = Catalog::open_in_memory().unwrap();
        let a = match c.insert_assets(&[new_asset(1)]).unwrap()[0] {
            InsertResult::Inserted(id) => id,
            _ => panic!(),
        };
        let (v, asset) = c.resolve_key(a.0).unwrap();
        assert_eq!(asset, a);
        assert!(c.edit_state(a.0).unwrap().recipe.is_identity());
        c.commit_edit(v, "exposure", &recipe(0.5)).unwrap();
        c.commit_edit(v, "exposure", &recipe(1.0)).unwrap();
        let s = c.edit_state(a.0).unwrap();
        assert_eq!((s.position, s.history.len(), s.can_undo, s.can_redo), (2, 3, true, false));
        assert_eq!(c.undo(v).unwrap().unwrap(), recipe(0.5));
        assert_eq!(c.undo(v).unwrap().unwrap(), EditRecipe::default());
        assert!(c.undo(v).unwrap().is_none(), "não há antes do original");
        assert_eq!(c.redo(v).unwrap().unwrap(), recipe(0.5));
        // Alterar depois de desfazer descarta o “futuro”.
        c.commit_edit(v, "contrast", &recipe(0.7)).unwrap();
        let s = c.edit_state(a.0).unwrap();
        assert_eq!(
            s.history.iter().map(|h| h.label.as_str()).collect::<Vec<_>>(),
            ["original", "exposure", "contrast"]
        );
        assert!(!s.can_redo);
        assert_eq!(c.goto_history(v, 1).unwrap(), recipe(0.5));
        // Instantâneos.
        let snap = c.create_snapshot(v, "Versão quente").unwrap();
        c.commit_edit(v, "exposure", &recipe(-1.0)).unwrap();
        assert_eq!(c.snapshot_recipe(snap).unwrap().2, recipe(0.5));
        assert_eq!(c.edit_state(a.0).unwrap().snapshots.len(), 1);
        assert!(c.create_snapshot(v, " ").is_err());
        // Cópia virtual: herda a receita; classificação e edição independentes.
        let key = c.create_virtual_copy(a.0).unwrap();
        assert!(key >= momento_core_types::VIRTUAL_KEY_BASE);
        let photos = c.list_photos().unwrap();
        assert_eq!(photos.len(), 2);
        let vc = photos.iter().find(|p| p.id.0 == key).unwrap();
        assert_eq!((vc.asset, vc.virtual_copy.as_deref(), vc.edited), (a, Some("1"), true));
        let (vv, _) = c.resolve_key(key).unwrap();
        assert_eq!(c.recipe_of(vv).unwrap(), recipe(-1.0));
        c.set_stars(&[AssetId(key)], 4).unwrap();
        let photos = c.list_photos().unwrap();
        assert_eq!(photos.iter().find(|p| p.id.0 == key).unwrap().stars, 4);
        assert_eq!(photos.iter().find(|p| p.id == a).unwrap().stars, 0, "principal intacta");
        assert!(c.delete_virtual_copy(a.0).is_err(), "a principal não se apaga");
        c.delete_virtual_copy(key).unwrap();
        assert_eq!(c.list_photos().unwrap().len(), 1);
        c.delete_snapshot(snap).unwrap();
        delete_noop(&c);
    }

    fn delete_noop(c: &Catalog) {
        assert!(c.snapshot_recipe(9999).is_err());
    }
}
