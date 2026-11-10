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
    // v3 — Fase 2: multiformato (RAW, HEIC, …), nível de suporte e novos estados.
    r#"
    ALTER TABLE asset ADD COLUMN bit_depth INTEGER;
    ALTER TABLE asset ADD COLUMN color_space TEXT;
    ALTER TABLE asset ADD COLUMN codec TEXT;
    ALTER TABLE asset ADD COLUMN codec_detail TEXT;
    ALTER TABLE asset ADD COLUMN embedded_preview TEXT;
    ALTER TABLE asset ADD COLUMN support TEXT NOT NULL DEFAULT 'full' CHECK (support IN ('full', 'partial'));
    ALTER TABLE asset ADD COLUMN support_note TEXT;

    CREATE TABLE import_item_v3 (
        id          INTEGER PRIMARY KEY,
        session_id  INTEGER NOT NULL REFERENCES import_session(id) ON DELETE CASCADE,
        source_path TEXT NOT NULL,
        state       TEXT NOT NULL CHECK (state IN ('pending', 'imported', 'duplicate', 'already_imported',
                                                   'unsupported', 'missing_codec', 'corrupt', 'error')),
        asset_id    INTEGER,
        message     TEXT,
        UNIQUE (session_id, source_path)
    ) STRICT;
    INSERT INTO import_item_v3(id, session_id, source_path, state, asset_id, message)
        SELECT id, session_id, source_path, state, asset_id, message FROM import_item;
    DROP TABLE import_item;
    ALTER TABLE import_item_v3 RENAME TO import_item;
    CREATE INDEX import_item_by_state ON import_item(session_id, state);
    "#,
    // v4 — Fase 3: análise, rostos, embeddings, grupos e recomendações.
    r#"
    ALTER TABLE asset ADD COLUMN analysis_state INTEGER NOT NULL DEFAULT 0 CHECK (analysis_state IN (0, 1, 2));
    ALTER TABLE asset ADD COLUMN analysis_error TEXT;

    CREATE TABLE analysis (
        asset_id           INTEGER PRIMARY KEY REFERENCES asset(id) ON DELETE CASCADE,
        version            INTEGER NOT NULL,
        analyzed_at        INTEGER NOT NULL,
        models             TEXT,
        width              INTEGER NOT NULL,
        height             INTEGER NOT NULL,
        sharpness          REAL NOT NULL,
        focus_peak         REAL NOT NULL,
        focus_center       REAL NOT NULL,
        motion_blur        REAL NOT NULL,
        brightness         REAL NOT NULL,
        highlights_clipped REAL NOT NULL,
        shadows_crushed    REAL NOT NULL,
        contrast           REAL NOT NULL,
        noise              REAL NOT NULL,
        tilt_degrees       REAL,
        phash              INTEGER NOT NULL,
        dhash              INTEGER NOT NULL,
        face_count         INTEGER NOT NULL,
        duration_ms        INTEGER NOT NULL
    ) STRICT;

    CREATE TABLE face (
        id            INTEGER PRIMARY KEY,
        asset_id      INTEGER NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
        x             REAL NOT NULL,
        y             REAL NOT NULL,
        w             REAL NOT NULL,
        h             REAL NOT NULL,
        px            REAL NOT NULL,
        det_score     REAL NOT NULL,
        presence      REAL NOT NULL,
        blink_left    REAL NOT NULL,
        blink_right   REAL NOT NULL,
        smile         REAL NOT NULL,
        jaw_open      REAL NOT NULL,
        sharpness     REAL NOT NULL,
        eye_sharpness REAL NOT NULL,
        cut           INTEGER NOT NULL,
        blendshapes   BLOB
    ) STRICT;
    CREATE INDEX face_by_asset ON face(asset_id);

    CREATE TABLE embedding (
        asset_id INTEGER NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
        model    TEXT NOT NULL,
        vector   BLOB NOT NULL,
        PRIMARY KEY (asset_id, model)
    ) STRICT;

    CREATE TABLE photo_group (
        id   INTEGER PRIMARY KEY,
        kind TEXT NOT NULL CHECK (kind IN ('sequence', 'duplicate', 'raw_jpeg')),
        size INTEGER NOT NULL
    ) STRICT;

    CREATE TABLE recommendation (
        asset_id    INTEGER PRIMARY KEY REFERENCES asset(id) ON DELETE CASCADE,
        label       TEXT NOT NULL CHECK (label IN ('best', 'alternative', 'review', 'reject', 'duplicate')),
        group_id    INTEGER REFERENCES photo_group(id) ON DELETE SET NULL,
        rank        INTEGER NOT NULL,
        group_size  INTEGER NOT NULL,
        summary     TEXT NOT NULL,
        reasons     TEXT NOT NULL,
        focus       REAL NOT NULL,
        motion      REAL NOT NULL,
        exposure    REAL NOT NULL,
        noise       REAL NOT NULL,
        eyes        REAL,
        expression  REAL,
        composition REAL NOT NULL,
        technical   REAL NOT NULL,
        people      REAL,
        overall     REAL NOT NULL,
        computed_at INTEGER NOT NULL
    ) STRICT;
    CREATE INDEX recommendation_by_group ON recommendation(group_id, rank);
    "#,
    // v5 — Fase 4: GPS, relógios das câmaras, pessoas, identidade facial, cena e momentos.
    r#"
    ALTER TABLE asset ADD COLUMN gps_lat REAL;
    ALTER TABLE asset ADD COLUMN gps_lon REAL;

    -- Correção de hora por câmara (marca + modelo + n.º de série). offset_ms soma-se à hora EXIF.
    CREATE TABLE camera_clock (
        camera_key TEXT PRIMARY KEY,
        offset_ms  INTEGER NOT NULL DEFAULT 0,
        reference  INTEGER NOT NULL DEFAULT 0 CHECK (reference IN (0, 1))
    ) STRICT;

    CREATE TABLE person (
        id         INTEGER PRIMARY KEY,
        name       TEXT,
        role       TEXT,
        hidden     INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1)),
        created_at INTEGER NOT NULL
    ) STRICT;

    -- identity: SFace 128-D; person_source: 0 automático, 1 confirmado pelo utilizador,
    -- 2 excluído pelo utilizador (“não é esta pessoa”: nunca volta a ser atribuído).
    ALTER TABLE face ADD COLUMN identity BLOB;
    ALTER TABLE face ADD COLUMN person_id INTEGER REFERENCES person(id) ON DELETE SET NULL;
    ALTER TABLE face ADD COLUMN person_source INTEGER NOT NULL DEFAULT 0;
    CREATE INDEX face_by_person ON face(person_id);

    -- Classes ImageNet mais prováveis (u16 índice + f32 probabilidade, 25 pares).
    CREATE TABLE scene (
        asset_id INTEGER PRIMARY KEY REFERENCES asset(id) ON DELETE CASCADE,
        top      BLOB NOT NULL
    ) STRICT;

    CREATE TABLE moment (
        id     INTEGER PRIMARY KEY,
        kind   TEXT UNIQUE,
        name   TEXT,
        sort   INTEGER NOT NULL,
        custom INTEGER NOT NULL DEFAULT 0 CHECK (custom IN (0, 1))
    ) STRICT;

    CREATE TABLE asset_moment (
        asset_id   INTEGER PRIMARY KEY REFERENCES asset(id) ON DELETE CASCADE,
        moment_id  INTEGER REFERENCES moment(id) ON DELETE CASCADE,
        confidence REAL,
        source     TEXT NOT NULL CHECK (source IN ('auto', 'user')),
        reasons    TEXT
    ) STRICT;
    CREATE INDEX asset_moment_by_moment ON asset_moment(moment_id);

    -- A análise ganha identidade facial e cena: as fotografias já analisadas ficam por
    -- reanalisar (as recomendações atuais mantêm-se até lá).
        UPDATE asset SET analysis_state = 0 WHERE analysis_state = 1;
    "#,
    // v6 — Fase 5: edição não destrutiva (receita por variante, histórico, instantâneos).
    r#"
    CREATE TABLE edit (
        variant_id INTEGER PRIMARY KEY REFERENCES variant(id) ON DELETE CASCADE,
        recipe     TEXT NOT NULL,
        position   INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL
    ) STRICT;

    CREATE TABLE edit_history (
        id         INTEGER PRIMARY KEY,
        variant_id INTEGER NOT NULL REFERENCES variant(id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        label      TEXT NOT NULL,
        recipe     TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        UNIQUE (variant_id, seq)
    ) STRICT;

    CREATE TABLE snapshot (
        id         INTEGER PRIMARY KEY,
        variant_id INTEGER NOT NULL REFERENCES variant(id) ON DELETE CASCADE,
        name       TEXT NOT NULL,
        recipe     TEXT NOT NULL,
        created_at INTEGER NOT NULL
    ) STRICT;
    "#,
    // v7 — Fase 7: características de luz por fotografia (consistência visual).
    r#"
    CREATE TABLE look_feature (
        asset_id    INTEGER PRIMARY KEY REFERENCES asset(id) ON DELETE CASCADE,
        ev          REAL,
        warmth      REAL NOT NULL,
        tint        REAL NOT NULL,
        key         REAL NOT NULL,
        flash       INTEGER,
        computed_at INTEGER NOT NULL
    ) STRICT;
    "#,
    // v8 — Fase 12: índices medidos com 15 000 fotografias (EXPLAIN QUERY PLAN): o índice
    // parcial de `variant` só serve a versão principal — contar cópias virtuais e apagar
    // fotografias (cascata) percorriam a tabela toda (0,41 → 0,004 ms; 300 apagadas:
    // 257 → 6 ms). `snapshot` também é apagada em cascata a partir da variante.
    r#"
    CREATE INDEX variant_by_asset ON variant(asset_id);
    CREATE INDEX snapshot_by_variant ON snapshot(variant_id);
    "#,
    // v9 — Fase 13: segurança. Cópia de segurança feita na importação, disponibilidade
    // dos originais (`status`: ok / offline / missing — a coluna já existia) e registo de
    // eventos de segurança (sessão interrompida, restauros, validações).
    r#"
    ALTER TABLE asset ADD COLUMN backup_path TEXT;
    CREATE INDEX asset_unavailable ON asset(status) WHERE status != 'ok';
    CREATE TABLE safety_log (
        id     INTEGER PRIMARY KEY,
        at     INTEGER NOT NULL,
        kind   TEXT NOT NULL,
        detail TEXT NOT NULL
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
    // v2 — Fase 7: presets do utilizador (os internos estão no código).
    r#"
    CREATE TABLE preset (
        id         TEXT PRIMARY KEY NOT NULL,
        name       TEXT NOT NULL,
        group_name TEXT NOT NULL DEFAULT '',
        position   INTEGER NOT NULL DEFAULT 0,
        body       TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    ) STRICT;
    "#,
    // v3 — Fase 9: receitas e planos de exportação do utilizador.
    r#"
    CREATE TABLE export_preset (
        id         TEXT PRIMARY KEY NOT NULL,
        kind       TEXT NOT NULL CHECK (kind IN ('recipe', 'plan')),
        name       TEXT NOT NULL,
        position   INTEGER NOT NULL DEFAULT 0,
        body       TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    ) STRICT;
    "#,
    // v4 — Fase 11: presets de laboratório dos álbuns.
    r#"
    CREATE TABLE album_lab (
        id         TEXT PRIMARY KEY NOT NULL,
        name       TEXT NOT NULL,
        body       TEXT NOT NULL,
        updated_at INTEGER NOT NULL
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
