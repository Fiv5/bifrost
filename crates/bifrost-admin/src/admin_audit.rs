use std::fs;
use std::path::PathBuf;

use bifrost_core::{BifrostError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminLoginAuditEntry {
    pub id: i64,
    pub ts: i64,
    pub username: String,
    pub ip: String,
    pub ua: String,
}

pub fn audit_db_path() -> Result<PathBuf> {
    let dir = bifrost_storage::data_dir().join("admin");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("audit.db"))
}

fn init_db(conn: &Connection) -> std::result::Result<(), rusqlite::Error> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;\
         PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = NORMAL;\
         CREATE TABLE IF NOT EXISTS admin_login_audit (\
           id INTEGER PRIMARY KEY AUTOINCREMENT,\
           ts INTEGER NOT NULL,\
           username TEXT NOT NULL,\
           ip TEXT NOT NULL,\
           ua TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_admin_login_audit_ts ON admin_login_audit(ts);\
         CREATE INDEX IF NOT EXISTS idx_admin_login_audit_username ON admin_login_audit(username);",
    )
}

pub fn record_login(username: &str, ip: &str, ua: &str) -> Result<()> {
    let db_path = audit_db_path()?;
    let conn = Connection::open(db_path)
        .map_err(|e| BifrostError::Storage(format!("Failed to open audit db: {e}")))?;
    init_db(&conn).map_err(|e| BifrostError::Storage(format!("Failed to init audit db: {e}")))?;

    let ts = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO admin_login_audit(ts, username, ip, ua) VALUES (?1, ?2, ?3, ?4)",
        params![ts, username, ip, ua],
    )
    .map_err(|e| BifrostError::Storage(format!("Failed to insert audit row: {e}")))?;
    Ok(())
}

pub fn list_logins(limit: usize, offset: usize) -> Result<Vec<AdminLoginAuditEntry>> {
    let db_path = audit_db_path()?;
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let conn = Connection::open(db_path)
        .map_err(|e| BifrostError::Storage(format!("Failed to open audit db: {e}")))?;
    init_db(&conn).map_err(|e| BifrostError::Storage(format!("Failed to init audit db: {e}")))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, ts, username, ip, ua \
             FROM admin_login_audit \
             ORDER BY id DESC \
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| BifrostError::Storage(format!("Failed to prepare query: {e}")))?;

    let rows = stmt
        .query_map(params![limit as i64, offset as i64], |row| {
            Ok(AdminLoginAuditEntry {
                id: row.get(0)?,
                ts: row.get(1)?,
                username: row.get(2)?,
                ip: row.get(3)?,
                ua: row.get(4)?,
            })
        })
        .map_err(|e| BifrostError::Storage(format!("Failed to query audit rows: {e}")))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| BifrostError::Storage(format!("Failed to read audit row: {e}")))?);
    }
    Ok(out)
}

pub fn count_logins() -> Result<i64> {
    let db_path = audit_db_path()?;
    if !db_path.exists() {
        return Ok(0);
    }
    let conn = Connection::open(db_path)
        .map_err(|e| BifrostError::Storage(format!("Failed to open audit db: {e}")))?;
    init_db(&conn).map_err(|e| BifrostError::Storage(format!("Failed to init audit db: {e}")))?;

    let count: i64 = conn
        .query_row("SELECT COUNT(1) FROM admin_login_audit", [], |row| {
            row.get(0)
        })
        .map_err(|e| BifrostError::Storage(format!("Failed to count audit rows: {e}")))?;
    Ok(count)
}
