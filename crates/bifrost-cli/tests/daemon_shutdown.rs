#[cfg(unix)]
mod unix_tests {
    use std::net::TcpListener;
    use std::process::Command;
    use std::thread::sleep;
    use std::time::Duration;

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

        let err_content = std::fs::read_to_string(&err_file).unwrap_or_default();
        let log_content = std::fs::read_to_string(&log_file).unwrap_or_default();
        assert!(
            err_content.contains("Received shutdown signal")
                || log_content.contains("Received shutdown signal"),
            "daemon did not report graceful shutdown, bifrost.err: {}, bifrost.log: {}",
            err_content,
            log_content
        );
    }
}
