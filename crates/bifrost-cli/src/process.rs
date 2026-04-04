use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::get_bifrost_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub pid: u32,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socks5_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

pub fn get_pid_file() -> bifrost_core::Result<PathBuf> {
    Ok(get_bifrost_dir()?.join("bifrost.pid"))
}

pub fn get_runtime_file() -> bifrost_core::Result<PathBuf> {
    Ok(get_bifrost_dir()?.join("runtime.json"))
}

pub fn read_pid() -> Option<u32> {
    if let Some(info) = read_runtime_info() {
        return Some(info.pid);
    }
    let pid_file = get_pid_file().ok()?;
    std::fs::read_to_string(&pid_file)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn read_runtime_info() -> Option<RuntimeInfo> {
    let runtime_file = get_runtime_file().ok()?;
    let content = std::fs::read_to_string(&runtime_file).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn read_runtime_port() -> Option<u16> {
    read_runtime_info().map(|info| info.port)
}

pub fn write_pid(pid: u32) -> bifrost_core::Result<()> {
    let pid_file = get_pid_file()?;
    if let Some(parent) = pid_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&pid_file, pid.to_string())?;
    Ok(())
}

pub fn write_runtime_info(info: &RuntimeInfo) -> bifrost_core::Result<()> {
    let runtime_file = get_runtime_file()?;
    if let Some(parent) = runtime_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(info).map_err(std::io::Error::other)?;
    std::fs::write(&runtime_file, json)?;
    write_pid(info.pid)?;
    Ok(())
}

pub fn remove_pid() -> bifrost_core::Result<()> {
    let pid_file = get_pid_file()?;
    if pid_file.exists() {
        std::fs::remove_file(&pid_file)?;
    }
    let runtime_file = get_runtime_file()?;
    if runtime_file.exists() {
        std::fs::remove_file(&runtime_file)?;
    }
    Ok(())
}

#[cfg(unix)]
pub fn is_process_running(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let p = Pid::from_raw(pid as i32);
    if kill(p, None).is_err() {
        return false;
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{}/stat", pid)) {
            if let Some(state_start) = stat.rfind(')') {
                let after_comm = &stat[state_start + 1..];
                let state = after_comm.trim_start().chars().next().unwrap_or(' ');
                if state == 'Z' {
                    return false;
                }
            }
        }
    }

    true
}

#[cfg(not(unix))]
pub fn is_process_running(_pid: u32) -> bool {
    false
}
