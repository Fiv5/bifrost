use std::path::PathBuf;
use std::sync::OnceLock;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_data_dir(dir: PathBuf) {
    let _ = DATA_DIR.set(dir);
}

pub fn data_dir() -> PathBuf {
    if let Some(dir) = DATA_DIR.get() {
        return dir.clone();
    }

    if let Ok(dir) = std::env::var("BIFROST_DATA_DIR") {
        return PathBuf::from(dir);
    }

    dirs::home_dir()
        .map(|h| h.join(".bifrost"))
        .unwrap_or_else(|| PathBuf::from(".bifrost"))
}
