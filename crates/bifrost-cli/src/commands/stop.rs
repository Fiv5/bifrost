use crate::process::{is_process_running, read_pid, remove_pid};

pub fn run_stop() -> bifrost_core::Result<()> {
    let pid = read_pid().ok_or_else(|| {
        bifrost_core::BifrostError::NotFound("No PID file found. Is the proxy running?".to_string())
    })?;

    if !is_process_running(pid) {
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

        for i in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if !is_process_running(pid) {
                remove_pid()?;
                println!("Bifrost proxy stopped.");
                return Ok(());
            }
            if i == 30 {
                println!("Sending SIGKILL...");
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }
        }

        remove_pid()?;
        println!("Bifrost proxy stopped (forced).");
    }

    #[cfg(not(unix))]
    {
        println!("Stop command is not supported on this platform.");
        println!("Please terminate the process manually (PID: {}).", pid);
    }

    Ok(())
}
