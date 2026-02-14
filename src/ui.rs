use crate::audio;
use crate::settings::Settings;
use crate::snip;
use crate::usage::{append_usage_line, session_usage_path};
use crate::state::{AppEvent, AppState, SessionUsage};
use eframe::egui;
use egui::{
    pos2, vec2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, TextureHandle,
    ViewportBuilder, ViewportCommand, ViewportId,
};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver as EventReceiver, Sender as EventSender};
use std::sync::Arc;
use std::time::Duration;

// Colors matching the original CSS theme
const TEXT_COLOR: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
const TEXT_MUTED: Color32 = Color32::from_rgb(0x9c, 0xa3, 0xaf);
const BTN_BG: Color32 = Color32::from_rgb(0x25, 0x28, 0x30);
const BTN_BORDER: Color32 = Color32::from_rgb(0x2c, 0x2f, 0x36);
const BTN_PRIMARY: Color32 = Color32::from_rgb(0x25, 0x63, 0xeb);
const SETTINGS_BG: Color32 = Color32::from_rgb(0x15, 0x18, 0x21);
const RED: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
const COMPACT_WINDOW_W_WITH_SNIP: f32 = 198.0;
const COMPACT_WINDOW_W_NO_SNIP: f32 = 176.0;
const COMPACT_WINDOW_H: f32 = 54.0;
const PROVIDER_ROWS: &[(&str, &str)] = &[
    ("deepgram", "Deepgram"),
    ("openai", "OpenAI Realtime"),
    ("elevenlabs", "ElevenLabs Realtime"),
    ("assemblyai", "AssemblyAI"),
];
const WINDOW_MONITOR_MODE_FIXED: &str = "fixed";
const WINDOW_ANCHOR_TOP_LEFT: &str = "top_left";
const WINDOW_ANCHOR_TOP_CENTER: &str = "top_center";
const WINDOW_ANCHOR_TOP_RIGHT: &str = "top_right";
const WINDOW_ANCHOR_BOTTOM_LEFT: &str = "bottom_left";
const WINDOW_ANCHOR_BOTTOM_CENTER: &str = "bottom_center";
const WINDOW_ANCHOR_BOTTOM_RIGHT: &str = "bottom_right";

#[derive(Clone)]
struct MonitorChoice {
    id: String,
    label: String,
}

#[derive(Clone, Copy)]
struct ThemePalette {
    text: Color32,
    text_muted: Color32,
    btn_bg: Color32,
    btn_border: Color32,
    settings_bg: Color32,
}

#[derive(Clone, Copy)]
struct AccentPalette {
    id: &'static str,
    name: &'static str,
    base: Color32,
    hover: Color32,
    ring: Color32,
    tint_bg: Color32,
}

#[derive(Clone)]
struct ControlTooltipState {
    key: String,
    text: String,
    until: f64,
}

fn theme_palette(_dark: bool) -> ThemePalette {
    ThemePalette {
        text: TEXT_COLOR,
        text_muted: TEXT_MUTED,
        btn_bg: BTN_BG,
        btn_border: BTN_BORDER,
        settings_bg: SETTINGS_BG,
    }
}

fn accent_palette(id: &str) -> AccentPalette {
    match id {
        "purple" => AccentPalette {
            id: "purple",
            name: "Purple",
            base: Color32::from_rgb(0xa8, 0x55, 0xf7),
            hover: Color32::from_rgb(0x93, 0x3d, 0xe8),
            ring: Color32::from_rgb(0x7e, 0x22, 0xce),
            tint_bg: Color32::from_rgb(0xdb, 0xbf, 0xff),
        },
        "blue" => AccentPalette {
            id: "blue",
            name: "Blue",
            base: Color32::from_rgb(0x3b, 0x82, 0xf6),
            hover: Color32::from_rgb(0x25, 0x63, 0xeb),
            ring: Color32::from_rgb(0x1d, 0x4e, 0xd8),
            tint_bg: Color32::from_rgb(0xbf, 0xdb, 0xfe),
        },
        "orange" => AccentPalette {
            id: "orange",
            name: "Orange",
            base: Color32::from_rgb(0xf5, 0x9e, 0x0b),
            hover: Color32::from_rgb(0xea, 0x8a, 0x00),
            ring: Color32::from_rgb(0xc2, 0x41, 0x0c),
            tint_bg: Color32::from_rgb(0xfe, 0xd7, 0xaa),
        },
        "pink" => AccentPalette {
            id: "pink",
            name: "Pink",
            base: Color32::from_rgb(0xec, 0x48, 0x99),
            hover: Color32::from_rgb(0xdb, 0x27, 0x7d),
            ring: Color32::from_rgb(0xbe, 0x18, 0x5d),
            tint_bg: Color32::from_rgb(0xfb, 0xbf, 0xdc),
        },
        _ => AccentPalette {
            id: "green",
            name: "Green",
            base: Color32::from_rgb(0x36, 0xd3, 0x99),
            hover: Color32::from_rgb(0x16, 0xa3, 0x4a),
            ring: Color32::from_rgb(0x16, 0xa3, 0x4a),
            tint_bg: Color32::from_rgb(0x9f, 0xef, 0xcd),
        },
    }
}

fn accent_options() -> [AccentPalette; 5] {
    [
        accent_palette("green"),
        accent_palette("purple"),
        accent_palette("blue"),
        accent_palette("orange"),
        accent_palette("pink"),
    ]
}

pub struct JarvisApp {
    pub state: Arc<AppState>,
    pub event_tx: EventSender<AppEvent>,
    pub event_rx: EventReceiver<AppEvent>,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub settings: Settings,
    pub settings_open: bool,
    pub settings_tab: String,
    pub status_text: String,
    pub status_state: String,
    pub is_recording: bool,
    pub audio_capture: Option<crate::audio::AudioCapture>,
    pub should_quit: bool,
    pub mic_devices: Vec<String>,

    // Tray icon (must stay alive or the icon disappears)
    pub _tray_icon: Option<tray_icon::TrayIcon>,

    // Snip overlay state
    pub snip_overlay_active: bool,
    pub snip_texture: Option<TextureHandle>,
    pub snip_drag_start: Option<Pos2>,
    pub snip_drag_current: Option<Pos2>,
    pub snip_bounds: Option<snip::MonitorBounds>,
    pub snip_copy_image: bool,
    pub snip_edit_after: bool,
    pub snip_focus_pending: bool,

    // Window positioning
    pub positioned: bool,
    pub initial_position_corrected: bool,
    pub compact_anchor_pos: Option<Pos2>,

    // Error auto-recovery
    pub error_time: Option<std::time::Instant>,

    // Settings form fields
    pub form_provider: String,
    pub form_api_keys: HashMap<String, String>,
    pub form_model: String,
    pub form_language: String,
    pub form_mic: String,
    pub form_vad_mode: String,
    pub form_screenshot_enabled: bool,
    pub form_screenshot_retention_count: u32,
    pub form_start_cue: String,
    pub form_text_size: String,
    pub form_accent_color: String,
    pub form_window_monitor_mode: String,
    pub form_window_monitor_id: String,
    pub form_window_anchor: String,
    pub form_snip_editor_path: String,
    pub form_chrome_path: String,
    pub form_paint_path: String,
    pub form_provider_inactivity_timeout_secs: u64,
    pub form_max_session_length_minutes: u64,
    pub form_url_commands: Vec<crate::settings::UrlCommand>,
    pub form_alias_commands: Vec<crate::settings::AliasCommand>,
    pub key_check_inflight: HashSet<String>,
    pub key_check_result: HashMap<String, (bool, String)>,
    pub last_validated_provider: Option<String>,
    pub session_history: Vec<SessionUsage>,
    control_tooltip: Option<ControlTooltipState>,
    recording_limit_token: u64,
    confirm_reset_totals: bool,
    confirm_reset_include_sessions: bool,
    selected_mic_unavailable: bool,
}

impl JarvisApp {
    fn current_accent(&self) -> AccentPalette {
        if self.settings_open {
            accent_palette(&self.form_accent_color)
        } else {
            accent_palette(&self.settings.accent_color)
        }
    }

    fn persist_accent_if_changed(&mut self) {
        if self.settings.accent_color == self.form_accent_color {
            return;
        }
        self.settings.accent_color = self.form_accent_color.clone();
        match crate::settings::save(&self.settings) {
            Ok(()) => {
                self._tray_icon = setup_tray(accent_palette(&self.settings.accent_color));
            }
            Err(e) => {
                self.set_status(&format!("Save failed: {}", e), "error");
            }
        }
    }

    fn provider_form_dirty(&self) -> bool {
        if self.form_provider != self.settings.provider {
            return true;
        }
        for (provider_id, _) in PROVIDER_ROWS {
            let form_val = self
                .form_api_keys
                .get(*provider_id)
                .map(|s| s.as_str())
                .unwrap_or("");
            let current_val = self.settings.api_key_for(provider_id);
            if form_val != current_val {
                return true;
            }
        }
        false
    }

    fn compact_window_width(&self) -> f32 {
        if self.settings.screenshot_enabled {
            COMPACT_WINDOW_W_WITH_SNIP
        } else {
            COMPACT_WINDOW_W_NO_SNIP
        }
    }

    fn monitor_choices(&self) -> Vec<MonitorChoice> {
        available_monitor_choices()
    }

    fn monitor_label_for_id(&self, id: &str) -> String {
        if id.trim().is_empty() {
            return "Auto (cursor monitor)".into();
        }
        self.monitor_choices()
            .into_iter()
            .find(|m| m.id == id)
            .map(|m| m.label)
            .unwrap_or_else(|| format!("{} (disconnected)", id))
    }

