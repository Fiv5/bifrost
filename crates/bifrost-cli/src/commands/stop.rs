use bifrost_storage::set_data_dir;

use crate::config::get_bifrost_dir;
use crate::process::{is_process_running, read_pid, read_runtime_info, remove_pid};

fn cleanup_proxy_state(bifrost_dir: &std::path::Path) {
    if let Err(e) = bifrost_core::SystemProxyManager::recover_from_crash(bifrost_dir) {
        eprintln!("Failed to recover system proxy: {}", e);
    }

    ensure_system_proxy_disabled(bifrost_dir);

    if let Err(e) = bifrost_core::ShellProxyManager::recover_from_crash(bifrost_dir) {
        eprintln!("Failed to recover CLI proxy: {}", e);
    }
    let mut shell_manager = bifrost_core::ShellProxyManager::new(bifrost_dir.to_path_buf());
    let _ = shell_manager.disable_persistent();
}

fn ensure_system_proxy_disabled(bifrost_dir: &std::path::Path) {
    if !bifrost_core::SystemProxyManager::is_supported() {
        return;
    }

    let runtime_port = read_runtime_info().map(|info| info.port);

    let current = match bifrost_core::SystemProxyManager::get_current() {
        Ok(c) => c,
        Err(_) => return,
    };

    if !current.enable {
        return;
    }

    let is_bifrost_proxy = match runtime_port {
        Some(port) => current.port == port,
        None => {
            let host = &current.host;
            (host == "127.0.0.1" || host == "localhost" || host == "::1") && current.port > 0
        }
    };

    if !is_bifrost_proxy {
        return;
    }

    let mut manager = bifrost_core::SystemProxyManager::new(bifrost_dir.to_path_buf());
    if let Err(e) = manager.force_disable() {
        eprintln!("Failed to disable system proxy: {}", e);
    } else {
        println!("System proxy disabled.");
    }
}

pub fn run_stop() -> bifrost_core::Result<()> {
    let bifrost_dir = get_bifrost_dir()?;
    set_data_dir(bifrost_dir.clone());

    let pid = read_pid().ok_or_else(|| {
        bifrost_core::BifrostError::NotFound("No PID file found. Is the proxy running?".to_string())
    })?;

    if !is_process_running(pid) {
        cleanup_proxy_state(&bifrost_dir);
        remove_pid()?;
        println!("Bifrost proxy is not running (stale PID file removed).");
        return Ok(());
    }

    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        println!("Stopping Bifrost proxy (PID: {})...", pid);
        kill(Pid::from_raw(pid as i32), Signal::SIGTERM).map_err(|e| {
            bifrost_core::BifrostError::Config(format!("Failed to send SIGTERM: {}", e))
        })?;

        for i in 0..300 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if !is_process_running(pid) {
                cleanup_proxy_state(&bifrost_dir);
                remove_pid()?;
                println!("Bifrost proxy stopped.");
                return Ok(());
            }
            if i == 250 {
                println!("Sending SIGKILL...");
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }
        }

        cleanup_proxy_state(&bifrost_dir);
        remove_pid()?;
        println!("Bifrost proxy stopped (forced).");
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
            PROCESS_TERMINATE,
        };

        println!("Stopping Bifrost proxy (PID: {})...", pid);

        let handle = unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, pid as u32) };

        if handle.is_null() {
            eprintln!(
                "Failed to open process (PID: {}). It may have already exited.",
                pid
            );
            cleanup_proxy_state(&bifrost_dir);
            remove_pid()?;
        } else {
            let terminated = unsafe { TerminateProcess(handle, 1) };
            if terminated != 0 {
                unsafe {
                    WaitForSingleObject(handle, 5000);
                }
                cleanup_proxy_state(&bifrost_dir);
                remove_pid()?;
                println!("Bifrost proxy stopped.");
            } else {
                eprintln!("Failed to terminate process (PID: {}).", pid);
                cleanup_proxy_state(&bifrost_dir);
            }
            unsafe {
                CloseHandle(handle);
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        println!("Stop command is not supported on this platform.");
        println!("Please terminate the process manually (PID: {}).", pid);
    }

    Ok(())
}
