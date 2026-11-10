//! Segurança do catálogo e dos originais (Fase 13): validação, checkpoints do WAL,
//! disponibilidade dos originais (online / offline / em falta), remoção só do catálogo e
//! relink. Nada aqui apaga ficheiros.

use crate::{db_err, now_unix, Catalog};
use momento_core_types::{AssetId, CoreError, CoreResult};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Disponibilidade do original de uma fotografia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    /// O ficheiro existe.
    Ok,
    /// A pasta de topo do original não está acessível (disco desligado, rede): a
    /// referência fica intacta e volta sozinha quando o disco regressar.
    Offline,
    /// O disco está lá, mas o ficheiro não (movido, renomeado, apagado fora da aplicação).
    Missing,
}

pub(crate) fn default_availability() -> Availability {
    Availability::Ok
}

impl Availability {
    pub fn as_str(self) -> &'static str {
        match self {
            Availability::Ok => "ok",
            Availability::Offline => "offline",
            Availability::Missing => "missing",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "offline" => Availability::Offline,
            "missing" => Availability::Missing,
            _ => Availability::Ok,
        }
    }
}

/// Original de uma fotografia, para verificar a disponibilidade e fazer relink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Original {
    pub id: AssetId,
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    pub content_hash: [u8; 32],
    pub status: Availability,
    pub backup_path: Option<String>,
}

/// Resultado de uma validação: lista vazia = íntegro.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    pub integrity: Vec<String>,
    pub foreign_keys: u64,
}

impl Validation {
    pub fn ok(&self) -> bool {
        self.integrity.is_empty() && self.foreign_keys == 0
    }
}

impl Catalog {
    /// `quick_check` (rápido: estrutura das páginas) ou `integrity_check` (completo:
    /// também índices) + chaves estrangeiras.
    pub fn validate(&self, full: bool) -> CoreResult<Validation> {
        let pragma = if full { "PRAGMA integrity_check(100)" } else { "PRAGMA quick_check(100)" };
        let mut st = self.connection().prepare(pragma).map_err(db_err)?;
        let integrity: Vec<String> = st
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?
            .into_iter()
            .filter(|s| s != "ok")
            .collect();
        let foreign_keys: u64 = self
            .connection()
            .prepare("PRAGMA foreign_key_check")
            .map_err(db_err)?
            .query_map([], |_| Ok(()))
            .map_err(db_err)?
            .count() as u64;
        Ok(Validation { integrity, foreign_keys })
    }