    fn anchor_label(anchor: &str) -> &'static str {
        match anchor {
            WINDOW_ANCHOR_TOP_LEFT => "Top Left",
            WINDOW_ANCHOR_TOP_CENTER => "Top Center",
            WINDOW_ANCHOR_TOP_RIGHT => "Top Right",
            WINDOW_ANCHOR_BOTTOM_LEFT => "Bottom Left",
            WINDOW_ANCHOR_BOTTOM_CENTER => "Bottom Center",
            _ => "Bottom Right",
        }
    }

    fn provider_color(provider_id: &str, p: ThemePalette) -> Color32 {
        match provider_id {
            "openai" => Color32::from_rgb(0x10, 0xb9, 0x81),
            "deepgram" => Color32::from_rgb(0x3b, 0x82, 0xf6),
            "elevenlabs" => Color32::from_rgb(0xf5, 0x9e, 0x0b),
            "assemblyai" => Color32::from_rgb(0xa8, 0x55, 0xf7),
            _ => p.text,
        }
    }

    fn provider_display_name(provider_id: &str) -> &str {
        PROVIDER_ROWS
            .iter()
            .find(|(id, _)| *id == provider_id)
            .map(|(_, name)| *name)
            .unwrap_or(provider_id)
    }

    fn sync_form_from_settings(&mut self) {
        self.form_provider = self.settings.provider.clone();
        self.form_api_keys = self.settings.api_keys.clone();
        for (id, _) in PROVIDER_ROWS {
            self.form_api_keys.entry((*id).to_string()).or_default();
        }
        self.form_model = self.settings.model.clone();
        self.form_language = self.settings.language.clone();
        self.form_mic = self.settings.mic_device.clone();
        self.form_vad_mode = self.settings.vad_mode.clone();
        self.form_screenshot_enabled = self.settings.screenshot_enabled;
        self.form_screenshot_retention_count = self.settings.screenshot_retention_count;
        self.form_start_cue = self.settings.start_cue.clone();
        self.form_text_size = self.settings.text_size.clone();
        self.form_accent_color = self.settings.accent_color.clone();
        self.form_window_monitor_mode = WINDOW_MONITOR_MODE_FIXED.to_string();
        self.form_window_monitor_id = self.settings.window_monitor_id.clone();
        self.form_window_anchor = self.settings.window_anchor.clone();
        self.form_snip_editor_path = self.settings.snip_editor_path.clone();
        self.form_chrome_path = self.settings.chrome_path.clone();
        self.form_paint_path = self.settings.paint_path.clone();
        self.form_provider_inactivity_timeout_secs =
            self.settings.provider_inactivity_timeout_secs;
        self.form_max_session_length_minutes = self.settings.max_session_length_minutes;
        self.form_url_commands = self.settings.url_commands.clone();
        self.form_alias_commands = self.settings.alias_commands.clone();
        self.key_check_inflight.clear();
        self.key_check_result.clear();
        self.last_validated_provider = None;
    }

    pub fn new(
        state: Arc<AppState>,
        event_tx: EventSender<AppEvent>,
        event_rx: EventReceiver<AppEvent>,
        runtime: Arc<tokio::runtime::Runtime>,
        settings: Settings,
        _egui_ctx: egui::Context,
    ) -> Self {
        let mic_devices = audio::list_input_devices();
        let form_provider = settings.provider.clone();
        let mut form_api_keys = settings.api_keys.clone();
        for (id, _) in PROVIDER_ROWS {
            form_api_keys.entry((*id).to_string()).or_default();
        }
        let form_model = settings.model.clone();
        let form_language = settings.language.clone();
        let form_mic = settings.mic_device.clone();
        let form_vad_mode = settings.vad_mode.clone();
        let form_screenshot_enabled = settings.screenshot_enabled;
        let form_screenshot_retention_count = settings.screenshot_retention_count;
        let form_start_cue = settings.start_cue.clone();
        let form_text_size = settings.text_size.clone();
        let form_accent_color = settings.accent_color.clone();
        let form_window_monitor_mode = WINDOW_MONITOR_MODE_FIXED.to_string();
        let form_window_monitor_id = settings.window_monitor_id.clone();
        let form_window_anchor = settings.window_anchor.clone();
        let form_snip_editor_path = settings.snip_editor_path.clone();
        let form_chrome_path = settings.chrome_path.clone();
        let form_paint_path = settings.paint_path.clone();
        let form_provider_inactivity_timeout_secs = settings.provider_inactivity_timeout_secs;
        let form_max_session_length_minutes = settings.max_session_length_minutes;
        let form_url_commands = settings.url_commands.clone();
        let form_alias_commands = settings.alias_commands.clone();

        // Create tray icon here (inside the event loop) so it stays alive
        let tray_icon = setup_tray(accent_palette(&settings.accent_color));
        println!("[tray] icon created: {}", tray_icon.is_some());

        // Background thread for tray events so quit is handled even if the UI thread stalls.
        {
            std::thread::spawn(move || {
                while let Ok(event) = tray_icon::menu::MenuEvent::receiver().recv() {
                    let id = event.id.0.as_str();
                    println!("[tray-thread] menu event: {}", id);
                    match id {
                        "quit" => {
                            println!("[tray-thread] quit — calling process::exit");
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                }
            });
        }

        let app = Self {
            state,
            event_tx,
            event_rx,
            runtime,
            settings,
            settings_open: false,
            settings_tab: "provider".into(),
            status_text: "Ready".into(),
            status_state: "idle".into(),
            is_recording: false,
            audio_capture: None,
            should_quit: false,
            mic_devices,
            _tray_icon: tray_icon,
            positioned: false,
            initial_position_corrected: false,
            compact_anchor_pos: None,
            snip_overlay_active: false,
            snip_texture: None,
            snip_drag_start: None,
            snip_drag_current: None,
            snip_bounds: None,
            snip_copy_image: false,
            snip_edit_after: false,
            snip_focus_pending: false,
            error_time: None,
            form_provider,
            form_api_keys,
            form_model,
            form_language,
            form_mic,
            form_vad_mode,
            form_screenshot_enabled,
            form_screenshot_retention_count,
            form_start_cue,
            form_text_size,
            form_accent_color,
            form_window_monitor_mode,
            form_window_monitor_id,
            form_window_anchor,
            form_snip_editor_path,
            form_chrome_path,
            form_paint_path,
            form_provider_inactivity_timeout_secs,
            form_max_session_length_minutes,
            form_url_commands,
            form_alias_commands,
            key_check_inflight: HashSet::new(),
            key_check_result: HashMap::new(),
            last_validated_provider: None,
            session_history: vec![],
            control_tooltip: None,
            recording_limit_token: 0,
            confirm_reset_totals: false,
            confirm_reset_include_sessions: false,
            selected_mic_unavailable: false,
        };
        app
    }

    fn selected_mic_unavailable_now(&self) -> bool {
        if self.settings.mic_device.trim().is_empty() {
            return false;
        }
        let devices = crate::audio::list_input_devices();
        !devices.iter().any(|d| d == &self.settings.mic_device)
    }

    

    fn apply_appearance(&self, ctx: &egui::Context) {
        // Only apply global appearance settings on the root viewport.
        // Applying zoom on immediate child viewports (snip overlay) causes jitter.
        if ctx.viewport_id() != ViewportId::ROOT {
            return;
        }

        let mut style = egui::Style::default();
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(8.0, 5.0);
        style.spacing.interact_size.y = 24.0;
        ctx.set_visuals(egui::Visuals::dark());
        // Keep zoom fixed to avoid snip coordinate distortion across mixed-DPI monitors.
        if (ctx.zoom_factor() - 1.0).abs() > 0.001 {
            ctx.set_zoom_factor(1.0);
        }
        ctx.set_style(style);
    }

    fn expanded_window_size(&self, ctx: &egui::Context) -> egui::Vec2 {
        if let Some(work) = work_area_rect_logical(
            ctx,
            &self.settings.window_monitor_mode,
            &self.settings.window_monitor_id,
        ) {
            let margin = 24.0;
            let monitor_w = work.width().max(COMPACT_WINDOW_W_WITH_SNIP + margin * 2.0);
            let monitor_h = work.height().max(420.0 + margin * 2.0);
            let max_w = (monitor_w - margin * 2.0).max(COMPACT_WINDOW_W_WITH_SNIP);
            let max_h = (monitor_h - margin * 2.0).max(420.0);
            let w = (monitor_w * 0.5).max(820.0).min(max_w);
            // Hard cap expanded height so settings never grow into taskbar on smaller displays.
            let h = (monitor_h * 0.72).max(520.0).min(max_h.min(760.0));
            return vec2(w, h);
        }
        vec2(980.0, 720.0)
    }

    fn apply_window_mode(&mut self, ctx: &egui::Context, settings_open: bool) {
        let target = if settings_open {
            self.expanded_window_size(ctx)
        } else {
            vec2(self.compact_window_width(), COMPACT_WINDOW_H)
        };
        if settings_open {
            // Remember exact compact location so collapse can restore pixel-perfect.
            if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                self.compact_anchor_pos = Some(outer.min);
                // Expand from compact anchor so right/bottom edges align with collapsed mode.
                let compact_w = outer.width();
                let compact_h = outer.height();
                let new_x = outer.min.x + compact_w - target.x;
                let new_y = outer.min.y + compact_h - target.y;
                let pos = clamp_window_pos(
                    ctx,
                    pos2(new_x, new_y),
                    target,
                    &self.settings.window_monitor_mode,
                    &self.settings.window_monitor_id,
                );
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
            }
        } else if self.settings.window_monitor_mode == WINDOW_MONITOR_MODE_FIXED {
            let _ = place_compact_fixed_native(
                target,
                &self.settings.window_monitor_id,
                &self.settings.window_anchor,
            );
        } else if let Some(anchor) = self.compact_anchor_pos {
            // Restore to the exact compact position where expansion started.
            let pos = clamp_window_pos(
                ctx,
                anchor,
                target,
                &self.settings.window_monitor_mode,
                &self.settings.window_monitor_id,
            );
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        } else if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
            // Fallback if no anchor was captured yet.
            let br = outer.max;
            let new_x = br.x - target.x;
            let new_y = br.y - target.y;
            let pos = clamp_window_pos(
                ctx,
                pos2(new_x, new_y),
                target,
                &self.settings.window_monitor_mode,
                &self.settings.window_monitor_id,
            );
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        }
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(target));
    }

    fn set_status(&mut self, text: &str, state: &str) {
        self.status_text = text.into();
        self.status_state = state.into();
        if state == "error" {
            self.error_time = Some(std::time::Instant::now());
        } else {
            self.error_time = None;
        }
    }

    fn start_recording(&mut self) {
        if self.is_recording {
            return;
        }
        let unavailable_now = self.selected_mic_unavailable_now();
        self.selected_mic_unavailable = unavailable_now;
        if unavailable_now {
            self.set_status("Device unavailable. Change in Settings.", "error");
            return;
        }

        if let Err(e) = crate::start_cue::play_start_cue(&self.settings.start_cue) {
            eprintln!("[ui] start cue error: {}", e);
        }

        self.is_recording = true;
        // Update VAD mode from settings (strict/lenient).
        let mode = match self.settings.vad_mode.as_str() {
            "lenient" => 1,
            _ => 0,
        };
        self.state.vad_mode.store(mode, Ordering::SeqCst);

        let (audio_tx, audio_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(256);
        if let Ok(mut tx) = self.state.audio_tx.lock() {
            *tx = Some(audio_tx.clone());
        }
        if let Ok(mut active) = self.state.session_active.lock() {
            *active = true;
        }

        let provider = crate::provider::create_provider(&self.settings.provider);
        let current_key = self.settings.api_key_for(&self.settings.provider).to_string();
        let provider_settings = crate::provider::ProviderSettings {
            api_key: current_key.clone(),
            model: self.settings.model.clone(),
            transcription_model: self.settings.transcription_model.clone(),
            language: self.settings.language.clone(),
        };
        let sample_rate = provider.sample_rate_hint();

        // Always start audio capture (drives the visualizer FFT)
        let mic = if self.settings.mic_device.is_empty() {
            None
        } else {
            Some(self.settings.mic_device.as_str())
        };
        match audio::AudioCapture::start(
            mic,
            audio_tx,
            self.event_tx.clone(),
            self.state.clone(),
            sample_rate,
        ) {
            Ok(capture) => {
                println!("[ui] audio capture started");
                self.audio_capture = Some(capture);
            }
            Err(e) => {
                eprintln!("[ui] audio capture error: {}", e);
                self.set_status(&format!("Mic error: {}", e), "error");
                self.is_recording = false;
                return;
            }
        }

        // Hard cap session length to prevent runaway costs/noise loops.
        self.recording_limit_token = self.recording_limit_token.saturating_add(1);
        let limit_token = self.recording_limit_token;
        let max_minutes = self.settings.max_session_length_minutes.clamp(1, 120);
        let max_duration = Duration::from_secs(max_minutes.saturating_mul(60));
        let max_event_tx = self.event_tx.clone();
        self.runtime.spawn(async move {
            tokio::time::sleep(max_duration).await;
            let _ = max_event_tx.send(AppEvent::SessionMaxDurationReached {
                token: limit_token,
                minutes: max_minutes,
            });
        });

        // Only connect to provider if we have an API key
        if current_key.is_empty() {
            self.set_status("Listening (no API key)", "live");
            return;
        }

        let gen = self.state.session_gen.fetch_add(1, Ordering::SeqCst) + 1;
        // Initialize per-session usage.
        let now = now_ms();
        if let Ok(mut session) = self.state.session_usage.lock() {
            *session = crate::state::SessionUsage {
                session_id: now,
                provider: self.settings.provider.clone(),
                bytes_sent: 0,
                ms_sent: 0,
                ms_suppressed: 0,
                commits: 0,
                finals: 0,
                started_ms: now,
                updated_ms: now,
            };
        }

        let event_tx = self.event_tx.clone();
        let state_clone = self.state.clone();
        let inactivity_timeout_secs = self.settings.provider_inactivity_timeout_secs;

        self.runtime.spawn(async move {
            crate::provider::session::run_session(
                provider,
                event_tx,
                state_clone.clone(),
                provider_settings,
                audio_rx,
                inactivity_timeout_secs,
            )
            .await;

            if state_clone.session_gen.load(Ordering::SeqCst) == gen {
                if let Ok(mut active) = state_clone.session_active.lock() {
                    *active = false;
                }
                if let Ok(mut tx) = state_clone.audio_tx.lock() {
                    *tx = None;
                }
                state_clone
                    .hotkey_recording
                    .store(false, Ordering::SeqCst);
            }
        });

        self.set_status("Connecting...", "live");
    }

    fn stop_recording(&mut self) {
        if !self.is_recording {
            return;
        }
        if let Err(e) = crate::start_cue::play_stop_cue() {
            eprintln!("[ui] stop cue error: {}", e);
        }
        self.is_recording = false;
        self.audio_capture = None;

        if let Ok(mut tx) = self.state.audio_tx.lock() {
            *tx = None;
        }
        if let Ok(mut active) = self.state.session_active.lock() {
            *active = false;
        }
        self.state.hotkey_recording.store(false, Ordering::SeqCst);

        if let Ok(mut data) = self.state.fft_data.lock() {
            *data = [0.0; 50];
        }

        self.set_status("Ready", "idle");

        // Persist and reset per-session usage (skip 0-byte sessions).
        if let Ok(mut session) = self.state.session_usage.lock() {
            if session.started_ms != 0 && session.bytes_sent > 0 {
                if let Ok(path) = session_usage_path() {
                    let snapshot = session.clone();
                    let _ = append_usage_line(&path, &snapshot);
                }
            }
            *session = crate::state::SessionUsage::default();
        }
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AppEvent::HotkeyPush => self.start_recording(),
                AppEvent::HotkeyRelease => self.stop_recording(),
                AppEvent::StatusUpdate { status, message } => self.set_status(&message, &status),
                AppEvent::TranscriptDelta(text) => {
                    let _ = text;
                }
                AppEvent::TranscriptFinal(text) => {
                    let _ = text;
                }
                AppEvent::SnipTrigger => self.trigger_snip(),
                AppEvent::SessionInactivityTimeout { seconds } => {
                    if self.is_recording {
                        self.stop_recording();
                        self.set_status(
                            &format!("Stopped after {}s inactivity", seconds),
                            "idle",
                        );
                    }
                }
                AppEvent::SessionMaxDurationReached { token, minutes } => {
                    if self.is_recording && token == self.recording_limit_token {
                        self.stop_recording();
                        self.set_status(
                            &format!("Stopped at max session length ({}m)", minutes),
                            "idle",
                        );
                    }
                }
                AppEvent::ApiKeyValidated {
                    provider,
                    ok,
                    message,
                } => {
                    self.key_check_inflight.remove(&provider);
                    self.last_validated_provider = Some(provider.clone());
                    self.key_check_result.insert(provider, (ok, message));
                }
                AppEvent::AudioInputLost { message } => {
                    eprintln!("[ui] audio input lost: {}", message);
                    if self.is_recording {
                        self.stop_recording();
                    }
                    if !self.settings.mic_device.trim().is_empty() {
                        self.selected_mic_unavailable = true;
                        self.set_status("Device unavailable. Change in Settings.", "error");
                    } else {
                        self.set_status("Mic disconnected", "error");
                    }
                }
            }
        }
    }

    fn trigger_snip(&mut self) {
        if !self.state.screenshot_enabled.load(Ordering::SeqCst) {
            return;
        }
        let cursor = self.state.cursor_pos.lock().ok().and_then(|v| *v);
        let state = self.state.clone();

        match snip::capture_screen(cursor) {
            Ok((img, bounds)) => {
                if let Ok(mut guard) = state.snip_image.lock() {
                    *guard = Some(img);
                }
                self.snip_bounds = Some(bounds);
                self.snip_overlay_active = true;
                self.snip_texture = None;
                self.snip_drag_start = None;
                self.snip_drag_current = None;
                self.snip_focus_pending = true;
            }
            Err(e) => {
                eprintln!("[ui] capture error: {}", e);
                state.snip_active.store(false, Ordering::SeqCst);
            }
        }
    }

    fn finish_snip(&mut self, x: u32, y: u32, w: u32, h: u32) {
        let img = {
            let mut guard = self.state.snip_image.lock().unwrap();
            guard.take()
        };
        if let Some(img) = img {
            match snip::crop_and_save(
                &img,
                x,
                y,
                w,
                h,
                self.settings.screenshot_retention_count as usize,
            ) {
                Ok((path, cropped)) => {
                    if self.snip_copy_image {
                        let _ = snip::copy_image_to_clipboard(&cropped);
                    } else {
                        let _ = snip::copy_path_to_clipboard(&path);
                    }
                    if self.snip_edit_after {
                        if let Err(e) = snip::open_in_editor(
                            &path,
                            Some(self.settings.snip_editor_path.as_str()),
                        ) {
                            eprintln!("[snip] editor error: {}", e);
                        }
                        self.snip_edit_after = false;
                    }
                    println!("[snip] saved to {}", path.to_string_lossy());
                }
                Err(e) => eprintln!("[snip] save error: {}", e),
            }
        }
        self.close_snip();
    }

    fn cancel_snip(&mut self) {
        if let Ok(mut guard) = self.state.snip_image.lock() {
            *guard = None;
        }
        self.close_snip();
        println!("[snip] cancelled");
    }

    fn close_snip(&mut self) {
        self.snip_overlay_active = false;
        self.snip_texture = None;
        self.snip_drag_start = None;
        self.snip_drag_current = None;
        self.snip_bounds = None;
        self.state.snip_active.store(false, Ordering::SeqCst);
    }

    fn paint_control_tooltip(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        key: &str,
        text: &str,
        persist_on_click: bool,
    ) {
        let accent = self.current_accent();
        let now = ctx.input(|i| i.time);
        if response.clicked() && persist_on_click {
            self.control_tooltip = Some(ControlTooltipState {
                key: key.to_string(),
                text: text.to_string(),
                until: now + 0.9,
            });
        }

        let persisted = self
            .control_tooltip
            .as_ref()
            .is_some_and(|t| t.key == key && t.until > now);
        if !response.hovered() && !persisted {
            return;
        }

        let label_text = if response.hovered() {
            text.to_string()
        } else if let Some(tip) = &self.control_tooltip {
            tip.text.clone()
        } else {
            text.to_string()
        };

        let pos = pos2(response.rect.center().x, response.rect.min.y - 6.0);
        egui::Area::new(egui::Id::new(format!("control_tooltip_{key}")))
            .order(egui::Order::Foreground)
            .interactable(false)
            .constrain(false)
            .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, 0.0))
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(accent.tint_bg)
                    .stroke(Stroke::new(1.0, accent.base))
                    .rounding(4.0)
                    .inner_margin(egui::Margin::symmetric(6.0, 3.0))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(label_text)
                                    .size(11.0)
                                    .color(Color32::BLACK),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
            });
    }

    fn render_main_ui(&mut self, ctx: &egui::Context) {
        let p = theme_palette(true);
        let accent = self.current_accent();
        let panel_fill = if self.settings_open {
            p.settings_bg
        } else {
            Color32::TRANSPARENT
        };
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(panel_fill)
                    .inner_margin(egui::Margin::symmetric(12.0, 12.0)),
            )
            .show(ctx, |ui| {
                // --- Top control row ---
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    let record_resp = record_toggle(ui, self.is_recording, accent);
                    let record_tip =
                        if self.is_recording { "Stop recording" } else { "Start recording" };
                    self.paint_control_tooltip(ctx, &record_resp, "record", record_tip, true);
                    if record_resp.clicked() {
                        if self.is_recording {
                            self.stop_recording();
                        } else {
                            self.start_recording();
                        }
                    }
                    // Manual preset picker (no popup, no cycling):
                    // P = Path, I = Image, E = Image + Edit.
                    let preset_btn = |ui: &mut egui::Ui,
                                      label: &str,
                                      active: bool,
                                      p: ThemePalette| {
                        ui.add(
                            egui::Button::new(
                                egui::RichText::new(label)
                                    .size(11.0)
                                    .strong()
                                    .color(if active { Color32::WHITE } else { p.text }),
                            )
                            .fill(if active { BTN_PRIMARY } else { p.btn_bg })
                            .stroke(Stroke::new(1.0, p.btn_border))
                            .rounding(4.0)
                            .min_size(vec2(20.0, 22.0)),
                        )
                    };

                    let show_screenshot_controls = self.settings.screenshot_enabled;
                    let settings_w = 28.0;
                    let row_gap = 4.0;
                    let right_edge_pad = 6.0;
                    let right_controls_w = if show_screenshot_controls {
                        20.0 * 3.0 + settings_w + row_gap * 3.0 + right_edge_pad
                    } else {
                        settings_w + right_edge_pad
                    };
                    let min_viz_w = if show_screenshot_controls { 36.0 } else { 56.0 };
                    let viz_w = (ui.available_width() - right_controls_w).max(min_viz_w);
                    let fft = self
                        .state
                        .fft_data
                        .lock()
                        .map(|d| *d)
                        .unwrap_or([0.0; 50]);
                    let t = ctx.input(|i| i.time) as f32;
                    let (viz_rect, _) =
                        ui.allocate_exact_size(vec2(viz_w, 20.0), Sense::hover());
                    draw_dancing_strings(
                        ui.painter(),
                        viz_rect,
                        t,
                        if self.is_recording { Some(&fft) } else { None },
                        accent,
                    );
                    if self.selected_mic_unavailable {
                        let icon_size = vec2(20.0, 22.0);
                        let icon_rect = Rect::from_center_size(viz_rect.center(), icon_size);
                        let mic_resp = mic_unavailable_badge(ui, icon_rect);
                        self.paint_control_tooltip(
                            ctx,
                            &mic_resp,
                            "mic_unavailable",
                            "Device unavailable. Open Settings.",
                            false,
                        );
                    }

                    if show_screenshot_controls {
                        let p_resp = preset_btn(
                            ui,
                            "P",
                            !self.snip_copy_image,
                            p,
                        );
                        self.paint_control_tooltip(
                            ctx,
                            &p_resp,
                            "preset_path",
                            "Preset: Path (copy file path)",
                            true,
                        );
                        if p_resp.clicked() {
                            self.snip_copy_image = false;
                            self.snip_edit_after = false;
                        }
                        let i_resp = preset_btn(
                            ui,
                            "I",
                            self.snip_copy_image && !self.snip_edit_after,
                            p,
                        );
                        self.paint_control_tooltip(
                            ctx,
                            &i_resp,
                            "preset_image",
                            "Preset: Image (copy image)",
                            true,
                        );
                        if i_resp.clicked() {
                            self.snip_copy_image = true;
                            self.snip_edit_after = false;
                        }
                        let e_resp = preset_btn(
                            ui,
                            "E",
                            self.snip_copy_image && self.snip_edit_after,
                            p,
                        );
                        self.paint_control_tooltip(
                            ctx,
                            &e_resp,
                            "preset_edit",
                            "Preset: Image + Edit",
                            true,
                        );
                        if e_resp.clicked() {
                            self.snip_copy_image = true;
                            self.snip_edit_after = true;
                        }
                    }
                    if self.settings_open {
                        if window_ctrl_btn(ui, "-", false).clicked() {
                            self.persist_accent_if_changed();
                            self.settings_open = false;
                            self.apply_window_mode(ctx, false);
                        }
                    } else {
                        let settings_resp = settings_toggle(ui, self.is_recording, accent);
                        self.paint_control_tooltip(
                            ctx,
                            &settings_resp,
                            "settings",
                            "Settings",
                            false,
                        );
                        if settings_resp.clicked() {
                            self.settings_open = true;
                            self.sync_form_from_settings();
                            self.session_history = crate::usage::load_recent_sessions(20);
                            self.apply_window_mode(ctx, true);
                        }
                    }
                    ui.add_space(right_edge_pad);
                });

                ui.add_space(if self.settings_open { 6.0 } else { 2.0 });

                // --- Collapsible settings panel ---
                if self.settings_open {
                    ui.add_space(8.0);
                    egui::Frame::none()
                        .fill(p.settings_bg)
                        .stroke(Stroke::new(1.0, p.btn_border))
                        .rounding(6.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;

                            // ── Tab bar ──
                            let prev_tab = self.settings_tab.clone();
                            ui.horizontal_top(|ui| {
                                let nav_w = 170.0;
                                ui.allocate_ui_with_layout(
                                    vec2(nav_w, ui.available_height()),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new("Settings")
                                                .size(12.0)
                                                .strong()
                                                .color(p.text_muted),
                                        );
                                        ui.add_space(6.0);

                                        for (id, label) in [
                                            ("provider", "Provider"),
                                            ("audio", "Audio"),
                                            ("color", "Color"),
                                            ("screenshot", "Screenshot"),
                                            ("advanced", "Advanced"),
                                            ("configurations", "Configurations"),
                                            ("commands", "Commands"),
                                            ("usage", "Usage"),
                                            ("faq", "FAQ"),
                                            ("about", "About"),
                                        ] {
                                            let active = self.settings_tab == id;
                                            let text = if active {
                                                egui::RichText::new(label)
                                                    .size(12.0)
                                                    .strong()
                                                    .color(p.text)
                                            } else {
                                                egui::RichText::new(label)
                                                    .size(12.0)
                                                    .color(p.text_muted)
                                            };
                                            let btn = egui::Button::new(text)
                                                .fill(if active {
                                                    accent.base
                                                } else {
                                                    Color32::TRANSPARENT
                                                })
                                                .stroke(Stroke::new(
                                                    1.0,
                                                    if active { accent.ring } else { p.btn_border },
                                                ))
                                                .rounding(6.0)
                                                .min_size(vec2(nav_w - 8.0, 28.0));
                                            if ui.add(btn).clicked() {
                                                self.settings_tab = id.to_string();
                                            }
                                        }
                                    },
                                );

                                ui.separator();
                                ui.add_space(8.0);
                                ui.vertical(|ui| {
                                    if self.settings_tab == "usage" && prev_tab != "usage" {
                                        self.session_history =
                                            crate::usage::load_recent_sessions(20);
                                    }
                                    ui.add_space(2.0);

                            // ── Tab content ──
                            match self.settings_tab.as_str() {
                                "provider" => {
                                    let current_provider_name = PROVIDER_ROWS
                                        .iter()
                                        .find(|(id, _)| *id == self.settings.provider.as_str())
                                        .map(|(_, name)| *name)
                                        .unwrap_or("Unknown");
                                    let current_provider_color =
                                        Self::provider_color(&self.settings.provider, p);
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Current Provider:")
                                                .size(14.0)
                                                .strong()
                                                .color(p.text_muted),
                                        );
                                        ui.label(
                                            egui::RichText::new(current_provider_name)
                                                .size(14.0)
                                                .strong()
                                                .color(current_provider_color),
                                        );
                                    });
                                    ui.add_space(8.0);

                                    let total_w = ui.available_width();
                                    let provider_w = 200.0;
                                    let validate_w = 92.0;
                                    let default_w = 72.0;
                                    let row_pad_x = 8.0;
                                    let spacing_w = 32.0;
                                    let api_w = (total_w
                                        - provider_w
                                        - validate_w
                                        - default_w
                                        - row_pad_x * 2.0
                                        - spacing_w)
                                        .max(160.0);

                                    ui.horizontal(|ui| {
                                        ui.set_width(total_w - row_pad_x * 2.0);
                                        ui.add_space(row_pad_x);
                                        ui.add_sized(
                                            [default_w, 20.0],
                                            egui::Label::new(
                                                egui::RichText::new("Default")
                                                    .size(13.0)
                                                    .strong()
                                                    .color(p.text_muted),
                                            ),
                                        );
                                        ui.add_sized(
                                            [provider_w, 20.0],
                                            egui::Label::new(
                                                egui::RichText::new("Provider")
                                                    .size(13.0)
                                                    .strong()
                                                    .color(p.text_muted),
                                            ),
                                        );
                                        ui.add_sized(
                                            [api_w, 20.0],
                                            egui::Label::new(
                                                egui::RichText::new("API Key")
                                                    .size(13.0)
                                                    .strong()
                                                    .color(p.text_muted),
                                            ),
                                        );
                                        ui.add_sized(
                                            [validate_w, 20.0],
                                            egui::Label::new(
                                                egui::RichText::new("Validate")
                                                    .size(13.0)
                                                    .strong()
                                                    .color(p.text_muted),
                                            ),
                                        );
                                    });
                                    ui.add_space(2.0);

                                    for (provider_id, provider_name) in PROVIDER_ROWS {
                                        let provider_id = (*provider_id).to_string();
                                        egui::Frame::none()
                                            .fill(p.btn_bg)
                                            .stroke(Stroke::new(1.0, p.btn_border))
                                            .rounding(6.0)
                                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                            .show(ui, |ui| {
                                                ui.set_width(total_w);
                                                ui.horizontal(|ui| {
                                                    let key_value = self
                                                        .form_api_keys
                                                        .entry(provider_id.clone())
                                                        .or_default();
                                                    let can_default = !key_value.trim().is_empty();
                                                    let is_default = self.form_provider == provider_id;
                                                    let default_resp = ui
                                                        .allocate_ui_with_layout(
                                                            vec2(default_w, 24.0),
                                                            egui::Layout::centered_and_justified(
                                                                egui::Direction::LeftToRight,
                                                            ),
                                                            |ui| {
                                                                provider_default_button(
                                                                    ui,
                                                                    can_default,
                                                                    is_default,
                                                                    accent,
                                                                )
                                                            },
                                                        )
                                                        .inner;
                                                    if default_resp.clicked() && can_default {
                                                        self.form_provider = provider_id.clone();
                                                    }

                                                    let provider_color =
                                                        Self::provider_color(&provider_id, p);
                                                    ui.add_sized(
                                                        [provider_w, 24.0],
                                                        egui::Label::new(
                                                            egui::RichText::new(*provider_name)
                                                                .size(13.0)
                                                                .strong()
                                                                .color(provider_color),
                                                        )
                                                        .wrap_mode(egui::TextWrapMode::Truncate),
                                                    );

                                                    let key_resp = ui
                                                        .scope(|ui| {
                                                            let dark = ui.visuals().dark_mode;
                                                            let input_bg = if dark {
                                                                Color32::from_rgb(0x1a, 0x1d, 0x24)
                                                            } else {
                                                                Color32::from_rgb(0xff, 0xff, 0xff)
                                                            };
                                                            let input_stroke = if dark {
                                                                Color32::from_rgb(0x2c, 0x2f, 0x36)
                                                            } else {
                                                                Color32::from_rgb(0xd1, 0xd5, 0xdb)
                                                            };
                                                            let visuals = ui.visuals_mut();
                                                            visuals.extreme_bg_color = input_bg;
                                                            visuals.widgets.inactive.bg_fill = input_bg;
                                                            visuals.widgets.hovered.bg_fill = input_bg;
                                                            visuals.widgets.active.bg_fill = input_bg;
                                                            visuals.widgets.inactive.bg_stroke =
                                                                Stroke::new(1.0, input_stroke);
                                                            visuals.widgets.hovered.bg_stroke =
                                                                Stroke::new(1.0, input_stroke);
                                                            visuals.widgets.active.bg_stroke =
                                                                Stroke::new(1.0, input_stroke);
                                                            ui.add_sized(
                                                                [api_w, 24.0],
                                                                egui::TextEdit::singleline(key_value)
                                                                    .password(true)
                                                                    .font(FontId::proportional(12.5)),
                                                            )
                                                        })
                                                        .inner;
                                                    if key_resp.changed() {
                                                        self.key_check_result.remove(&provider_id);
                                                        if self
                                                            .last_validated_provider
                                                            .as_deref()
                                                            == Some(provider_id.as_str())
                                                        {
                                                            self.last_validated_provider = None;
                                                        }
                                                    }

                                                    let key_present = !key_value.trim().is_empty();
                                                    let inflight =
                                                        self.key_check_inflight.contains(&provider_id);
                                                    let result =
                                                        self.key_check_result.get(&provider_id).cloned();
                                                    let validate_resp = ui
                                                        .allocate_ui_with_layout(
                                                            vec2(validate_w, 24.0),
                                                            egui::Layout::centered_and_justified(
                                                                egui::Direction::LeftToRight,
                                                            ),
                                                            |ui| {
                                                                provider_validate_button(
                                                                    ui,
                                                                    key_present,
                                                                    inflight,
                                                                    result
                                                                        .as_ref()
                                                                        .map(|(ok, _)| *ok),
                                                                    accent,
                                                                )
                                                            },
                                                        )
                                                        .inner;
                                                    if validate_resp.clicked() && key_present && !inflight {
                                                        self.key_check_inflight.insert(provider_id.clone());
                                                        self.key_check_result.remove(&provider_id);
                                                        self.last_validated_provider =
                                                            Some(provider_id.clone());
                                                        let provider_name = PROVIDER_ROWS
                                                            .iter()
                                                            .find(|(id, _)| *id == provider_id.as_str())
                                                            .map(|(_, name)| (*name).to_string())
                                                            .unwrap_or_else(|| provider_id.clone());
                                                        let provider = crate::provider::create_provider(&provider_id);
                                                        let provider_settings = crate::provider::ProviderSettings {
                                                            api_key: key_value.clone(),
                                                            model: self.form_model.clone(),
                                                            transcription_model: self.settings.transcription_model.clone(),
                                                            language: self.form_language.clone(),
                                                        };
                                                        let event_tx = self.event_tx.clone();
                                                        let validated_provider_id = provider_id.clone();
                                                        self.runtime.spawn(async move {
                                                            let result = crate::provider::session::validate_key(
                                                                provider,
                                                                provider_settings,
                                                            )
                                                            .await;
                                                            let (ok, message) = match result {
                                                                Ok(()) => (
                                                                    true,
                                                                    format!(
                                                                        "{} API key is valid",
                                                                        provider_name
                                                                    ),
                                                                ),
                                                                Err(e) => (
                                                                    false,
                                                                    format!(
                                                                        "{} validation failed: {}",
                                                                        provider_name, e
                                                                    ),
                                                                ),
                                                            };
                                                            let _ = event_tx.send(AppEvent::ApiKeyValidated {
                                                                provider: validated_provider_id,
                                                                ok,
                                                                message,
                                                            });
                                                        });
                                                    }
                                                    validate_resp.on_hover_text(
                                                        if inflight {
                                                            "Validating..."
                                                        } else if let Some((ok, msg)) = &result {
                                                            if *ok { "Validated" } else { msg.as_str() }
                                                        } else if key_present {
                                                            "Validate key"
                                                        } else {
                                                            "Enter API key first"
                                                        },
                                                    );
                                                    default_resp.on_hover_text(if can_default {
                                                        if is_default {
                                                            "Default provider"
                                                        } else {
                                                            "Set as default provider"
                                                        }
                                                    } else {
                                                        "Enter API key first"
                                                    });

                                                });
                                            });
                                        ui.add_space(6.0);
                                    }

                                    if let Some(provider_id) =
                                        self.last_validated_provider.as_ref()
                                    {
                                        if let Some((ok, msg)) =
                                            self.key_check_result.get(provider_id)
                                        {
                                            let color = if *ok { accent.base } else { RED };
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new(msg)
                                                    .size(11.0)
                                                    .color(color),
                                            );
                                        }
                                    }
                                    if self
                                        .form_api_keys
                                        .get(&self.form_provider)
                                        .map(|k| k.trim().is_empty())
                                        .unwrap_or(true)
                                    {
                                        ui.add_space(2.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "Default provider must have an API key.",
                                            )
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                        );
                                    }
                                }
                                "audio" => {
                                  egui::ScrollArea::vertical()
                                    .max_height(ui.available_height())
                                    .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("VAD Mode")
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    egui::ComboBox::from_id_salt("vad_mode")
                                        .selected_text(
                                            match self.form_vad_mode.as_str() {
                                                "lenient" => "Lenient",
                                                _ => "Strict",
                                            },
                                        )
                                        .width(ui.available_width())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.form_vad_mode,
                                                "strict".to_string(),
                                                "Strict",
                                            );
                                            ui.selectable_value(
                                                &mut self.form_vad_mode,
                                                "lenient".to_string(),
                                                "Lenient",
                                            );
                                        });
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new("Microphone")
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.horizontal(|ui| {
                                        let combo_width = (ui.available_width() - 74.0).max(120.0);
                                        egui::ComboBox::from_id_salt("mic_select")
                                            .selected_text(if self.form_mic.is_empty() {
                                                "Default"
                                            } else {
                                                &self.form_mic
                                            })
                                            .width(combo_width)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut self.form_mic,
                                                    String::new(),
                                                    "Default",
                                                );
                                                for dev in &self.mic_devices {
                                                    ui.selectable_value(
                                                        &mut self.form_mic,
                                                        dev.clone(),
                                                        dev,
                                                    );
                                                }
                                            });
                                        if ui
                                            .add_sized([68.0, 22.0], egui::Button::new("Refresh"))
                                            .clicked()
                                        {
                                            self.mic_devices = audio::list_input_devices();
                                            if !self.form_mic.is_empty()
                                                && !self.mic_devices.contains(&self.form_mic)
                                            {
                                                self.form_mic.clear();
                                            }
                                        }
                                    });
                                    }); // end ScrollArea
                                }
                                "color" => {
                                    egui::ScrollArea::vertical()
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                            let total_w = ui.available_width();
                                            let select_w = 56.0;
                                            let color_w = (total_w - select_w - 16.0).max(120.0);

                                            ui.horizontal(|ui| {
                                                ui.add_sized(
                                                    [select_w, 20.0],
                                                    egui::Label::new(
                                                        egui::RichText::new("Select")
                                                            .size(13.0)
                                                            .strong()
                                                            .color(TEXT_MUTED),
                                                    ),
                                                );
                                                ui.add_sized(
                                                    [color_w, 20.0],
                                                    egui::Label::new(
                                                        egui::RichText::new("Color")
                                                            .size(13.0)
                                                            .strong()
                                                            .color(TEXT_MUTED),
                                                    ),
                                                );
                                            });
                                            ui.add_space(2.0);

                                            for choice in accent_options() {
                                                let is_selected =
                                                    self.form_accent_color == choice.id;
                                                egui::Frame::none()
                                                    .fill(p.btn_bg)
                                                    .stroke(Stroke::new(1.0, p.btn_border))
                                                    .rounding(6.0)
                                                    .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                                    .show(ui, |ui| {
                                                        ui.set_width(total_w);
                                                        ui.horizontal(|ui| {
                                                            let selector = ui
                                                                .allocate_ui_with_layout(
                                                                    vec2(select_w, 24.0),
                                                                    egui::Layout::centered_and_justified(
                                                                        egui::Direction::LeftToRight,
                                                                    ),
                                                                    |ui| {
                                                                        provider_default_button(
                                                                            ui,
                                                                            true,
                                                                            is_selected,
                                                                            accent,
                                                                        )
                                                                    },
                                                                )
                                                                .inner;
                                                            if selector.clicked() {
                                                                self.form_accent_color =
                                                                    choice.id.to_string();
                                                            }
                                                            ui.add_sized(
                                                                [color_w, 24.0],
                                                                egui::Label::new(
                                                                    egui::RichText::new(choice.name)
                                                                        .size(13.0)
                                                                        .strong()
                                                                        .color(if is_selected {
                                                                            accent.base
                                                                        } else {
                                                                            TEXT_COLOR
                                                                        }),
                                                                ),
                                                            );
                                                        });
                                                    });
                                                ui.add_space(2.0);
                                            }
                                            ui.label(
                                                egui::RichText::new(
                                                    "Applies to visualizer, start/settings controls, and accent highlights.",
                                                )
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                        });
                                }
                                "screenshot" => {
                                    egui::ScrollArea::vertical()
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new("Screenshot")
                                                    .size(12.0)
                                                    .strong()
                                                    .color(TEXT_COLOR),
                                            );
                                            ui.add_space(4.0);
                                            ui.checkbox(
                                                &mut self.form_screenshot_enabled,
                                                egui::RichText::new("Enable screenshot capture (Right Alt)")
                                                    .size(11.0)
                                                    .color(TEXT_COLOR),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new("Retention count")
                                                    .size(11.0)
                                                    .color(TEXT_MUTED),
                                            );
                                            ui.add(
                                                egui::Slider::new(
                                                    &mut self.form_screenshot_retention_count,
                                                    1..=200,
                                                )
                                                .text("images"),
                                            );
                                            ui.label(
                                                egui::RichText::new(
                                                    "When enabled, P / I / E buttons are shown and Right Alt triggers screenshot capture.",
                                                )
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                            ui.label(
                                                egui::RichText::new(
                                                    "When disabled, Right Alt behaves normally and screenshot controls are hidden.",
                                                )
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                            ui.add_space(10.0);
                                            section_header(ui, "Aliases");
                                            ui.label(
                                                egui::RichText::new(
                                                    "Experimental aliases: when trigger is heard, type replacement text.",
                                                )
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                            ui.add_space(2.0);

                                            let mut delete_alias_idx: Option<usize> = None;
                                            for (i, cmd) in
                                                self.form_alias_commands.iter_mut().enumerate()
                                            {
                                                let row_w = ui.available_width();
                                                let trigger_w = 120.0;
                                                let delete_w = 20.0;
                                                let spacing = ui.spacing().item_spacing.x;
                                                let replacement_w =
                                                    (row_w - trigger_w - delete_w - spacing * 2.0)
                                                        .max(180.0);

                                                ui.horizontal(|ui| {
                                                    ui.set_width(row_w);
                                                    ui.visuals_mut().extreme_bg_color =
                                                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                                                    ui.add_sized(
                                                        [trigger_w, 18.0],
                                                        egui::TextEdit::singleline(
                                                            &mut cmd.trigger,
                                                        )
                                                        .font(FontId::proportional(11.0))
                                                        .text_color(TEXT_COLOR),
                                                    );
                                                    ui.visuals_mut().extreme_bg_color =
                                                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                                                    ui.add_sized(
                                                        [replacement_w, 18.0],
                                                        egui::TextEdit::singleline(
                                                            &mut cmd.replacement,
                                                        )
                                                        .font(FontId::proportional(11.0))
                                                        .text_color(TEXT_COLOR),
                                                    );
                                                    if ui
                                                        .add_sized(
                                                            [delete_w, 18.0],
                                                            egui::Button::new(
                                                                egui::RichText::new("x")
                                                                    .size(11.0)
                                                                    .color(RED),
                                                            )
                                                            .fill(BTN_BG)
                                                            .stroke(Stroke::new(0.5, BTN_BORDER)),
                                                        )
                                                        .clicked()
                                                    {
                                                        delete_alias_idx = Some(i);
                                                    }
                                                });
                                            }
                                            if let Some(idx) = delete_alias_idx {
                                                self.form_alias_commands.remove(idx);
                                            }

                                            if ui
                                                .add_sized(
                                                    [ui.available_width(), 20.0],
                                                    egui::Button::new(
                                                        egui::RichText::new("+ Add Alias")
                                                            .size(11.0)
                                                            .color(TEXT_COLOR),
                                                    )
                                                    .fill(BTN_BG)
                                                    .stroke(Stroke::new(0.5, BTN_BORDER)),
                                                )
                                                .clicked()
                                            {
                                                self.form_alias_commands.push(
                                                    crate::settings::AliasCommand {
                                                        trigger: String::new(),
                                                        replacement: String::new(),
                                                    },
                                                );
                                            }
                                        });
                                }
                                "advanced" => {
                                    egui::ScrollArea::vertical()
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("Text Size")
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    egui::ComboBox::from_id_salt("text_size_select")
                                        .selected_text(match self.form_text_size.as_str() {
                                            "small" => "Small",
                                            "large" => "Large",
                                            _ => "Medium",
                                        })
                                        .width(ui.available_width())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.form_text_size,
                                                "small".to_string(),
                                                "Small",
                                            );
                                            ui.selectable_value(
                                                &mut self.form_text_size,
                                                "medium".to_string(),
                                                "Medium",
                                            );
                                            ui.selectable_value(
                                                &mut self.form_text_size,
                                                "large".to_string(),
                                                "Large",
                                            );
                                        });
                                    ui.add_space(8.0);
                                    section_header(ui, "Window Placement");
                                    ui.label(
                                        egui::RichText::new("Monitor")
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    let choices = self.monitor_choices();
                                    egui::ComboBox::from_id_salt("window_monitor_id_select")
                                        .selected_text(if self.form_window_monitor_id.trim().is_empty() {
                                            "Primary monitor".to_string()
                                        } else {
                                            self.monitor_label_for_id(&self.form_window_monitor_id)
                                        })
                                        .width(ui.available_width())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.form_window_monitor_id,
                                                String::new(),
                                                "Primary monitor",
                                            );
                                            for m in choices {
                                                ui.selectable_value(
                                                    &mut self.form_window_monitor_id,
                                                    m.id.clone(),
                                                    m.label,
                                                );
                                            }
                                        });
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new("Anchor")
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    egui::ComboBox::from_id_salt("window_anchor_select")
                                        .selected_text(Self::anchor_label(&self.form_window_anchor))
                                        .width(ui.available_width())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.form_window_anchor,
                                                WINDOW_ANCHOR_TOP_LEFT.to_string(),
                                                "Top Left",
                                            );
                                            ui.selectable_value(
                                                &mut self.form_window_anchor,
                                                WINDOW_ANCHOR_TOP_CENTER.to_string(),
                                                "Top Center",
                                            );
                                            ui.selectable_value(
                                                &mut self.form_window_anchor,
                                                WINDOW_ANCHOR_TOP_RIGHT.to_string(),
                                                "Top Right",
                                            );
                                            ui.selectable_value(
                                                &mut self.form_window_anchor,
                                                WINDOW_ANCHOR_BOTTOM_LEFT.to_string(),
                                                "Bottom Left",
                                            );
                                            ui.selectable_value(
                                                &mut self.form_window_anchor,
                                                WINDOW_ANCHOR_BOTTOM_CENTER.to_string(),
                                                "Bottom Center",
                                            );
                                            ui.selectable_value(
                                                &mut self.form_window_anchor,
                                                WINDOW_ANCHOR_BOTTOM_RIGHT.to_string(),
                                                "Bottom Right",
                                            );
                                        });
                                    ui.label(
                                        egui::RichText::new(
                                            "Choose monitor + anchor for compact mode startup/collapse placement.",
                                        )
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                    );
                                    ui.add_space(10.0);
                                    section_header(ui, "App Paths");
                                    field(
                                        ui,
                                        "Chrome",
                                        &mut self.form_chrome_path,
                                        false,
                                    );
                                    ui.add_space(2.0);
                                    field(
                                        ui,
                                        "Paint",
                                        &mut self.form_paint_path,
                                        false,
                                    );
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Use Save to apply and persist advanced settings.",
                                        )
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                    );
                                    }); // end ScrollArea
                                }
                                "configurations" => {
                                    egui::ScrollArea::vertical()
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new("Session Timeouts")
                                                    .size(12.0)
                                                    .strong()
                                                    .color(TEXT_COLOR),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new(
                                                    "Max session length (minutes)",
                                                )
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                            ui.add(
                                                egui::Slider::new(
                                                    &mut self.form_max_session_length_minutes,
                                                    1..=120,
                                                )
                                                .text("min"),
                                            );
                                            ui.label(
                                                egui::RichText::new(
                                                    "Hard cap: recording always stops at this duration, even if activity continues.",
                                                )
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                            ui.add_space(6.0);
                                            ui.label(
                                                egui::RichText::new(
                                                    "Provider inactivity timeout (seconds)",
                                                )
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                            ui.add(
                                                egui::Slider::new(
                                                    &mut self.form_provider_inactivity_timeout_secs,
                                                    5..=300,
                                                )
                                                .text("s"),
                                            );
                                            ui.label(
                                                egui::RichText::new(
                                                    "When no provider activity is observed, the app closes the live session and stops recording.",
                                                )
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                        });
                                }
                                "commands" => {
                                    egui::ScrollArea::vertical()
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            "Configure browser commands and text aliases.",
                                        )
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                    );
                                    ui.add_space(2.0);
                                    section_header(ui, "Browser Commands");
                                    ui.label(
                                        egui::RichText::new(
                                            "URL/browser commands: say the trigger to open in Chrome. 'explorer' opens File Explorer at configured path.",
                                        )
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                    );
                                    ui.add_space(2.0);

                                    let mut delete_url_idx: Option<usize> = None;
                                    for (i, cmd) in
                                        self.form_url_commands.iter_mut().enumerate()
                                    {
                                        let row_w = ui.available_width();
                                        let trigger_w = 84.0;
                                        let delete_w = 20.0;
                                        let spacing = ui.spacing().item_spacing.x;
                                        let url_w = (row_w - trigger_w - delete_w - spacing * 2.0)
                                            .max(140.0);

                                        ui.horizontal(|ui| {
                                            ui.set_width(row_w);
                                            ui.visuals_mut().extreme_bg_color =
                                                Color32::from_rgb(0x1a, 0x1d, 0x24);
                                            ui.add_sized(
                                                [trigger_w, 18.0],
                                                egui::TextEdit::singleline(
                                                    &mut cmd.trigger,
                                                )
                                                .interactive(!cmd.builtin)
                                                .font(FontId::proportional(11.0))
                                                .text_color(TEXT_COLOR),
                                            );
                                            ui.visuals_mut().extreme_bg_color =
                                                Color32::from_rgb(0x1a, 0x1d, 0x24);
                                            ui.add_sized(
                                                [url_w, 18.0],
                                                egui::TextEdit::singleline(&mut cmd.url)
                                                    .font(FontId::proportional(11.0))
                                                    .text_color(TEXT_COLOR),
                                            );
                                            if !cmd.builtin {
                                                if ui
                                                    .add_sized(
                                                        [delete_w, 18.0],
                                                        egui::Button::new(
                                                            egui::RichText::new("x")
                                                                .size(11.0)
                                                                .color(RED),
                                                        )
                                                        .fill(BTN_BG)
                                                        .stroke(Stroke::new(
                                                            0.5, BTN_BORDER,
                                                        )),
                                                    )
                                                    .clicked()
                                                {
                                                    delete_url_idx = Some(i);
                                                }
                                            }
                                            if cmd.builtin {
                                                ui.add_sized(
                                                    [delete_w, 18.0],
                                                    egui::Label::new(""),
                                                );
                                            }
                                        });
                                    }
                                    if let Some(idx) = delete_url_idx {
                                        self.form_url_commands.remove(idx);
                                    }

                                    if ui
                                        .add_sized(
                                            [ui.available_width(), 20.0],
                                            egui::Button::new(
                                                egui::RichText::new("+ Add Command")
                                                    .size(11.0)
                                                    .color(TEXT_COLOR),
                                            )
                                            .fill(BTN_BG)
                                            .stroke(Stroke::new(0.5, BTN_BORDER)),
                                        )
                                        .clicked()
                                    {
                                        self.form_url_commands.push(
                                            crate::settings::UrlCommand {
                                                trigger: String::new(),
                                                url: String::new(),
                                                builtin: false,
                                            },
                                        );
                                    }

                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Use Save to apply and persist command settings.",
                                        )
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                    );
                                    }); // end ScrollArea
                                }
                                "usage" => {
                                    // 4-column stat cards (full width)
                                    if let Ok(u) = self.state.usage.lock() {
                                        ui.columns(4, |cols| {
                                            stat_card(
                                                &mut cols[0],
                                                "Sent",
                                                &fmt_duration_ms(u.ms_sent),
                                            );
                                            stat_card(
                                                &mut cols[1],
                                                "Suppressed",
                                                &fmt_duration_ms(u.ms_suppressed),
                                            );
                                            stat_card(
                                                &mut cols[2],
                                                "Data",
                                                &fmt_bytes(u.bytes_sent),
                                            );
                                            stat_card(
                                                &mut cols[3],
                                                "Transcripts",
                                                &u.finals.to_string(),
                                            );
                                        });
                                    }
                                    // Per-provider breakdown
                                    if let Ok(pt) = self.state.provider_totals.lock() {
                                        if !pt.is_empty() {
                                            ui.add_space(4.0);
                                            let mut providers: Vec<_> = pt.iter().collect();
                                            providers.sort_by(|a, b| b.1.ms_sent.cmp(&a.1.ms_sent));
                                            for (provider_id, pu) in &providers {
                                                let color = Self::provider_color(provider_id, theme_palette(ui.visuals().dark_mode));
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{}: {} sent | {} suppressed | {} | {} transcripts",
                                                        Self::provider_display_name(provider_id),
                                                        fmt_duration_ms(pu.ms_sent),
                                                        fmt_duration_ms(pu.ms_suppressed),
                                                        fmt_bytes(pu.bytes_sent),
                                                        pu.finals,
                                                    ))
                                                    .size(10.0)
                                                    .color(color),
                                                );
                                            }
                                        }
                                    }
                                    // Live session
                                    if self.is_recording {
                                        if let Ok(s) =
                                            self.state.session_usage.lock()
                                        {
                                            if s.started_ms != 0 {
                                                let elapsed = now_ms()
                                                    .saturating_sub(s.started_ms);
                                                ui.add_space(4.0);
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "Live: {} | {} | {} transcripts",
                                                        fmt_duration_ms(elapsed),
                                                        fmt_bytes(s.bytes_sent),
                                                        s.finals,
                                                    ))
                                                    .size(11.0)
                                                    .color(accent.base),
                                                );
                                            }
                                        }
                                    }
                                    // Action buttons
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new(
                                                        "Reset Totals",
                                                    )
                                                    .size(11.0)
                                                    .color(TEXT_COLOR),
                                                )
                                                .fill(BTN_BG)
                                                .stroke(Stroke::new(
                                                    1.0, BTN_BORDER,
                                                ))
                                                .rounding(4.0),
                                            )
                                            .clicked()
                                        {
                                            self.confirm_reset_totals = true;
                                            self.confirm_reset_include_sessions = false;
                                        }
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new(
                                                        "Open Log Folder",
                                                    )
                                                    .size(11.0)
                                                    .color(TEXT_COLOR),
                                                )
                                                .fill(BTN_BG)
                                                .stroke(Stroke::new(
                                                    1.0, BTN_BORDER,
                                                ))
                                                .rounding(4.0),
                                            )
                                            .clicked()
                                        {
                                            if let Some(dir) =
                                                crate::usage::data_dir()
                                            {
                                                let _ = std::process::Command::new(
                                                    "explorer",
                                                )
                                                .arg(&dir)
                                                .spawn();
                                            }
                                        }
                                    });
                                    if self.confirm_reset_totals {
                                        let mut close_dialog = false;
                                        egui::Window::new("Reset Totals?")
                                            .collapsible(false)
                                            .resizable(false)
                                            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
                                            .show(ctx, |ui| {
                                                ui.label(
                                                    egui::RichText::new(
                                                        "This deletes usage totals files and clears current totals. Continue?",
                                                    )
                                                    .size(11.0)
                                                    .color(TEXT_COLOR),
                                                );
                                                ui.add_space(4.0);
                                                ui.checkbox(
                                                    &mut self.confirm_reset_include_sessions,
                                                    egui::RichText::new(
                                                        "Also clear recent sessions (usage-session.jsonl)",
                                                    )
                                                    .size(11.0)
                                                    .color(TEXT_COLOR),
                                                );
                                                ui.add_space(8.0);
                                                ui.horizontal(|ui| {
                                                    if ui.button("Cancel").clicked() {
                                                        close_dialog = true;
                                                    }
                                                    if ui
                                                        .add(
                                                            egui::Button::new("Yes, Reset")
                                                                .fill(RED)
                                                                .stroke(Stroke::new(1.0, RED)),
                                                        )
                                                        .clicked()
                                                    {
                                                        if let Ok(mut u) = self.state.usage.lock() {
                                                            *u = crate::state::UsageTotals::default();
                                                        }
                                                        if let Ok(mut pt) = self.state.provider_totals.lock() {
                                                            pt.clear();
                                                        }
                                                        let _ = crate::usage::reset_totals_file();
                                                        let _ = crate::usage::reset_provider_totals_file();
                                                        if self.confirm_reset_include_sessions {
                                                            let _ = crate::usage::reset_session_file();
                                                            self.session_history.clear();
                                                        }
                                                        self.set_status("Totals reset", "idle");
                                                        close_dialog = true;
                                                    }
                                                });
                                            });
                                        if close_dialog {
                                            self.confirm_reset_totals = false;
                                            self.confirm_reset_include_sessions = false;
                                        }
                                    }
                                    // Session history table
                                    if !self.session_history.is_empty() {
                                        section_header(ui, "Recent Sessions");
                                        egui::ScrollArea::vertical()
                                            .max_height(ui.available_height())
                                            .show(ui, |ui| {
                                                egui::Grid::new("session_table")
                                                    .striped(true)
                                                    .num_columns(6)
                                                    .spacing([8.0, 2.0])
                                                    .show(ui, |ui| {
                                                        for h in [
                                                            "When",
                                                            "Provider",
                                                            "Duration",
                                                            "Audio",
                                                            "Data",
                                                            "Transcripts",
                                                        ] {
                                                            ui.label(
                                                                egui::RichText::new(h)
                                                                    .size(10.0)
                                                                    .strong()
                                                                    .color(TEXT_MUTED),
                                                            );
                                                        }
                                                        ui.end_row();
                                                        for s in
                                                            &self.session_history
                                                        {
                                                            let dur = s
                                                                .updated_ms
                                                                .saturating_sub(
                                                                    s.started_ms,
                                                                );
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    fmt_relative_time(
                                                                        s.started_ms,
                                                                    ),
                                                                )
                                                                .size(10.0)
                                                                .color(TEXT_MUTED),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    &s.provider,
                                                                )
                                                                .size(10.0)
                                                                .color(TEXT_COLOR),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    fmt_duration_ms(dur),
                                                                )
                                                                .size(10.0)
                                                                .color(TEXT_COLOR),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    fmt_duration_ms(
                                                                        s.ms_sent,
                                                                    ),
                                                                )
                                                                .size(10.0)
                                                                .color(TEXT_COLOR),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    fmt_bytes(
                                                                        s.bytes_sent,
                                                                    ),
                                                                )
                                                                .size(10.0)
                                                                .color(TEXT_COLOR),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    s.finals
                                                                        .to_string(),
                                                                )
                                                                .size(10.0)
                                                                .color(TEXT_COLOR),
                                                            );
                                                            ui.end_row();
                                                        }
                                                    });
                                            });
                                    } else {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "No session history yet",
                                            )
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                        );
                                    }
                                }
                                "about" => {
                                    egui::ScrollArea::vertical()
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.label(
                                            egui::RichText::new(
                                                "Jarvis \u{2014} Voice Dictation",
                                            )
                                            .size(13.0)
                                            .strong()
                                            .color(TEXT_COLOR),
                                        );

                                        section_header(ui, "Keyboard Shortcuts");
                                        for (key, desc) in [
                                            ("Right Ctrl (hold)", "Push-to-talk dictation"),
                                            ("Right Ctrl (tap)", "Quick toggle recording"),
                                            ("Escape", "Cancel snip overlay"),
                                        ] {
                                            ui.columns(2, |cols| {
                                                cols[0].label(
                                                    egui::RichText::new(key)
                                                        .size(11.0)
                                                        .strong()
                                                        .color(TEXT_COLOR),
                                                );
                                                cols[1].label(
                                                    egui::RichText::new(desc)
                                                        .size(11.0)
                                                        .color(TEXT_MUTED),
                                                );
                                            });
                                        }

                                        section_header(ui, "Voice Commands");
                                        for (cmd, desc) in [
                                            ("\"back\"", "Delete previous word"),
                                            ("\"new line\"", "Insert line break"),
                                            ("\"new paragraph\"", "Double line break"),
                                            ("\"undo\" / \"redo\"", "Undo or redo"),
                                            ("\"open <trigger>\"", "Open URL (see Audio tab)"),
                                        ] {
                                            ui.columns(2, |cols| {
                                                cols[0].label(
                                                    egui::RichText::new(cmd)
                                                        .size(11.0)
                                                        .color(accent.base),
                                                );
                                                cols[1].label(
                                                    egui::RichText::new(desc)
                                                        .size(11.0)
                                                        .color(TEXT_MUTED),
                                                );
                                            });
                                        }
                                    });
                                }
                                "faq" => {
                                    egui::ScrollArea::vertical()
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                            ui.set_min_width(ui.available_width());
                                            ui.label(
                                                egui::RichText::new("Frequently Asked Questions")
                                                    .size(13.0)
                                                    .strong()
                                                    .color(TEXT_COLOR),
                                            );
                                            ui.add_space(6.0);

                                            for (q, a) in [
                                                (
                                                    "How do I start dictating?",
                                                    "Hold Right Ctrl and speak. Release to commit the transcription to the active text field.",
                                                ),
                                                (
                                                    "What providers are supported?",
                                                    "OpenAI Realtime, Deepgram, ElevenLabs Realtime, and AssemblyAI. Select your provider in the Provider tab.",
                                                ),
                                                (
                                                    "How does VAD mode work?",
                                                    "Strict: only sends audio during speech. Lenient: lower threshold. Off: streams all audio.",
                                                ),
                                                (
                                                    "Where are settings stored?",
                                                    "In AppData/Local/Jarvis/settings.json on Windows. Usage logs are in the same folder.",
                                                ),
                                                (
                                                    "Can I use this with any app?",
                                                    "Yes \u{2014} Jarvis types into whatever window has focus when you release the hotkey.",
                                                ),
                                                (
                                                    "How do I change the hotkey?",
                                                    "The hotkey is currently Right Ctrl. Custom hotkeys are planned for a future release.",
                                                ),
                                            ] {
                                                egui::CollapsingHeader::new(
                                                    egui::RichText::new(q)
                                                        .size(11.0)
                                                        .color(TEXT_COLOR),
                                                )
                                                .default_open(false)
                                                .show(ui, |ui| {
                                                    ui.set_min_width(ui.available_width());
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(a)
                                                                .size(11.0)
                                                                .color(TEXT_MUTED),
                                                        )
                                                        .wrap(),
                                                    );
                                                });
                                            }
                                        });
                                }
                                _ => {}
                            }

                            // Save button (only on settings tabs)
                            if matches!(
                                self.settings_tab.as_str(),
                                "provider"
                                    | "audio"
                                    | "color"
                                    | "screenshot"
                                    | "advanced"
                                    | "configurations"
                                    | "commands"
                            ) {
                                ui.add_space(6.0);
                                    let provider_dirty =
                                        self.settings_tab == "provider" && self.provider_form_dirty();
                                    let show_exit =
                                        self.settings_tab == "provider" && !provider_dirty;
                                    let save_label = if show_exit { "Exit" } else { "Save" };
                                    let save = ui.add_sized(
                                        [ui.available_width(), 24.0],
                                        egui::Button::new(
                                            egui::RichText::new(save_label)
                                                .size(13.0)
                                                .color(TEXT_COLOR),
                                        )
                                        .fill(if show_exit { BTN_BG } else { accent.base })
                                        .stroke(Stroke::new(
                                            1.0,
                                            if show_exit { BTN_BORDER } else { accent.ring },
                                        )),
                                );
                                if save.clicked() {
                                    if show_exit {
                                        self.persist_accent_if_changed();
                                        self.settings_open = false;
                                        self.apply_window_mode(ctx, false);
                                        return;
                                    }
                                    let default_key_present = self
                                        .form_api_keys
                                        .get(&self.form_provider)
                                        .map(|k| !k.trim().is_empty())
                                        .unwrap_or(false);
                                    if self.settings_tab == "provider" && !default_key_present {
                                        self.set_status(
                                            "Select a default provider with an API key",
                                            "error",
                                        );
                                    } else {
                                        self.settings.provider =
                                            self.form_provider.clone();
                                        for (provider_id, _) in PROVIDER_ROWS {
                                            let value = self
                                                .form_api_keys
                                                .get(*provider_id)
                                                .cloned()
                                                .unwrap_or_default();
                                            self.settings.set_api_key(provider_id, value);
                                        }
                                        self.settings.mic_device =
                                            self.form_mic.clone();
                                        self.selected_mic_unavailable = self.selected_mic_unavailable_now();
                                        self.settings.vad_mode =
                                            self.form_vad_mode.clone();
                                        self.settings.screenshot_enabled =
                                            self.form_screenshot_enabled;
                                        self.settings.screenshot_retention_count =
                                            self.form_screenshot_retention_count.clamp(1, 200);
                                        self.settings.start_cue =
                                            self.form_start_cue.clone();
                                        self.settings.theme = "dark".to_string();
                                        self.settings.text_size =
                                            self.form_text_size.clone();
                                        self.settings.accent_color =
                                            self.form_accent_color.clone();
                                        self.settings.window_monitor_mode =
                                            WINDOW_MONITOR_MODE_FIXED.to_string();
                                        self.settings.window_monitor_id =
                                            self.form_window_monitor_id.clone();
                                        self.settings.window_anchor =
                                            self.form_window_anchor.clone();
                                        self.settings.snip_editor_path =
                                            self.form_snip_editor_path.clone();
                                        self.settings.chrome_path =
                                            self.form_chrome_path.clone();
                                        self.settings.paint_path =
                                            self.form_paint_path.clone();
                                        self.settings.provider_inactivity_timeout_secs =
                                            self.form_provider_inactivity_timeout_secs.clamp(5, 300);
                                        self.settings.max_session_length_minutes =
                                            self.form_max_session_length_minutes.clamp(1, 120);
                                        self.settings.url_commands =
                                            self.form_url_commands.clone();
                                        self.settings.alias_commands =
                                            self.form_alias_commands.clone();
                                        match crate::settings::save(&self.settings) {
                                            Ok(()) => {
                                                // Update AppState so background threads pick up changes
                                                if let Ok(mut p) = self.state.chrome_path.lock() {
                                                    *p = self.settings.chrome_path.clone();
                                                }
                                                if let Ok(mut p) = self.state.paint_path.lock() {
                                                    *p = self.settings.paint_path.clone();
                                                }
                                                if let Ok(mut v) = self.state.url_commands.lock() {
                                                    *v = self.settings.url_commands.iter()
                                                        .map(|c| (c.trigger.clone(), c.url.clone()))
                                                        .collect();
                                                }
                                                if let Ok(mut v) = self.state.alias_commands.lock() {
                                                    *v = self.settings.alias_commands.iter()
                                                        .map(|c| (c.trigger.clone(), c.replacement.clone()))
                                                        .collect();
                                                }
                                                self._tray_icon =
                                                    setup_tray(self.current_accent());
                                                self.state.screenshot_enabled.store(
                                                    self.settings.screenshot_enabled,
                                                    Ordering::SeqCst,
                                                );
                                                if self.settings_tab == "provider" {
                                                    let was_recording = self.is_recording;
                                                    if was_recording {
                                                        self.stop_recording();
                                                        self.start_recording();
                                                    }
                                                    self.compact_anchor_pos = None;
                                                    self.set_status("Saved", "idle");
                                                    self.settings_open = false;
                                                    self.apply_window_mode(ctx, false);
                                                } else {
                                                    self.apply_appearance(ctx);
                                                    self.compact_anchor_pos = None;
                                                    self.set_status("Saved", "idle");
                                                    self.settings_open = false;
                                                    self.apply_window_mode(ctx, false);
                                                }
                                            }
                                            Err(e) => {
                                                self.set_status(
                                                    &format!("Save failed: {}", e),
                                                    "error",
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                                });
                            });
                        });
                }
            });
    }

    fn render_snip_overlay(&mut self, ctx: &egui::Context) {
        if self.snip_focus_pending {
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.snip_focus_pending = false;
        }
        // Load texture on first render
        if self.snip_texture.is_none() {
            if let Ok(guard) = self.state.snip_image.lock() {
                if let Some(ref img) = *guard {
                    let size = [img.width() as usize, img.height() as usize];
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                    self.snip_texture = Some(ctx.load_texture(
                        "snip-screenshot",
                        color_image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }

        ctx.set_cursor_icon(CursorIcon::Crosshair);

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_snip();
            return;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::BLACK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, Sense::drag());

                if response.drag_started() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.snip_drag_start = Some(pos);
                        self.snip_drag_current = Some(pos);
                    }
                }
                if response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.snip_drag_current = Some(pos);
                    }
                }

                let painter = ui.painter();

                // Screenshot background
                if let Some(ref tex) = self.snip_texture {
                    painter.image(
                        tex.id(),
                        rect,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                // Dark tint
                painter.rect_filled(rect, 0.0, Color32::from_black_alpha(100));

                // Selection rectangle
                if let (Some(start), Some(current)) =
                    (self.snip_drag_start, self.snip_drag_current)
                {
                    let sel = Rect::from_two_pos(start, current);
                    if sel.width() > 0.0 && sel.height() > 0.0 {
                        // Bright region (no tint)
                        if let Some(ref tex) = self.snip_texture {
                            let uv = Rect::from_min_max(
                                pos2(sel.min.x / rect.width(), sel.min.y / rect.height()),
                                pos2(sel.max.x / rect.width(), sel.max.y / rect.height()),
                            );
                            painter.image(tex.id(), sel, uv, Color32::WHITE);
                        }
                        painter.rect_stroke(
                            sel,
                            0.0,
                            Stroke::new(1.0, Color32::from_white_alpha(230)),
                        );

                        // Dimension label
                        let label = format!("{}x{}", sel.width() as u32, sel.height() as u32);
                        let lpos =
                            pos2(sel.min.x + 8.0, (sel.min.y - 28.0).max(8.0));
                        let galley = painter.layout_no_wrap(
                            label,
                            FontId::proportional(13.0),
                            TEXT_COLOR,
                        );
                        let bg =
                            Rect::from_min_size(lpos, galley.size() + vec2(12.0, 6.0));
                        painter.rect_filled(bg, 3.0, Color32::from_black_alpha(150));
                        painter.galley(lpos + vec2(6.0, 3.0), galley, TEXT_COLOR);
                    }
                }

                // Hint
                painter.text(
                    pos2(rect.center().x, 24.0),
                    egui::Align2::CENTER_CENTER,
                    "Drag to select. Escape to cancel.",
                    FontId::proportional(14.0),
                    Color32::from_white_alpha(200),
                );

                // Drag end → finish/cancel
                if response.drag_stopped() {
                    if let (Some(s), Some(c)) =
                        (self.snip_drag_start, self.snip_drag_current)
                    {
                        let sel = Rect::from_two_pos(s, c);
                        if sel.width() >= 5.0 && sel.height() >= 5.0 {
                            let sx = self
                                .snip_texture
                                .as_ref()
                                .map(|t| t.size()[0] as f32 / rect.width())
                                .unwrap_or(1.0);
                            let sy = self
                                .snip_texture
                                .as_ref()
                                .map(|t| t.size()[1] as f32 / rect.height())
                                .unwrap_or(1.0);
                            self.finish_snip(
                                (sel.min.x * sx) as u32,
                                (sel.min.y * sy) as u32,
                                (sel.width() * sx) as u32,
                                (sel.height() * sy) as u32,
                            );
                        } else {
                            self.cancel_snip();
                        }
                    } else {
                        self.cancel_snip();
                    }
                }
            });
    }
}

