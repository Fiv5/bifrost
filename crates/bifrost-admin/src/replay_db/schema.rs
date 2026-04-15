use rusqlite::Connection;

pub const SCHEMA_VERSION: u32 = 3;

#[derive(Debug)]
pub enum InitError {
    Sqlite(rusqlite::Error),
}

impl From<rusqlite::Error> for InitError {
    fn from(e: rusqlite::Error) -> Self {
        InitError::Sqlite(e)
    }
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::Sqlite(e) => write!(f, "SQLite error: {}", e),
        }
    }
}

impl std::error::Error for InitError {}

pub fn init_database(conn: &Connection) -> Result<(), InitError> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = 1000;
        PRAGMA temp_store = MEMORY;
        PRAGMA mmap_size = 134217728;
        PRAGMA foreign_keys = ON;
        ",
    )?;

    conn.execute_batch(SCHEMA_SQL)?;

    run_migrations(conn)?;

    conn.execute(
        "INSERT OR REPLACE INTO replay_metadata (key, value) VALUES ('schema_version', ?)",
        [SCHEMA_VERSION.to_string()],
    )?;

    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<(), InitError> {
    let current_version: u32 = conn
        .query_row(
            "SELECT value FROM replay_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if current_version < 2 {
        let has_request_type: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('replay_requests') WHERE name = 'request_type'",
                [],
                |row| row.get::<_, i32>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        if !has_request_type {
            conn.execute(
                "ALTER TABLE replay_requests ADD COLUMN request_type TEXT NOT NULL DEFAULT 'http'",
                [],
            )?;
        }
    }

    if current_version < 3 {
        let has_source: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('replay_requests') WHERE name = 'source'",
                [],
                |row| row.get::<_, i32>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        if !has_source {
            conn.execute(
                "ALTER TABLE replay_requests ADD COLUMN source TEXT NOT NULL DEFAULT 'internal'",
                [],
            )?;
        }
    }

    Ok(())
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS replay_groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    parent_id TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (parent_id) REFERENCES replay_groups(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_replay_groups_parent ON replay_groups(parent_id);
CREATE INDEX IF NOT EXISTS idx_replay_groups_sort ON replay_groups(sort_order);

CREATE TABLE IF NOT EXISTS replay_requests (
    id TEXT PRIMARY KEY NOT NULL,
    group_id TEXT,
    name TEXT,
    request_type TEXT NOT NULL DEFAULT 'http',
    method TEXT NOT NULL,
    url TEXT NOT NULL,
    headers_blob BLOB,
    body_blob BLOB,
    is_saved INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'internal',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (group_id) REFERENCES replay_groups(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_replay_requests_group ON replay_requests(group_id);
CREATE INDEX IF NOT EXISTS idx_replay_requests_saved ON replay_requests(is_saved, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_replay_requests_updated ON replay_requests(updated_at DESC);

CREATE TABLE IF NOT EXISTS replay_history (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT,
    traffic_id TEXT NOT NULL,
    method TEXT NOT NULL,
    url TEXT NOT NULL,
    status INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    executed_at INTEGER NOT NULL,
    rule_config_blob BLOB,
    FOREIGN KEY (request_id) REFERENCES replay_requests(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_replay_history_executed ON replay_history(executed_at DESC);
CREATE INDEX IF NOT EXISTS idx_replay_history_request ON replay_history(request_id);

CREATE TABLE IF NOT EXISTS replay_metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
"#;

pub fn get_insert_request_sql() -> &'static str {
    r#"
    INSERT INTO replay_requests (
        id, group_id, name, request_type, method, url,
        headers_blob, body_blob, is_saved, sort_order,
        source, created_at, updated_at
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
    "#
}

pub fn get_update_request_sql() -> &'static str {
    r#"
    UPDATE replay_requests SET
        group_id = ?1,
        name = ?2,
        request_type = ?3,
        method = ?4,
        url = ?5,
        headers_blob = ?6,
        body_blob = ?7,
        is_saved = ?8,
        sort_order = ?9,
        source = ?10,
        updated_at = ?11
    WHERE id = ?12
    "#
}

pub fn get_insert_group_sql() -> &'static str {
    r#"
    INSERT INTO replay_groups (
        id, name, parent_id, sort_order, created_at, updated_at
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
    "#
}

pub fn get_update_group_sql() -> &'static str {
    r#"
    UPDATE replay_groups SET
        name = ?1,
        parent_id = ?2,
        sort_order = ?3,
        updated_at = ?4
    WHERE id = ?5
    "#
}

pub fn get_insert_history_sql() -> &'static str {
    r#"
    INSERT INTO replay_history (
        id, request_id, traffic_id, method, url,
        status, duration_ms, executed_at, rule_config_blob
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
    "#
}
