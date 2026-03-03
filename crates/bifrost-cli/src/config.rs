use std::path::PathBuf;

pub fn get_bifrost_dir() -> bifrost_core::Result<PathBuf> {
    Ok(bifrost_storage::data_dir())
}