impl eframe::App for JarvisApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.settings_open {
            SETTINGS_BG.to_normalized_gamma_f32()
        } else {
            Color32::TRANSPARENT.to_normalized_gamma_f32()
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_appearance(ctx);
        self.process_events();

        // Position bottom-right on first frame
        if !self.positioned {
            let compact_size = vec2(self.compact_window_width(), COMPACT_WINDOW_H);
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(compact_size));
            if self.settings.window_monitor_mode == WINDOW_MONITOR_MODE_FIXED {
                let placed = place_compact_fixed_native(
                    compact_size,
                    &self.settings.window_monitor_id,
                    &self.settings.window_anchor,
                );
                self.positioned = placed;
                self.initial_position_corrected = placed;
                if !placed {
                }
            }
            if !self.positioned {
                if let Some(pos) = default_compact_position_for_size(
                    ctx,
                    compact_size,
                    &self.settings.window_monitor_mode,
                    &self.settings.window_monitor_id,
                    &self.settings.window_anchor,
                ) {
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
                    self.compact_anchor_pos = Some(pos);
                    self.positioned = true;
                } else if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                let win = outer.size();
                if let Some(pos) = default_compact_position_for_size(
                    ctx,
                    win,
                    &self.settings.window_monitor_mode,
                    &self.settings.window_monitor_id,
                    &self.settings.window_anchor,
                ) {
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
                    self.compact_anchor_pos = Some(pos);
                    self.positioned = true;
                    self.initial_position_corrected = true;
                }
                }
            }
        }
        // Compact mode should never maximize/snap-maximize.
        if !self.settings_open
            && ctx
                .input(|i| i.viewport().maximized)
                .unwrap_or(false)
        {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
            self.apply_window_mode(ctx, false);
        }
        // One-time startup correction using the actual first rendered outer size.
        // This prevents initial tray overlap on some DPI/taskbar layouts.
        if self.positioned
            && !self.initial_position_corrected
            && !self.settings_open
            && self.settings.window_monitor_mode != WINDOW_MONITOR_MODE_FIXED
        {
            if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                let size = outer.size();
                let clamped = clamp_window_pos(
                    ctx,
                    outer.min,
                    size,
                    &self.settings.window_monitor_mode,
                    &self.settings.window_monitor_id,
                );
                if (clamped.x - outer.min.x).abs() > 0.5 || (clamped.y - outer.min.y).abs() > 0.5 {
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(clamped));
                    self.compact_anchor_pos = Some(clamped);
                }
                self.initial_position_corrected = true;
            }
        }

        // Auto-recover from error after 4s
        if let Some(t) = self.error_time {
            if t.elapsed() > Duration::from_secs(4) && self.status_state == "error" {
                self.set_status("Ready", "idle");
            }
        }

        // Close → quit app directly.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.should_quit = true;
        }

        self.render_main_ui(ctx);

        // Snip overlay viewport
        if self.snip_overlay_active {
            let vp = if let Some(b) = &self.snip_bounds {
                let scale = if b.scale_factor > 0.0 { b.scale_factor } else { 1.0 };
                let logical_x = b.x as f32 / scale;
                let logical_y = b.y as f32 / scale;
                let logical_w = b.width as f32 / scale;
                let logical_h = b.height as f32 / scale;
                ViewportBuilder::default()
                    .with_position(pos2(logical_x, logical_y))
                    .with_inner_size(vec2(logical_w, logical_h))
                    .with_decorations(false)
                    .with_always_on_top()
                    .with_resizable(false)
                    .with_taskbar(false)
            } else {
                ViewportBuilder::default()
                    .with_decorations(false)
                    .with_always_on_top()
                    .with_maximized(true)
            };

            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("snip-overlay"),
                vp,
                |ctx, _class| {
                    self.render_snip_overlay(ctx);
                },
            );
        }

        // Repaint rate
        if self.is_recording {
            ctx.request_repaint();
        } else {
            // Keep a gentle refresh cadence so the idle dancing strings animate.
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
}

