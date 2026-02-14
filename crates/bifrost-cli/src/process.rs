use std::path::PathBuf;

use crate::config::get_bifrost_dir;

pub fn get_pid_file() -> bifrost_core::Result<PathBuf> {
    Ok(get_bifrost_dir()?.join("bifrost.pid"))
}

pub fn read_pid() -> Option<u32> {
    let pid_file = get_pid_file().ok()?;
    std::fs::read_to_string(&pid_file)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn write_pid(pid: u32) -> bifrost_core::Result<()> {
    let pid_file = get_pid_file()?;
    if let Some(parent) = pid_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&pid_file, pid.to_string())?;
    Ok(())
}

pub fn remove_pid() -> bifrost_core::Result<()> {
    let pid_file = get_pid_file()?;
    if pid_file.exists() {
        std::fs::remove_file(&pid_file)?;
    }
    Ok(())
}

#[cfg(unix)]
pub fn is_process_running(pid: u32) -> bool {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGCONT).is_ok()
}

#[cfg(not(unix))]
pub fn is_process_running(_pid: u32) -> bool {
    false
}
