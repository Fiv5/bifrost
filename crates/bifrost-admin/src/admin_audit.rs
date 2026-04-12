use std::fs;
use std::path::PathBuf;

use bifrost_core::{BifrostError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

const MAX_LOGIN_RECORDS: i64 = 100;
const MAX_LOGIN_AGE_DAYS: i64 = 30;

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
           ua TEXT NOT NULL,\
           success INTEGER NOT NULL DEFAULT 1\
         );\
         CREATE INDEX IF NOT EXISTS idx_admin_login_audit_ts ON admin_login_audit(ts);\
         CREATE INDEX IF NOT EXISTS idx_admin_login_audit_username ON admin_login_audit(username);",
    )
}

pub fn record_login(username: &str, ip: &str, ua: &str) -> Result<()> {
    record_login_inner(username, ip, ua, true)
}

pub fn record_failed_login_attempt(username: &str, ip: &str, ua: &str) -> Result<()> {
    record_login_inner(username, ip, ua, false)
}

fn record_login_inner(username: &str, ip: &str, ua: &str, success: bool) -> Result<()> {
    let db_path = audit_db_path()?;
    let conn = Connection::open(db_path)
        .map_err(|e| BifrostError::Storage(format!("Failed to open audit db: {e}")))?;
    init_db(&conn).map_err(|e| BifrostError::Storage(format!("Failed to init audit db: {e}")))?;

    let ts = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO admin_login_audit(ts, username, ip, ua, success) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![ts, username, ip, ua, success as i32],
    )
    .map_err(|e| BifrostError::Storage(format!("Failed to insert audit row: {e}")))?;

    cleanup_old_records(&conn)
        .map_err(|e| BifrostError::Storage(format!("Failed to cleanup audit records: {e}")))?;

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

fn cleanup_old_records(conn: &Connection) -> std::result::Result<(), rusqlite::Error> {
    let cutoff_ts = chrono::Utc::now().timestamp() - MAX_LOGIN_AGE_DAYS * 86400;
    conn.execute(
        "DELETE FROM admin_login_audit WHERE ts < ?1",
        params![cutoff_ts],
    )?;

    conn.execute(
        "DELETE FROM admin_login_audit WHERE id NOT IN \
         (SELECT id FROM admin_login_audit ORDER BY id DESC LIMIT ?1)",
        params![MAX_LOGIN_RECORDS],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(1) FROM admin_login_audit", [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    #[test]
    fn test_cleanup_expired_records() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("audit.db");
        let conn = Connection::open(&db_path).unwrap();
        init_db(&conn).unwrap();

        let now = chrono::Utc::now().timestamp();
        let expired_ts = now - 31 * 86400;

        for i in 0..5 {
            conn.execute(
                "INSERT INTO admin_login_audit(ts, username, ip, ua) VALUES (?1, ?2, ?3, ?4)",
                params![expired_ts, format!("old-{i}"), "10.0.0.1", "old-ua"],
            )
            .unwrap();
        }
        for i in 0..3 {
            conn.execute(
                "INSERT INTO admin_login_audit(ts, username, ip, ua) VALUES (?1, ?2, ?3, ?4)",
                params![now, format!("fresh-{i}"), "10.0.0.2", "new-ua"],
            )
            .unwrap();
        }
        assert_eq!(count(&conn), 8);

        cleanup_old_records(&conn).unwrap();

        assert_eq!(count(&conn), 3);
        let mut stmt = conn
            .prepare("SELECT username FROM admin_login_audit")
            .unwrap();
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for name in &names {
            assert!(name.starts_with("fresh-"), "unexpected record: {name}");
        }
    }

    #[test]
    fn test_cleanup_excess_records() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("audit.db");
        let conn = Connection::open(&db_path).unwrap();
        init_db(&conn).unwrap();

        let now = chrono::Utc::now().timestamp();
        for i in 0..110 {
            conn.execute(
                "INSERT INTO admin_login_audit(ts, username, ip, ua) VALUES (?1, ?2, ?3, ?4)",
                params![now, format!("user-{i:04}"), "10.0.0.1", "ua"],
            )
            .unwrap();
        }
        assert_eq!(count(&conn), 110);

        cleanup_old_records(&conn).unwrap();

        let remaining = count(&conn);
        assert_eq!(remaining, MAX_LOGIN_RECORDS);

        let max_id: i64 = conn
            .query_row("SELECT MAX(id) FROM admin_login_audit", [], |row| {
                row.get(0)
            })
            .unwrap();
        let min_id: i64 = conn
            .query_row("SELECT MIN(id) FROM admin_login_audit", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            max_id - min_id + 1,
            MAX_LOGIN_RECORDS,
            "should retain the latest 100 consecutive records"
        );
    }

    #[test]
    fn test_cleanup_combined_expired_and_excess() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("audit.db");
        let conn = Connection::open(&db_path).unwrap();
        init_db(&conn).unwrap();

        let now = chrono::Utc::now().timestamp();
        let expired_ts = now - 31 * 86400;

        for i in 0..50 {
            conn.execute(
                "INSERT INTO admin_login_audit(ts, username, ip, ua) VALUES (?1, ?2, ?3, ?4)",
                params![expired_ts, format!("expired-{i}"), "10.0.0.1", "ua"],
            )
            .unwrap();
        }
        for i in 0..80 {
            conn.execute(
                "INSERT INTO admin_login_audit(ts, username, ip, ua) VALUES (?1, ?2, ?3, ?4)",
                params![now, format!("fresh-{i}"), "10.0.0.2", "ua"],
            )
            .unwrap();
        }
        assert_eq!(count(&conn), 130);

        cleanup_old_records(&conn).unwrap();

        let remaining = count(&conn);
        assert_eq!(
            remaining, 80,
            "all expired removed, 80 fresh remain (< 100 limit)"
        );
    }
}