// --- Helpers ---

fn settings_toggle(
    ui: &mut egui::Ui,
    is_recording: bool,
    accent: AccentPalette,
) -> egui::Response {
    let size = 28.0;
    let radius = size / 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());

    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let hovered = response.hovered();
        let idle_ring = Color32::from_rgba_unmultiplied(255, 255, 255, 180);

        let (fill, ring, glyph) = if is_recording {
            let fill = if hovered { accent.hover } else { accent.base };
            (fill, accent.ring, Color32::WHITE)
        } else {
            let gray = Color32::from_rgb(0x3a, 0x3d, 0x45);
            let gray_hover = Color32::from_rgb(0x4a, 0x4d, 0x55);
            let fill = if hovered { gray_hover } else { gray };
            (fill, idle_ring, Color32::WHITE)
        };

        ui.painter()
            .circle_stroke(center, radius, Stroke::new(1.5, ring));
        ui.painter().circle_filled(center, radius - 2.5, fill);
        // Draw a small cog manually so color is fully controllable across fonts/platforms.
        let gear_stroke = Stroke::new(1.2, glyph);
        let r_inner = 2.0;
        let r_ring = 4.2;
        let r_tooth_outer = 6.0;
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            let dir = vec2(a.cos(), a.sin());
            let p1 = center + dir * r_ring;
            let p2 = center + dir * r_tooth_outer;
            ui.painter().line_segment([p1, p2], gear_stroke);
        }
        ui.painter()
            .circle_stroke(center, r_ring, Stroke::new(1.2, glyph));
        ui.painter().circle_filled(center, r_inner, glyph);
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

