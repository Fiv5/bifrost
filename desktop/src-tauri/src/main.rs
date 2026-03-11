use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, State};

const BACKEND_HOST: &str = "127.0.0.1";
const DEFAULT_BACKEND_PORT: u16 = 9900;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopConfig {
    proxy_port: u16,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            proxy_port: DEFAULT_BACKEND_PORT,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopRuntimeInfo {
    proxy_port: u16,
    platform: &'static str,
}

struct BackendState {
    binary_path: PathBuf,
    data_dir: PathBuf,
    config_path: PathBuf,
    port: Mutex<u16>,
    child: Mutex<Option<Child>>,
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_desktop_runtime,
            update_desktop_proxy_port
        ])
        .setup(|app| {
            let main_window = app
                .get_webview_window("main")
                .ok_or_else(|| anyhow("missing main window".to_string()))?;
            main_window.hide()?;

            #[cfg(target_os = "windows")]
            main_window.set_decorations(false)?;

            let binary_path = resolve_bifrost_binary(app.handle())?;
            let app_config_dir = app.path().app_config_dir()?;
            let app_data_dir = app.path().app_local_data_dir()?;

            fs::create_dir_all(&app_config_dir)
                .map_err(|error| anyhow(format!("failed to create config dir: {error}")))?;
            fs::create_dir_all(&app_data_dir)
                .map_err(|error| anyhow(format!("failed to create data dir: {error}")))?;

            let config_path = app_config_dir.join("desktop-config.json");
            let config = load_desktop_config(&config_path)?;
            let child = start_backend(&binary_path, &app_data_dir, config.proxy_port)?;
            wait_for_backend(config.proxy_port, Duration::from_secs(20))?;

            app.manage(BackendState {
                binary_path,
                data_dir: app_data_dir,
                config_path,
                port: Mutex::new(config.proxy_port),
                child: Mutex::new(Some(child)),
            });

            main_window.show()?;
            main_window.set_focus()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                stop_backend(window.app_handle());
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build desktop app")
        .run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                stop_backend(app_handle);
            }
        });
}

fn resolve_bifrost_binary(app: &AppHandle) -> tauri::Result<PathBuf> {
    let binary_name = if cfg!(target_os = "windows") {
        "bifrost.exe"
    } else {
        "bifrost"
    };

    if cfg!(debug_assertions) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        return Ok(manifest_dir
            .join("..")
            .join("..")
            .join("target")
            .join("debug")
            .join(binary_name));
    }

    let resource_dir = app.path().resource_dir()?;
    Ok(resource_dir.join("bin").join(binary_name))
}

fn start_backend(binary_path: &Path, data_dir: &Path, port: u16) -> tauri::Result<Child> {
    let port = port.to_string();

    Command::new(binary_path)
        .args([
            "start",
            "--host",
            BACKEND_HOST,
            "--port",
            &port,
            "--skip-cert-check",
        ])
        .env("BIFROST_DATA_DIR", data_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| anyhow(format!("failed to start backend: {error}")))
}

fn wait_for_backend(port: u16, timeout: Duration) -> tauri::Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if is_backend_ready(port) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    Err(anyhow(format!(
        "backend did not become ready at http://{BACKEND_HOST}:{port}"
    )))
}

fn is_backend_ready(port: u16) -> bool {
    std::net::TcpStream::connect((BACKEND_HOST, port)).is_ok()
}

fn stop_backend(app: &AppHandle) {
    let Some(state) = app.try_state::<BackendState>() else {
        return;
    };
    stop_backend_process(&state);
}

fn stop_backend_process(state: &BackendState) {
    let Ok(mut child_guard) = state.child.lock() else {
        return;
    };

    if child_guard.is_none() {
        return;
    }

    let _ = Command::new(&state.binary_path)
        .arg("stop")
        .env("BIFROST_DATA_DIR", &state.data_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if let Some(mut child) = child_guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn load_desktop_config(config_path: &Path) -> tauri::Result<DesktopConfig> {
    if !config_path.exists() {
        let config = DesktopConfig::default();
        save_desktop_config(config_path, &config)?;
        return Ok(config);
    }

    let content = fs::read_to_string(config_path)
        .map_err(|error| anyhow(format!("failed to read desktop config: {error}")))?;
    serde_json::from_str(&content)
        .map_err(|error| anyhow(format!("failed to parse desktop config: {error}")))
}

fn save_desktop_config(config_path: &Path, config: &DesktopConfig) -> tauri::Result<()> {
    let content = serde_json::to_string_pretty(config)
        .map_err(|error| anyhow(format!("failed to encode desktop config: {error}")))?;
    fs::write(config_path, format!("{content}\n"))
        .map_err(|error| anyhow(format!("failed to write desktop config: {error}")))
}

#[tauri::command]
fn get_desktop_runtime(state: State<'_, BackendState>) -> Result<DesktopRuntimeInfo, String> {
    let port = *state
        .port
        .lock()
        .map_err(|_| "failed to read desktop proxy port".to_string())?;

    Ok(DesktopRuntimeInfo {
        proxy_port: port,
        platform: std::env::consts::OS,
    })
}

#[tauri::command]
fn update_desktop_proxy_port(
    state: State<'_, BackendState>,
    port: u16,
) -> Result<DesktopRuntimeInfo, String> {
    if port == 0 {
        return Err("proxy port must be greater than 0".to_string());
    }

    {
        let current_port = state
            .port
            .lock()
            .map_err(|_| "failed to access current desktop port".to_string())?;
        if *current_port == port {
            return Ok(DesktopRuntimeInfo {
                proxy_port: port,
                platform: std::env::consts::OS,
            });
        }
    }

    stop_backend_process(&state);
    save_desktop_config(&state.config_path, &DesktopConfig { proxy_port: port })
        .map_err(|error| error.to_string())?;

    let child = start_backend(&state.binary_path, &state.data_dir, port)
        .map_err(|error| error.to_string())?;
    wait_for_backend(port, Duration::from_secs(20)).map_err(|error| error.to_string())?;

    {
        let mut child_guard = state
            .child
            .lock()
            .map_err(|_| "failed to store desktop backend child".to_string())?;
        *child_guard = Some(child);
    }
    {
        let mut current_port = state
            .port
            .lock()
            .map_err(|_| "failed to update desktop proxy port".to_string())?;
        *current_port = port;
    }

    Ok(DesktopRuntimeInfo {
        proxy_port: port,
        platform: std::env::consts::OS,
    })
}

fn anyhow(message: String) -> tauri::Error {
    let error: Box<dyn std::error::Error> = Box::new(std::io::Error::other(message));
    tauri::Error::Setup(error.into())
}
