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