fn mic_unavailable_badge(ui: &mut egui::Ui, rect: Rect) -> egui::Response {
    let response = ui.interact(rect, egui::Id::new("mic_unavailable_badge"), Sense::hover());
    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let ring = Color32::from_rgb(0xef, 0x44, 0x44);
        let icon = Color32::from_rgb(0xf3, 0xf4, 0xf6);

        // Simple mic glyph (capsule + stem + base) with a diagonal strike-through.
        let capsule = Rect::from_center_size(center + vec2(0.0, -2.0), vec2(5.0, 8.0));
        ui.painter().rect_filled(capsule, 2.0, icon);
        ui.painter().line_segment(
            [center + vec2(0.0, 2.0), center + vec2(0.0, 5.5)],
            Stroke::new(1.4, icon),
        );
        ui.painter().line_segment(
            [center + vec2(-3.0, 6.0), center + vec2(3.0, 6.0)],
            Stroke::new(1.4, icon),
        );
        ui.painter().line_segment(
            [rect.left_top() + vec2(3.0, 3.0), rect.right_bottom() - vec2(3.0, 3.0)],
            Stroke::new(1.8, ring),
        );
    }
    response
}

fn window_ctrl_btn(ui: &mut egui::Ui, label: &str, danger: bool) -> egui::Response {
    let p = theme_palette(ui.visuals().dark_mode);
    let fill = if danger {
        Color32::from_rgb(0x2d, 0x1f, 0x22)
    } else {
        p.btn_bg
    };
    let stroke = if danger {
        Color32::from_rgb(0x5b, 0x2a, 0x32)
    } else {
        p.btn_border
    };
    let btn = egui::Button::new(
        egui::RichText::new(label)
            .size(11.0)
            .strong()
            .color(p.text),
    )
    .fill(fill)
    .stroke(Stroke::new(1.0, stroke))
    .rounding(4.0)
    .min_size(vec2(24.0, 18.0));
    ui.add(btn)
}

