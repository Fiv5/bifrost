use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, trace, warn};

pub struct AppIconCache {
    cache_dir: PathBuf,
    memory_cache: RwLock<HashMap<String, Option<Vec<u8>>>>,
}

impl AppIconCache {
    pub fn new(data_dir: &Path) -> Self {
        let cache_dir = data_dir.join("icons");
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            warn!(error = %e, "Failed to create icon cache directory");
        }

        Self {
            cache_dir,
            memory_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_icon(&self, app_name: &str, app_path: Option<&str>) -> Option<Vec<u8>> {
        let cache_key = sanitize_app_name(app_name);

        if let Some(cached) = self.get_from_memory(&cache_key) {
            return cached;
        }

        if let Some(cached) = self.get_from_disk(&cache_key) {
            self.set_memory_cache(&cache_key, Some(cached.clone()));
            return Some(cached);
        }

        #[cfg(target_os = "macos")]
        if let Some(path) = app_path {
            if let Some(icon_data) = extract_app_icon_macos(path) {
                self.save_to_disk(&cache_key, &icon_data);
                self.set_memory_cache(&cache_key, Some(icon_data.clone()));
                return Some(icon_data);
            }
        }

        self.set_memory_cache(&cache_key, None);
        None
    }

    fn get_from_memory(&self, cache_key: &str) -> Option<Option<Vec<u8>>> {
        let cache = self.memory_cache.read();
        cache.get(cache_key).cloned()
    }

    fn set_memory_cache(&self, cache_key: &str, data: Option<Vec<u8>>) {
        let mut cache = self.memory_cache.write();
        cache.insert(cache_key.to_string(), data);

        if cache.len() > 500 {
            let keys: Vec<String> = cache.keys().take(100).cloned().collect();
            for key in keys {
                cache.remove(&key);
            }
        }
    }

    fn get_from_disk(&self, cache_key: &str) -> Option<Vec<u8>> {
        let file_path = self.cache_dir.join(format!("{}.png", cache_key));
        std::fs::read(&file_path).ok()
    }

    fn save_to_disk(&self, cache_key: &str, data: &[u8]) {
        let file_path = self.cache_dir.join(format!("{}.png", cache_key));
        if let Err(e) = std::fs::write(&file_path, data) {
            warn!(error = %e, cache_key = cache_key, "Failed to save icon to disk");
        } else {
            debug!(cache_key = cache_key, "Saved app icon to disk cache");
        }
    }

    pub fn clear_cache(&self) {
        let mut cache = self.memory_cache.write();
        cache.clear();

        if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                if entry
                    .path()
                    .extension()
                    .map(|e| e == "png")
                    .unwrap_or(false)
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}

fn sanitize_app_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn extract_app_icon_macos(app_path: &str) -> Option<Vec<u8>> {
    info!(app_path = %app_path, "Extracting app icon from macOS app bundle");

    let path = Path::new(app_path);
    let app_bundle = find_app_bundle(path)?;

    info!(app_bundle = %app_bundle.display(), "Found app bundle");

    let info_plist_path = app_bundle.join("Contents/Info.plist");
    info!(plist_path = %info_plist_path.display(), "Reading Info.plist");

    let plist_value: plist::Value = plist::from_file(&info_plist_path).ok()?;
    let dict = plist_value.as_dictionary()?;

    let icon_file = dict
        .get("CFBundleIconFile")
        .or_else(|| dict.get("CFBundleIconName"))
        .and_then(|v| v.as_string())?;

    let icon_name = if icon_file.ends_with(".icns") {
        icon_file.to_string()
    } else {
        format!("{}.icns", icon_file)
    };

    let icon_path = app_bundle.join("Contents/Resources").join(&icon_name);
    debug!(icon_path = %icon_path.display(), "Found icon file");

    if !icon_path.exists() {
        warn!(icon_path = %icon_path.display(), "Icon file not found");
        return None;
    }

    let file = std::fs::File::open(&icon_path).ok()?;
    let icon_family = icns::IconFamily::read(file).ok()?;

    let icon_types = [
        icns::IconType::RGBA32_32x32_2x,
        icns::IconType::RGBA32_32x32,
        icns::IconType::RGBA32_64x64,
        icns::IconType::RGBA32_128x128,
        icns::IconType::RGBA32_16x16_2x,
        icns::IconType::RGBA32_16x16,
    ];

    for icon_type in icon_types {
        if let Ok(image) = icon_family.get_icon_with_type(icon_type) {
            let mut png_data = Vec::new();
            if image.write_png(&mut png_data).is_ok() {
                trace!(
                    icon_type = ?icon_type,
                    size = png_data.len(),
                    "Extracted icon from icns"
                );
                return Some(png_data);
            }
        }
    }

    warn!(icon_path = %icon_path.display(), "No suitable icon found in icns file");
    None
}

#[cfg(target_os = "macos")]
fn find_app_bundle(path: &Path) -> Option<PathBuf> {
    let mut found_bundles: Vec<PathBuf> = Vec::new();

    for ancestor in path.ancestors() {
        if ancestor.extension().map(|e| e == "app").unwrap_or(false) {
            found_bundles.push(ancestor.to_path_buf());
        }
    }

    for bundle in &found_bundles {
        let info_plist = bundle.join("Contents/Info.plist");
        if info_plist.exists() {
            if let Ok(plist_value) = plist::from_file::<_, plist::Value>(&info_plist) {
                if let Some(dict) = plist_value.as_dictionary() {
                    if dict.get("CFBundleIconFile").is_some()
                        || dict.get("CFBundleIconName").is_some()
                    {
                        info!(
                            app_bundle = %bundle.display(),
                            "Found app bundle with icon"
                        );
                        return Some(bundle.clone());
                    }
                }
            }
        }
    }

    found_bundles.first().cloned()
}

#[cfg(not(target_os = "macos"))]
fn extract_app_icon_macos(_app_path: &str) -> Option<Vec<u8>> {
    None
}

pub type SharedAppIconCache = Arc<AppIconCache>;

pub fn create_app_icon_cache(data_dir: &Path) -> SharedAppIconCache {
    Arc::new(AppIconCache::new(data_dir))
}
