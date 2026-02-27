use rusqlite::Connection;

pub const SCHEMA_VERSION: u32 = 1;

pub fn init_database(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = 10000;
        PRAGMA temp_store = MEMORY;
        PRAGMA mmap_size = 268435456;
        ",
    )?;

    conn.execute_batch(SCHEMA_SQL)?;

    conn.execute(
        "INSERT OR IGNORE INTO metadata (key, value) VALUES ('schema_version', ?)",
        [SCHEMA_VERSION.to_string()],
    )?;

    Ok(())
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS traffic_records (
    sequence INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    timestamp INTEGER NOT NULL,
    host TEXT NOT NULL,
    method TEXT NOT NULL,
    status INTEGER NOT NULL DEFAULT 0,
    protocol TEXT NOT NULL,
    url TEXT NOT NULL,
    path TEXT NOT NULL,
    content_type TEXT,
    request_content_type TEXT,
    request_size INTEGER NOT NULL DEFAULT 0,
    response_size INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    client_ip TEXT NOT NULL DEFAULT '',
    client_app TEXT,
    client_pid INTEGER,
    client_path TEXT,
    flags INTEGER NOT NULL DEFAULT 0,
    frame_count INTEGER NOT NULL DEFAULT 0,
    last_frame_id INTEGER NOT NULL DEFAULT 0,
    timing_blob BLOB,
    request_headers_blob BLOB,
    response_headers_blob BLOB,
    matched_rules_blob BLOB,
    socket_status_blob BLOB,
    request_body_ref_blob BLOB,
    response_body_ref_blob BLOB
);

CREATE INDEX IF NOT EXISTS idx_id ON traffic_records(id);
CREATE INDEX IF NOT EXISTS idx_timestamp ON traffic_records(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_host ON traffic_records(host);
CREATE INDEX IF NOT EXISTS idx_status ON traffic_records(status) WHERE status > 0;
CREATE INDEX IF NOT EXISTS idx_method ON traffic_records(method);
CREATE INDEX IF NOT EXISTS idx_client_app ON traffic_records(client_app) WHERE client_app IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_flags ON traffic_records(flags);
CREATE INDEX IF NOT EXISTS idx_seq_desc ON traffic_records(sequence DESC);
CREATE INDEX IF NOT EXISTS idx_host_seq ON traffic_records(host, sequence DESC);
CREATE INDEX IF NOT EXISTS idx_status_seq ON traffic_records(status, sequence DESC);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
"#;

pub fn get_insert_sql() -> &'static str {
    r#"
    INSERT INTO traffic_records (
        sequence, id, timestamp, host, method, status, protocol,
        url, path, content_type, request_content_type,
        request_size, response_size, duration_ms,
        client_ip, client_app, client_pid, client_path,
        flags, frame_count, last_frame_id,
        timing_blob, request_headers_blob, response_headers_blob,
        matched_rules_blob, socket_status_blob,
        request_body_ref_blob, response_body_ref_blob
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7,
        ?8, ?9, ?10, ?11,
        ?12, ?13, ?14,
        ?15, ?16, ?17, ?18,
        ?19, ?20, ?21,
        ?22, ?23, ?24,
        ?25, ?26,
        ?27, ?28
    )
    "#
}

pub fn get_update_sql() -> &'static str {
    r#"
    UPDATE traffic_records SET
        status = ?1,
        content_type = ?2,
        request_size = ?3,
        response_size = ?4,
        duration_ms = ?5,
        client_app = ?6,
        client_pid = ?7,
        client_path = ?8,
        flags = ?9,
        frame_count = ?10,
        last_frame_id = ?11,
        timing_blob = ?12,
        request_headers_blob = ?13,
        response_headers_blob = ?14,
        matched_rules_blob = ?15,
        socket_status_blob = ?16,
        request_body_ref_blob = ?17,
        response_body_ref_blob = ?18
    WHERE id = ?19
    "#
}
