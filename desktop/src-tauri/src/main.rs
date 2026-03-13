use bifrost_tls::{ensure_valid_ca, generate_root_ca, save_root_ca, CertInstaller, CertStatus};
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant, SystemTime};

use tauri::{
    image::Image,
    window::{Effect, EffectState, EffectsBuilder},
    AppHandle, Manager, State, WebviewWindow,
};

const BACKEND_HOST: &str = "127.0.0.1";
const DEFAULT_BACKEND_PORT: u16 = 9900;
const MAX_PORT_INCREMENT_ATTEMPTS: u16 = 64;

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
    expected_proxy_port: u16,
    proxy_port: u16,
    platform: &'static str,
    startup_ready: bool,
    startup_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopPortUpdateResponse {
    #[serde(alias = "expectedPort")]
    expected_port: u16,
    #[serde(alias = "actualPort")]
    actual_port: u16,
}

struct BackendState {
    binary_path: PathBuf,
    data_dir: PathBuf,
    config_path: PathBuf,
    expected_port: Mutex<u16>,
    port: Mutex<u16>,
    child: Mutex<Option<Child>>,
    shutdown_started: AtomicBool,
    force_exit: AtomicBool,
    startup_ready: AtomicBool,
    startup_error: Mutex<Option<String>>,
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
            main_window.set_icon(load_app_icon()?)?;

            apply_window_effects(&main_window)?;

            let binary_path = resolve_bifrost_binary(app.handle())?;
            let app_data_dir = resolve_desktop_data_dir()?;
            let config_path = resolve_desktop_config_path(&app_data_dir);
            let app_config_dir = config_path
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| anyhow("missing desktop config dir".to_string()))?;

            fs::create_dir_all(&app_config_dir)
                .map_err(|error| anyhow(format!("failed to create config dir: {error}")))?;
            fs::create_dir_all(&app_data_dir)
                .map_err(|error| anyhow(format!("failed to create data dir: {error}")))?;
            append_desktop_bootstrap_log(
                &app_data_dir,
                format!(
                    "desktop setup started; binary_path={} data_dir={} config_dir={}",
                    binary_path.display(),
                    app_data_dir.display(),
                    app_config_dir.display()
                ),
            );
            let config = load_desktop_config(&config_path)?;

            app.manage(BackendState {
                binary_path,
                data_dir: app_data_dir,
                config_path,
                expected_port: Mutex::new(config.proxy_port),
                port: Mutex::new(config.proxy_port),
                child: Mutex::new(None),
                shutdown_started: AtomicBool::new(false),
                force_exit: AtomicBool::new(false),
                startup_ready: AtomicBool::new(false),
                startup_error: Mutex::new(None),
            });

            main_window.show()?;
            main_window.set_focus()?;

            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                bootstrap_desktop_backend(&app_handle);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                request_desktop_shutdown(window.app_handle());
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build desktop app")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if should_intercept_exit(app_handle) {
                    api.prevent_exit();
                    request_desktop_shutdown(app_handle);
                }
            }
        });
}

fn should_intercept_exit(app: &AppHandle) -> bool {
    let Some(state) = app.try_state::<BackendState>() else {
        return false;
    };

    !state.force_exit.load(Ordering::SeqCst)
}

fn load_app_icon() -> tauri::Result<Image<'static>> {
    Image::from_bytes(include_bytes!("../../../assets/bifrost.png"))
}

fn apply_window_effects(window: &WebviewWindow) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    window.set_effects(
        EffectsBuilder::new()
            .effects([Effect::UnderWindowBackground, Effect::Sidebar])
            .state(EffectState::Active)
            .radius(18.0)
            .build(),
    )?;

    #[cfg(target_os = "windows")]
    window.set_effects(EffectsBuilder::new().effect(Effect::Mica).build())?;

    Ok(())
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
    let bundled_path = resource_dir.join("resources").join("bin").join(binary_name);
    if bundled_path.exists() {
        return Ok(bundled_path);
    }

    Ok(resource_dir.join("bin").join(binary_name))
}

