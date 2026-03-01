use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::broadcast;

use super::query::{Direction, QueryParams, QueryResult};
use super::schema::{get_insert_sql, get_update_sql, init_database, InitError};
use super::types::{encode_flags, TrafficDbStats, TrafficSummaryCompact};
use crate::traffic::{SocketStatus, TrafficRecord};

const DEFAULT_CACHE_SIZE: usize = 500;
const CLEANUP_CHECK_INTERVAL: u64 = 100;

pub type SharedTrafficDbStore = Arc<TrafficDbStore>;

pub struct TrafficDbStore {
    db_path: PathBuf,
    write_conn: Mutex<Connection>,
    read_conn: Mutex<Connection>,
    max_records: AtomicUsize,
    retention_hours: AtomicU64,
    tx: broadcast::Sender<TrafficRecord>,
    current_sequence: AtomicU64,
    recent_cache: RwLock<LruCache<String, TrafficRecord>>,
    write_count: AtomicU64,
}

impl TrafficDbStore {
    pub fn new(
        db_dir: PathBuf,
        max_records: usize,
        retention_hours: Option<u64>,
    ) -> Result<Self, rusqlite::Error> {
        if !db_dir.exists() {
            fs::create_dir_all(&db_dir).ok();
        }

        let db_path = db_dir.join("traffic.db");

        tracing::info!(
            db_path = %db_path.display(),
            max_records = max_records,
            retention_hours = retention_hours.unwrap_or(168),
            "[TRAFFIC_DB] Initializing SQLite traffic store"
        );

        let write_conn = match Self::open_or_reset_database(&db_path) {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!(error = %e, "[TRAFFIC_DB] Failed to open database");
                return Err(e);
            }
        };

        let read_conn = Connection::open(&db_path)?;
        read_conn.execute_batch(
            "PRAGMA query_only = true; PRAGMA cache_size = 5000; PRAGMA mmap_size = 134217728;",
        )?;

        let current_seq = match Self::resequence_records(&write_conn) {
            Ok(count) => count,
            Err(e) => {
                tracing::warn!(error = %e, "[TRAFFIC_DB] Failed to resequence, using max sequence");
                Self::get_max_sequence(&write_conn).unwrap_or(0)
            }
        };

        let (tx, _) = broadcast::channel(1024);

        let cache_size = std::num::NonZeroUsize::new(DEFAULT_CACHE_SIZE).unwrap();

        tracing::info!(
            current_sequence = current_seq,
            "[TRAFFIC_DB] SQLite traffic store initialized"
        );

