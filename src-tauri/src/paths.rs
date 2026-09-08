use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::error::AppResult;

#[derive(Clone)]
pub struct AppPaths {
    pub tools: PathBuf,
    pub models: PathBuf,
    pub tracks: PathBuf,
    pub db: PathBuf,
}

const LEGACY_ID: &str = "dev.z3itt.stems";

fn xdg_home(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
            PathBuf::from(home).join(fallback)
        })
}

fn dir_has_entries(path: &std::path::Path) -> bool {
    path.read_dir()
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some()
}

fn has_app_data(path: &std::path::Path) -> bool {
    path.join("library.db").is_file() || dir_has_entries(&path.join("tracks"))
}

fn migrate_legacy_dir(old: &std::path::Path, new: &std::path::Path) {
    if !old.exists() || old == new || has_app_data(new) {
        return;
    }
    if !new.exists() {
        if let Some(parent) = new.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::rename(old, new).is_ok() {
            return;
        }
        let _ = copy_dir_all(old, new);
        return;
    }
    let _ = copy_dir_all(old, new);
}

fn copy_dir_all(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

impl AppPaths {
    pub fn resolve(app: &AppHandle) -> AppResult<Self> {
        let cache = app
            .path()
            .app_cache_dir()
            .map_err(|e| crate::error::AppError::msg(format!("cache dir: {e}")))?;
        let data = app
            .path()
            .app_data_dir()
            .map_err(|e| crate::error::AppError::msg(format!("data dir: {e}")))?;
        migrate_legacy_dir(&xdg_home("XDG_CACHE_HOME", ".cache").join(LEGACY_ID), &cache);
        migrate_legacy_dir(&xdg_home("XDG_DATA_HOME", ".local/share").join(LEGACY_ID), &data);
        let tools = cache.join("tools");
        let models = cache.join("models");
        let tracks = cache.join("tracks");
        std::fs::create_dir_all(&tools)?;
        std::fs::create_dir_all(&models)?;
        std::fs::create_dir_all(&tracks)?;
        std::fs::create_dir_all(&data)?;
        Ok(Self {
            db: data.join("library.db"),
            tools,
            models,
            tracks,
        })
    }

    pub fn track_dir(&self, id: &str) -> PathBuf {
        self.tracks.join(id)
    }

    pub fn playlist_dir(&self, id: &str) -> PathBuf {
        self.tracks.join(format!("_playlist_{id}"))
    }
}
