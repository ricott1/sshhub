use crate::AppResult;
use anyhow::anyhow;
use std::path::PathBuf;

pub fn store_path(filename: &str) -> AppResult<PathBuf> {
    let dirs = directories::ProjectDirs::from("org", "frittura", "sshhub")
        .ok_or_else(|| anyhow!("Failed to get directories"))?;
    let config_dir = dirs.config_dir();
    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir)?;
    }
    Ok(config_dir.join(filename))
}
