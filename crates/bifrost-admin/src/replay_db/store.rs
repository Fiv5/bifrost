use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};

use super::schema::{
    get_insert_group_sql, get_insert_history_sql, get_insert_request_sql, get_update_group_sql,
    get_update_request_sql, init_database, InitError,
};
use super::types::{
    ReplayDbStats, ReplayGroup, ReplayHistory, ReplayRequest, ReplayRequestSummary, RequestSource,
    RequestType, RuleConfig, MAX_HISTORY, MAX_REQUESTS,
};

pub type SharedReplayDbStore = Arc<ReplayDbStore>;

const CLEANUP_CHECK_INTERVAL: usize = 100;

pub struct ReplayDbStore {
    db_path: PathBuf,
    write_conn: Mutex<Connection>,
    read_conn: Mutex<Connection>,
    insert_counter: AtomicUsize,
}

impl ReplayDbStore {
    pub fn new(db_dir: PathBuf) -> Result<Self, rusqlite::Error> {
        if !db_dir.exists() {
            fs::create_dir_all(&db_dir).ok();
        }

        let db_path = db_dir.join("replay.db");

        tracing::info!(
            db_path = %db_path.display(),
            "[REPLAY_DB] Initializing SQLite replay store"
        );

        let write_conn = match Self::open_or_reset_database(&db_path) {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!(error = %e, "[REPLAY_DB] Failed to open database");
                return Err(e);
            }
        };

        let read_conn = Connection::open(&db_path)?;
        read_conn.execute_batch(
            "PRAGMA query_only = true; PRAGMA cache_size = 2000; PRAGMA mmap_size = 67108864;",
        )?;

        tracing::info!("[REPLAY_DB] SQLite replay store initialized");