fn record_toggle(
    ui: &mut egui::Ui,
    is_recording: bool,
    accent: AccentPalette,
) -> egui::Response {
    let size = 28.0;
    let radius = size / 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());

    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let hovered = response.hovered();

        let (fill, ring) = if is_recording {
            // Active: accent color with brighter hover
            if hovered {
                (accent.hover, accent.base)
            } else {
                (accent.base, accent.ring)
            }
        } else {
            // Idle: muted gray with white ring
            let gray = Color32::from_rgb(0x3a, 0x3d, 0x45);
            let gray_hover = Color32::from_rgb(0x4a, 0x4d, 0x55);
            let idle_ring = Color32::from_rgba_unmultiplied(255, 255, 255, 180);
            if hovered { (gray_hover, idle_ring) } else { (gray, idle_ring) }
        };

        // Outer ring
        ui.painter().circle_stroke(center, radius, Stroke::new(1.5, ring));
        // Filled circle
        ui.painter().circle_filled(center, radius - 2.5, fill);

        // Inner icon: square (stop) when recording, circle (record) when idle
        if is_recording {
            let sq = 7.0;
            let sq_rect = egui::Rect::from_center_size(center, vec2(sq, sq));
            ui.painter().rect_filled(sq_rect, 1.5, Color32::WHITE);
        } else {
            ui.painter().circle_filled(center, 5.0, Color32::WHITE);
        }
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

