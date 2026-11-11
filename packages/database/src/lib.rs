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

pub mod analysis;
pub mod assets;
pub mod clock;
pub mod consistency;
pub mod edits;
pub mod import;
pub mod library;
pub mod migrations;
pub mod moments;
pub mod people;
pub mod project;
pub mod safety;
pub mod ui_state;

use momento_core_types::{CoreError, CoreResult};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use rusqlite;

/// Versão de esquema do catálogo que esta compilação conhece.
/// Chave de câmara (marca + modelo + n.º de série) de um `asset` com o alias `a`.
pub const CAMERA_KEY_SQL: &str = "trim(coalesce(a.camera_make, '') || ' ' || coalesce(a.camera_model, '') || ' ' || coalesce(a.camera_serial, ''))";
/// Junção com a correção de hora da câmara (alias `cc`).
pub const CLOCK_JOIN_SQL: &str = "LEFT JOIN camera_clock cc ON cc.camera_key = trim(coalesce(a.camera_make, '') || ' ' || coalesce(a.camera_model, '') || ' ' || coalesce(a.camera_serial, ''))";
/// Hora de captura corrigida (texto ISO) — igual à original quando não há correção.
pub const CORRECTED_TIME_SQL: &str = "CASE WHEN a.capture_time IS NULL OR coalesce(cc.offset_ms, 0) = 0 THEN a.capture_time ELSE strftime('%Y-%m-%dT%H:%M:%f', julianday(a.capture_time) + cc.offset_ms / 86400000.0) END";
/// Ordem cronológica corrigida.
pub const CORRECTED_ORDER_SQL: &str = "a.capture_time IS NULL, julianday(a.capture_time) + coalesce(cc.offset_ms, 0) / 86400000.0, a.file_name, a.id";

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
    let current = migrations::user_version(&conn)?;
    if current > 0 && (current as usize) < list.len() {
        // Atualização da aplicação (Fase 16): antes de mudar o esquema de um catálogo
        // existente, uma cópia completa fica em `backups/` ao lado dele. Sem cópia, não
        // se migra — o catálogo fica intacto e o erro é mostrado.
        let copy = backup_before_migration(&conn, path, current).map_err(|e| {
            CoreError::Other(format!(
                "não foi possível guardar uma cópia do catálogo antes de o atualizar ({e}); nada foi alterado"
            ))
        })?;
        tracing::info!(de = current, para = list.len(), copia = %copy.display(), "catálogo atualizado; cópia anterior guardada");
    }
    migrations::apply(&conn, list)?;
    Ok(conn)
}

/// Copia o catálogo (instantâneo consistente, inclui o WAL) para
/// `backups/<nome>-antes-da-atualizacao-v<versão>-<segundos>.db`.
fn backup_before_migration(conn: &Connection, path: &Path, version: u32) -> CoreResult<PathBuf> {
    let dir = path.parent().unwrap_or(Path::new(".")).join("backups");
    std::fs::create_dir_all(&dir)?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("catalog");
    let dest = dir.join(format!("{stem}-antes-da-atualizacao-v{version}-{}.db", now_unix()));
    let tmp = dest.with_extension("db.tmp");
    {
        let mut out = Connection::open(&tmp).map_err(db_err)?;
        let b = rusqlite::backup::Backup::new(conn, &mut out).map_err(db_err)?;
        b.run_to_completion(256, std::time::Duration::from_millis(0), None).map_err(db_err)?;
        drop(b);
        // Ficheiro único e autónomo (sem -wal/-shm ao lado quando for aberto).
        out.execute_batch("PRAGMA journal_mode = DELETE;").map_err(db_err)?;
    }
    // Com escrita: no Windows, forçar para o disco (FlushFileBuffers) num ficheiro aberto só
    // para leitura dá "Access is denied" — a migração era recusada.
    std::fs::OpenOptions::new().write(true).open(&tmp)?.sync_all()?;
    std::fs::rename(&tmp, &dest)?;
    Ok(dest)
}

fn configure(conn: &Connection) -> CoreResult<()> {
    conn.execute_batch(
        // FULL (Fase 13): cada transação confirmada está no disco antes de a função
        // voltar — um corte de energia não perde a última classificação/edição. Com WAL,
        // NORMAL nunca corrompia, mas podia perder as últimas transações.
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;
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

    /// Fase 16: um catálogo de uma versão anterior é copiado para `backups/` antes da
    /// migração; a cópia abre e mantém o esquema e os dados antigos.
    #[test]
    fn older_catalog_is_backed_up_before_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        let old = (migrations::CATALOG.len() - 2) as u32;
        {
            let c = Connection::open(&path).unwrap();
            configure(&c).unwrap();
            migrations::apply(&c, &migrations::CATALOG[..old as usize]).unwrap();
            c.execute("INSERT INTO catalog_info(key, value) VALUES('marca', 'antes')", []).unwrap();
        }
        let cat = Catalog::open(&path).unwrap();
        assert_eq!(cat.schema_version().unwrap(), SCHEMA_VERSION);
        let copies: Vec<_> = std::fs::read_dir(dir.path().join("backups"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(copies.len(), 1, "{copies:#?}");
        let name = copies[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with(&format!("catalog-antes-da-atualizacao-v{old}-"))
                && name.ends_with(".db"),
            "{name}"
        );
        let c = Connection::open(&copies[0]).unwrap();
        assert_eq!(migrations::user_version(&c).unwrap(), old, "a cópia tem o esquema anterior");
        let v: String = c
            .query_row("SELECT value FROM catalog_info WHERE key = 'marca'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "antes");
        // Abrir de novo (já atualizado) não faz mais cópias.
        drop(cat);
        Catalog::open(&path).unwrap();
        assert_eq!(std::fs::read_dir(dir.path().join("backups")).unwrap().count(), 1);
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
    fn migration_v2_to_v3_preserves_import_items_and_assets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        {
            let c = Connection::open(&path).unwrap();
            migrations::apply(&c, &migrations::CATALOG[..2]).unwrap();
            c.execute_batch(
                "INSERT INTO import_session(id, created_at, mode, recursive, state) VALUES(1, 0, 'reference', 1, 'running');
                 INSERT INTO import_item(session_id, source_path, state) VALUES(1, '/a.jpg', 'pending'), (1, '/b.txt', 'unsupported');
                 INSERT INTO asset(path, file_name, file_size, content_hash, format, width, height, imported_at)
                 VALUES('/c.jpg', 'c.jpg', 1, x'00', 'jpeg', 10, 10, 0);",
            )
            .unwrap();
        }
        let c = Catalog::open(&path).unwrap();
        assert_eq!(c.schema_version().unwrap(), SCHEMA_VERSION);
        let pending = c.pending_import_items(momento_core_types::ImportSessionId(1)).unwrap();
        assert_eq!(pending, vec!["/a.jpg"]);
        let support: String =
            c.connection().query_row("SELECT support FROM asset", [], |r| r.get(0)).unwrap();
        assert_eq!(support, "full", "fotografias antigas ficam com suporte completo");
        // Novos estados aceites pela restrição CHECK.
        c.connection()
            .execute(
                "UPDATE import_item SET state = 'missing_codec' WHERE source_path = '/a.jpg'",
                [],
            )
            .unwrap();
        c.connection()
            .execute("UPDATE import_item SET state = 'corrupt' WHERE source_path = '/b.txt'", [])
            .unwrap();
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
