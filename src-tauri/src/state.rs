use parking_lot::Mutex;
use rusqlite::Connection;
use tokio::sync::Mutex as AsyncMutex;

use crate::infer::LoadedModel;
use crate::queue::WorkQueue;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub model: AsyncMutex<Option<LoadedModel>>,
    pub infer_lock: AsyncMutex<()>,
    pub work: WorkQueue,
    pub playback_port: u16,
}
