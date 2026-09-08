use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::db::{self, LibrarySnapshot, Playlist, Track};
use crate::download;
use crate::error::{AppError, AppResult};
use crate::infer::{self, LoadedModel, SeparationResult};
use crate::models::{self, ModelInfo};
use crate::paths::AppPaths;
use crate::state::AppState;
use crate::queue::WorkItem;
use crate::tools::{self, RuntimeInfo};

const SETTING_MODEL: &str = "selected_model";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub track_id: String,
    pub stage: String,
    pub progress: f32,
    pub message: String,
    pub status: String,
    pub eta_seconds: Option<f32>,
}

fn emit_job(app: &AppHandle, event: JobEvent) {
    let _ = app.emit("job-progress", event);
}

fn save_track(state: &AppState, track: &Track) -> AppResult<()> {
    let db = state.db.lock();
    db::upsert_track(&db, track)
}

fn load_track(state: &AppState, id: &str) -> AppResult<Track> {
    let db = state.db.lock();
    db::get_track(&db, id)?.ok_or_else(|| AppError::msg("Track not found"))
}

fn selected_model_id(state: &AppState) -> String {
    let db = state.db.lock();
    db::get_setting(&db, SETTING_MODEL)
        .ok()
        .flatten()
        .unwrap_or_else(|| models::DEFAULT_MODEL_ID.into())
}

fn default_cookies_browser() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "firefox"
    }
    #[cfg(target_os = "macos")]
    {
        "chrome"
    }
    #[cfg(target_os = "windows")]
    {
        "chrome"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "firefox"
    }
}

fn cookies_browser(state: &AppState) -> String {
    let db = state.db.lock();
    db::get_setting(&db, download::SETTING_COOKIES_BROWSER)
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_cookies_browser().into())
}

fn cookies_file(state: &AppState) -> Option<String> {
    let db = state.db.lock();
    db::get_setting(&db, download::SETTING_COOKIES_FILE)
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
}

fn download_format(state: &AppState) -> String {
    let db = state.db.lock();
    db::get_setting(&db, download::SETTING_DOWNLOAD_FORMAT)
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| download::DEFAULT_DOWNLOAD_FORMAT.into())
}

fn runtime_payload(state: &AppState, paths: &AppPaths, ep: String) -> RuntimeInfo {
    let selected = selected_model_id(state);
    let spec = models::require(&selected);
    let (ytdlp_path, ytdlp_source) = match tools::resolve_ytdlp(paths) {
        Some((path, source)) => (Some(path.to_string_lossy().into()), source.to_string()),
        None => (None, "missing".into()),
    };
    RuntimeInfo {
        ytdlp_path,
        ytdlp_source,
        model_path: Some(tools::model_file(paths, spec).to_string_lossy().into()),
        model_ready: tools::model_ready(paths, spec),
        selected_model: selected,
        execution_provider: ep,
        compiled_providers: tools::compiled_providers(),
        models: tools::list_models(paths),
        cookies_browser: cookies_browser(state),
        cookies_file: cookies_file(state),
        download_format: download_format(state),
        playback_port: state.playback_port,
    }
}

#[tauri::command]
pub async fn ingest(app: AppHandle, state: State<'_, AppState>, inputs: Vec<String>) -> AppResult<LibrarySnapshot> {
    let paths = AppPaths::resolve(&app)?;
    let browser = cookies_browser(&state);
    let file = cookies_file(&state);
    let auth = download::YtAuth {
        browser: Some(browser.as_str()),
        cookies_file: file.as_deref(),
    };
    let batches = download::expand_inputs(&paths, &inputs, &auth).await?;
    for batch in batches {
        let playlist_id = if let Some(meta) = batch.playlist {
            let id = Uuid::new_v4().to_string();
            let now = db::now_secs();
            let mut cover_path = None;
            if let Some(url) = meta.cover_url.as_deref() {
                let dir = paths.playlist_dir(&id);
                let _ = fs::create_dir_all(&dir);
                let dest = dir.join("cover.jpg");
                if download::download_cover(url, &dest).await.is_ok() {
                    cover_path = Some(dest.to_string_lossy().into());
                }
            }
            let playlist = Playlist {
                id: id.clone(),
                title: meta.title,
                artist: meta.artist,
                source_url: Some(meta.source_url),
                source_kind: meta.source_kind,
                cover_path,
                created_at: now,
                updated_at: now,
                track_count: batch.items.len() as i64,
                ready_count: 0,
            };
            {
                let db = state.db.lock();
                db::upsert_playlist(&db, &playlist)?;
            }
            Some(id)
        } else {
            None
        };

        for (index, item) in batch.items.into_iter().enumerate() {
            let id = Uuid::new_v4().to_string();
            let now = db::now_secs();
            let track = Track {
                id: id.clone(),
                title: item.title.clone(),
                artist: item.artist.clone(),
                source_url: Some(item.source_url.clone()),
                source_kind: item.source_kind.clone(),
                source_path: None,
                vocals_path: None,
                instrumental_path: None,
                cover_path: None,
                duration_ms: None,
                sample_rate: None,
                status: "queued".into(),
                error: None,
                created_at: now,
                updated_at: now,
                peaks: None,
                playlist_id: playlist_id.clone(),
                playlist_index: Some(index as i64),
                ytdlp_query: Some(item.ytdlp_query.clone()),
            };
            save_track(&state, &track)?;
            state.work.enqueue(
                WorkItem {
                    track_id: id,
                    query: item.ytdlp_query,
                },
                false,
            );
        }
    }
    let db = state.db.lock();
    db::snapshot(&db)
}

