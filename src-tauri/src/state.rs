use image::RgbaImage;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Debug, Default, serde::Deserialize, serde::Serialize, Clone)]
pub struct UsageTotals {
    pub bytes_sent: u64,
    pub ms_sent: u64,
    pub ms_suppressed: u64,
    pub commits: u64,
    pub last_update_ms: u64,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize, Clone)]
pub struct SessionUsage {
    pub session_id: u64,
    pub bytes_sent: u64,
    pub ms_sent: u64,
    pub ms_suppressed: u64,
    pub commits: u64,
    pub started_ms: u64,
    pub updated_ms: u64,
}

pub struct AppState {
    pub armed: Mutex<bool>,
    pub audio_tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    pub last_transcript: Mutex<String>,
    pub session_active: Mutex<bool>,
    pub session_gen: AtomicU64,
    pub hotkey_recording: AtomicBool,
    pub snip_image: Mutex<Option<RgbaImage>>,
    pub snip_active: AtomicBool,
    pub snip_started_ms: AtomicU64,
    pub cursor_pos: Mutex<Option<(i32, i32)>>,
    pub usage: Mutex<UsageTotals>,
    pub usage_session_counter: AtomicU64,
    pub session_usage: Mutex<SessionUsage>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            armed: Mutex::new(false),
            audio_tx: Mutex::new(None),
            last_transcript: Mutex::new(String::new()),
            session_active: Mutex::new(false),
            session_gen: AtomicU64::new(0),
            hotkey_recording: AtomicBool::new(false),
            snip_image: Mutex::new(None),
            snip_active: AtomicBool::new(false),
            snip_started_ms: AtomicU64::new(0),
            cursor_pos: Mutex::new(None),
            usage: Mutex::new(UsageTotals::default()),
            usage_session_counter: AtomicU64::new(0),
            session_usage: Mutex::new(SessionUsage::default()),
        }
    }
}
