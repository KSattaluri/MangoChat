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
const BG_COLOR: Color32 = Color32::from_rgb(0x1a, 0x1d, 0x24);
const TEXT_COLOR: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
const TEXT_MUTED: Color32 = Color32::from_rgb(0x9c, 0xa3, 0xaf);
const BTN_BG: Color32 = Color32::from_rgb(0x25, 0x28, 0x30);
const BTN_BORDER: Color32 = Color32::from_rgb(0x2c, 0x2f, 0x36);
const BTN_PRIMARY: Color32 = Color32::from_rgb(0x25, 0x63, 0xeb);
const BTN_PRIMARY_HOVER: Color32 = Color32::from_rgb(0x1d, 0x4e, 0xd8);
const SETTINGS_BG: Color32 = Color32::from_rgb(0x15, 0x18, 0x21);
const GREEN: Color32 = Color32::from_rgb(0x36, 0xd3, 0x99);
const RED: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
const GRAY_DOT: Color32 = Color32::from_rgb(0x7b, 0x7b, 0x7b);
const COMPACT_WINDOW_W: f32 = 360.0;
const COMPACT_WINDOW_H: f32 = 54.0;
const PROVIDER_ROWS: &[(&str, &str)] = &[
    ("deepgram", "Deepgram"),
    ("openai", "OpenAI Realtime"),
    ("elevenlabs", "ElevenLabs Realtime"),
];

#[derive(Clone, Copy)]
struct ThemePalette {
    bg: Color32,
    text: Color32,
    text_muted: Color32,
    btn_bg: Color32,
    btn_border: Color32,
    settings_bg: Color32,
}

fn theme_palette(_dark: bool) -> ThemePalette {
    ThemePalette {
        bg: BG_COLOR,
        text: TEXT_COLOR,
        text_muted: TEXT_MUTED,
        btn_bg: BTN_BG,
        btn_border: BTN_BORDER,
        settings_bg: SETTINGS_BG,
    }
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
    pub visible: bool,
    pub mic_devices: Vec<String>,

    // Tray icon (must stay alive or the icon disappears)
    pub _tray_icon: Option<tray_icon::TrayIcon>,
    // Flags set by the tray background thread (works even when window is hidden)
    pub tray_open_flag: Arc<std::sync::atomic::AtomicBool>,
    pub tray_toggle_flag: Arc<std::sync::atomic::AtomicBool>,
    pub tray_click_flag: Arc<std::sync::atomic::AtomicBool>,

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
    pub form_start_cue: String,
    pub form_text_size: String,
    pub form_snip_editor_path: String,
    pub form_chrome_path: String,
    pub form_paint_path: String,
    pub form_url_commands: Vec<crate::settings::UrlCommand>,
    pub key_check_inflight: HashSet<String>,
    pub key_check_result: HashMap<String, (bool, String)>,
    pub last_armed: bool,
    pub tray_toggle: Option<tray_icon::menu::MenuItem>,
    pub session_history: Vec<SessionUsage>,
}

impl JarvisApp {
    pub fn new(
        state: Arc<AppState>,
        event_tx: EventSender<AppEvent>,
        event_rx: EventReceiver<AppEvent>,
        runtime: Arc<tokio::runtime::Runtime>,
        settings: Settings,
        egui_ctx: egui::Context,
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
        let form_start_cue = settings.start_cue.clone();
        let form_text_size = settings.text_size.clone();
        let form_snip_editor_path = settings.snip_editor_path.clone();
        let form_chrome_path = settings.chrome_path.clone();
        let form_paint_path = settings.paint_path.clone();
        let form_url_commands = settings.url_commands.clone();

        // Create tray icon here (inside the event loop) so it stays alive
        let (tray_icon, tray_toggle) =
            setup_tray(state.armed.load(Ordering::SeqCst));
        println!("[tray] icon created: {}", tray_icon.is_some());

        // Background threads to handle tray events independently of the eframe
        // event loop. When the window is hidden, eframe may stop calling update(),
        // so tray events (especially quit) must be processed from a separate thread.
        let tray_open_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let tray_toggle_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let tray_click_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let open_flag = tray_open_flag.clone();
            let toggle_flag = tray_toggle_flag.clone();
            let ctx = egui_ctx.clone();
            std::thread::spawn(move || {
                while let Ok(event) = tray_icon::menu::MenuEvent::receiver().recv() {
                    let id = event.id.0.as_str();
                    println!("[tray-thread] menu event: {}", id);
                    match id {
                        "quit" => {
                            println!("[tray-thread] quit — calling process::exit");
                            std::process::exit(0);
                        }
                        "open" => {
                            open_flag.store(true, Ordering::SeqCst);
                            ctx.request_repaint();
                        }
                        "toggle_armed" => {
                            toggle_flag.store(true, Ordering::SeqCst);
                            ctx.request_repaint();
                        }
                        _ => {}
                    }
                }
            });
        }
        {
            let click_flag = tray_click_flag.clone();
            let ctx = egui_ctx;
            std::thread::spawn(move || {
                while let Ok(event) = tray_icon::TrayIconEvent::receiver().recv() {
                    if matches!(event, tray_icon::TrayIconEvent::Click { .. }) {
                        click_flag.store(true, Ordering::SeqCst);
                        ctx.request_repaint();
                    }
                }
            });
        }