fn provider_default_button(
    ui: &mut egui::Ui,
    enabled: bool,
    is_default: bool,
    accent: AccentPalette,
) -> egui::Response {
    let size = vec2(22.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();

    let (fill, stroke, dot) = if !enabled {
        (
            Color32::from_rgb(0x42, 0x46, 0x52),
            Color32::from_rgb(0x36, 0x3b, 0x48),
            Color32::from_rgb(0x6b, 0x72, 0x80),
        )
    } else if is_default {
        (
            if hovered {
                accent.hover
            } else {
                accent.base
            },
            accent.ring,
            Color32::WHITE,
        )
    } else if hovered {
        (
            Color32::from_rgb(0x58, 0x62, 0x76),
            Color32::from_rgb(0x8b, 0x96, 0xab),
            Color32::from_rgb(0xc7, 0xcf, 0xde),
        )
    } else {
        (
            Color32::from_rgb(0x4a, 0x50, 0x60),
            Color32::from_rgb(0x6f, 0x7a, 0x92),
            Color32::from_rgb(0x9c, 0xa3, 0xaf),
        )
    };

    ui.painter()
        .circle_filled(rect.center(), rect.width() * 0.46, fill);
    ui.painter().circle_stroke(
        rect.center(),
        rect.width() * 0.46,
        Stroke::new(1.2, stroke),
    );
    ui.painter()
        .circle_filled(rect.center(), rect.width() * 0.16, dot);

    response
}

fn provider_validate_button(
    ui: &mut egui::Ui,
    enabled: bool,
    inflight: bool,
    result_ok: Option<bool>,
    accent: AccentPalette,
) -> egui::Response {
    let size = vec2(24.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();

    let (fill, stroke, glyph, glyph_color) = if !enabled {
        (
            Color32::from_rgb(0x4f, 0x55, 0x63),
            Color32::from_rgb(0x3a, 0x3f, 0x4a),
            "",
            TEXT_COLOR,
        )
    } else if inflight {
        (
            if hovered {
                Color32::from_rgb(0x2d, 0x73, 0xff)
            } else {
                Color32::from_rgb(0x25, 0x63, 0xeb)
            },
            Color32::from_rgb(0x1d, 0x4e, 0xd8),
            "...",
            Color32::WHITE,
        )
    } else if result_ok == Some(true) {
        (
            if hovered {
                accent.hover
            } else {
                accent.base
            },
            accent.ring,
            "\u{2713}",
            Color32::WHITE,
        )
    } else if result_ok == Some(false) {
        (
            if hovered {
                Color32::from_rgb(0xe8, 0x34, 0x34)
            } else {
                Color32::from_rgb(0xdc, 0x26, 0x26)
            },
            Color32::from_rgb(0xb9, 0x1c, 0x1c),
            "!",
            Color32::WHITE,
        )
    } else {
        (
            if hovered {
                Color32::from_rgb(0x74, 0x7f, 0x94)
            } else {
                Color32::from_rgb(0x66, 0x6f, 0x80)
            },
            Color32::from_rgb(0x8b, 0x96, 0xab),
            "?",
            Color32::WHITE,
        )
    };

    ui.painter()
        .circle_filled(rect.center(), rect.width() * 0.45, fill);
    ui.painter().circle_stroke(
        rect.center(),
        rect.width() * 0.45,
        Stroke::new(1.0, stroke),
    );
    if result_ok == Some(true) {
        let c = rect.center();
        let w = rect.width() * 0.16;
        let check = Stroke::new(2.0, Color32::WHITE);
        ui.painter()
            .line_segment([pos2(c.x - w, c.y), pos2(c.x - w * 0.2, c.y + w * 0.8)], check);
        ui.painter()
            .line_segment([pos2(c.x - w * 0.2, c.y + w * 0.8), pos2(c.x + w * 1.2, c.y - w * 0.7)], check);
    } else if result_ok == Some(false) {
        let c = rect.center();
        let w = rect.width() * 0.18;
        let cross = Stroke::new(2.0, Color32::WHITE);
        ui.painter()
            .line_segment([pos2(c.x - w, c.y - w), pos2(c.x + w, c.y + w)], cross);
        ui.painter()
            .line_segment([pos2(c.x + w, c.y - w), pos2(c.x - w, c.y + w)], cross);
    } else if !glyph.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(12.0),
            glyph_color,
        );
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

fn draw_dancing_strings(
    painter: &egui::Painter,
    rect: Rect,
    t: f32,
    live_fft: Option<&[f32; 50]>,
    accent: AccentPalette,
) {
    let is_live = live_fft.is_some();
    let lines = 4usize;
    let samples = 72usize;
    let width = rect.width().max(1.0);
    let base_y = rect.center().y;
    let amp_base = if is_live { 2.4 } else { 1.8 };

    for line in 0..lines {
        let phase = t * (1.6 + line as f32 * 0.28) + line as f32 * 0.9;
        let amp = amp_base + (t * 1.2 + line as f32).sin() * 0.35;
        let color = if is_live {
            Color32::from_rgba_unmultiplied(
                accent.base.r(),
                accent.base.g(),
                accent.base.b(),
                110 + line as u8 * 24,
            )
        } else {
            Color32::from_rgba_unmultiplied(184, 192, 204, 90 + line as u8 * 20)
        };

        let mut points = Vec::with_capacity(samples);
        for i in 0..samples {
            let x = rect.min.x + width * (i as f32 / (samples - 1) as f32);
            let nx = (x - rect.min.x) / width;
            // Pin all strings to the same start/end point.
            let envelope = (std::f32::consts::PI * nx).sin().powf(1.15);
            let wave = (nx * std::f32::consts::TAU * (1.2 + line as f32 * 0.25) - phase).sin();
            let y = base_y + wave * amp * envelope;
            points.push(pos2(x, y));
        }
        painter.add(egui::Shape::line(points, Stroke::new(1.0, color)));
    }

    if let Some(fft) = live_fft {
        // Overlay wide equalizer when speaking, tapered near the edges.
        let bar_count = 42usize;
        let gap = 1.1;
        let overlay_w = rect.width() * 0.94;
        let left = rect.center().x - overlay_w * 0.5;
        let bar_w = ((overlay_w - gap * (bar_count as f32 - 1.0)) / bar_count as f32).max(1.0);
        for i in 0..bar_count {
            let idx = ((i as f32 / (bar_count - 1) as f32) * 49.0) as usize;
            let boosted = (fft[idx] * 50.0).min(1.0);
            let value = boosted.sqrt().max(0.05);
            let nx = i as f32 / (bar_count - 1) as f32;
            let envelope = (std::f32::consts::PI * nx).sin().powf(0.8);
            let h = (value * rect.height() * (0.45 + envelope * 0.75)).max(1.5);
            let x = left + i as f32 * (bar_w + gap);
            let y = rect.center().y - h * 0.5;
            painter.rect_filled(
                Rect::from_min_size(pos2(x, y), vec2(bar_w, h)),
                1.0,
                Color32::from_rgba_unmultiplied(
                    accent.base.r(),
                    accent.base.g(),
                    accent.base.b(),
                    195,
                ),
            );
        }
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, password: bool) -> egui::Response {
    let p = theme_palette(ui.visuals().dark_mode);
    ui.label(egui::RichText::new(label).size(11.0).color(p.text_muted));
    // Match text input surface to active theme.
    ui.visuals_mut().extreme_bg_color = if ui.visuals().dark_mode {
        Color32::from_rgb(0x1a, 0x1d, 0x24)
    } else {
        Color32::from_rgb(0xff, 0xff, 0xff)
    };
    let mut te = egui::TextEdit::singleline(value)
        .font(FontId::proportional(12.0))
        .text_color(p.text)
        .desired_width(f32::INFINITY);
    if password {
        te = te.password(true);
    }
    ui.add(te)
}

fn section_header(ui: &mut egui::Ui, text: &str) {
    let p = theme_palette(ui.visuals().dark_mode);
    ui.add_space(4.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [pos2(rect.min.x, rect.min.y), pos2(rect.max.x, rect.min.y)],
        Stroke::new(0.5, p.btn_border),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .strong()
            .color(p.text_muted),
    );
}

fn fmt_duration_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn fmt_relative_time(ms: u64) -> String {
    if ms == 0 {
        return "\u{2014}".into();
    }
    let now = now_ms();
    let ago = now.saturating_sub(ms) / 1000;
    if ago < 60 {
        "just now".into()
    } else if ago < 3600 {
        format!("{}m ago", ago / 60)
    } else if ago < 86400 {
        format!("{}h ago", ago / 3600)
    } else {
        format!("{}d ago", ago / 86400)
    }
}

fn stat_card(ui: &mut egui::Ui, label: &str, value: &str) {
    let p = theme_palette(ui.visuals().dark_mode);
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(label).size(9.0).color(p.text_muted));
        ui.label(
            egui::RichText::new(value)
                .size(13.0)
                .strong()
                .color(p.text),
        );
    });
}

