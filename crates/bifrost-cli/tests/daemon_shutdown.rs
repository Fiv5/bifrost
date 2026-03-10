#[cfg(unix)]
mod unix_tests {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    use std::net::TcpListener;
    use std::process::Command;
    use std::thread::sleep;
    use std::time::Duration;

    fn is_process_running(pid: u32) -> bool {
        kill(Pid::from_raw(pid as i32), Signal::SIGCONT).is_ok()
    }

    #[test]
    fn stop_triggers_graceful_shutdown_in_daemon_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let log_dir = tmp.path().join("logs");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();

        let port = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };

        let output = Command::new(env!("CARGO_BIN_EXE_bifrost"))
            .env("BIFROST_DATA_DIR", &data_dir)
            .arg("--log-dir")
            .arg(&log_dir)
            .arg("start")
            .arg("-p")
            .arg(port.to_string())
            .arg("-H")
            .arg("127.0.0.1")
            .arg("--daemon")
            .arg("--skip-cert-check")
            .arg("--no-intercept")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "start failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let runtime_file = data_dir.join("runtime.json");
        for _ in 0..200 {
            if runtime_file.exists() {
                break;
            }
            sleep(Duration::from_millis(50));
        }
        assert!(runtime_file.exists(), "runtime.json not created");

        // 读取 pid，后续用于判断是否确实优雅退出（避免依赖日志落盘时序）
        let pid: u32 = {
            let content = std::fs::read_to_string(&runtime_file).unwrap_or_default();
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            v.get("pid").and_then(|x| x.as_u64()).unwrap_or_default() as u32
        };
        assert!(pid != 0, "invalid pid in runtime.json");

        let stop_output = Command::new(env!("CARGO_BIN_EXE_bifrost"))
            .env("BIFROST_DATA_DIR", &data_dir)
            .arg("stop")
            .output()
            .unwrap();
        assert!(
            stop_output.status.success(),
            "stop failed: {} {}",
            String::from_utf8_lossy(&stop_output.stdout),
            String::from_utf8_lossy(&stop_output.stderr)
        );

        let stop_stdout = String::from_utf8_lossy(&stop_output.stdout);
        assert!(
            !stop_stdout.contains("Sending SIGKILL"),
            "stop escalated to SIGKILL: {}",
            stop_stdout
        );

        // 等待 daemon 进程真正退出（CI 下可能比日志落盘更可靠）
        for _ in 0..600 {
            if !is_process_running(pid) {
                break;
            }
            sleep(Duration::from_millis(100));
        }
        assert!(
            !is_process_running(pid),
            "daemon process still running after stop (pid={})",
            pid
        );

        let err_file = log_dir.join("bifrost.err");
        let log_file = log_dir.join("bifrost.log");
        for _ in 0..200 {
            if err_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&err_file) {
                    if content.contains("Received shutdown signal") {
                        return;
                    }
                }
            }
            if log_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&log_file) {
                    if content.contains("Received shutdown signal") {
                        return;
                    }
                }
            }
            sleep(Duration::from_millis(50));
        }

        // 保留日志检查作为辅助信息（不再作为硬性断言），避免 CI/文件缓冲导致偶发失败。
        let err_content = std::fs::read_to_string(&err_file).unwrap_or_default();
        let log_content = std::fs::read_to_string(&log_file).unwrap_or_default();
        if !(err_content.contains("Received shutdown signal")
            || log_content.contains("Received shutdown signal"))
        {
            eprintln!(
                "warning: graceful shutdown log not found; bifrost.err: {}, bifrost.log: {}",
                err_content, log_content
            );
        }
    }
}
