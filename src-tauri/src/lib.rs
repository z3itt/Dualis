mod commands;
mod db;
mod download;
mod dsp;
mod error;
mod infer;
mod models;
mod paths;
mod playback;
mod queue;
mod state;
mod tools;

use db::open;
use paths::AppPaths;
use queue::WorkQueue;
use state::AppState;
use tauri::image::Image;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = AppPaths::resolve(app.handle()).map_err(|e| e.to_string())?;
            let conn = open(&paths.db).map_err(|e| e.to_string())?;
            let playback_port = playback::start(paths.tracks.clone()).map_err(|e| e.to_string())?;
            app.manage(AppState {
                db: parking_lot::Mutex::new(conn),
                model: tokio::sync::Mutex::new(None),
                infer_lock: tokio::sync::Mutex::new(()),
                work: WorkQueue::default(),
                playback_port,
            });
            let worker = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                queue::run_worker(worker).await;
            });
            queue::resume_pending(app.handle());
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(icon) = Image::from_bytes(include_bytes!("../icons/icon.png")) {
                    let _ = window.set_icon(icon);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ingest,
            commands::ingest_local,
            commands::separate_stems,
            commands::get_library,
            commands::delete_track,
            commands::runtime_info,
            commands::ensure_runtime,
            commands::export_stems,
            commands::export_stems_batch,
            commands::retry_track,
            commands::prioritize_track,
            commands::delete_tracks,
            commands::delete_playlist,
            commands::set_model,
            commands::set_cookies_browser,
            commands::set_cookies_file,
            commands::set_download_format,
            commands::list_models,
            commands::read_stem_file,
            commands::asset_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Dualis");
}
