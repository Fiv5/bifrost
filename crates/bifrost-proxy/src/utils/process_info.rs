use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, trace, warn};

#[derive(Debug, Clone)]
pub struct ClientProcess {
    pub pid: u32,
    pub name: String,
    pub path: Option<String>,
}

impl ClientProcess {
    pub fn display_name(&self) -> String {
        self.name.clone()
    }
}

struct CachedProcess {
    process: Option<ClientProcess>,
    expires_at: Instant,
}

struct SocketSnapshot {
    ports_to_pids: HashMap<u16, u32>,
    expires_at: Instant,
}

pub struct ProcessResolver {
    cache: RwLock<HashMap<u16, CachedProcess>>,
    pid_cache: RwLock<HashMap<u32, CachedProcess>>,
    socket_snapshot: RwLock<Option<SocketSnapshot>>,
    cache_ttl: Duration,
    negative_cache_ttl: Duration,
    socket_snapshot_ttl: Duration,
}

impl Default for ProcessResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessResolver {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            pid_cache: RwLock::new(HashMap::new()),
            socket_snapshot: RwLock::new(None),
            cache_ttl: Duration::from_secs(30),
            negative_cache_ttl: Duration::from_secs(5),
            socket_snapshot_ttl: Duration::from_millis(250),
        }
    }

    pub fn resolve(&self, peer_addr: &SocketAddr) -> Option<ClientProcess> {
        let port = peer_addr.port();

        if let Some(cached) = self.get_from_cache(port) {
            return cached;
        }

        let process = self.lookup_process(peer_addr);

        self.update_cache(port, process.clone());

        process
    }

    pub fn resolve_cached(&self, peer_addr: &SocketAddr) -> Option<ClientProcess> {
        self.get_from_cache(peer_addr.port()).flatten()
    }

    pub fn resolve_with_retry(
        &self,
        peer_addr: &SocketAddr,
        max_retries: u32,
        delay_ms: u64,
    ) -> Option<ClientProcess> {
        let port = peer_addr.port();

        for attempt in 0..=max_retries {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }

            let process = self.lookup_process(peer_addr);
            if process.is_some() {
                self.update_cache(port, process.clone());
                return process;
            }
        }

        self.update_cache(port, None);
        None
    }

    pub async fn resolve_async(
        &self,
        peer_addr: SocketAddr,
        max_retries: u32,
        delay_ms: u64,
    ) -> Option<ClientProcess> {
        if let Some(cached) = self.get_from_cache(peer_addr.port()) {
            return cached;
        }

        if !peer_addr.ip().is_loopback() {
            return None;
        }

        // Keep the method for tests and low-frequency callers. Production async lookups
        // go through the static wrappers below so they can reuse the shared resolver caches
        // from a blocking worker without rebuilding socket snapshots.
        self.resolve_with_retry(&peer_addr, max_retries, delay_ms)
    }

    fn get_from_cache(&self, port: u16) -> Option<Option<ClientProcess>> {
        let cache = self.cache.read().ok()?;
        if let Some(cached) = cache.get(&port) {
            if cached.expires_at > Instant::now() {
                trace!(port = port, "Process info cache hit");
                return Some(cached.process.clone());
            }
        }
        None
    }

    fn update_cache(&self, port: u16, process: Option<ClientProcess>) {
        if let Ok(mut cache) = self.cache.write() {
            let ttl = if process.is_some() {
                self.cache_ttl
            } else {
                self.negative_cache_ttl
            };
            cache.insert(
                port,
                CachedProcess {
                    process,
                    expires_at: Instant::now() + ttl,
                },
            );

            if cache.len() > 10000 {
                let now = Instant::now();
                cache.retain(|_, v| v.expires_at > now);
            }
        }
    }

    fn lookup_process(&self, peer_addr: &SocketAddr) -> Option<ClientProcess> {
        let pid = self.lookup_pid(peer_addr)?;
        self.lookup_cached_process_by_pid(pid)
    }

    fn lookup_pid(&self, peer_addr: &SocketAddr) -> Option<u32> {
        let port = peer_addr.port();
        let now = Instant::now();

        if let Ok(snapshot_guard) = self.socket_snapshot.read() {
            if let Some(snapshot) = snapshot_guard.as_ref() {
                if snapshot.expires_at > now {
                    return snapshot.ports_to_pids.get(&port).copied();
                }
            }
        }

        let ports_to_pids = lookup_socket_pid_map();
        let pid = ports_to_pids.get(&port).copied();

        if let Ok(mut snapshot_guard) = self.socket_snapshot.write() {
            *snapshot_guard = Some(SocketSnapshot {
                ports_to_pids,
                expires_at: now + self.socket_snapshot_ttl,
            });
        }

        pid
    }

    fn lookup_cached_process_by_pid(&self, pid: u32) -> Option<ClientProcess> {
        let now = Instant::now();

        if let Ok(cache) = self.pid_cache.read() {
            if let Some(cached) = cache.get(&pid) {
                if cached.expires_at > now {
                    trace!(pid = pid, "Process info pid cache hit");
                    return cached.process.clone();
                }
            }
        }

        let (name, path) = get_process_info(pid);
        let process = Some(ClientProcess { pid, name, path });

        if let Ok(mut cache) = self.pid_cache.write() {
            cache.insert(
                pid,
                CachedProcess {
                    process: process.clone(),
                    expires_at: now + self.cache_ttl,
                },
            );
            if cache.len() > 10000 {
                cache.retain(|_, v| v.expires_at > now);
            }
        }

        process
    }

    pub fn cleanup_expired(&self) {
        if let Ok(mut cache) = self.cache.write() {
            let now = Instant::now();
            let before = cache.len();
            cache.retain(|_, v| v.expires_at > now);
            let after = cache.len();
            if before != after {
                debug!(
                    removed = before - after,
                    remaining = after,
                    "Cleaned up expired process cache entries"
                );
            }
        }

        if let Ok(mut cache) = self.pid_cache.write() {
            cache.retain(|_, v| v.expires_at > Instant::now());
        }
    }
}