fn resolve_desktop_data_dir() -> tauri::Result<PathBuf> {
    if let Some(path) = std::env::var_os("BIFROST_DATA_DIR") {
        let path = PathBuf::from(path);
        if !path.as_os_str().is_empty() {
            return Ok(path);
        }
    }

    Ok(default_bifrost_data_dir())
}

fn resolve_desktop_config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("desktop-config.json")
}

fn default_bifrost_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|home| home.join(".bifrost"))
        .unwrap_or_else(|| PathBuf::from(".bifrost"))
}

fn ensure_desktop_cert_ready(data_dir: &Path) {
    match prepare_desktop_certificates(data_dir) {
        Ok(CertStatus::InstalledAndTrusted) => append_desktop_bootstrap_log(
            data_dir,
            "desktop certificate preflight complete; CA already installed and trusted",
        ),
        Ok(CertStatus::InstalledNotTrusted) => append_desktop_bootstrap_log(
            data_dir,
            "desktop certificate preflight complete; CA trust was repaired",
        ),
        Ok(CertStatus::NotInstalled) => append_desktop_bootstrap_log(
            data_dir,
            "desktop certificate preflight complete; CA was installed and trusted",
        ),
        Err(error) => {
            let message = error.to_string();
            if message.contains("UserCancelled") {
                append_desktop_bootstrap_log(
                    data_dir,
                    "desktop certificate preflight cancelled by user; continuing startup without trusted CA",
                );
            } else {
                append_desktop_bootstrap_log(
                    data_dir,
                    format!(
                        "desktop certificate preflight failed; continuing startup without trusted CA: {error}"
                    ),
                );
            }
        }
    }
}

fn prepare_desktop_certificates(data_dir: &Path) -> Result<CertStatus, String> {
    let cert_dir = data_dir.join("certs");
    let ca_cert_path = cert_dir.join("ca.crt");
    let ca_key_path = cert_dir.join("ca.key");

    fs::create_dir_all(&cert_dir).map_err(|error| format!("failed to create cert dir: {error}"))?;

    let ca_valid = ensure_valid_ca(&ca_cert_path, &ca_key_path)
        .map_err(|error| format!("failed to validate CA certificate: {error}"))?;
    if !ca_valid {
        let ca = generate_root_ca().map_err(|error| format!("failed to generate CA: {error}"))?;
        save_root_ca(&ca_cert_path, &ca_key_path, &ca)
            .map_err(|error| format!("failed to save CA files: {error}"))?;
        append_desktop_bootstrap_log(
            data_dir,
            format!(
                "generated desktop CA certificate at {}",
                ca_cert_path.display()
            ),
        );
    }

    let installer = CertInstaller::new(&ca_cert_path);
    let status = installer
        .check_status()
        .map_err(|error| format!("failed to check CA trust status: {error}"))?;

    if status == CertStatus::InstalledAndTrusted {
        return Ok(status);
    }

    append_desktop_bootstrap_log(
        data_dir,
        format!("desktop CA status is {status}; attempting GUI install/trust"),
    );
    installer
        .install_and_trust_gui()
        .map_err(|error| format!("failed to install/trust desktop CA via GUI flow: {error}"))?;

    installer
        .check_status()
        .map_err(|error| format!("failed to re-check CA trust status: {error}"))
}

fn start_backend(binary_path: &Path, data_dir: &Path, port: u16) -> tauri::Result<Child> {
    let port = port.to_string();
    let stdout_log = open_sidecar_log_file(data_dir, "desktop-sidecar.out.log")?;
    let stderr_log = open_sidecar_log_file(data_dir, "desktop-sidecar.err.log")?;

    append_desktop_bootstrap_log(
        data_dir,
        format!(
            "starting sidecar; binary_path={} data_dir={} port={} stdout_log={} stderr_log={}",
            binary_path.display(),
            data_dir.display(),
            port,
            log_dir(data_dir).join("desktop-sidecar.out.log").display(),
            log_dir(data_dir).join("desktop-sidecar.err.log").display()
        ),
    );

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
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .map_err(|error| anyhow(format!("failed to start backend: {error}")))
}