        Ok(Self {
            db_path,
            write_conn: Mutex::new(write_conn),
            read_conn: Mutex::new(read_conn),
            max_records: AtomicUsize::new(max_records),
            retention_hours: AtomicU64::new(retention_hours.unwrap_or(168)),
            tx,
            current_sequence: AtomicU64::new(current_seq + 1),
            recent_cache: RwLock::new(LruCache::new(cache_size)),
            write_count: AtomicU64::new(0),
        })
    }

    fn open_or_reset_database(db_path: &PathBuf) -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open(db_path)?;

        match init_database(&conn) {
            Ok(()) => Ok(conn),
            Err(InitError::VersionMismatch { current, expected }) => {
                tracing::warn!(
                    current_version = current,
                    expected_version = expected,
                    "[TRAFFIC_DB] Schema version mismatch, resetting database"
                );
                drop(conn);

                let wal_path = db_path.with_extension("db-wal");
                let shm_path = db_path.with_extension("db-shm");
                if let Err(e) = fs::remove_file(db_path) {
                    tracing::warn!(error = %e, "[TRAFFIC_DB] Failed to remove old database file");
                }
                if wal_path.exists() {
                    fs::remove_file(&wal_path).ok();
                }
                if shm_path.exists() {
                    fs::remove_file(&shm_path).ok();
                }

                let new_conn = Connection::open(db_path)?;
                init_database(&new_conn).map_err(|e| match e {
                    InitError::Sqlite(e) => e,
                    InitError::VersionMismatch { .. } => rusqlite::Error::QueryReturnedNoRows,
                })?;
                tracing::info!("[TRAFFIC_DB] Database reset successfully");
                Ok(new_conn)
            }
            Err(InitError::Sqlite(e)) => Err(e),
        }
    }

    fn get_max_sequence(conn: &Connection) -> Option<u64> {
        conn.query_row("SELECT MAX(sequence) FROM traffic_records", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .ok()
        .flatten()
        .map(|v| v as u64)
    }

    fn resequence_records(conn: &Connection) -> rusqlite::Result<u64> {
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM traffic_records", [], |row| row.get(0))?;

        if count == 0 {
            return Ok(0);
        }

        let mut stmt = conn.prepare("SELECT id FROM traffic_records ORDER BY sequence ASC")?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        for (idx, id) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE traffic_records SET sequence = ? WHERE id = ?",
                rusqlite::params![(idx + 1) as i64, id],
            )?;
        }

        tracing::info!(
            record_count = ids.len(),
            "[TRAFFIC_DB] Resequenced existing records (1 to {})",
            ids.len()
        );

        Ok(ids.len() as u64)
    }

    pub fn record(&self, mut record: TrafficRecord) {
        let seq = self.current_sequence.fetch_add(1, Ordering::SeqCst);
        record.sequence = seq;

        let _ = self.tx.send(record.clone());

        {
            let mut cache = self.recent_cache.write();
            cache.put(record.id.clone(), record.clone());
        }

        let conn = self.write_conn.lock();
        let flags = encode_flags(&record);

        let timing_blob = record
            .timing
            .as_ref()
            .and_then(|t| bincode::serialize(t).ok());
        let req_headers_blob = record
            .request_headers
            .as_ref()
            .and_then(|h| bincode::serialize(h).ok());
        let res_headers_blob = record
            .response_headers
            .as_ref()
            .and_then(|h| bincode::serialize(h).ok());
        let rules_blob = record
            .matched_rules
            .as_ref()
            .and_then(|r| bincode::serialize(r).ok());
        let socket_blob = record
            .socket_status
            .as_ref()
            .and_then(|s| bincode::serialize(s).ok());
        let req_body_blob = record
            .request_body_ref
            .as_ref()
            .and_then(|b| bincode::serialize(b).ok());
        let res_body_blob = record
            .response_body_ref
            .as_ref()
            .and_then(|b| bincode::serialize(b).ok());
        let orig_req_headers_blob = record
            .original_request_headers
            .as_ref()
            .and_then(|h| bincode::serialize(h).ok());
        let actual_res_headers_blob = record
            .actual_response_headers
            .as_ref()
            .and_then(|h| bincode::serialize(h).ok());
        let req_script_results_blob = record
            .req_script_results
            .as_ref()
            .and_then(|r| bincode::serialize(r).ok());
        let res_script_results_blob = record
            .res_script_results
            .as_ref()
            .and_then(|r| bincode::serialize(r).ok());

        let result = conn.execute(
            get_insert_sql(),
            params![
                seq as i64,
                &record.id,
                record.timestamp as i64,
                &record.host,
                &record.method,
                record.status as i32,
                &record.protocol,
                &record.url,
                &record.path,
                &record.content_type,
                &record.request_content_type,
                record.request_size as i64,
                record.response_size as i64,
                record.duration_ms as i64,
                &record.client_ip,
                &record.client_app,
                record.client_pid.map(|p| p as i32),
                &record.client_path,
                flags as i32,
                record.frame_count as i64,
                record.last_frame_id as i64,
                timing_blob,
                req_headers_blob,
                res_headers_blob,
                rules_blob,
                socket_blob,
                req_body_blob,
                res_body_blob,
                &record.actual_url,
                &record.actual_host,
                orig_req_headers_blob,
                actual_res_headers_blob,
                req_script_results_blob,
                res_script_results_blob,
                &record.error_message,
            ],
        );

        if let Err(e) = result {
            tracing::error!(error = %e, id = %record.id, "[TRAFFIC_DB] Failed to insert record");
        }

        let count = self.write_count.fetch_add(1, Ordering::Relaxed);
        if count.is_multiple_of(CLEANUP_CHECK_INTERVAL) {
            self.maybe_cleanup(&conn);
        }
    }

    pub fn update_by_id<F>(&self, id: &str, updater: F) -> bool
    where
        F: FnOnce(&mut TrafficRecord),
    {
        {
            let mut cache = self.recent_cache.write();
            if let Some(record) = cache.get_mut(id) {
                updater(record);
                let updated = record.clone();
                drop(cache);
                self.persist_update(&updated);
                let _ = self.tx.send(updated);
                return true;
            }
        }

        if let Some(mut record) = self.get_by_id_from_db(id) {
            updater(&mut record);
            self.persist_update(&record);
            {
                let mut cache = self.recent_cache.write();
                cache.put(record.id.clone(), record.clone());
            }
            let _ = self.tx.send(record);
            return true;
        }

        false
    }

    fn persist_update(&self, record: &TrafficRecord) {
        let conn = self.write_conn.lock();
        let flags = encode_flags(record);

        let timing_blob = record
            .timing
            .as_ref()
            .and_then(|t| bincode::serialize(t).ok());
        let req_headers_blob = record
            .request_headers
            .as_ref()
            .and_then(|h| bincode::serialize(h).ok());
        let res_headers_blob = record
            .response_headers
            .as_ref()
            .and_then(|h| bincode::serialize(h).ok());
        let rules_blob = record
            .matched_rules
            .as_ref()
            .and_then(|r| bincode::serialize(r).ok());
        let socket_blob = record
            .socket_status
            .as_ref()
            .and_then(|s| bincode::serialize(s).ok());
        let req_body_blob = record
            .request_body_ref
            .as_ref()
            .and_then(|b| bincode::serialize(b).ok());
        let res_body_blob = record
            .response_body_ref
            .as_ref()
            .and_then(|b| bincode::serialize(b).ok());
        let orig_req_headers_blob = record
            .original_request_headers
            .as_ref()
            .and_then(|h| bincode::serialize(h).ok());
        let actual_res_headers_blob = record
            .actual_response_headers
            .as_ref()
            .and_then(|h| bincode::serialize(h).ok());
        let req_script_results_blob = record
            .req_script_results
            .as_ref()
            .and_then(|r| bincode::serialize(r).ok());
        let res_script_results_blob = record
            .res_script_results
            .as_ref()
            .and_then(|r| bincode::serialize(r).ok());

        let result = conn.execute(
            get_update_sql(),
            params![
                record.status as i32,
                &record.content_type,
                record.request_size as i64,
                record.response_size as i64,
                record.duration_ms as i64,
                &record.client_app,
                record.client_pid.map(|p| p as i32),
                &record.client_path,
                flags as i32,
                record.frame_count as i64,
                record.last_frame_id as i64,
                timing_blob,
                req_headers_blob,
                res_headers_blob,
                rules_blob,
                socket_blob,
                req_body_blob,
                res_body_blob,
                &record.actual_url,
                &record.actual_host,
                orig_req_headers_blob,
                actual_res_headers_blob,
                req_script_results_blob,
                res_script_results_blob,
                &record.error_message,
                &record.id,
            ],
        );

        if let Err(e) = result {
            tracing::error!(error = %e, id = %record.id, "[TRAFFIC_DB] Failed to update record");
        }
    }

    pub fn query(&self, params: &QueryParams) -> QueryResult {
        let conn = self.read_conn.lock();
        let (sql, values) = params.build_select_sql();
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "[TRAFFIC_DB] Failed to prepare query");
                return QueryResult {
                    records: vec![],
                    next_cursor: None,
                    prev_cursor: None,
                    has_more: false,
                    total: 0,
                    server_sequence: self.current_sequence.load(Ordering::Relaxed),
                };
            }
        };

        let records: Vec<TrafficSummaryCompact> = stmt
            .query_map(param_refs.as_slice(), |row| {
                let socket_blob: Option<Vec<u8>> = row.get(18)?;
                let socket_status: Option<SocketStatus> =
                    socket_blob.and_then(|b| bincode::deserialize(&b).ok());

                let rules_blob: Option<Vec<u8>> = row.get(19)?;
                let matched_rules: Vec<crate::traffic::MatchedRule> = rules_blob
                    .and_then(|b| bincode::deserialize(&b).ok())
                    .unwrap_or_default();
                let rc = matched_rules.len();
                let rp: Vec<String> = matched_rules
                    .iter()
                    .map(|r| r.protocol.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                Ok(TrafficSummaryCompact {
                    seq: row.get::<_, i64>(0)? as u64,
                    id: row.get(1)?,
                    ts: row.get::<_, i64>(2)? as u64,
                    h: row.get(3)?,
                    m: row.get(4)?,
                    s: row.get::<_, i32>(5)? as u16,
                    proto: row.get(6)?,
                    p: row.get(8)?,
                    ct: row.get(9)?,
                    req_sz: row.get::<_, i64>(10)? as usize,
                    res_sz: row.get::<_, i64>(11)? as usize,
                    dur: row.get::<_, i64>(12)? as u64,
                    cip: row.get(13)?,
                    capp: row.get(14)?,
                    cpid: row.get::<_, Option<i32>>(15)?.map(|v| v as u32),
                    flags: row.get::<_, i32>(16)? as u32,
                    fc: row.get::<_, i64>(17)? as usize,
                    ss: socket_status,
                    st: format_timestamp_ms(row.get::<_, i64>(2)? as u64),
                    et: {
                        let ts = row.get::<_, i64>(2)? as u64;
                        let dur = row.get::<_, i64>(12)? as u64;
                        if dur > 0 {
                            Some(format_timestamp_ms(ts + dur))
                        } else {
                            None
                        }
                    },
                    rc,
                    rp,
                })
            })
            .map(|r| r.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        let has_more = records.len() >= params.limit.unwrap_or(100);

        let (next_cursor, prev_cursor) = if records.is_empty() {
            (None, None)
        } else {
            match params.direction {
                Direction::Forward => (
                    records.last().map(|r| r.seq),
                    records.first().map(|r| r.seq),
                ),
                Direction::Backward => (
                    records.last().map(|r| r.seq),
                    records.first().map(|r| r.seq),
                ),
            }
        };

        let total = self.count_with_conn(&conn, params);

        QueryResult {
            records,
            next_cursor,
            prev_cursor,
            has_more,
            total,
            server_sequence: self.current_sequence.load(Ordering::Relaxed),
        }
    }

    fn count_with_conn(
        &self,
        conn: &parking_lot::MutexGuard<'_, Connection>,
        params: &QueryParams,
    ) -> usize {
        let (sql, values) = params.build_count_sql();
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();

        conn.query_row(&sql, param_refs.as_slice(), |row| row.get::<_, i64>(0))
            .map(|v| v as usize)
            .unwrap_or(0)
    }

    pub fn get_by_id(&self, id: &str) -> Option<TrafficRecord> {
        {
            let cache = self.recent_cache.read();
            if let Some(record) = cache.peek(id) {
                return Some(record.clone());
            }
        }
        self.get_by_id_from_db(id)
    }

    fn get_by_id_from_db(&self, id: &str) -> Option<TrafficRecord> {
        let conn = self.read_conn.lock();

        conn.query_row("SELECT * FROM traffic_records WHERE id = ?", [id], |row| {
            Self::row_to_record(row)
        })
        .optional()
        .ok()
        .flatten()
    }

    pub fn get_by_ids(&self, ids: &[&str]) -> Vec<TrafficSummaryCompact> {
        if ids.is_empty() {
            return vec![];
        }

        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT sequence, id, timestamp, host, method, status, protocol, \
             url, path, content_type, request_size, response_size, duration_ms, \
             client_ip, client_app, client_pid, flags, frame_count, socket_status_blob, \
             matched_rules_blob \
             FROM traffic_records WHERE id IN ({}) ORDER BY sequence DESC",
            placeholders.join(",")
        );

        let conn = self.read_conn.lock();
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

        stmt.query_map(params.as_slice(), |row| {
            let socket_blob: Option<Vec<u8>> = row.get(18)?;
            let socket_status: Option<SocketStatus> =
                socket_blob.and_then(|b| bincode::deserialize(&b).ok());

            let rules_blob: Option<Vec<u8>> = row.get(19)?;
            let matched_rules: Vec<crate::traffic::MatchedRule> = rules_blob
                .and_then(|b| bincode::deserialize(&b).ok())
                .unwrap_or_default();
            let rc = matched_rules.len();
            let rp: Vec<String> = matched_rules
                .iter()
                .map(|r| r.protocol.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            Ok(TrafficSummaryCompact {
                seq: row.get::<_, i64>(0)? as u64,
                id: row.get(1)?,
                ts: row.get::<_, i64>(2)? as u64,
                h: row.get(3)?,
                m: row.get(4)?,
                s: row.get::<_, i32>(5)? as u16,
                proto: row.get(6)?,
                p: row.get(8)?,
                ct: row.get(9)?,
                req_sz: row.get::<_, i64>(10)? as usize,
                res_sz: row.get::<_, i64>(11)? as usize,
                dur: row.get::<_, i64>(12)? as u64,
                cip: row.get(13)?,
                capp: row.get(14)?,
                cpid: row.get::<_, Option<i32>>(15)?.map(|v| v as u32),
                flags: row.get::<_, i32>(16)? as u32,
                fc: row.get::<_, i64>(17)? as usize,
                ss: socket_status,
                st: format_timestamp_ms(row.get::<_, i64>(2)? as u64),
                et: {
                    let ts = row.get::<_, i64>(2)? as u64;
                    let dur = row.get::<_, i64>(12)? as u64;
                    if dur > 0 {
                        Some(format_timestamp_ms(ts + dur))
                    } else {
                        None
                    }
                },
                rc,
                rp,
            })
        })
        .map(|r| r.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<TrafficRecord> {
        let timing_blob: Option<Vec<u8>> = row.get("timing_blob")?;
        let req_headers_blob: Option<Vec<u8>> = row.get("request_headers_blob")?;
        let res_headers_blob: Option<Vec<u8>> = row.get("response_headers_blob")?;
        let rules_blob: Option<Vec<u8>> = row.get("matched_rules_blob")?;
        let socket_blob: Option<Vec<u8>> = row.get("socket_status_blob")?;
        let req_body_blob: Option<Vec<u8>> = row.get("request_body_ref_blob")?;
        let res_body_blob: Option<Vec<u8>> = row.get("response_body_ref_blob")?;
        let orig_req_headers_blob: Option<Vec<u8>> = row.get("original_request_headers_blob")?;
        let actual_res_headers_blob: Option<Vec<u8>> = row.get("actual_response_headers_blob")?;
        let req_script_results_blob: Option<Vec<u8>> = row.get("req_script_results_blob")?;
        let res_script_results_blob: Option<Vec<u8>> = row.get("res_script_results_blob")?;

        let flags: i32 = row.get("flags")?;

        Ok(TrafficRecord {
            sequence: row.get::<_, i64>("sequence")? as u64,
            id: row.get("id")?,
            timestamp: row.get::<_, i64>("timestamp")? as u64,
            host: row.get("host")?,
            method: row.get("method")?,
            status: row.get::<_, i32>("status")? as u16,
            protocol: row.get("protocol")?,
            url: row.get("url")?,
            path: row.get("path")?,
            content_type: row.get("content_type")?,
            request_content_type: row.get("request_content_type")?,
            request_size: row.get::<_, i64>("request_size")? as usize,
            response_size: row.get::<_, i64>("response_size")? as usize,
            duration_ms: row.get::<_, i64>("duration_ms")? as u64,
            client_ip: row.get("client_ip")?,
            client_app: row.get("client_app")?,
            client_pid: row.get::<_, Option<i32>>("client_pid")?.map(|v| v as u32),
            client_path: row.get("client_path")?,
            is_tunnel: flags & 1 != 0,
            is_websocket: flags & 2 != 0,
            is_sse: flags & 4 != 0,
            is_h3: flags & 8 != 0,
            has_rule_hit: flags & 16 != 0,
            is_replay: flags & 32 != 0,
            frame_count: row.get::<_, i64>("frame_count")? as usize,
            last_frame_id: row.get::<_, i64>("last_frame_id")? as u64,
            timing: timing_blob.and_then(|b| bincode::deserialize(&b).ok()),
            request_headers: req_headers_blob.and_then(|b| bincode::deserialize(&b).ok()),
            response_headers: res_headers_blob.and_then(|b| bincode::deserialize(&b).ok()),
            matched_rules: rules_blob.and_then(|b| bincode::deserialize(&b).ok()),
            socket_status: socket_blob.and_then(|b| bincode::deserialize(&b).ok()),
            request_body_ref: req_body_blob.and_then(|b| bincode::deserialize(&b).ok()),
            response_body_ref: res_body_blob.and_then(|b| bincode::deserialize(&b).ok()),
            actual_url: row.get("actual_url")?,
            actual_host: row.get("actual_host")?,
            original_request_headers: orig_req_headers_blob
                .and_then(|b| bincode::deserialize(&b).ok()),
            actual_response_headers: actual_res_headers_blob
                .and_then(|b| bincode::deserialize(&b).ok()),
            error_message: row.get("error_message")?,
            req_script_results: req_script_results_blob.and_then(|b| bincode::deserialize(&b).ok()),
            res_script_results: res_script_results_blob.and_then(|b| bincode::deserialize(&b).ok()),
        })
    }

    pub fn clear(&self) {
        self.clear_with_active_ids(&[]);
    }

    pub fn clear_with_active_ids(&self, active_connection_ids: &[String]) {
        let conn = self.write_conn.lock();

        let active_ids_set: std::collections::HashSet<&str> =
            active_connection_ids.iter().map(|s| s.as_str()).collect();

        let pending_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM traffic_records WHERE status = 0",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        tracing::info!(
            pending = pending_count,
            active_connections = active_connection_ids.len(),
            "[TRAFFIC_DB] Clearing traffic records, preserving active"
        );

        if active_connection_ids.is_empty() {
            if let Err(e) = conn.execute("DELETE FROM traffic_records WHERE status != 0", []) {
                tracing::error!(error = %e, "[TRAFFIC_DB] Failed to clear completed records");
            }
        } else {
            let placeholders: String = active_connection_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            let sql = format!(
                "DELETE FROM traffic_records WHERE status != 0 AND id NOT IN ({})",
                placeholders
            );

            if let Err(e) = conn.execute(
                &sql,
                rusqlite::params_from_iter(active_connection_ids.iter()),
            ) {
                tracing::error!(error = %e, "[TRAFFIC_DB] Failed to clear completed records");
            }
        }

        let mut cache = self.recent_cache.write();
        let preserved_ids: Vec<String> = cache
            .iter()
            .filter(|(id, record)| {
                record.status == 0
                    || active_ids_set.contains(id.as_str())
                    || (record.is_websocket
                        && record.socket_status.as_ref().is_some_and(|s| s.is_open))
            })
            .map(|(k, _)| k.clone())
            .collect();

        let mut new_cache = LruCache::new(
            std::num::NonZeroUsize::new(cache.cap().get())
                .unwrap_or(std::num::NonZeroUsize::new(1000).unwrap()),
        );
        for id in preserved_ids {
            if let Some(record) = cache.pop(&id) {
                new_cache.put(id, record);
            }
        }
        *cache = new_cache;

        tracing::info!("[TRAFFIC_DB] Traffic records cleared (active preserved)");
    }

    fn maybe_cleanup(&self, conn: &Connection) {
        let max = self.max_records.load(Ordering::Relaxed);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM traffic_records", [], |row| row.get(0))
            .unwrap_or(0);

        if count as usize > max {
            let excess = count as usize - max;
            let result = conn.execute(
                "DELETE FROM traffic_records WHERE sequence IN (
                    SELECT sequence FROM traffic_records ORDER BY sequence ASC LIMIT ?
                )",
                [excess as i64],
            );

            if let Ok(deleted) = result {
                if deleted > 0 {
                    tracing::debug!(
                        deleted = deleted,
                        max = max,
                        "[TRAFFIC_DB] Cleaned up old records"
                    );
                }
            }
        }
    }

    pub fn cleanup_expired_records(&self) -> usize {
        let retention_hours = self.retention_hours.load(Ordering::Relaxed);
        let retention_ms = retention_hours * 60 * 60 * 1000;
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let cutoff = now.saturating_sub(retention_ms);

        let conn = self.write_conn.lock();
        let result = conn.execute(
            "DELETE FROM traffic_records WHERE timestamp < ?",
            [cutoff as i64],
        );

        match result {
            Ok(deleted) => {
                if deleted > 0 {
                    tracing::info!(
                        deleted = deleted,
                        retention_hours = retention_hours,
                        "[TRAFFIC_DB] Cleaned up expired records"
                    );
                }
                deleted
            }
            Err(e) => {
                tracing::error!(error = %e, "[TRAFFIC_DB] Failed to cleanup expired records");
                0
            }
        }
    }

    pub fn count(&self) -> usize {
        let conn = self.read_conn.lock();
        conn.query_row("SELECT COUNT(*) FROM traffic_records", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|v| v as usize)
        .unwrap_or(0)
    }

    pub fn stats(&self) -> TrafficDbStats {
        let count = self.count();
        let db_size = fs::metadata(&self.db_path).map(|m| m.len()).unwrap_or(0);

        let conn = self.read_conn.lock();
        let oldest: Option<u64> = conn
            .query_row("SELECT MIN(timestamp) FROM traffic_records", [], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .ok()
            .flatten()
            .map(|v| v as u64);

        let newest: Option<u64> = conn
            .query_row("SELECT MAX(timestamp) FROM traffic_records", [], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .ok()
            .flatten()
            .map(|v| v as u64);

        TrafficDbStats {
            record_count: count,
            db_size,
            db_path: self.db_path.display().to_string(),
            max_records: self.max_records.load(Ordering::Relaxed),
            retention_hours: self.retention_hours.load(Ordering::Relaxed),
            current_sequence: self.current_sequence.load(Ordering::Relaxed),
            oldest_timestamp: oldest,
            newest_timestamp: newest,
        }
    }

    pub fn current_sequence(&self) -> u64 {
        self.current_sequence.load(Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TrafficRecord> {
        self.tx.subscribe()
    }

    pub fn set_max_records(&self, max: usize) {
        let old = self.max_records.swap(max, Ordering::SeqCst);
        if old != max {
            tracing::info!(old = old, new = max, "[TRAFFIC_DB] Max records updated");
            let conn = self.write_conn.lock();
            self.maybe_cleanup(&conn);
        }
    }

    pub fn set_retention_hours(&self, hours: u64) {
        let old = self.retention_hours.swap(hours, Ordering::SeqCst);
        if old != hours {
            tracing::info!(
                old = old,
                new = hours,
                "[TRAFFIC_DB] Retention hours updated"
            );
        }
    }

    pub fn checkpoint(&self) {
        let conn = self.write_conn.lock();
        if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)") {
            tracing::warn!(error = %e, "[TRAFFIC_DB] WAL checkpoint failed");
        }
    }
}

fn format_timestamp_ms(timestamp_ms: u64) -> String {
    use chrono::{Local, TimeZone};
    let secs = (timestamp_ms / 1000) as i64;
    let nanos = ((timestamp_ms % 1000) * 1_000_000) as u32;
    Local
        .timestamp_opt(secs, nanos)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub fn start_db_cleanup_task(store: SharedTrafficDbStore) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let deleted = store.cleanup_expired_records();
            if deleted > 0 {
                store.checkpoint();
            }
        }
    });
}