#[cfg(target_os = "macos")]
fn get_process_info(pid: u32) -> (String, Option<String>) {
    let name = get_process_name_macos(pid).unwrap_or_else(|| format!("PID:{}", pid));
    let path = get_process_path_macos(pid);
    (name, path)
}

#[cfg(target_os = "macos")]
fn get_process_name_macos(pid: u32) -> Option<String> {
    use std::ffi::CStr;

    let mut buffer = [0u8; 1024];
    let len = unsafe {
        libc::proc_name(
            pid as i32,
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len() as u32,
        )
    };

    if len > 0 {
        CStr::from_bytes_until_nul(&buffer[..])
            .ok()
            .map(|s| s.to_string_lossy().into_owned())
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn get_process_path_macos(pid: u32) -> Option<String> {
    use std::ffi::CStr;

    let mut buffer = [0u8; 4096];
    let len = unsafe {
        libc::proc_pidpath(
            pid as i32,
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len() as u32,
        )
    };

    if len > 0 {
        CStr::from_bytes_until_nul(&buffer[..])
            .ok()
            .map(|s| s.to_string_lossy().into_owned())
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn get_process_info(pid: u32) -> (String, Option<String>) {
    let path = get_process_path_windows(pid);
    let name = path
        .as_ref()
        .and_then(|p| {
            std::path::Path::new(p)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| format!("PID:{}", pid));
    (name, path)
}

#[cfg(target_os = "windows")]
fn get_process_path_windows(pid: u32) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut buffer = vec![0u16; 1024];
        let mut size = buffer.len() as u32;

        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );

        let _ = CloseHandle(handle);

        if result.is_ok() {
            let path = OsString::from_wide(&buffer[..size as usize]);
            path.into_string().ok()
        } else {
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn get_process_info(pid: u32) -> (String, Option<String>) {
    let path = get_process_path_linux(pid);
    let name = get_process_name_linux(pid).unwrap_or_else(|| {
        path.as_ref()
            .and_then(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| format!("PID:{}", pid))
    });
    (name, path)
}

#[cfg(target_os = "linux")]
fn get_process_name_linux(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{}/comm", pid))
        .ok()
        .map(|s| s.trim().to_string())
}

#[cfg(target_os = "linux")]
fn get_process_path_linux(pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{}/exe", pid))
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn get_process_info(_pid: u32) -> (String, Option<String>) {
    ("Unknown".to_string(), None)
}

lazy_static::lazy_static! {
    pub static ref PROCESS_RESOLVER: ProcessResolver = ProcessResolver::new();
}

static BACKGROUND_PROCESS_RESOLUTION_CONCURRENCY: std::sync::LazyLock<usize> =
    std::sync::LazyLock::new(|| {
        std::env::var("BIFROST_BACKGROUND_PROCESS_RESOLUTION_CONCURRENCY")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(8)
    });

static BACKGROUND_PROCESS_RESOLUTION_SEMAPHORE: std::sync::LazyLock<Semaphore> =
    std::sync::LazyLock::new(|| Semaphore::new(*BACKGROUND_PROCESS_RESOLUTION_CONCURRENCY));

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn lookup_socket_pid_map() -> HashMap<u16, u32> {
    use netstat2::{
        get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState,
    };

    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP;

    let sockets = match get_sockets_info(af_flags, proto_flags) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "Failed to get socket info");
            return HashMap::new();
        }
    };

    let mut ports_to_pids = HashMap::new();
    for socket in sockets {
        if let ProtocolSocketInfo::Tcp(tcp) = socket.protocol_socket_info {
            if matches!(
                tcp.state,
                TcpState::Established
                    | TcpState::SynSent
                    | TcpState::SynReceived
                    | TcpState::FinWait1
                    | TcpState::FinWait2
                    | TcpState::CloseWait
                    | TcpState::LastAck
            ) {
                if let Some(&pid) = socket.associated_pids.first() {
                    ports_to_pids.entry(tcp.local_port).or_insert(pid);
                }
            }
        }
    }

    debug!(
        socket_count = ports_to_pids.len(),
        "Refreshed client socket pid snapshot"
    );
    ports_to_pids
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn lookup_socket_pid_map() -> HashMap<u16, u32> {
    HashMap::new()
}

pub fn resolve_client_process(peer_addr: &SocketAddr) -> Option<ClientProcess> {
    PROCESS_RESOLVER.resolve(peer_addr)
}

pub fn resolve_client_process_cached(peer_addr: &SocketAddr) -> Option<ClientProcess> {
    PROCESS_RESOLVER.resolve_cached(peer_addr)
}

pub fn resolve_client_process_with_retry(
    peer_addr: &SocketAddr,
    max_retries: u32,
    delay_ms: u64,
) -> Option<ClientProcess> {
    PROCESS_RESOLVER.resolve_with_retry(peer_addr, max_retries, delay_ms)
}

pub async fn resolve_client_process_async(peer_addr: &SocketAddr) -> Option<ClientProcess> {
    resolve_client_process_async_with_retry(peer_addr, 3, 10).await
}

pub async fn resolve_client_process_async_with_retry(
    peer_addr: &SocketAddr,
    max_retries: u32,
    delay_ms: u64,
) -> Option<ClientProcess> {
    if let Some(cached) = PROCESS_RESOLVER.resolve_cached(peer_addr) {
        return Some(cached);
    }

    if !peer_addr.ip().is_loopback() {
        return None;
    }

    let peer_addr = *peer_addr;
    match tokio::task::spawn_blocking(move || {
        PROCESS_RESOLVER.resolve_with_retry(&peer_addr, max_retries, delay_ms)
    })
    .await
    {
        Ok(process) => process,
        Err(err) => {
            warn!(peer_addr = %peer_addr, error = %err, "Async process resolution task failed");
            None
        }
    }
}

pub fn spawn_async_process_resolver<F>(peer_addr: SocketAddr, record_id: String, callback: F)
where
    F: FnOnce(String, ClientProcess) + Send + 'static,
{
    tokio::spawn(async move {
        let permit = match BACKGROUND_PROCESS_RESOLUTION_SEMAPHORE.acquire().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        // Defer a beat so the socket table has time to reflect the accepted connection,
        // then do a single lookup instead of retrying on the hot path.
        tokio::time::sleep(Duration::from_millis(25)).await;
        let result =
            tokio::task::spawn_blocking(move || PROCESS_RESOLVER.resolve(&peer_addr)).await;
        drop(permit);

        if let Ok(Some(process)) = result {
            callback(record_id, process);
        }
    });
}

pub fn format_client_info(peer_addr: &SocketAddr, process: Option<&ClientProcess>) -> String {
    match process {
        Some(p) => p.display_name(),
        None => peer_addr.ip().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_format_client_info_with_process() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
        let process = ClientProcess {
            pid: 1234,
            name: "Chrome".to_string(),
            path: Some("/Applications/Chrome.app".to_string()),
        };
        let result = format_client_info(&addr, Some(&process));
        assert_eq!(result, "Chrome");
    }

    #[test]
    fn test_format_client_info_without_process() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345);
        let result = format_client_info(&addr, None);
        assert_eq!(result, "192.168.1.100");
    }

    #[test]
    fn test_process_resolver_cache() {
        let resolver = ProcessResolver::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 54321);

        let _ = resolver.resolve(&addr);

        let cached = resolver.get_from_cache(54321);
        assert!(cached.is_some());
    }

    #[test]
    fn test_process_resolver_cached_lookup_miss() {
        let resolver = ProcessResolver::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 54321);

        let cached = resolver.resolve_cached(&addr);
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_process_resolver_async_returns_cached_hit() {
        let resolver = ProcessResolver::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 54321);
        let process = ClientProcess {
            pid: 1234,
            name: "Chrome".to_string(),
            path: Some("/Applications/Chrome.app".to_string()),
        };

        resolver.update_cache(addr.port(), Some(process.clone()));

        let resolved = resolver.resolve_async(addr, 3, 10).await;
        assert_eq!(resolved.as_ref().map(|p| p.name.as_str()), Some("Chrome"));
        assert_eq!(resolved.as_ref().map(|p| p.pid), Some(1234));
    }

    #[test]
    fn test_process_resolver_retry_caches_miss() {
        let resolver = ProcessResolver::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1);

        let resolved = resolver.resolve_with_retry(&addr, 0, 0);
        assert!(resolved.is_none());
        assert!(matches!(resolver.get_from_cache(addr.port()), Some(None)));
    }
}
