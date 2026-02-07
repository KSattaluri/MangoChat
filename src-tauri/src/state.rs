use std::sync::Mutex;
use tokio::sync::mpsc;

pub struct AppState {
    pub armed: Mutex<bool>,
    pub audio_tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    pub last_transcript: Mutex<String>,
    pub session_active: Mutex<bool>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            armed: Mutex::new(false),
            audio_tx: Mutex::new(None),
            last_transcript: Mutex::new(String::new()),
            session_active: Mutex::new(false),
        }
    }
}