fn setup_tray(accent: AccentPalette) -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::TrayIconBuilder;

    let menu = Menu::new();
    let quit = MenuItem::with_id("quit", "Quit", true, None);

    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    let icon = match make_tray_icon(accent.base) {
        Some(i) => i,
        None => return None,
    };

    let tray = match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Jarvis")
        .with_icon(icon)
        .build()
    {
        Ok(tray) => {
            println!("[tray] built successfully");
            Some(tray)
        }
        Err(e) => {
            eprintln!("[tray] build error: {}", e);
            None
        }
    };

    tray
}

fn make_tray_icon(color: Color32) -> Option<tray_icon::Icon> {
    let mut icon_data = vec![0u8; 16 * 16 * 4];
    for pixel in icon_data.chunks_exact_mut(4) {
        pixel[0] = color.r();
        pixel[1] = color.g();
        pixel[2] = color.b();
        pixel[3] = 0xFF;
    }
    match tray_icon::Icon::from_rgba(icon_data, 16, 16) {
        Ok(i) => Some(i),
        Err(e) => {
            eprintln!("[tray] icon error: {}", e);
            None
        }
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn default_compact_position_for_size(
    ctx: &egui::Context,
    size: egui::Vec2,
    monitor_mode: &str,
    monitor_id: &str,
    anchor: &str,
) -> Option<Pos2> {
    let work = work_area_rect_logical(ctx, monitor_mode, monitor_id)?;
    anchored_position_in_work_area(work, size, anchor)
}

fn anchored_position_in_work_area(work: Rect, size: egui::Vec2, anchor: &str) -> Option<Pos2> {
    let margin = 10.0;
    let min_x = work.min.x + margin;
    let min_y = work.min.y + margin;
    let max_x = work.max.x - size.x - margin;
    let max_y = work.max.y - size.y - margin;

    if min_x > max_x || min_y > max_y {
        return None;
    }

    let (x, y) = match anchor {
        WINDOW_ANCHOR_TOP_LEFT => (min_x, min_y),
        WINDOW_ANCHOR_TOP_CENTER => ((work.center().x - size.x * 0.5).clamp(min_x, max_x), min_y),
        WINDOW_ANCHOR_TOP_RIGHT => (max_x, min_y),
        WINDOW_ANCHOR_BOTTOM_LEFT => (min_x, max_y),
        WINDOW_ANCHOR_BOTTOM_CENTER => {
            ((work.center().x - size.x * 0.5).clamp(min_x, max_x), max_y)
        }
        _ => (max_x, max_y),
    };

    Some(pos2(x, y))
}

fn clamp_window_pos(
    ctx: &egui::Context,
    pos: Pos2,
    size: egui::Vec2,
    monitor_mode: &str,
    monitor_id: &str,
) -> Pos2 {
    let Some(work) = work_area_rect_logical(ctx, monitor_mode, monitor_id) else {
        return pos;
    };
    let margin = 8.0;
    let min_x = work.min.x + margin;
    let min_y = work.min.y + margin;
    let max_x = work.max.x - size.x - margin;
    let max_y = work.max.y - size.y - margin;
    let x = if min_x <= max_x { pos.x.clamp(min_x, max_x) } else { min_x };
    let y = if min_y <= max_y { pos.y.clamp(min_y, max_y) } else { min_y };
    pos2(x, y)
}

#[derive(Clone)]
struct MonitorWorkArea {
    id: String,
    work_px: windows::Win32::Foundation::RECT,
    is_primary: bool,
    scale_factor: f32,
}

#[cfg(windows)]
fn enumerate_monitor_work_areas() -> Vec<MonitorWorkArea> {
    use std::mem::size_of;
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    unsafe extern "system" fn enum_proc(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let out = &mut *(lparam.0 as *mut Vec<MonitorWorkArea>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _).as_bool() {
            let nul = info
                .szDevice
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(info.szDevice.len());
            let id = String::from_utf16_lossy(&info.szDevice[..nul]);
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            let scale_factor =
                if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
                    (dpi_x as f32 / 96.0).max(0.5)
                } else {
                    1.0
                };
            out.push(MonitorWorkArea {
                id,
                work_px: info.monitorInfo.rcWork,
                is_primary: (info.monitorInfo.dwFlags & 1) != 0,
                scale_factor,
            });
        }

        BOOL(1)
    }

    let mut out: Vec<MonitorWorkArea> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut out as *mut Vec<MonitorWorkArea> as isize),
        );
    }
    out
}

#[cfg(not(windows))]
fn enumerate_monitor_work_areas() -> Vec<MonitorWorkArea> {
    Vec::new()
}

fn available_monitor_choices() -> Vec<MonitorChoice> {
    enumerate_monitor_work_areas()
        .into_iter()
        .enumerate()
        .map(|(idx, m)| MonitorChoice {
            id: m.id.clone(),
            label: format!(
                "{}{}",
                m.id,
                if m.is_primary {
                    " (primary)".into()
                } else {
                    format!(" (monitor {})", idx + 1)
                }
            ),
        })
        .collect()
}

fn resolve_target_monitor(monitor_id: &str) -> Option<MonitorWorkArea> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    let monitors = enumerate_monitor_work_areas();
    if monitors.is_empty() {
        return None;
    }

    let mut primary_work = RECT::default();
    let have_primary_work = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut primary_work as *mut RECT).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok();

    if !monitor_id.trim().is_empty() {
        if let Some(m) = monitors.iter().find(|m| m.id == monitor_id) {
            return Some(m.clone());
        }
    }

    if have_primary_work {
        if let Some(m) = monitors.iter().find(|m| {
            m.work_px.left == primary_work.left
                && m.work_px.top == primary_work.top
                && m.work_px.right == primary_work.right
                && m.work_px.bottom == primary_work.bottom
        }) {
            return Some(m.clone());
        }
    }

    monitors
        .iter()
        .find(|m| m.is_primary)
        .cloned()
        .or_else(|| monitors.first().cloned())
}

#[cfg(windows)]
fn move_window_physical(x: i32, y: i32) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetWindowPos, SWP_NOSIZE, SWP_NOZORDER,
    };

    let title: Vec<u16> = "Jarvis\0".encode_utf16().collect();
    if let Ok(hwnd) = unsafe { FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) } {
        if !hwnd.is_invalid() {
            let _ = unsafe { SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER) };
        }
    }
}

#[cfg(not(windows))]
fn move_window_physical(_x: i32, _y: i32) {}

fn anchored_pos_physical(work: windows::Win32::Foundation::RECT, size_px: (i32, i32), anchor: &str) -> (i32, i32) {
    let margin = 10;
    let w = size_px.0.max(1);
    let h = size_px.1.max(1);
    let min_x = work.left + margin;
    let min_y = work.top + margin;
    let max_x = (work.right - w - margin).max(min_x);
    let max_y = (work.bottom - h - margin).max(min_y);
    match anchor {
        WINDOW_ANCHOR_TOP_LEFT => (min_x, min_y),
        WINDOW_ANCHOR_TOP_CENTER => ((work.left + work.right - w) / 2, min_y),
        WINDOW_ANCHOR_TOP_RIGHT => (max_x, min_y),
        WINDOW_ANCHOR_BOTTOM_LEFT => (min_x, max_y),
        WINDOW_ANCHOR_BOTTOM_CENTER => ((work.left + work.right - w) / 2, max_y),
        _ => (max_x, max_y),
    }
}

fn place_compact_fixed_native(size_logical: egui::Vec2, monitor_id: &str, anchor: &str) -> bool {
    let Some(m) = resolve_target_monitor(monitor_id) else {
        return false;
    };
    let sf = m.scale_factor.max(0.5);
    let size_px = (
        (size_logical.x * sf).round() as i32,
        (size_logical.y * sf).round() as i32,
    );
    let (x, y) = anchored_pos_physical(m.work_px, size_px, anchor);
    move_window_physical(x, y);
    true
}

fn work_area_rect_logical(_ctx: &egui::Context, monitor_mode: &str, monitor_id: &str) -> Option<Rect> {
    let _ = monitor_mode;
    let chosen = resolve_target_monitor(monitor_id);

    if let Some(m) = chosen {
        let sf = m.scale_factor.max(0.5);
        return Some(Rect::from_min_max(
            pos2(m.work_px.left as f32 / sf, m.work_px.top as f32 / sf),
            pos2(m.work_px.right as f32 / sf, m.work_px.bottom as f32 / sf),
        ));
    }

    None
}
