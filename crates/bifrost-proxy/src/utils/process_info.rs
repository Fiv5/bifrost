use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::RwLock;
use std::time::{Duration, Instant};
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

pub struct ProcessResolver {
    cache: RwLock<HashMap<u16, CachedProcess>>,
    cache_ttl: Duration,
    negative_cache_ttl: Duration,
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
            cache_ttl: Duration::from_secs(30),
            negative_cache_ttl: Duration::from_secs(5),
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

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    fn lookup_process(&self, peer_addr: &SocketAddr) -> Option<ClientProcess> {
        use netstat2::{
            get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState,
        };

        let port = peer_addr.port();
        let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
        let proto_flags = ProtocolFlags::TCP;

        let sockets = match get_sockets_info(af_flags, proto_flags) {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "Failed to get socket info");
                return None;
            }
        };

        for socket in sockets {
            if let ProtocolSocketInfo::Tcp(tcp) = socket.protocol_socket_info {
                if tcp.local_port == port && tcp.state == TcpState::Established {
                    if let Some(&pid) = socket.associated_pids.first() {
                        let (name, path) = get_process_info(pid);
                        debug!(
                            port = port,
                            pid = pid,
                            name = %name,
                            "Resolved client process"
                        );
                        return Some(ClientProcess { pid, name, path });
                    }
                }
            }
        }

        trace!(port = port, "No process found for port");
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    fn lookup_process(&self, _peer_addr: &SocketAddr) -> Option<ClientProcess> {
        None
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

pub fn resolve_client_process(peer_addr: &SocketAddr) -> Option<ClientProcess> {
    PROCESS_RESOLVER.resolve(peer_addr)
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
}