fn ensure_backend_running(
    binary_path: &Path,
    data_dir: &Path,
    preferred_port: u16,
) -> tauri::Result<(Child, u16)> {
    append_desktop_bootstrap_log(
        data_dir,
        format!(
            "ensuring backend is running; preferred_port={} data_dir={}",
            preferred_port,
            data_dir.display()
        ),
    );
    cleanup_existing_backend(binary_path, data_dir);

    for offset in 0..=MAX_PORT_INCREMENT_ATTEMPTS {
        let port = preferred_port.saturating_add(offset);
        if port == 0 {
            continue;
        }
        if !is_port_available(port) {
            continue;
        }

        let child = start_backend(binary_path, data_dir, port)?;
        match wait_for_backend(port, Duration::from_secs(20)) {
            Ok(()) => {
                append_desktop_bootstrap_log(
                    data_dir,
                    format!("backend became ready at http://{BACKEND_HOST}:{port}"),
                );
                return Ok((child, port));
            }
            Err(error) => {
                append_desktop_bootstrap_log(
                    data_dir,
                    format!("backend failed to become ready on port {port}: {error}"),
                );
                let _ = stop_backend_with_binary(binary_path, data_dir);
                let _ = terminate_child(child);
                if offset == MAX_PORT_INCREMENT_ATTEMPTS {
                    return Err(error);
                }
            }
        }
    }

    Err(anyhow(format!(
        "failed to find an available backend port starting from {preferred_port}"
    )))
}

fn bootstrap_desktop_backend(app: &AppHandle) {
    let Some(state) = app.try_state::<BackendState>() else {
        return;
    };

    append_desktop_bootstrap_log(
        &state.data_dir,
        "desktop backend bootstrap started asynchronously",
    );

    let preferred_port = match state.expected_port.lock() {
        Ok(port) => *port,
        Err(_) => {
            record_startup_error(
                &state,
                "failed to read desktop expected proxy port during startup".to_string(),
            );
            return;
        }
    };

    match ensure_backend_running(&state.binary_path, &state.data_dir, preferred_port) {
        Ok((child, port)) => {
            if let Ok(mut child_guard) = state.child.lock() {
                *child_guard = Some(child);
            }

            if let Ok(mut current_port) = state.port.lock() {
                *current_port = port;
            }

            if let Ok(mut startup_error) = state.startup_error.lock() {
                *startup_error = None;
            }

            state.startup_ready.store(true, Ordering::SeqCst);
            append_desktop_bootstrap_log(
                &state.data_dir,
                format!("desktop backend bootstrap finished; active_port={port}"),
            );
            schedule_desktop_cert_ready(&state.data_dir);
        }
        Err(error) => {
            record_startup_error(&state, error.to_string());
            request_desktop_shutdown(app);
        }
    }
}

fn schedule_desktop_cert_ready(data_dir: &Path) {
    let data_dir = data_dir.to_path_buf();
    std::thread::spawn(move || {
        // Wait briefly so the window and embedded core can settle before any
        // macOS trust prompt interrupts the startup flow.
        std::thread::sleep(Duration::from_secs(2));
        append_desktop_bootstrap_log(
            &data_dir,
            "starting deferred desktop certificate preflight after startup",
        );
        ensure_desktop_cert_ready(&data_dir);
    });
}