        Ok(Self {
            db_path,
            write_conn: Mutex::new(write_conn),
            read_conn: Mutex::new(read_conn),
            insert_counter: AtomicUsize::new(0),
        })
    }

    fn open_or_reset_database(db_path: &PathBuf) -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open(db_path)?;

        match init_database(&conn) {
            Ok(()) => Ok(conn),
            Err(InitError::Sqlite(e)) => Err(e),
        }
    }

    pub fn create_group(&self, group: &ReplayGroup) -> Result<(), rusqlite::Error> {
        let conn = self.write_conn.lock();
        conn.execute(
            get_insert_group_sql(),
            params![
                &group.id,
                &group.name,
                &group.parent_id,
                group.sort_order,
                group.created_at as i64,
                group.updated_at as i64,
            ],
        )?;
        Ok(())
    }

    pub fn update_group(&self, group: &ReplayGroup) -> Result<(), rusqlite::Error> {
        let conn = self.write_conn.lock();
        conn.execute(
            get_update_group_sql(),
            params![
                &group.name,
                &group.parent_id,
                group.sort_order,
                group.updated_at as i64,
                &group.id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_group(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.write_conn.lock();
        conn.execute("DELETE FROM replay_groups WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn get_group(&self, id: &str) -> Option<ReplayGroup> {
        let conn = self.read_conn.lock();
        conn.query_row(
            "SELECT id, name, parent_id, sort_order, created_at, updated_at FROM replay_groups WHERE id = ?",
            [id],
            |row| {
                Ok(ReplayGroup {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    sort_order: row.get(3)?,
                    created_at: row.get::<_, i64>(4)? as u64,
                    updated_at: row.get::<_, i64>(5)? as u64,
                })
            },
        )
        .optional()
        .ok()
        .flatten()
    }

    pub fn list_groups(&self) -> Vec<ReplayGroup> {
        let conn = self.read_conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, name, parent_id, sort_order, created_at, updated_at FROM replay_groups ORDER BY sort_order ASC, created_at ASC"
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        stmt.query_map([], |row| {
            Ok(ReplayGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                sort_order: row.get(3)?,
                created_at: row.get::<_, i64>(4)? as u64,
                updated_at: row.get::<_, i64>(5)? as u64,
            })
        })
        .map(|r| r.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn count_groups(&self) -> usize {
        let conn = self.read_conn.lock();
        conn.query_row("SELECT COUNT(*) FROM replay_groups", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|v| v as usize)
        .unwrap_or(0)
    }

    pub fn create_request(&self, request: &ReplayRequest) -> Result<(), rusqlite::Error> {
        let count = self.count_requests();
        if count >= MAX_REQUESTS {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_FULL),
                Some(format!("Maximum request limit ({}) reached", MAX_REQUESTS)),
            ));
        }

        let headers_json = serde_json::to_string(&request.headers).ok();
        let body_json = request
            .body
            .as_ref()
            .and_then(|b| serde_json::to_string(b).ok());
        let request_type = request_type_to_str(&request.request_type);
        let source = request_source_to_str(&request.source);

        let conn = self.write_conn.lock();
        conn.execute(
            get_insert_request_sql(),
            params![
                &request.id,
                &request.group_id,
                &request.name,
                request_type,
                &request.method,
                &request.url,
                headers_json,
                body_json,
                request.is_saved as i32,
                request.sort_order,
                source,
                request.created_at as i64,
                request.updated_at as i64,
            ],
        )?;
        Ok(())
    }

    pub fn update_request(&self, request: &ReplayRequest) -> Result<(), rusqlite::Error> {
        let headers_json = serde_json::to_string(&request.headers).ok();
        let body_json = request
            .body
            .as_ref()
            .and_then(|b| serde_json::to_string(b).ok());
        let request_type = request_type_to_str(&request.request_type);
        let source = request_source_to_str(&request.source);

        let conn = self.write_conn.lock();
        conn.execute(
            get_update_request_sql(),
            params![
                &request.group_id,
                &request.name,
                request_type,
                &request.method,
                &request.url,
                headers_json,
                body_json,
                request.is_saved as i32,
                request.sort_order,
                source,
                request.updated_at as i64,
                &request.id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_request(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.write_conn.lock();
        conn.execute("DELETE FROM replay_requests WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn get_request(&self, id: &str) -> Option<ReplayRequest> {
        let conn = self.read_conn.lock();
        conn.query_row(
            "SELECT id, group_id, name, request_type, method, url, headers_blob, body_blob, is_saved, sort_order, source, created_at, updated_at FROM replay_requests WHERE id = ?",
            [id],
            Self::row_to_request,
        )
        .optional()
        .ok()
        .flatten()
    }

    pub fn list_requests(
        &self,
        saved_only: Option<bool>,
        group_id: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Vec<ReplayRequestSummary> {
        let conn = self.read_conn.lock();

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(saved) = saved_only {
            conditions.push("is_saved = ?");
            params.push(Box::new(saved as i32));
        }

        if let Some(gid) = group_id {
            if gid.is_empty() {
                conditions.push("group_id IS NULL");
            } else {
                conditions.push("group_id = ?");
                params.push(Box::new(gid.to_string()));
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = match (limit, offset) {
            (Some(l), Some(o)) => format!("LIMIT {} OFFSET {}", l, o),
            (Some(l), None) => format!("LIMIT {}", l),
            _ => String::new(),
        };

        let sql = format!(
            "SELECT id, group_id, name, method, url, is_saved, source, created_at, updated_at \
             FROM replay_requests {} ORDER BY updated_at DESC {}",
            where_clause, limit_clause
        );

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        stmt.query_map(param_refs.as_slice(), |row| {
            let source_str: String = row.get(6)?;
            Ok(ReplayRequestSummary {
                id: row.get(0)?,
                group_id: row.get(1)?,
                name: row.get(2)?,
                method: row.get(3)?,
                url: row.get(4)?,
                is_saved: row.get::<_, i32>(5)? != 0,
                source: str_to_request_source(&source_str),
                created_at: row.get::<_, i64>(7)? as u64,
                updated_at: row.get::<_, i64>(8)? as u64,
            })
        })
        .map(|r| r.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn count_requests(&self) -> usize {
        let conn = self.read_conn.lock();
        conn.query_row("SELECT COUNT(*) FROM replay_requests", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|v| v as usize)
        .unwrap_or(0)
    }

    pub fn next_imported_sequence(&self) -> usize {
        let conn = self.read_conn.lock();
        conn.query_row(
            "SELECT MAX(CAST(SUBSTR(id, 5) AS INTEGER)) FROM replay_requests WHERE id LIKE 'OUT-%'",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
        .map(|v| v as usize + 1)
        .unwrap_or(1)
    }

    pub fn move_request_to_group(
        &self,
        request_id: &str,
        group_id: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.write_conn.lock();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE replay_requests SET group_id = ?, updated_at = ? WHERE id = ?",
            params![group_id, now, request_id],
        )?;
        Ok(())
    }

    fn row_to_request(row: &rusqlite::Row) -> rusqlite::Result<ReplayRequest> {
        let request_type_str: String = row.get(3)?;
        let headers_json: Option<String> = row.get(6)?;
        let body_json: Option<String> = row.get(7)?;
        let source_str: String = row.get(10)?;

        let headers = headers_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let body = body_json.and_then(|s| serde_json::from_str(&s).ok());

        Ok(ReplayRequest {
            id: row.get(0)?,
            group_id: row.get(1)?,
            name: row.get(2)?,
            request_type: str_to_request_type(&request_type_str),
            method: row.get(4)?,
            url: row.get(5)?,
            headers,
            body,
            is_saved: row.get::<_, i32>(8)? != 0,
            sort_order: row.get(9)?,
            source: str_to_request_source(&source_str),
            created_at: row.get::<_, i64>(11)? as u64,
            updated_at: row.get::<_, i64>(12)? as u64,
        })
    }

    pub fn create_history(&self, history: &ReplayHistory) -> Result<(), rusqlite::Error> {
        let rule_config_json = history
            .rule_config
            .as_ref()
            .and_then(|r| serde_json::to_string(r).ok());

        let conn = self.write_conn.lock();
        conn.execute(
            get_insert_history_sql(),
            params![
                &history.id,
                &history.request_id,
                &history.traffic_id,
                &history.method,
                &history.url,
                history.status as i32,
                history.duration_ms as i64,
                history.executed_at as i64,
                rule_config_json,
            ],
        )?;

        self.maybe_cleanup_history(&conn);

        Ok(())
    }

    pub fn delete_history(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.write_conn.lock();
        conn.execute("DELETE FROM replay_history WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn clear_history(&self, request_id: Option<&str>) -> Result<usize, rusqlite::Error> {
        let conn = self.write_conn.lock();
        let deleted = if let Some(rid) = request_id {
            conn.execute("DELETE FROM replay_history WHERE request_id = ?", [rid])?
        } else {
            conn.execute("DELETE FROM replay_history", [])?
        };
        Ok(deleted)
    }

    pub fn list_history(
        &self,
        request_id: Option<&str>,
        request_unbound_only: bool,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Vec<ReplayHistory> {
        let conn = self.read_conn.lock();

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(rid) = request_id {
            conditions.push("request_id = ?");
            params.push(Box::new(rid.to_string()));
        } else if request_unbound_only {
            conditions.push("request_id IS NULL");
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = match (limit, offset) {
            (Some(_), Some(_)) => "LIMIT ? OFFSET ?".to_string(),
            (Some(_), None) => "LIMIT ?".to_string(),
            (None, Some(_)) => "LIMIT -1 OFFSET ?".to_string(),
            (None, None) => String::new(),
        };

        if let Some(l) = limit {
            params.push(Box::new(l as i64));
        }
        if let Some(o) = offset {
            params.push(Box::new(o as i64));
        }

        let sql = format!(
            "SELECT id, request_id, traffic_id, method, url, status, duration_ms, executed_at, rule_config_blob \
             FROM replay_history {} ORDER BY executed_at DESC {}",
            where_clause, limit_clause
        );

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        stmt.query_map(params_from_iter(param_refs), |row| {
            let rule_config_json: Option<String> = row.get(8)?;
            let rule_config: Option<RuleConfig> =
                rule_config_json.and_then(|s| serde_json::from_str(&s).ok());

            Ok(ReplayHistory {
                id: row.get(0)?,
                request_id: row.get(1)?,
                traffic_id: row.get(2)?,
                method: row.get(3)?,
                url: row.get(4)?,
                status: row.get::<_, i32>(5)? as u16,
                duration_ms: row.get::<_, i64>(6)? as u64,
                executed_at: row.get::<_, i64>(7)? as u64,
                rule_config,
            })
        })
        .map(|r| r.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn count_history(&self, request_id: Option<&str>, request_unbound_only: bool) -> usize {
        let conn = self.read_conn.lock();
        let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(rid) = request_id {
            (
                "SELECT COUNT(*) FROM replay_history WHERE request_id = ?",
                vec![Box::new(rid.to_string())],
            )
        } else if request_unbound_only {
            (
                "SELECT COUNT(*) FROM replay_history WHERE request_id IS NULL",
                Vec::new(),
            )
        } else {
            ("SELECT COUNT(*) FROM replay_history", Vec::new())
        };

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        conn.query_row(sql, params_from_iter(param_refs), |row| {
            row.get::<_, i64>(0)
        })
        .map(|v| v as usize)
        .unwrap_or(0)
    }

    fn maybe_cleanup_history(&self, conn: &Connection) {
        let counter = self.insert_counter.fetch_add(1, Ordering::Relaxed);
        if !counter.is_multiple_of(CLEANUP_CHECK_INTERVAL) {
            return;
        }

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM replay_history", [], |row| row.get(0))
            .unwrap_or(0);

        if count as usize > MAX_HISTORY {
            let excess = count as usize - MAX_HISTORY;
            match conn.execute(
                "DELETE FROM replay_history WHERE id IN (
                    SELECT id FROM replay_history ORDER BY executed_at ASC LIMIT ?
                )",
                [excess as i64],
            ) {
                Ok(deleted) => {
                    tracing::info!(
                        deleted = deleted,
                        total_before = count,
                        max_limit = MAX_HISTORY,
                        "[REPLAY_DB] Auto cleanup: removed {} oldest history records",
                        deleted
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "[REPLAY_DB] Failed to cleanup history records"
                    );
                }
            }
        }
    }

    pub fn stats(&self) -> ReplayDbStats {
        let db_size = fs::metadata(&self.db_path).map(|m| m.len()).unwrap_or(0);

        ReplayDbStats {
            request_count: self.count_requests(),
            history_count: self.count_history(None, false),
            group_count: self.count_groups(),
            db_size,
            db_path: self.db_path.display().to_string(),
        }
    }

    pub fn checkpoint(&self) {
        let conn = self.write_conn.lock();
        if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)") {
            tracing::warn!(error = %e, "[REPLAY_DB] WAL checkpoint failed");
        }
    }
}

fn request_type_to_str(rt: &RequestType) -> &'static str {
    match rt {
        RequestType::Http => "http",
        RequestType::Sse => "sse",
        RequestType::WebSocket => "websocket",
    }
}

fn str_to_request_type(s: &str) -> RequestType {
    match s {
        "sse" => RequestType::Sse,
        "websocket" => RequestType::WebSocket,
        _ => RequestType::Http,
    }
}

fn request_source_to_str(source: &RequestSource) -> &'static str {
    match source {
        RequestSource::Internal => "internal",
        RequestSource::Imported => "imported",
    }
}

fn str_to_request_source(s: &str) -> RequestSource {
    match s {
        "imported" => RequestSource::Imported,
        _ => RequestSource::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store() -> ReplayDbStore {
        let dir = TempDir::new().expect("temp dir");
        ReplayDbStore::new(dir.keep()).expect("replay db store")
    }

    fn make_history(id: &str, request_id: Option<&str>, executed_at: u64) -> ReplayHistory {
        ReplayHistory {
            id: id.to_string(),
            request_id: request_id.map(str::to_string),
            traffic_id: format!("traffic-{id}"),
            method: "GET".to_string(),
            url: format!("https://example.com/{id}"),
            status: 200,
            duration_ms: 10,
            executed_at,
            rule_config: None,
        }
    }

    fn make_request(id: &str) -> ReplayRequest {
        let mut request = ReplayRequest::new("GET".to_string(), "https://example.com".to_string());
        request.id = id.to_string();
        request.is_saved = true;
        request
    }

    #[test]
    fn list_and_count_unbound_history() {
        let store = make_store();
        store
            .create_request(&make_request("req-1"))
            .expect("save request");

        store
            .create_history(&make_history("saved-1", Some("req-1"), 1))
            .expect("save bound history");
        store
            .create_history(&make_history("unbound-1", None, 2))
            .expect("save unbound history");
        store
            .create_history(&make_history("unbound-2", None, 3))
            .expect("save unbound history");

        let all_history = store.list_history(None, false, Some(10), Some(0));
        let unbound_history = store.list_history(None, true, Some(10), Some(0));
        let request_history = store.list_history(Some("req-1"), false, Some(10), Some(0));

        assert_eq!(store.count_history(None, false), 3);
        assert_eq!(store.count_history(None, true), 2);
        assert_eq!(store.count_history(Some("req-1"), false), 1);
        assert_eq!(all_history.len(), 3);
        assert_eq!(unbound_history.len(), 2);
        assert!(unbound_history.iter().all(|item| item.request_id.is_none()));
        assert_eq!(request_history.len(), 1);
        assert_eq!(request_history[0].request_id.as_deref(), Some("req-1"));
    }

    #[test]
    fn create_history_auto_cleans_up_oldest_records() {
        let store = make_store();

        for idx in 0..(MAX_HISTORY + CLEANUP_CHECK_INTERVAL + 1) {
            store
                .create_history(&make_history(&format!("history-{idx}"), None, idx as u64))
                .expect("save history");
        }

        let history = store.list_history(None, false, Some(MAX_HISTORY + 10), Some(0));

        assert_eq!(store.count_history(None, false), MAX_HISTORY);
        assert_eq!(history.len(), MAX_HISTORY);
        assert_eq!(
            history
                .last()
                .and_then(|item| item.executed_at.checked_sub(0)),
            Some((CLEANUP_CHECK_INTERVAL + 1) as u64)
        );
    }
}
