//! # momento-database
//!
//! Catálogo SQLite (um ficheiro por projeto) e biblioteca global (projetos recentes).
//!
//! - abertura com pragmas corretos (WAL, chaves estrangeiras, `busy_timeout`);
//! - migrações sequenciais controladas por `PRAGMA user_version`;
//! - recusa de catálogos criados por versões mais recentes;
//! - repositórios: [`project`], [`assets`], [`import`], [`ui_state`];
//! - [`library`]: base global de projetos recentes.
//!
//! Nunca se edita uma migração já publicada: cria-se uma nova no fim da lista.

pub mod assets;
pub mod import;
pub mod library;
pub mod migrations;
pub mod project;
pub mod ui_state;

use momento_core_types::{CoreError, CoreResult};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use rusqlite;

/// Versão de esquema do catálogo que esta compilação conhece.
pub const SCHEMA_VERSION: u32 = migrations::CATALOG.len() as u32;

pub(crate) fn db_err(e: rusqlite::Error) -> CoreError {
    CoreError::Database(e.to_string())
}

/// Segundos desde 1970 (UTC).
pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Milissegundos desde 1970 (UTC).
pub fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Abre uma ligação com os pragmas comuns e aplica as migrações indicadas.
pub(crate) fn open_with_migrations(path: &Path, list: &[&str]) -> CoreResult<Connection> {
    let conn = Connection::open(path).map_err(db_err)?;
    configure(&conn)?;
    migrations::apply(&conn, list)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> CoreResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 10000;",
    )
    .map_err(db_err)
}

/// Catálogo de um projeto.
pub struct Catalog {
    conn: Connection,
    path: Option<PathBuf>,
}

impl Catalog {
    /// Abre (ou cria) o catálogo em `path` e aplica migrações pendentes.
    ///
    /// O ficheiro tem de estar num disco local (interno ou externo USB/Thunderbolt). O
    /// modo WAL do SQLite não é seguro em partilhas de rede (SMB/NFS/AFP).
    pub fn open(path: &Path) -> CoreResult<Self> {
        let conn = open_with_migrations(path, migrations::CATALOG)?;
        Ok(Catalog { conn, path: Some(path.to_path_buf()) })
    }

    /// Milissegundos desde 1970 (UTC).
    pub fn now_millis() -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    }

    /// Abre uma ligação adicional a um catálogo já migrado (leitores, workers).
    /// Falha se o catálogo não estiver na versão atual.
    pub fn open_existing(path: &Path) -> CoreResult<Self> {
        let conn = Connection::open(path).map_err(db_err)?;
        configure(&conn)?;
        let v = migrations::user_version(&conn)?;
        if v != SCHEMA_VERSION {
            return Err(CoreError::Database(format!(
                "catálogo na versão {v}, esperado {SCHEMA_VERSION}"
            )));
        }
        Ok(Catalog { conn, path: Some(path.to_path_buf()) })
    }

    /// Catálogo em memória, para testes.
    pub fn open_in_memory() -> CoreResult<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        configure(&conn)?;
        migrations::apply(&conn, migrations::CATALOG)?;
        Ok(Catalog { conn, path: None })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn schema_version(&self) -> CoreResult<u32> {
        migrations::user_version(&self.conn)
    }

    pub fn journal_mode(&self) -> CoreResult<String> {
        self.conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).map_err(db_err)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Cópia de segurança consistente do catálogo (API de backup online do SQLite),
    /// sem bloquear a aplicação.
    pub fn backup_to(&self, dest: &Path) -> CoreResult<()> {
        let mut dst = Connection::open(dest).map_err(db_err)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dst).map_err(db_err)?;
        backup.run_to_completion(256, std::time::Duration::from_millis(0), None).map_err(db_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_catalog_is_migrated_and_uses_wal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        let cat = Catalog::open(&path).unwrap();
        assert_eq!(cat.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(cat.journal_mode().unwrap(), "wal");
    }

    #[test]
    fn reopening_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        drop(Catalog::open(&path).unwrap());
        let cat = Catalog::open(&path).unwrap();
        assert_eq!(cat.schema_version().unwrap(), SCHEMA_VERSION);
        let n: i64 = cat
            .connection()
            .query_row(
                "SELECT count(*) FROM catalog_info WHERE key='core_version_last_migration'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn refuses_catalog_from_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch("PRAGMA user_version = 999;").unwrap();
        }
        match Catalog::open(&path) {
            Err(CoreError::CatalogTooNew { found: 999, .. }) => {}
            Err(e) => panic!("erro inesperado: {e}"),
            Ok(_) => panic!("devia recusar catálogo mais recente"),
        }
    }

    #[test]
    fn migrates_v1_catalog_to_current() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        {
            let c = Connection::open(&path).unwrap();
            migrations::apply(&c, &migrations::CATALOG[..1]).unwrap();
            assert_eq!(migrations::user_version(&c).unwrap(), 1);
        }
        let cat = Catalog::open(&path).unwrap();
        assert_eq!(cat.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn backup_produces_readable_copy() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("catalog.db")).unwrap();
        let dest = dir.path().join("backup.db");
        cat.backup_to(&dest).unwrap();
        let copy = Catalog::open_existing(&dest).unwrap();
        assert_eq!(copy.schema_version().unwrap(), SCHEMA_VERSION);
    }
}
