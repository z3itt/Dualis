use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

use crate::db;
use crate::state::AppState;

#[derive(Clone, Debug)]
pub struct WorkItem {
    pub track_id: String,
    pub query: String,
}

#[derive(Clone, Default)]
pub struct WorkQueue {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    pending: VecDeque<WorkItem>,
    queued: HashSet<String>,
}

impl WorkQueue {
    pub fn enqueue(&self, item: WorkItem, front: bool) {
        let mut inner = self.inner.lock();
        if inner.queued.contains(&item.track_id) {
            if front {
                if let Some(pos) = inner.pending.iter().position(|w| w.track_id == item.track_id) {
                    if let Some(existing) = inner.pending.remove(pos) {
                        inner.pending.push_front(existing);
                    }
                }
            }
            return;
        }
        inner.queued.insert(item.track_id.clone());
        if front {
            inner.pending.push_front(item);
        } else {
            inner.pending.push_back(item);
        }
    }

    pub fn finish(&self, track_id: &str) {
        let mut inner = self.inner.lock();
        inner.queued.remove(track_id);
        inner.pending.retain(|item| item.track_id != track_id);
    }

    fn pop(&self) -> Option<WorkItem> {
        self.inner.lock().pending.pop_front()
    }

    #[allow(dead_code)]
    pub fn waiting(&self) -> usize {
        self.inner.lock().pending.len()
    }
}

pub async fn run_worker(app: AppHandle) {
    loop {
        let item = {
            let state = app.state::<AppState>();
            state.work.pop()
        };
        match item {
            Some(item) => {
                let _ = crate::commands::run_queued_job(app.clone(), item.track_id.clone(), item.query).await;
                let state = app.state::<AppState>();
                state.work.finish(&item.track_id);
            }
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
    }
}

pub fn resume_pending(app: &AppHandle) {
    let state = app.state::<AppState>();
    let tracks = {
        let db = state.db.lock();
        db::list_tracks(&db).unwrap_or_default()
    };
    for track in tracks {
        if track.status == "ready" || track.status == "error" {
            continue;
        }
        let query = crate::download::resolve_download_query(
            &track.source_kind,
            &track.title,
            &track.artist,
            track.source_url.as_deref(),
            track.ytdlp_query.as_deref(),
        );
        state.work.enqueue(
            WorkItem {
                track_id: track.id,
                query,
            },
            false,
        );
    }
}
