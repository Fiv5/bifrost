use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tracing::warn;

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::metrics::SystemInfo;
use crate::state::SharedAdminState;

pub async fn handle_system(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();
    let query = req.uri().query();

    match path {
        "/api/system" | "/api/system/" => match method {
            Method::GET => get_system_info(state).await,
            _ => method_not_allowed(),
        },
        "/api/system/overview" => match method {
            Method::GET => get_overview(state).await,
            _ => method_not_allowed(),
        },
        "/api/system/memory" | "/api/system/memory/" => match method {
            Method::GET => get_memory_diagnostics(state).await,
            _ => method_not_allowed(),
        },
        "/api/system/version-check" => match method {
            Method::GET => check_version(state, query).await,
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_system_info(state: SharedAdminState) -> Response<BoxBody> {
    let info = SystemInfo::new(state.start_time);
    json_response(&info)
}

#[derive(Debug, serde::Serialize)]
struct ProcessMemoryInfo {
    pid: u32,
    /// 进程 RSS（KiB），来自 sysinfo
    rss_kib: u64,
    /// 进程虚拟内存（KiB），来自 sysinfo
    vms_kib: u64,
    /// 进程 CPU 使用率（%），来自 sysinfo
    cpu_usage_percent: f32,
    /// 系统总内存（KiB），来自 sysinfo
    system_total_kib: u64,
}

#[derive(Debug, serde::Serialize)]
struct AdminMemoryDiagnostics {
    system: SystemInfo,
    process: ProcessMemoryInfo,
    traffic_db: Option<serde_json::Value>,
    connections: serde_json::Value,
    stores: serde_json::Value,
}

async fn get_memory_diagnostics(state: SharedAdminState) -> Response<BoxBody> {
    // 进程级信息：这里做一次“即时刷新”，避免仅依赖 metrics 缓存。
    let pid_u32 = std::process::id();
    let pid = Pid::from_u32(pid_u32);
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]));

    let (rss_kib, vms_kib, cpu_usage_percent) = if let Some(p) = system.process(pid) {
        (p.memory(), p.virtual_memory(), p.cpu_usage())
    } else {
        warn!(pid = pid_u32, "[SYSTEM] sysinfo missing process info");
        (0, 0, 0.0)
    };

    let process = ProcessMemoryInfo {
        pid: pid_u32,
        rss_kib,
        vms_kib,
        cpu_usage_percent,
        system_total_kib: system.total_memory(),
    };

    let traffic_db = state.traffic_db_store.as_ref().map(|db| {
        let stats = db.stats();
        let cache = db.recent_cache_stats();
        serde_json::json!({
            "db": stats,
            "recent_cache": cache,
        })
    });

    // 连接/活跃对象：这些通常是“管理端页面关闭后依然可能占用内存”的主要线索。
    let ws_monitor_stats = state.connection_monitor.memory_stats();
    let tunnel_registry_active = state.connection_registry.active_count();
    let sse_total = state.sse_hub.connection_count();
    let sse_open = state.sse_hub.open_connection_count();

    let connections = serde_json::json!({
        "tunnel_registry_active": tunnel_registry_active,
        "ws_monitor": ws_monitor_stats,
        "sse": {
            "connections": sse_total,
            "open": sse_open,
        }
    });

    // 存储/缓存（主要是“内存侧的 pending/buffer”与“磁盘侧 size”同时给出，便于区分 RSS vs 磁盘占用）。
    let body_store = state.body_store.as_ref().map(|s| s.read().stats());
    let frame_store_stats = state.frame_store.as_ref().map(|s| {
        serde_json::json!({
            "disk": s.stats(),
            "memory": s.memory_stats(),
        })
    });
    let ws_payload_store_stats = state.ws_payload_store.as_ref().map(|s| {
        serde_json::json!({
            "disk": s.stats(),
            "memory": s.memory_stats(),
        })
    });

    let stores = serde_json::json!({
        "body_store": body_store,
        "frame_store": frame_store_stats,
        "ws_payload_store": ws_payload_store_stats,
        "max_body_buffer_size": state.get_max_body_buffer_size(),
        "max_body_probe_size": state.get_max_body_probe_size(),
    });

    let out = AdminMemoryDiagnostics {
        system: SystemInfo::new(state.start_time),
        process,
        traffic_db,
        connections,
        stores,
    };

    json_response(&out)
}

async fn get_overview(state: SharedAdminState) -> Response<BoxBody> {
    let system_info = SystemInfo::new(state.start_time);
    let metrics = state.metrics_collector.get_current();
    let traffic_count = if let Some(ref db_store) = state.traffic_db_store {
        db_store.stats().record_count
    } else {
        0
    };

    let (rules_total, rules_enabled) = match state.rules_storage.load_all() {
        Ok(rules) => {
            let enabled = rules.iter().filter(|r| r.enabled).count();
            (rules.len(), enabled)
        }
        Err(_) => (0, 0),
    };

    let pending_count = if let Some(ref access_control) = state.access_control {
        let ac = access_control.read().await;
        ac.pending_authorization_count()
    } else {
        0
    };

    let overview = serde_json::json!({
        "system": system_info,
        "metrics": metrics,
        "rules": {
            "total": rules_total,
            "enabled": rules_enabled
        },
        "traffic": {
            "recorded": traffic_count
        },
        "server": {
            "port": state.port(),
            "admin_url": format!("http://127.0.0.1:{}/_bifrost/", state.port())
        },
        "pending_authorizations": pending_count
    });

    json_response(&overview)
}

async fn check_version(state: SharedAdminState, query: Option<&str>) -> Response<BoxBody> {
    let force_refresh = query
        .map(|q| q.contains("refresh=true") || q.contains("refresh=1"))
        .unwrap_or(false);

    let response = state.version_checker.check(force_refresh).await;
    json_response(&response)
}
