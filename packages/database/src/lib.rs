//! # momento-database
//!
//! Catálogo SQLite (um ficheiro por projeto). Responsabilidades na Fase 0:
//!
//! - abrir/criar o catálogo com os pragmas corretos (WAL, chaves estrangeiras);
//! - aplicar migrações sequenciais controladas por `PRAGMA user_version`;
//! - recusar abrir um catálogo criado por uma versão mais recente da aplicação.
//!
//! O esquema completo (asset, variant, rating, …) é introduzido por migrações nas fases
//! seguintes — ver ARCHITECTURE.md §4. Nunca se edita uma migração já publicada: cria-se
//! uma nova.

use momento_core_types::{CoreError, CoreResult};
use rusqlite::Connection;
use std::path::Path;

/// Migrações por ordem. O índice + 1 é a versão resultante.
const MIGRATIONS: &[&str] = &[
    // v1 — Fase 0: metadados do próprio catálogo.
    r#"
    CREATE TABLE catalog_info (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) STRICT;
    "#,
];

/// Versão de esquema que esta compilação conhece.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

fn db_err(e: rusqlite::Error) -> CoreError {
    CoreError::Database(e.to_string())
}

pub struct Catalog {
    conn: Connection,
}

impl Catalog {
    /// Abre (ou cria) o catálogo em `path` e aplica migrações pendentes.
    ///
    /// O ficheiro tem de estar num disco local (interno ou externo USB/Thunderbolt). O
    /// modo WAL do SQLite não é seguro em partilhas de rede (SMB/NFS/AFP).
    pub fn open(path: &Path) -> CoreResult<Self> {
        let conn = Connection::open(path).map_err(db_err)?;
        Self::init(conn)
    }

    /// Catálogo em memória, para testes.
    pub fn open_in_memory() -> CoreResult<Self> {
        Self::init(Connection::open_in_memory().map_err(db_err)?)
    }

    fn init(conn: Connection) -> CoreResult<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(db_err)?;
        let mut cat = Catalog { conn };
        cat.migrate()?;
        Ok(cat)
    }

    pub fn schema_version(&self) -> CoreResult<u32> {
        self.conn.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0)).map_err(db_err)
    }

    pub fn journal_mode(&self) -> CoreResult<String> {
        self.conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).map_err(db_err)
    }

    fn migrate(&mut self) -> CoreResult<()> {
        let current = self.schema_version()?;
        if current > SCHEMA_VERSION {
            return Err(CoreError::CatalogTooNew { found: current, supported: SCHEMA_VERSION });
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let target = i as u32 + 1;
            let tx = self.conn.transaction().map_err(db_err)?;
            tx.execute_batch(sql).map_err(db_err)?;
            // user_version não aceita parâmetros; `target` é um inteiro interno.
            tx.execute_batch(&format!("PRAGMA user_version = {target};")).map_err(db_err)?;
            tx.execute(
                "INSERT INTO catalog_info(key, value) VALUES('core_version_last_migration', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [momento_core_types::CORE_VERSION],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }
        Ok(())
    }

    /// Acesso à ligação para os repositórios dos outros módulos (a partir da Fase 1).
    pub fn connection(&self) -> &Connection {
        &self.conn
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
            .query_row("SELECT count(*) FROM catalog_info", [], |r| r.get(0))
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
}