fn record_startup_error(state: &BackendState, error: String) {
    append_desktop_bootstrap_log(
        &state.data_dir,
        format!("desktop backend bootstrap failed: {error}"),
    );

    if let Ok(mut startup_error) = state.startup_error.lock() {
        *startup_error = Some(error);
    }
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

fn is_port_available(port: u16) -> bool {
    TcpListener::bind((BACKEND_HOST, port)).is_ok()
}

fn has_runtime_marker(data_dir: &Path) -> bool {
    data_dir.join("bifrost.pid").exists() || data_dir.join("runtime.json").exists()
}

fn cleanup_existing_backend(binary_path: &Path, data_dir: &Path) {
    if has_runtime_marker(data_dir) {
        append_desktop_bootstrap_log(
            data_dir,
            format!(
                "found existing backend runtime markers under {}; stopping stale backend",
                data_dir.display()
            ),
        );
        let _ = stop_backend_with_binary(binary_path, data_dir);
    }
}

fn stop_backend_with_binary(binary_path: &Path, data_dir: &Path) -> tauri::Result<()> {
    append_desktop_bootstrap_log(
        data_dir,
        format!(
            "running synchronous backend stop; binary_path={} data_dir={}",
            binary_path.display(),
            data_dir.display()
        ),
    );
    let status = Command::new(binary_path)
        .arg("stop")
        .env("BIFROST_DATA_DIR", data_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| anyhow(format!("failed to stop backend: {error}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow(format!(
            "backend stop command exited with status {status}"
        )))
    }
}

fn spawn_backend_stop(binary_path: &Path, data_dir: &Path) -> tauri::Result<Child> {
    append_desktop_bootstrap_log(
        data_dir,
        format!(
            "spawning asynchronous backend stop; binary_path={} data_dir={}",
            binary_path.display(),
            data_dir.display()
        ),
    );
    let stdout_log = open_sidecar_log_file(data_dir, "desktop-sidecar.out.log")?;
    let stderr_log = open_sidecar_log_file(data_dir, "desktop-sidecar.err.log")?;

    Command::new(binary_path)
        .arg("stop")
        .env("BIFROST_DATA_DIR", data_dir)
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .map_err(|error| anyhow(format!("failed to spawn backend stop: {error}")))
}

fn terminate_child(mut child: Child) -> tauri::Result<()> {
    let _ = child.kill();
    child
        .wait()
        .map_err(|error| anyhow(format!("failed to wait for backend child: {error}")))?;
    Ok(())
}

fn request_desktop_shutdown(app: &AppHandle) {
    let Some(state) = app.try_state::<BackendState>() else {
        app.exit(0);
        return;
    };

    if state.shutdown_started.swap(true, Ordering::SeqCst) {
        return;
    }

    append_desktop_bootstrap_log(
        &state.data_dir,
        "desktop shutdown requested; hiding window and stopping backend asynchronously",
    );
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    let app_handle = app.clone();
    std::thread::spawn(move || {
        complete_desktop_shutdown(&app_handle);
    });
}

