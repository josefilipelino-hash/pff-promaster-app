//! Sessões e itens de importação. Cada ficheiro tem um estado persistido, o que permite
//! retomar uma importação interrompida (fecho da aplicação, falha, falta de energia).

use crate::{db_err, now_unix, Catalog};
use momento_core_types::{AssetId, CoreResult, ImportSessionId};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportMode {
    /// Os originais ficam onde estão; o catálogo guarda o caminho.
    Reference,
    /// Os originais são copiados para a pasta dos originais do projeto, com verificação.
    Copy,
}

impl ImportMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ImportMode::Reference => "reference",
            ImportMode::Copy => "copy",
        }
    }
    pub fn parse(s: &str) -> Self {
        if s == "copy" {
            ImportMode::Copy
        } else {
            ImportMode::Reference
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemState {
    Pending,
    Imported,
    Duplicate,
    AlreadyImported,
    /// Formato desconhecido ou que não é uma fotografia.
    Unsupported,
    /// Formato reconhecido, mas sem descodificador nesta plataforma/compilação.
    MissingCodec,
    /// Formato reconhecido, mas os dados estão danificados ou incompletos.
    Corrupt,
    /// Erro de leitura/escrita (permissões, disco, …).
    Error,
}

impl ItemState {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemState::Pending => "pending",
            ItemState::Imported => "imported",
            ItemState::Duplicate => "duplicate",
            ItemState::AlreadyImported => "already_imported",
            ItemState::Unsupported => "unsupported",
            ItemState::MissingCodec => "missing_codec",
            ItemState::Corrupt => "corrupt",
            ItemState::Error => "error",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "imported" => ItemState::Imported,
            "duplicate" => ItemState::Duplicate,
            "already_imported" => ItemState::AlreadyImported,
            "unsupported" => ItemState::Unsupported,
            "missing_codec" => ItemState::MissingCodec,
            "corrupt" => ItemState::Corrupt,
            "error" => ItemState::Error,
            _ => ItemState::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: ImportSessionId,
    pub mode: ImportMode,
    pub recursive: bool,
    pub state: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCounts {
    pub total: u64,
    pub pending: u64,
    pub imported: u64,
    pub duplicate: u64,
    pub already_imported: u64,
    pub unsupported: u64,
    pub missing_codec: u64,
    pub corrupt: u64,
    pub error: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemIssue {
    pub source_path: String,
    pub state: ItemState,
    pub message: Option<String>,
}

/// Resultado de um item processado, para gravação em lote.
#[derive(Debug, Clone)]
pub struct ItemOutcome {
    pub source_path: String,
    pub state: ItemState,
    pub asset_id: Option<AssetId>,
    pub message: Option<String>,
}

impl Catalog {
    pub fn create_import_session(
        &self,
        mode: ImportMode,
        recursive: bool,
    ) -> CoreResult<ImportSessionId> {
        self.connection()
            .execute(
                "INSERT INTO import_session(created_at, mode, recursive, state)
                 VALUES(?1, ?2, ?3, 'scanning')",
                params![now_unix(), mode.as_str(), recursive],
            )
            .map_err(db_err)?;
        Ok(ImportSessionId(self.connection().last_insert_rowid()))
    }

    /// Regista os ficheiros encontrados (estado `pending`) e passa a sessão a `running`.
    pub fn add_import_items(
        &mut self,
        session: ImportSessionId,
        paths: &[String],
    ) -> CoreResult<()> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        {
            let mut st = tx
                .prepare(
                    "INSERT OR IGNORE INTO import_item(session_id, source_path, state)
                     VALUES(?1, ?2, 'pending')",
                )
                .map_err(db_err)?;
            for p in paths {
                st.execute(params![session.0, p]).map_err(db_err)?;
            }
        }
        tx.execute("UPDATE import_session SET state = 'running' WHERE id = ?1", [session.0])
            .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    pub fn pending_import_items(&self, session: ImportSessionId) -> CoreResult<Vec<String>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT source_path FROM import_item WHERE session_id = ?1 AND state = 'pending'
                 ORDER BY id",
            )
            .map_err(db_err)?;
        let rows = st.query_map([session.0], |r| r.get(0)).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn record_import_outcomes(
        &mut self,
        session: ImportSessionId,
        outcomes: &[ItemOutcome],
    ) -> CoreResult<()> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        {
            let mut st = tx
                .prepare(
                    "UPDATE import_item SET state = ?1, asset_id = ?2, message = ?3
                     WHERE session_id = ?4 AND source_path = ?5",
                )
                .map_err(db_err)?;
            for o in outcomes {
                st.execute(params![
                    o.state.as_str(),
                    o.asset_id.map(|a| a.0),
                    o.message,
                    session.0,
                    o.source_path
                ])
                .map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    pub fn finish_import_session(
        &self,
        session: ImportSessionId,
        cancelled: bool,
    ) -> CoreResult<()> {
        self.connection()
            .execute(
                "UPDATE import_session SET state = ?1, finished_at = ?2 WHERE id = ?3",
                params![if cancelled { "cancelled" } else { "finished" }, now_unix(), session.0],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Sessão interrompida (estado `scanning` ou `running`) a retomar no arranque.
    pub fn unfinished_import_session(&self) -> CoreResult<Option<SessionInfo>> {
        self.connection()
            .query_row(
                "SELECT id, mode, recursive, state FROM import_session
                 WHERE state IN ('scanning', 'running') ORDER BY id DESC LIMIT 1",
                [],
                |r| {
                    Ok(SessionInfo {
                        id: ImportSessionId(r.get(0)?),
                        mode: ImportMode::parse(&r.get::<_, String>(1)?),
                        recursive: r.get(2)?,
                        state: r.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(db_err)
    }

    pub fn import_session_counts(&self, session: ImportSessionId) -> CoreResult<SessionCounts> {
        let mut c = SessionCounts::default();
        let mut st = self
            .connection()
            .prepare("SELECT state, count(*) FROM import_item WHERE session_id = ?1 GROUP BY state")
            .map_err(db_err)?;
        let rows = st
            .query_map([session.0], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64)))
            .map_err(db_err)?;
        for row in rows {
            let (s, n) = row.map_err(db_err)?;
            c.total += n;
            match ItemState::parse(&s) {
                ItemState::Pending => c.pending += n,
                ItemState::Imported => c.imported += n,
                ItemState::Duplicate => c.duplicate += n,
                ItemState::AlreadyImported => c.already_imported += n,
                ItemState::Unsupported => c.unsupported += n,
                ItemState::MissingCodec => c.missing_codec += n,
                ItemState::Corrupt => c.corrupt += n,
                ItemState::Error => c.error += n,
            }
        }
        Ok(c)
    }

    /// Itens que não foram importados (duplicados, não suportados, erros), para o relatório.
    pub fn import_session_issues(
        &self,
        session: ImportSessionId,
        limit: u32,
    ) -> CoreResult<Vec<ItemIssue>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT source_path, state, message FROM import_item
                 WHERE session_id = ?1 AND state NOT IN ('imported', 'pending')
                 ORDER BY CASE state WHEN 'corrupt' THEN 0 WHEN 'error' THEN 1 WHEN 'missing_codec' THEN 2 WHEN 'unsupported' THEN 3 ELSE 4 END, id
                 LIMIT ?2",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map(params![session.0, limit], |r| {
                Ok(ItemIssue {
                    source_path: r.get(0)?,
                    state: ItemState::parse(&r.get::<_, String>(1)?),
                    message: r.get(2)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lifecycle_and_resume() {
        let mut c = Catalog::open_in_memory().unwrap();
        let s = c.create_import_session(ImportMode::Reference, true).unwrap();
        c.add_import_items(
            s,
            &["/a.jpg".into(), "/b.jpg".into(), "/c.txt".into(), "/a.jpg".into()],
        )
        .unwrap();
        assert_eq!(c.pending_import_items(s).unwrap().len(), 3, "caminhos repetidos ignorados");
        c.record_import_outcomes(
            s,
            &[ItemOutcome {
                source_path: "/c.txt".into(),
                state: ItemState::Unsupported,
                asset_id: None,
                message: Some("formato".into()),
            }],
        )
        .unwrap();
        // Simula fecho da aplicação: a sessão continua por terminar.
        let u = c.unfinished_import_session().unwrap().unwrap();
        assert_eq!(u.id, s);
        assert_eq!(c.pending_import_items(s).unwrap(), vec!["/a.jpg", "/b.jpg"]);
        let counts = c.import_session_counts(s).unwrap();
        assert_eq!((counts.total, counts.pending, counts.unsupported), (3, 2, 1));
        assert_eq!(c.import_session_issues(s, 10).unwrap().len(), 1);
        c.finish_import_session(s, false).unwrap();
        assert!(c.unfinished_import_session().unwrap().is_none());
    }
}