#[tauri::command]
pub async fn ingest_local(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<Track> {
    let src = PathBuf::from(&path);
    if !src.is_file() {
        return Err(AppError::msg("That file does not exist"));
    }
    let id = Uuid::new_v4().to_string();
    let paths = AppPaths::resolve(&app)?;
    let dest_dir = paths.track_dir(&id);
    fs::create_dir_all(&dest_dir)?;
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("bin");
    let dest = dest_dir.join(format!("source.{ext}"));
    fs::copy(&src, &dest)?;
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Local file")
        .to_string();
    let now = db::now_secs();
    let track = Track {
        id: id.clone(),
        title: stem,
        artist: "Local".into(),
        source_url: Some(path.clone()),
        source_kind: "local".into(),
        source_path: Some(dest.to_string_lossy().into()),
        vocals_path: None,
        instrumental_path: None,
        cover_path: None,
        duration_ms: None,
        sample_rate: None,
        status: "downloaded".into(),
        error: None,
        created_at: now,
        updated_at: now,
        peaks: None,
        playlist_id: None,
        playlist_index: None,
        ytdlp_query: None,
    };
    save_track(&state, &track)?;
    state.work.enqueue(
        WorkItem {
            track_id: id,
            query: path,
        },
        false,
    );
    Ok(track)
}

async fn process_download(app: AppHandle, track_id: String, query: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let paths = AppPaths::resolve(&app)?;
    let mut track = load_track(&state, &track_id)?;
    track.status = "downloading".into();
    track.updated_at = db::now_secs();
    save_track(&state, &track)?;
    emit_job(
        &app,
        JobEvent {
            track_id: track_id.clone(),
            stage: "download".into(),
            progress: 0.0,
            message: "Starting download".into(),
            status: "downloading".into(),
            eta_seconds: None,
        },
    );

    let dest_dir = paths.track_dir(&track_id);
    let browser = cookies_browser(&state);
    let file = cookies_file(&state);
    let format = download_format(&state);
    let auth = download::YtAuth {
        browser: Some(browser.as_str()),
        cookies_file: file.as_deref(),
    };
    let result = download::download_audio(&paths, &dest_dir, &query, &auth, &format, |progress, message| {
        emit_job(
            &app,
            JobEvent {
                track_id: track_id.clone(),
                stage: "download".into(),
                progress,
                message,
                status: "downloading".into(),
                eta_seconds: None,
            },
        );
    })
    .await;

    match result {
        Ok(dl) => {
            track.title = dl.title;
            track.artist = dl.artist;
            track.source_path = Some(dl.path.to_string_lossy().into());
            track.cover_path = dl.cover_path.map(|p| p.to_string_lossy().into());
            track.duration_ms = dl.duration_ms;
            track.status = "downloaded".into();
            track.error = None;
            track.updated_at = db::now_secs();
            save_track(&state, &track)?;
            drop(state);
            separate_stems_inner(app, track_id).await?;
            Ok(())
        }
        Err(err) => {
            track.status = "error".into();
            track.error = Some(err.to_string());
            track.updated_at = db::now_secs();
            save_track(&state, &track)?;
            emit_job(
                &app,
                JobEvent {
                    track_id,
                    stage: "download".into(),
                    progress: 0.0,
                    message: err.to_string(),
                    status: "error".into(),
                    eta_seconds: None,
                },
            );
            Err(err)
        }
    }
}

#[tauri::command]
pub async fn separate_stems(app: AppHandle, track_id: String) -> AppResult<SeparationResult> {
    separate_stems_inner(app, track_id).await
}

#[tauri::command]
pub async fn retry_track(app: AppHandle, track_id: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let mut track = load_track(&state, &track_id)?;
    let query = download::resolve_download_query(
        &track.source_kind,
        &track.title,
        &track.artist,
        track.source_url.as_deref(),
        track.ytdlp_query.as_deref(),
    );
    track.status = "queued".into();
    track.error = None;
    track.updated_at = db::now_secs();
    save_track(&state, &track)?;
    state.work.enqueue(
        WorkItem {
            track_id,
            query,
        },
        true,
    );
    Ok(())
}

#[tauri::command]
pub async fn prioritize_track(app: AppHandle, track_id: String) -> AppResult<()> {
    retry_track(app, track_id).await
}

pub(crate) async fn run_queued_job(app: AppHandle, track_id: String, query: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let track = load_track(&state, &track_id)?;
    if track.status == "ready" && track.vocals_path.is_some() {
        return Ok(());
    }
    if track.source_path.is_some() {
        drop(state);
        return separate_stems_inner(app, track_id).await.map(|_| ());
    }
    let mut query = query;
    if query.trim().is_empty() {
        query = download::resolve_download_query(
            &track.source_kind,
            &track.title,
            &track.artist,
            track.source_url.as_deref(),
            track.ytdlp_query.as_deref(),
        );
    }
    drop(state);
    process_download(app, track_id, query).await
}

async fn separate_stems_inner(app: AppHandle, track_id: String) -> AppResult<SeparationResult> {
    let state = app.state::<AppState>();
    let paths = AppPaths::resolve(&app)?;
    let mut track = load_track(&state, &track_id)?;
    let source = track
        .source_path
        .clone()
        .ok_or_else(|| AppError::msg("Track has no downloaded audio yet"))?;
    let spec = models::require(&selected_model_id(&state)).clone();

    track.status = "separating".into();
    track.error = None;
    track.updated_at = db::now_secs();
    save_track(&state, &track)?;
    emit_job(
        &app,
        JobEvent {
            track_id: track_id.clone(),
            stage: "decode".into(),
            progress: 0.0,
            message: format!("Preparing {}", spec.name),
            status: "separating".into(),
            eta_seconds: None,
        },
    );

    tools::ensure_model(&paths, &spec).await?;
    let _infer_guard = state.infer_lock.lock().await;

    {
        let mut loaded = state.model.lock().await;
        let needs_reload = loaded
            .as_ref()
            .map(|m| m.model_id != spec.id)
            .unwrap_or(true);
        if needs_reload {
            let prefer_gpu = !track
                .error
                .as_ref()
                .is_some_and(|err| infer::should_fallback_to_cpu(&AppError::msg(err.clone())));
            *loaded = Some(infer::load_model(&paths, &spec, prefer_gpu)?);
        } else if track
            .error
            .as_ref()
            .is_some_and(|err| infer::should_fallback_to_cpu(&AppError::msg(err.clone())))
        {
            // Retry after a GPU/WebGPU failure: drop the cached GPU session.
            *loaded = Some(infer::load_model(&paths, &spec, false)?);
        }
    }

    let dest_dir = paths.track_dir(&track_id);
    let app_for_progress = app.clone();
    let id_for_progress = track_id.clone();
    let spec_for_reload = spec.clone();
    let paths_for_reload = paths.clone();

    let result = tokio::task::spawn_blocking(move || {
        let app = app_for_progress;
        let state = app.state::<AppState>();
        let mut loaded = state.model.blocking_lock();
        let model = loaded
            .as_mut()
            .ok_or_else(|| AppError::msg("Model session missing"))?;
        let run = |model: &mut LoadedModel| {
            infer::separate_file(model, std::path::Path::new(&source), &dest_dir, |progress, message, eta| {
                let stage = if message.to_lowercase().contains("decod") {
                    "decode"
                } else if message.to_lowercase().contains("reconstr") || message.to_lowercase().contains("stems ready") {
                    "export"
                } else {
                    "infer"
                };
                emit_job(
                    &app,
                    JobEvent {
                        track_id: id_for_progress.clone(),
                        stage: stage.into(),
                        progress,
                        message,
                        status: "separating".into(),
                        eta_seconds: eta,
                    },
                );
            })
        };
        match run(model) {
            Ok(result) => Ok(result),
            Err(err) if infer::should_fallback_to_cpu(&err) => {
                emit_job(
                    &app,
                    JobEvent {
                        track_id: id_for_progress.clone(),
                        stage: "infer".into(),
                        progress: 0.1,
                        message: infer::cpu_fallback_message(&err),
                        status: "separating".into(),
                        eta_seconds: None,
                    },
                );
                *loaded = Some(infer::load_model(&paths_for_reload, &spec_for_reload, false)?);
                let cpu_model = loaded.as_mut().unwrap();
                run(cpu_model)
            }
            Err(err) => Err(err),
        }
    })
    .await
    .map_err(|e| AppError::msg(format!("Separation worker failed: {e}")))?;

    let result = match result {
        Ok(result) => result,
        Err(err) => {
            track.status = "error".into();
            track.error = Some(err.to_string());
            track.updated_at = db::now_secs();
            save_track(&state, &track)?;
            emit_job(
                &app,
                JobEvent {
                    track_id,
                    stage: "infer".into(),
                    progress: 0.0,
                    message: err.to_string(),
                    status: "error".into(),
                    eta_seconds: None,
                },
            );
            return Err(err);
        }
    };
    track.vocals_path = Some(result.vocals_path.clone());
    track.instrumental_path = Some(result.instrumental_path.clone());
    track.duration_ms = Some(result.duration_ms);
    track.sample_rate = Some(result.sample_rate as i64);
    track.status = "ready".into();
    track.error = None;
    track.updated_at = db::now_secs();
    save_track(&state, &track)?;
    emit_job(
        &app,
        JobEvent {
            track_id,
            stage: "export".into(),
            progress: 1.0,
            message: format!("Ready on {} · 32-bit float WAV", result.execution_provider),
            status: "ready".into(),
            eta_seconds: Some(0.0),
        },
    );
    Ok(result)
}

#[tauri::command]
pub fn get_library(state: State<'_, AppState>) -> AppResult<LibrarySnapshot> {
    let db = state.db.lock();
    db::snapshot(&db)
}

#[tauri::command]
pub fn delete_track(app: AppHandle, state: State<'_, AppState>, track_id: String) -> AppResult<()> {
    let paths = AppPaths::resolve(&app)?;
    {
        let db = state.db.lock();
        db::delete_track(&db, &track_id)?;
    }
    let dir = paths.track_dir(&track_id);
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[tauri::command]
pub fn delete_tracks(app: AppHandle, state: State<'_, AppState>, track_ids: Vec<String>) -> AppResult<()> {
    let paths = AppPaths::resolve(&app)?;
    for id in track_ids {
        {
            let db = state.db.lock();
            db::delete_track(&db, &id)?;
        }
        let _ = fs::remove_dir_all(paths.track_dir(&id));
    }
    Ok(())
}

#[tauri::command]
pub fn delete_playlist(app: AppHandle, state: State<'_, AppState>, playlist_id: String) -> AppResult<()> {
    let paths = AppPaths::resolve(&app)?;
    let track_ids = {
        let db = state.db.lock();
        db::playlist_track_ids(&db, &playlist_id)?
    };
    {
        let db = state.db.lock();
        db::delete_playlist(&db, &playlist_id)?;
    }
    for id in track_ids {
        let _ = fs::remove_dir_all(paths.track_dir(&id));
    }
    let _ = fs::remove_dir_all(paths.playlist_dir(&playlist_id));
    Ok(())
}

#[tauri::command]
pub async fn runtime_info(app: AppHandle, state: State<'_, AppState>) -> AppResult<RuntimeInfo> {
    let paths = AppPaths::resolve(&app)?;
    let ep = state
        .model
        .lock()
        .await
        .as_ref()
        .map(|m| m.ep_name.clone())
        .unwrap_or_else(|| "Idle".into());
    Ok(runtime_payload(&state, &paths, ep))
}

#[tauri::command]
pub async fn ensure_runtime(app: AppHandle, state: State<'_, AppState>) -> AppResult<RuntimeInfo> {
    let paths = AppPaths::resolve(&app)?;
    let _ = tools::ensure_ytdlp(&paths).await?;
    Ok(runtime_payload(&state, &paths, "Idle".into()))
}

#[tauri::command]
pub async fn set_model(app: AppHandle, state: State<'_, AppState>, model_id: String) -> AppResult<RuntimeInfo> {
    let spec = models::by_id(&model_id).ok_or_else(|| AppError::msg("Unknown model"))?;
    {
        let db = state.db.lock();
        db::set_setting(&db, SETTING_MODEL, spec.id)?;
    }
    let paths = AppPaths::resolve(&app)?;
    tools::ensure_model(&paths, spec).await?;
    {
        let mut loaded = state.model.lock().await;
        *loaded = None;
    }
    Ok(runtime_payload(&state, &paths, "Idle".into()))
}

#[tauri::command]
pub async fn set_cookies_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<RuntimeInfo> {
    {
        let db = state.db.lock();
        if path.trim().is_empty() {
            db::set_setting(&db, download::SETTING_COOKIES_FILE, "")?;
        } else {
            let file = PathBuf::from(path.trim());
            if !file.is_file() {
                return Err(AppError::msg("That cookies file does not exist"));
            }
            db::set_setting(
                &db,
                download::SETTING_COOKIES_FILE,
                file.to_string_lossy().as_ref(),
            )?;
        }
    }
    let paths = AppPaths::resolve(&app)?;
    Ok(runtime_payload(&state, &paths, "Idle".into()))
}

#[tauri::command]
pub async fn set_download_format(
    app: AppHandle,
    state: State<'_, AppState>,
    format: String,
) -> AppResult<RuntimeInfo> {
    let value = match format.trim().to_lowercase().as_str() {
        "auto" | "best" | "any" | "fast" => format.trim().to_lowercase(),
        _ => return Err(AppError::msg("Unknown download quality preset")),
    };
    {
        let db = state.db.lock();
        db::set_setting(&db, download::SETTING_DOWNLOAD_FORMAT, &value)?;
    }
    let paths = AppPaths::resolve(&app)?;
    Ok(runtime_payload(&state, &paths, "Idle".into()))
}

#[tauri::command]
pub async fn set_cookies_browser(
    app: AppHandle,
    state: State<'_, AppState>,
    browser: String,
) -> AppResult<RuntimeInfo> {
    {
        let db = state.db.lock();
        db::set_setting(&db, download::SETTING_COOKIES_BROWSER, browser.trim())?;
    }
    let paths = AppPaths::resolve(&app)?;
    Ok(runtime_payload(&state, &paths, "Idle".into()))
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> AppResult<Vec<ModelInfo>> {
    let paths = AppPaths::resolve(&app)?;
    Ok(tools::list_models(&paths))
}

fn copy_stems(track: &Track, dest: &std::path::Path) -> AppResult<()> {
    fs::create_dir_all(dest)?;
    let slug = format!("{} - {}", track.artist, track.title)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if let Some(path) = track.vocals_path.as_ref() {
        fs::copy(path, dest.join(format!("{slug}.vocals.wav")))?;
    }
    if let Some(path) = track.instrumental_path.as_ref() {
        fs::copy(path, dest.join(format!("{slug}.instrumental.wav")))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn export_stems(app: AppHandle, state: State<'_, AppState>, track_id: String, dest_dir: String) -> AppResult<()> {
    let track = load_track(&state, &track_id)?;
    copy_stems(&track, &PathBuf::from(dest_dir))?;
    let _ = app;
    Ok(())
}

#[tauri::command]
pub async fn export_stems_batch(
    state: State<'_, AppState>,
    track_ids: Vec<String>,
    dest_dir: String,
) -> AppResult<u32> {
    let dest = PathBuf::from(dest_dir);
    let mut count = 0u32;
    for id in track_ids {
        let track = load_track(&state, &id)?;
        if track.status != "ready" {
            continue;
        }
        copy_stems(&track, &dest)?;
        count += 1;
    }
    Ok(count)
}

#[tauri::command]
pub fn read_stem_file(app: AppHandle, path: String) -> AppResult<Vec<u8>> {
    let paths = AppPaths::resolve(&app)?;
    let file = validate_stem_path(&paths, &path)?;
    fs::read(&file).map_err(|e| AppError::msg(format!("Could not read stem audio: {e}")))
}

fn validate_stem_path(paths: &AppPaths, path: &str) -> AppResult<PathBuf> {
    let candidate = PathBuf::from(path);
    let file = candidate
        .canonicalize()
        .map_err(|_| AppError::msg("Stem file not found"))?;
    let tracks_root = paths
        .tracks
        .canonicalize()
        .map_err(|e| AppError::msg(format!("Tracks directory missing: {e}")))?;
    if !file.starts_with(&tracks_root) {
        return Err(AppError::msg("Stem path is outside the app library"));
    }
    Ok(file)
}

#[tauri::command]
pub fn asset_url(path: String) -> String {
    path
}