fn complete_desktop_shutdown(app: &AppHandle) {
    let Some(state) = app.try_state::<BackendState>() else {
        app.exit(0);
        return;
    };

    match spawn_backend_stop(&state.binary_path, &state.data_dir) {
        Ok(child) => {
            append_desktop_bootstrap_log(
                &state.data_dir,
                format!("spawned backend stop helper pid={}", child.id()),
            );
        }
        Err(error) => {
            append_desktop_bootstrap_log(
                &state.data_dir,
                format!("failed to spawn backend stop helper: {error}"),
            );
        }
    }

    let Ok(mut child_guard) = state.child.lock() else {
        state.force_exit.store(true, Ordering::SeqCst);
        app.exit(0);
        return;
    };

    if let Some(child) = child_guard.take() {
        append_desktop_bootstrap_log(
            &state.data_dir,
            format!(
                "detached backend child pid={} so desktop UI can exit immediately",
                child.id()
            ),
        );
    }

    state.force_exit.store(true, Ordering::SeqCst);
    append_desktop_bootstrap_log(
        &state.data_dir,
        "desktop shutdown handoff complete; requesting final app exit",
    );
    app.exit(0);
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
    let expected_port = *state
        .expected_port
        .lock()
        .map_err(|_| "failed to read desktop expected proxy port".to_string())?;
    let port = *state
        .port
        .lock()
        .map_err(|_| "failed to read desktop proxy port".to_string())?;
    let startup_error = state
        .startup_error
        .lock()
        .map_err(|_| "failed to read desktop startup error".to_string())?
        .clone();

    Ok(DesktopRuntimeInfo {
        expected_proxy_port: expected_port,
        proxy_port: port,
        platform: std::env::consts::OS,
        startup_ready: state.startup_ready.load(Ordering::SeqCst),
        startup_error,
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
        let current_expected_port = state
            .expected_port
            .lock()
            .map_err(|_| "failed to access current desktop expected port".to_string())?;
        if *current_expected_port == port {
            let current_port = *state
                .port
                .lock()
                .map_err(|_| "failed to access current desktop port".to_string())?;
            return Ok(DesktopRuntimeInfo {
                expected_proxy_port: port,
                proxy_port: current_port,
                platform: std::env::consts::OS,
                startup_ready: state.startup_ready.load(Ordering::SeqCst),
                startup_error: state
                    .startup_error
                    .lock()
                    .map_err(|_| "failed to read desktop startup error".to_string())?
                    .clone(),
            });
        }
    }

    let current_port = *state
        .port
        .lock()
        .map_err(|_| "failed to access current desktop port".to_string())?;
    let updated_runtime =
        rebind_backend_port(current_port, port).map_err(|error| error.to_string())?;
    save_desktop_config(&state.config_path, &DesktopConfig { proxy_port: port })
        .map_err(|error| error.to_string())?;

    {
        let mut expected_port = state
            .expected_port
            .lock()
            .map_err(|_| "failed to update desktop expected proxy port".to_string())?;
        *expected_port = port;
    }
    {
        let mut current_port = state
            .port
            .lock()
            .map_err(|_| "failed to update desktop proxy port".to_string())?;
        *current_port = updated_runtime.actual_port;
    }

    Ok(DesktopRuntimeInfo {
        expected_proxy_port: port,
        proxy_port: updated_runtime.actual_port,
        platform: std::env::consts::OS,
        startup_ready: state.startup_ready.load(Ordering::SeqCst),
        startup_error: state
            .startup_error
            .lock()
            .map_err(|_| "failed to read desktop startup error".to_string())?
            .clone(),
    })
}

fn anyhow(message: String) -> tauri::Error {
    let error: Box<dyn std::error::Error> = Box::new(std::io::Error::other(message));
    tauri::Error::Setup(error.into())
}

fn log_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}

fn append_desktop_bootstrap_log(data_dir: &Path, message: impl AsRef<str>) {
    let log_dir = log_dir(data_dir);
    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    let log_path = log_dir.join("desktop-bootstrap.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };

    let _ = writeln!(file, "[{:?}] {}", SystemTime::now(), message.as_ref());
}

fn open_sidecar_log_file(data_dir: &Path, file_name: &str) -> tauri::Result<fs::File> {
    let log_dir = log_dir(data_dir);
    fs::create_dir_all(&log_dir)
        .map_err(|error| anyhow(format!("failed to create log dir: {error}")))?;

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(file_name))
        .map_err(|error| anyhow(format!("failed to open {file_name}: {error}")))
}

fn rebind_backend_port(
    current_port: u16,
    expected_port: u16,
) -> tauri::Result<DesktopPortUpdateResponse> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| anyhow(format!("failed to build backend rebind client: {error}")))?;
    let url = format!("http://{BACKEND_HOST}:{current_port}/_bifrost/api/config/server");
    let response = client
        .put(url)
        .json(&serde_json::json!({ "port": expected_port }))
        .send()
        .map_err(|error| anyhow(format!("failed to call backend port rebind API: {error}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(anyhow(format!(
            "backend port rebind API failed with status {}: {}",
            status, body
        )));
    }

    response
        .json::<DesktopPortUpdateResponse>()
        .map_err(|error| {
            anyhow(format!(
                "failed to decode backend port rebind response: {error}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::resolve_desktop_config_path;
    use std::path::PathBuf;

    #[test]
    fn desktop_config_uses_shared_data_dir() {
        let target = resolve_desktop_config_path(&PathBuf::from("/tmp/shared-bifrost"));
        assert_eq!(
            target,
            PathBuf::from("/tmp/shared-bifrost/desktop-config.json")
        );
    }
}
