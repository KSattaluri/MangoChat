use image::RgbaImage;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use tokio::sync::mpsc;

/// Events sent from background threads to the UI.
#[derive(Debug, Clone)]
pub enum AppEvent {
    HotkeyPush,
    HotkeyRelease,
    StatusUpdate { status: String, message: String },
    TranscriptDelta(String),
    TranscriptFinal(String),
    SnipTrigger,
    ApiKeyValidated { ok: bool, message: String },
}

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
    pub provider: String,
    pub bytes_sent: u64,
    pub ms_sent: u64,
    pub ms_suppressed: u64,
    pub commits: u64,
    pub started_ms: u64,
    pub updated_ms: u64,
}

pub struct AppState {
    pub armed: AtomicBool,
    pub audio_tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    pub last_transcript: Mutex<String>,
    pub session_active: Mutex<bool>,
    pub session_gen: AtomicU64,
    pub hotkey_recording: AtomicBool,
    pub snip_image: Mutex<Option<RgbaImage>>,
    pub snip_active: AtomicBool,
    pub snip_started_ms: AtomicU64,
    pub cursor_pos: Mutex<Option<(i32, i32)>>,
    /// 0 = strict, 1 = lenient, 2 = off
    pub vad_mode: AtomicU64,
    pub usage: Mutex<UsageTotals>,
    pub session_usage: Mutex<SessionUsage>,
    /// FFT magnitudes for the visualizer bars (0.0–1.0 range).
    pub fft_data: Mutex<[f32; 50]>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            armed: AtomicBool::new(false),
            audio_tx: Mutex::new(None),
            last_transcript: Mutex::new(String::new()),
            session_active: Mutex::new(false),
            session_gen: AtomicU64::new(0),
            hotkey_recording: AtomicBool::new(false),
            snip_image: Mutex::new(None),
            snip_active: AtomicBool::new(false),
            snip_started_ms: AtomicU64::new(0),
            cursor_pos: Mutex::new(None),
            vad_mode: AtomicU64::new(0),
            usage: Mutex::new(UsageTotals::default()),
            session_usage: Mutex::new(SessionUsage::default()),
            fft_data: Mutex::new([0.0; 50]),
        }
    }
}
