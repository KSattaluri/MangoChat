use image::RgbaImage;
use std::collections::HashMap;
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
    SessionInactivityTimeout { seconds: u64 },
    SessionMaxDurationReached { token: u64, minutes: u64 },
    ApiKeyValidated { provider: String, ok: bool, message: String },
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize, Clone)]
pub struct UsageTotals {
    pub bytes_sent: u64,
    pub ms_sent: u64,
    pub ms_suppressed: u64,
    pub commits: u64,
    pub finals: u64,
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
    pub finals: u64,
    pub started_ms: u64,
    pub updated_ms: u64,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize, Clone)]
pub struct ProviderUsage {
    pub ms_sent: u64,
    pub ms_suppressed: u64,
    pub bytes_sent: u64,
    pub finals: u64,
}

pub struct AppState {
    pub audio_tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    pub last_transcript: Mutex<String>,
    pub session_active: Mutex<bool>,
    pub session_gen: AtomicU64,
    pub hotkey_recording: AtomicBool,
    pub snip_image: Mutex<Option<RgbaImage>>,
    pub snip_active: AtomicBool,
    pub snip_started_ms: AtomicU64,
    pub cursor_pos: Mutex<Option<(i32, i32)>>,
    /// 0 = strict, 1 = lenient, 2 = legacy off (not user-selectable)
    pub vad_mode: AtomicU64,
    pub screenshot_enabled: AtomicBool,
    pub usage: Mutex<UsageTotals>,
    pub session_usage: Mutex<SessionUsage>,
    pub provider_totals: Mutex<HashMap<String, ProviderUsage>>,
    /// FFT magnitudes for the visualizer bars (0.0–1.0 range).
    pub fft_data: Mutex<[f32; 50]>,
    /// Configurable app path for Chrome (used by URL commands).
    pub chrome_path: Mutex<String>,
    /// Configurable app path for Paint.
    pub paint_path: Mutex<String>,
    /// Dynamic URL voice commands: (trigger, url).
    pub url_commands: Mutex<Vec<(String, String)>>,
    /// Dynamic alias voice commands: (trigger, replacement text).
    pub alias_commands: Mutex<Vec<(String, String)>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
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
            screenshot_enabled: AtomicBool::new(false),
            usage: Mutex::new(UsageTotals::default()),
            session_usage: Mutex::new(SessionUsage::default()),
            provider_totals: Mutex::new(HashMap::new()),
            fft_data: Mutex::new([0.0; 50]),
            chrome_path: Mutex::new(r"C:\Program Files\Google\Chrome\Application\chrome.exe".into()),
            paint_path: Mutex::new(r"C:\Windows\System32\mspaint.exe".into()),
            url_commands: Mutex::new(vec![]),
            alias_commands: Mutex::new(vec![]),
        }
    }
}
