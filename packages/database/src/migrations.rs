//! Migrações. O índice + 1 é a versão resultante. **Nunca editar uma migração publicada.**

use crate::db_err;
use momento_core_types::{CoreError, CoreResult};
use rusqlite::Connection;

/// Migrações do catálogo de projeto.
pub const CATALOG: &[&str] = &[
    // v1 — Fase 0: metadados do próprio catálogo.
    r#"
    CREATE TABLE catalog_info (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) STRICT;
    "#,
    // v2 — Fase 1: projeto, fotografias, variantes, classificação e importação.
    r#"
    CREATE TABLE project (
        id            INTEGER PRIMARY KEY CHECK (id = 1),
        name          TEXT NOT NULL,
        client_name   TEXT NOT NULL,
        event_type    TEXT NOT NULL,
        event_date    TEXT,
        location      TEXT NOT NULL DEFAULT '',
        originals_dir TEXT,
        status        TEXT NOT NULL DEFAULT 'created',
        notes         TEXT NOT NULL DEFAULT '',
        created_at    INTEGER NOT NULL,
        updated_at    INTEGER NOT NULL
    ) STRICT;

    CREATE TABLE import_session (
        id          INTEGER PRIMARY KEY,
        created_at  INTEGER NOT NULL,
        mode        TEXT NOT NULL CHECK (mode IN ('reference', 'copy')),
        recursive   INTEGER NOT NULL,
        state       TEXT NOT NULL CHECK (state IN ('scanning', 'running', 'finished', 'cancelled')),
        finished_at INTEGER
    ) STRICT;

    CREATE TABLE import_item (
        id          INTEGER PRIMARY KEY,
        session_id  INTEGER NOT NULL REFERENCES import_session(id) ON DELETE CASCADE,
        source_path TEXT NOT NULL,
        state       TEXT NOT NULL CHECK (state IN ('pending', 'imported', 'duplicate', 'already_imported', 'unsupported', 'error')),
        asset_id    INTEGER,
        message     TEXT,
        UNIQUE (session_id, source_path)
    ) STRICT;
    CREATE INDEX import_item_by_state ON import_item(session_id, state);

    CREATE TABLE asset (
        id                INTEGER PRIMARY KEY,
        path              TEXT NOT NULL UNIQUE,
        source_path       TEXT,
        file_name         TEXT NOT NULL,
        file_size         INTEGER NOT NULL,
        file_mtime        INTEGER,
        content_hash      BLOB NOT NULL UNIQUE,
        format            TEXT NOT NULL,
        width             INTEGER NOT NULL,
        height            INTEGER NOT NULL,
        orientation       INTEGER NOT NULL DEFAULT 1,
        capture_time      TEXT,
        capture_offset    TEXT,
        camera_make       TEXT,
        camera_model      TEXT,
        camera_serial     TEXT,
        lens              TEXT,
        iso               INTEGER,
        exposure_time     REAL,
        f_number          REAL,
        focal_length      REAL,
        focal_length_35mm INTEGER,
        import_session_id INTEGER REFERENCES import_session(id) ON DELETE SET NULL,
        imported_at       INTEGER NOT NULL,
        thumb_state       INTEGER NOT NULL DEFAULT 0 CHECK (thumb_state IN (0, 1, 2)),
        thumb_error       TEXT,
        status            TEXT NOT NULL DEFAULT 'ok'
    ) STRICT;
    CREATE INDEX asset_by_capture ON asset(capture_time, file_name);
    CREATE INDEX asset_thumb_pending ON asset(thumb_state) WHERE thumb_state = 0;

    CREATE TABLE variant (
        id         INTEGER PRIMARY KEY,
        asset_id   INTEGER NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
        name       TEXT NOT NULL DEFAULT '',
        is_master  INTEGER NOT NULL DEFAULT 1,
        created_at INTEGER NOT NULL
    ) STRICT;
    CREATE UNIQUE INDEX variant_one_master ON variant(asset_id) WHERE is_master = 1;

    CREATE TABLE rating (
        variant_id INTEGER PRIMARY KEY REFERENCES variant(id) ON DELETE CASCADE,
        stars      INTEGER NOT NULL DEFAULT 0 CHECK (stars BETWEEN 0 AND 5),
        flag       INTEGER NOT NULL DEFAULT 0 CHECK (flag IN (-1, 0, 1)),
        color      TEXT CHECK (color IN ('red', 'yellow', 'green', 'blue', 'purple')),
        updated_at INTEGER NOT NULL
    ) STRICT;
    "#,
];

/// Migrações da biblioteca global (projetos recentes).
pub const LIBRARY: &[&str] = &[
    // v1 — Fase 1
    r#"
    CREATE TABLE catalog_info (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) STRICT;
    CREATE TABLE recent_project (
        path        TEXT PRIMARY KEY NOT NULL,
        name        TEXT NOT NULL,
        client_name TEXT NOT NULL,
        event_type  TEXT NOT NULL,
        event_date  TEXT,
        location    TEXT NOT NULL DEFAULT '',
        photo_count INTEGER NOT NULL DEFAULT 0,
        last_opened INTEGER NOT NULL
    ) STRICT;
    "#,
];

pub fn user_version(conn: &Connection) -> CoreResult<u32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0)).map_err(db_err)
}

/// Aplica as migrações em falta, cada uma numa transação.
pub fn apply(conn: &Connection, list: &[&str]) -> CoreResult<()> {
    let current = user_version(conn)?;
    let supported = list.len() as u32;
    if current > supported {
        return Err(CoreError::CatalogTooNew { found: current, supported });
    }
    for (i, sql) in list.iter().enumerate().skip(current as usize) {
        let target = i as u32 + 1;
        let tx = conn.unchecked_transaction().map_err(db_err)?;
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