    /// Checkpoint do WAL: as alterações passam para o ficheiro principal e o WAL é
    /// truncado. Devolve as páginas que ficaram por copiar (0 = completo; > 0 se um leitor
    /// ainda segura páginas antigas).
    pub fn checkpoint(&self) -> CoreResult<i64> {
        self.connection()
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                let busy: i64 = r.get(0)?;
                let log: i64 = r.get(1)?;
                let done: i64 = r.get(2)?;
                Ok(if busy != 0 { log.max(1) } else { (log - done).max(0) })
            })
            .map_err(db_err)
    }

    /// Todos os originais (verificação de disponibilidade).
    pub fn originals(&self) -> CoreResult<Vec<Original>> {
        self.originals_query("", [])
    }

    pub fn original(&self, id: AssetId) -> CoreResult<Option<Original>> {
        Ok(self.originals_query("WHERE id = ?1", [id.0])?.pop())
    }

    fn originals_query<P: rusqlite::Params>(
        &self,
        filter: &str,
        p: P,
    ) -> CoreResult<Vec<Original>> {
        let mut st = self
            .connection()
            .prepare(&format!(
                "SELECT id, path, file_name, file_size, content_hash, status, backup_path FROM asset {filter} ORDER BY id"
            ))
            .map_err(db_err)?;
        let rows = st
            .query_map(p, |r| {
                let h: Vec<u8> = r.get(4)?;
                let mut hash = [0u8; 32];
                if h.len() == 32 {
                    hash.copy_from_slice(&h);
                }
                Ok(Original {
                    id: AssetId(r.get(0)?),
                    path: r.get(1)?,
                    file_name: r.get(2)?,
                    file_size: r.get::<_, i64>(3)? as u64,
                    content_hash: hash,
                    status: Availability::parse(&r.get::<_, String>(5)?),
                    backup_path: r.get(6)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Grava a disponibilidade (só as que mudaram). Devolve quantas mudaram.
    pub fn set_availability(&mut self, changes: &[(AssetId, Availability)]) -> CoreResult<usize> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        let mut n = 0;
        {
            let mut st = tx
                .prepare("UPDATE asset SET status = ?1 WHERE id = ?2 AND status != ?1")
                .map_err(db_err)?;
            for (id, a) in changes {
                n += st.execute(params![a.as_str(), id.0]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(n)
    }

    /// Contagens por disponibilidade (ok, offline, em falta).
    pub fn availability_counts(&self) -> CoreResult<(u64, u64, u64)> {
        self.connection()
            .query_row(
                "SELECT sum(status = 'ok'), sum(status = 'offline'), sum(status = 'missing') FROM asset",
                [],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?.unwrap_or(0) as u64,
                        r.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64,
                        r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
                    ))
                },
            )
            .map_err(db_err)
    }

    /// Remove fotografias **só do catálogo** (classificações, edições, análise — por
    /// cascata). Os ficheiros não são tocados. Devolve quantas foram removidas.
    pub fn remove_assets(&mut self, ids: &[AssetId]) -> CoreResult<usize> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        let mut n = 0;
        {
            // Itens de importação apontam para o asset: ficam no histórico, sem ligação.
            let mut unlink = tx
                .prepare("UPDATE import_item SET asset_id = NULL WHERE asset_id = ?1")
                .map_err(db_err)?;
            let mut del = tx.prepare("DELETE FROM asset WHERE id = ?1").map_err(db_err)?;
            for id in ids {
                unlink.execute([id.0]).map_err(db_err)?;
                n += del.execute([id.0]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(n)
    }

    /// Novo caminho para o original de uma fotografia (o conteúdo já foi verificado).
    pub fn relink(&mut self, id: AssetId, path: &str, mtime: Option<i64>) -> CoreResult<()> {
        let taken: Option<i64> = self
            .connection()
            .query_row(
                "SELECT id FROM asset WHERE path = ?1 AND id != ?2",
                params![path, id.0],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        if taken.is_some() {
            return Err(CoreError::Other(
                "esse ficheiro já pertence a outra fotografia do catálogo".into(),
            ));
        }
        let file_name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let n = self
            .connection()
            .execute(
                "UPDATE asset SET path = ?1, file_name = ?2, file_mtime = coalesce(?3, file_mtime), status = 'ok' WHERE id = ?4",
                params![path, file_name, mtime, id.0],
            )
            .map_err(db_err)?;
        if n == 0 {
            return Err(CoreError::Other("fotografia inexistente".into()));
        }
        Ok(())
    }

    /// Caminho da cópia de segurança feita na importação.
    pub fn set_backup_path(&mut self, id: AssetId, path: &str) -> CoreResult<()> {
        self.connection()
            .execute("UPDATE asset SET backup_path = ?1 WHERE id = ?2", params![path, id.0])
            .map_err(db_err)?;
        Ok(())
    }

    /// Registo de segurança (ex.: "sessão anterior interrompida", "catálogo restaurado").
    pub fn log_event(&self, kind: &str, detail: &str) -> CoreResult<()> {
        self.connection()
            .execute(
                "INSERT INTO safety_log (at, kind, detail) VALUES (?1, ?2, ?3)",
                params![now_unix(), kind, detail],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn recent_events(&self, limit: u32) -> CoreResult<Vec<(i64, String, String)>> {
        let mut st = self
            .connection()
            .prepare("SELECT at, kind, detail FROM safety_log ORDER BY id DESC LIMIT ?1")
            .map_err(db_err)?;
        let rows =
            st.query_map([limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::NewAsset;

    fn asset(cat: &mut Catalog, path: &str, h: u8) -> AssetId {
        let a = NewAsset {
            path: path.into(),
            file_name: path.rsplit('/').next().unwrap().into(),
            file_size: 10,
            content_hash: [h; 32],
            format: "jpeg".into(),
            width: 10,
            height: 10,
            ..Default::default()
        };
        match cat.insert_assets(std::slice::from_ref(&a)).unwrap().remove(0) {
            crate::assets::InsertResult::Inserted(id) => id,
            r => panic!("{r:?}"),
        }
    }

    #[test]
    fn availability_remove_and_relink() {
        let mut cat = Catalog::open_in_memory().unwrap();
        let a = asset(&mut cat, "/disco/a.jpg", 1);
        let b = asset(&mut cat, "/disco/b.jpg", 2);
        assert!(cat.validate(true).unwrap().ok());
        assert_eq!(
            cat.set_availability(&[(a, Availability::Offline), (b, Availability::Ok)]).unwrap(),
            1
        );
        assert_eq!(cat.availability_counts().unwrap(), (1, 1, 0));
        assert_eq!(cat.original(a).unwrap().unwrap().status, Availability::Offline);
        // Relink: caminho novo, volta a "ok"; caminho de outra fotografia é recusado.
        cat.relink(a, "/novo/a.jpg", Some(5)).unwrap();
        let o = cat.original(a).unwrap().unwrap();
        assert_eq!((o.path.as_str(), o.status), ("/novo/a.jpg", Availability::Ok));
        assert!(cat.relink(a, "/disco/b.jpg", None).is_err());
        // Remover só do catálogo.
        assert_eq!(cat.remove_assets(&[b]).unwrap(), 1);
        assert_eq!(cat.originals().unwrap().len(), 1);
        assert!(cat.validate(true).unwrap().ok(), "sem órfãos depois da cascata");
        cat.log_event("teste", "x").unwrap();
        assert_eq!(cat.recent_events(5).unwrap()[0].1, "teste");
    }
}