        let mut app = Self {
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
            visible: true,
            mic_devices,
            _tray_icon: tray_icon,
            tray_open_flag,
            tray_toggle_flag,
            tray_click_flag,
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
            form_start_cue,
            form_text_size,
            form_snip_editor_path,
            form_chrome_path,
            form_paint_path,
            form_url_commands,
            key_check_inflight: HashSet::new(),
            key_check_result: HashMap::new(),
            last_armed: false,
            tray_toggle,
            session_history: vec![],
        };
        app.last_armed = app.state.armed.load(Ordering::SeqCst);
        app.update_tray_icon();
        app
    }

    fn update_tray_icon(&self) {
        let armed = self.state.armed.load(Ordering::SeqCst);
        let color = if armed { GREEN } else { GRAY_DOT };
        if let Some(ref tray) = self._tray_icon {
            if let Some(icon) = make_tray_icon(color) {
                let _ = tray.set_icon(Some(icon));
                let _ = tray.set_tooltip(Some(if armed { "Jarvis - Armed" } else { "Jarvis - Disarmed" }));
            }
        }
        if let Some(ref item) = self.tray_toggle {
            let label = if armed { "Disarm Jarvis" } else { "Arm Jarvis" };
            let _ = item.set_text(label);
        }
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
        if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
            let margin = 24.0;
            let max_w = (monitor.x - margin * 2.0).max(COMPACT_WINDOW_W);
            let max_h = (monitor.y - margin * 2.0).max(420.0);
            let w = (monitor.x * 0.5).max(820.0).min(max_w);
            let h = (monitor.y * 0.82).max(620.0).min(max_h);
            return vec2(w, h);
        }
        vec2(980.0, 720.0)
    }

    fn apply_window_mode(&mut self, ctx: &egui::Context, settings_open: bool) {
        let target = if settings_open {
            self.expanded_window_size(ctx)
        } else {
            vec2(COMPACT_WINDOW_W, COMPACT_WINDOW_H)
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
                let pos = clamp_window_pos(ctx, pos2(new_x, new_y), target);
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
            }
        } else if let Some(anchor) = self.compact_anchor_pos {
            // Restore to the exact compact position where expansion started.
            let pos = clamp_window_pos(ctx, anchor, target);
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        } else if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
            // Fallback if no anchor was captured yet.
            let br = outer.max;
            let new_x = br.x - target.x;
            let new_y = br.y - target.y;
            let pos = clamp_window_pos(ctx, pos2(new_x, new_y), target);
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
        let config = provider.connection_config(&provider_settings);

        // Always start audio capture (drives the visualizer FFT)
        let mic = if self.settings.mic_device.is_empty() {
            None
        } else {
            Some(self.settings.mic_device.as_str())
        };
        match audio::AudioCapture::start(
            mic,
            audio_tx,
            self.state.clone(),
            config.sample_rate,
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
                started_ms: now,
                updated_ms: now,
            };
        }

        let event_tx = self.event_tx.clone();
        let state_clone = self.state.clone();

        self.runtime.spawn(async move {
            crate::provider::session::run_session(
                provider,
                event_tx,
                state_clone.clone(),
                provider_settings,
                audio_rx,
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

        // Persist and reset per-session usage.
        if let Ok(mut session) = self.state.session_usage.lock() {
            if session.started_ms != 0 {
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
                AppEvent::ApiKeyValidated {
                    provider,
                    ok,
                    message,
                } => {
                    self.key_check_inflight.remove(&provider);
                    self.key_check_result.insert(provider, (ok, message));
                }
            }
        }
    }

    fn trigger_snip(&mut self) {
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
            match snip::crop_and_save(&img, x, y, w, h) {
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

    fn render_main_ui(&mut self, ctx: &egui::Context) {
        let p = theme_palette(true);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(p.bg)
                    .inner_margin(egui::Margin::symmetric(12.0, 12.0)),
            )
            .show(ctx, |ui| {
                // Drag anywhere on window background to move it
                let bg = ui.interact(
                    ui.max_rect(),
                    egui::Id::new("window_drag"),
                    Sense::click_and_drag(),
                );
                if bg.drag_started() || bg.dragged() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }

                // --- Top control row ---
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    if record_toggle(ui, self.is_recording).clicked() {
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
                                      tip: &str,
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
                            .on_hover_text(tip)
                    };

                    let right_controls_w = 24.0 * 5.0 + 4.0 * 4.0;
                    let viz_w = (ui.available_width() - right_controls_w).max(120.0);
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
                    );

                    if preset_btn(
                        ui,
                        "P",
                        !self.snip_copy_image,
                        "Preset: Path (copy file path)",
                        p,
                    )
                    .clicked()
                    {
                        self.snip_copy_image = false;
                        self.snip_edit_after = false;
                    }
                    if preset_btn(
                        ui,
                        "I",
                        self.snip_copy_image && !self.snip_edit_after,
                        "Preset: Image (copy image)",
                        p,
                    )
                    .clicked()
                    {
                        self.snip_copy_image = true;
                        self.snip_edit_after = false;
                    }
                    if preset_btn(
                        ui,
                        "E",
                        self.snip_copy_image && self.snip_edit_after,
                        "Preset: Image + Edit",
                        p,
                    )
                    .clicked()
                    {
                        self.snip_copy_image = true;
                        self.snip_edit_after = true;
                    }
                    if icon_btn(ui, "\u{2699}", "Settings").clicked() {
                        self.settings_open = !self.settings_open;
                        if self.settings_open {
                            // Stop recording when opening settings to avoid
                            // provider mismatch between UI and active session.
                            if self.is_recording {
                                self.stop_recording();
                            }
                            self.session_history = crate::usage::load_recent_sessions(20);
                        }
                        self.apply_window_mode(ctx, self.settings_open);
                    }
                    if window_ctrl_btn(ui, "-", false).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
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
                                            ("advanced", "Advanced"),
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
                                                    BTN_PRIMARY
                                                } else {
                                                    Color32::TRANSPARENT
                                                })
                                                .stroke(Stroke::new(1.0, p.btn_border))
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
                                    ui.label(
                                        egui::RichText::new("Provider Credentials")
                                            .size(14.0)
                                            .strong()
                                            .color(p.text),
                                    );
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
                                        ui.add_sized(
                                            [default_w, 20.0],
                                            egui::Label::new(
                                                egui::RichText::new("Default")
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
                                                    let provider_color = match provider_id.as_str() {
                                                        "openai" => Color32::from_rgb(0x10, 0xb9, 0x81),
                                                        "deepgram" => Color32::from_rgb(0x3b, 0x82, 0xf6),
                                                        "elevenlabs" => Color32::from_rgb(0xf5, 0x9e, 0x0b),
                                                        _ => p.text,
                                                    };
                                                    ui.add_sized(
                                                        [provider_w, 24.0],
                                                        egui::Label::new(
                                                            egui::RichText::new(*provider_name)
                                                                .size(13.0)
                                                                .strong()
                                                                .color(provider_color),
                                                        ),
                                                    );

                                                    let key_value = self
                                                        .form_api_keys
                                                        .entry(provider_id.clone())
                                                        .or_default();
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
                                                                )
                                                            },
                                                        )
                                                        .inner;
                                                    if validate_resp.clicked() && key_present && !inflight {
                                                        self.key_check_inflight.insert(provider_id.clone());
                                                        self.key_check_result.remove(&provider_id);
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
                                                                Ok(()) => (true, "API key is valid".to_string()),
                                                                Err(e) => (false, e),
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

                                                    let can_default = key_present;
                                                    let is_default = self.form_provider == provider_id;
                                                    let radio = ui
                                                        .allocate_ui_with_layout(
                                                            vec2(default_w, 24.0),
                                                            egui::Layout::centered_and_justified(
                                                                egui::Direction::LeftToRight,
                                                            ),
                                                            |ui| {
                                                                ui.add_enabled(
                                                                    can_default,
                                                                    egui::RadioButton::new(
                                                                        is_default,
                                                                        "",
                                                                    ),
                                                                )
                                                            },
                                                        )
                                                        .inner;
                                                    if radio.clicked() && can_default {
                                                        self.form_provider = provider_id.clone();
                                                    }
                                                });
                                            });
                                        ui.add_space(6.0);
                                    }

                                    if let Some((ok, msg)) = self.key_check_result.get(&self.form_provider) {
                                        let color = if *ok { GREEN } else { RED };
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(msg).size(11.0).color(color));
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
                                    egui::ComboBox::from_id_salt("mic_select")
                                        .selected_text(if self.form_mic.is_empty() {
                                            "Default"
                                        } else {
                                            &self.form_mic
                                        })
                                        .width(ui.available_width())
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
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new("Start Cue")
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    let cue_label = crate::start_cue::START_CUES
                                        .iter()
                                        .find(|(id, _)| *id == self.form_start_cue)
                                        .map(|(_, label)| *label)
                                        .unwrap_or("Audio 1");
                                    egui::ComboBox::from_id_salt("start_cue_select")
                                        .selected_text(cue_label)
                                        .width(ui.available_width())
                                        .show_ui(ui, |ui| {
                                            for (id, label) in crate::start_cue::START_CUES {
                                                ui.selectable_value(
                                                    &mut self.form_start_cue,
                                                    (*id).to_string(),
                                                    *label,
                                                );
                                            }
                                        });
                                    }); // end ScrollArea
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
                                "commands" => {
                                    egui::ScrollArea::vertical()
                                        .max_height(ui.available_height())
                                        .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            "Voice URL commands, say the trigger word to open the URL in Chrome.",
                                        )
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                    );
                                    ui.add_space(2.0);

                                    let mut delete_idx: Option<usize> = None;
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
                                                    delete_idx = Some(i);
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
                                    if let Some(idx) = delete_idx {
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
                                                "Commits",
                                                &u.commits.to_string(),
                                            );
                                        });
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
                                                        "Live: {} | {} | {} commits",
                                                        fmt_duration_ms(elapsed),
                                                        fmt_bytes(s.bytes_sent),
                                                        s.commits,
                                                    ))
                                                    .size(11.0)
                                                    .color(GREEN),
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
                                            if let Ok(mut u) =
                                                self.state.usage.lock()
                                            {
                                                *u = crate::state::UsageTotals::default();
                                            }
                                            let _ =
                                                crate::usage::reset_totals_file();
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
                                    // Session history table
                                    if !self.session_history.is_empty() {
                                        section_header(ui, "Recent Sessions");
                                        egui::ScrollArea::vertical()
                                            .max_height(ui.available_height())
                                            .show(ui, |ui| {
                                                egui::Grid::new("session_table")
                                                    .striped(true)
                                                    .num_columns(5)
                                                    .spacing([8.0, 2.0])
                                                    .show(ui, |ui| {
                                                        for h in [
                                                            "When",
                                                            "Provider",
                                                            "Duration",
                                                            "Data",
                                                            "Commits",
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
                                                                    fmt_bytes(
                                                                        s.bytes_sent,
                                                                    ),
                                                                )
                                                                .size(10.0)
                                                                .color(TEXT_COLOR),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    s.commits
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
                                                        .color(GREEN),
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
                                                    "OpenAI Realtime, Deepgram, and ElevenLabs Realtime. Select your provider in the Provider tab.",
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
                                "provider" | "audio" | "advanced" | "commands"
                            ) {
                                ui.add_space(6.0);
                                    let save = ui.add_sized(
                                        [ui.available_width(), 24.0],
                                        egui::Button::new(
                                            egui::RichText::new("Save")
                                                .size(13.0)
                                                .color(TEXT_COLOR),
                                        )
                                        .fill(BTN_PRIMARY)
                                        .stroke(Stroke::new(1.0, BTN_PRIMARY_HOVER)),
                                );
                                if save.clicked() {
                                    let default_key_present = self
                                        .form_api_keys
                                        .get(&self.form_provider)
                                        .map(|k| !k.trim().is_empty())
                                        .unwrap_or(false);
                                    if !default_key_present {
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
                                        self.settings.vad_mode =
                                            self.form_vad_mode.clone();
                                        self.settings.start_cue =
                                            self.form_start_cue.clone();
                                        self.settings.theme = "dark".to_string();
                                        self.settings.text_size =
                                            self.form_text_size.clone();
                                        self.settings.snip_editor_path =
                                            self.form_snip_editor_path.clone();
                                        self.settings.chrome_path =
                                            self.form_chrome_path.clone();
                                        self.settings.paint_path =
                                            self.form_paint_path.clone();
                                        self.settings.url_commands =
                                            self.form_url_commands.clone();
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
                                                self.apply_appearance(ctx);
                                                self.set_status("Saved", "idle");
                                                self.settings_open = false;
                                                self.apply_window_mode(ctx, false);
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_appearance(ctx);
        self.process_events();

        let armed = self.state.armed.load(Ordering::SeqCst);
        if armed != self.last_armed {
            self.last_armed = armed;
            self.update_tray_icon();
        }

        // Position bottom-right on first frame
        if !self.positioned {
            if let Some(pos) = default_compact_position(ctx) {
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
                self.compact_anchor_pos = Some(pos);
                self.positioned = true;
            } else if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
                let win = vec2(COMPACT_WINDOW_W, COMPACT_WINDOW_H);
                let pos = pos2(
                    monitor.x - win.x - 16.0,
                    monitor.y - win.y - 64.0, // stay above taskbar in compact mode
                );
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
                self.compact_anchor_pos = Some(pos);
                self.positioned = true;
            }
        }
        // One-time startup correction using the actual first rendered outer size.
        // This prevents initial tray overlap on some DPI/taskbar layouts.
        if self.positioned && !self.initial_position_corrected && !self.settings_open {
            if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                let size = outer.size();
                let clamped = clamp_window_pos(ctx, outer.min, size);
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

        // Tray menu events — flags are set by background threads so quit works
        // even when the window is hidden and eframe isn't calling update().
        if self.tray_open_flag.swap(false, Ordering::SeqCst) {
            self.visible = true;
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        if self.tray_toggle_flag.swap(false, Ordering::SeqCst) {
            let was = self.state.armed.load(Ordering::SeqCst);
            let now_armed = !was;
            self.state.armed.store(now_armed, Ordering::SeqCst);
            if !now_armed {
                self.stop_recording();
            }
            println!("[tray] armed = {}", now_armed);
            self.update_tray_icon();
        }

        // Tray icon click → show window
        if self.tray_click_flag.swap(false, Ordering::SeqCst) && !self.visible {
            self.visible = true;
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }

        // Close → hide to tray (unless quitting)
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.should_quit {
                // allow
            } else {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                self.visible = false;
            }
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

fn icon_btn(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
    let p = theme_palette(ui.visuals().dark_mode);
    let btn = egui::Button::new(egui::RichText::new(icon).size(13.0).color(p.text))
        .fill(p.btn_bg)
        .stroke(Stroke::new(1.0, p.btn_border))
        .rounding(4.0)
        .min_size(vec2(24.0, 22.0));
    ui.add(btn).on_hover_text(tooltip)
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

fn record_toggle(ui: &mut egui::Ui, is_recording: bool) -> egui::Response {
    let size = 28.0;
    let radius = size / 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());

    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let hovered = response.hovered();

        let (fill, ring) = if is_recording {
            // Active: green with brighter hover
            let green = Color32::from_rgb(0x22, 0xc5, 0x5e);
            let green_hover = Color32::from_rgb(0x16, 0xa3, 0x4a);
            if hovered { (green_hover, green) } else { (green, green_hover) }
        } else {
            // Idle: muted gray with subtle hover lift
            let gray = Color32::from_rgb(0x3a, 0x3d, 0x45);
            let gray_hover = Color32::from_rgb(0x4a, 0x4d, 0x55);
            if hovered { (gray_hover, gray) } else { (gray, gray_hover) }
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

    let tooltip = if is_recording { "Stop recording" } else { "Start recording" };
    response.on_hover_text(tooltip)
}

fn provider_validate_button(
    ui: &mut egui::Ui,
    enabled: bool,
    inflight: bool,
    result_ok: Option<bool>,
) -> egui::Response {
    let size = vec2(22.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let (fill, stroke, glyph, glyph_color) = if !enabled {
        (
            Color32::from_rgb(0x52, 0x56, 0x60),
            Color32::from_rgb(0x3a, 0x3f, 0x4a),
            "",
            TEXT_COLOR,
        )
    } else if inflight {
        (
            Color32::from_rgb(0x25, 0x63, 0xeb),
            Color32::from_rgb(0x1d, 0x4e, 0xd8),
            "...",
            Color32::WHITE,
        )
    } else if result_ok == Some(true) {
        (
            Color32::from_rgb(0x16, 0xa3, 0x4a),
            Color32::from_rgb(0x15, 0x86, 0x40),
            "v",
            Color32::WHITE,
        )
    } else if result_ok == Some(false) {
        (
            Color32::from_rgb(0xdc, 0x26, 0x26),
            Color32::from_rgb(0xb9, 0x1c, 0x1c),
            "!",
            Color32::WHITE,
        )
    } else {
        (
            Color32::from_rgb(0x66, 0x6f, 0x80),
            Color32::from_rgb(0x8b, 0x96, 0xab),
            "",
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
    if !glyph.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(11.0),
            glyph_color,
        );
    }

    response
}

fn draw_dancing_strings(
    painter: &egui::Painter,
    rect: Rect,
    t: f32,
    live_fft: Option<&[f32; 50]>,
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
            Color32::from_rgba_unmultiplied(54, 211, 153, 110 + line as u8 * 24)
        } else {
            Color32::from_rgba_unmultiplied(184, 192, 204, 90 + line as u8 * 20)
        };

        let mut points = Vec::with_capacity(samples);
        for i in 0..samples {
            let x = rect.min.x + width * (i as f32 / (samples - 1) as f32);
            let nx = (x - rect.min.x) / width;
            // Pin all strings to the same start/end point.
            let envelope = (std::f32::consts::PI * nx).sin().powf(1.15);
            let wave = (nx * std::f32::consts::TAU * (1.2 + line as f32 * 0.25) + phase).sin();
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
                Color32::from_rgba_unmultiplied(54, 211, 153, 195),
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

fn setup_tray(
    armed: bool,
) -> (Option<tray_icon::TrayIcon>, Option<tray_icon::menu::MenuItem>) {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::TrayIconBuilder;

    let menu = Menu::new();
    let open = MenuItem::with_id("open", "Open Jarvis", true, None);
    let toggle_label = if armed { "Disarm Jarvis" } else { "Arm Jarvis" };
    let toggle_armed = MenuItem::with_id("toggle_armed", toggle_label, true, None);
    let quit = MenuItem::with_id("quit", "Quit", true, None);

    let _ = menu.append(&open);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&toggle_armed);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    let icon = match make_tray_icon(GRAY_DOT) {
        Some(i) => i,
        None => return (None, None),
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

    (tray, Some(toggle_armed))
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

fn default_compact_position(ctx: &egui::Context) -> Option<Pos2> {
    let work = work_area_rect_logical(ctx)?;
    let margin = 10.0;
    let x = work.max.x - COMPACT_WINDOW_W - margin;
    let y = work.max.y - COMPACT_WINDOW_H - margin;
    Some(pos2(x.max(work.min.x), y.max(work.min.y)))
}

fn clamp_window_pos(ctx: &egui::Context, pos: Pos2, size: egui::Vec2) -> Pos2 {
    let Some(work) = work_area_rect_logical(ctx) else {
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

fn work_area_rect_logical(ctx: &egui::Context) -> Option<Rect> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    let mut rect = RECT::default();
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut rect as *mut RECT).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if ok.is_err() {
        return None;
    }

    let ppp = ctx.pixels_per_point().max(0.1);
    Some(Rect::from_min_max(
        pos2(rect.left as f32 / ppp, rect.top as f32 / ppp),
        pos2(rect.right as f32 / ppp, rect.bottom as f32 / ppp),
    ))
}
