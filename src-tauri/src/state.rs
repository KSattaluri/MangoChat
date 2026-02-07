use image::RgbaImage;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use tokio::sync::mpsc;

pub struct AppState {
    pub armed: Mutex<bool>,
    pub audio_tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    pub last_transcript: Mutex<String>,
    pub session_active: Mutex<bool>,
    pub session_gen: AtomicU64,
    pub hotkey_recording: AtomicBool,
    pub snip_image: Mutex<Option<RgbaImage>>,
    pub snip_active: AtomicBool,
    pub cursor_pos: Mutex<Option<(i32, i32)>>,
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
            cursor_pos: Mutex::new(None),
        }
    }
}
