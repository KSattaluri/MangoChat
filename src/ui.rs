use crate::audio;
use crate::settings::Settings;
use crate::snip;
use crate::usage::{append_usage_line, session_usage_path};
use crate::state::{AppEvent, AppState};
use eframe::egui;
use egui::{
    pos2, vec2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, TextureHandle,
    ViewportBuilder, ViewportCommand, ViewportId,
};
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

pub struct JarvisApp {
    pub state: Arc<AppState>,
    pub event_tx: EventSender<AppEvent>,
    pub event_rx: EventReceiver<AppEvent>,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub settings: Settings,
    pub settings_open: bool,
    pub status_text: String,
    pub status_state: String,
    pub is_recording: bool,
    pub audio_capture: Option<crate::audio::AudioCapture>,
    pub should_quit: bool,
    pub visible: bool,
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

    // Error auto-recovery
    pub error_time: Option<std::time::Instant>,

    // Settings form fields
    pub form_provider: String,
    pub form_api_key: String,
    pub form_model: String,
    pub form_language: String,
    pub form_mic: String,
    pub form_vad_mode: String,
    pub form_snip_editor_path: String,
    pub key_check_inflight: bool,
    pub key_check_result: Option<(bool, String)>,
    pub last_armed: bool,
    pub tray_toggle: Option<tray_icon::menu::MenuItem>,
}

impl JarvisApp {
    pub fn new(
        state: Arc<AppState>,
        event_tx: EventSender<AppEvent>,
        event_rx: EventReceiver<AppEvent>,
        runtime: Arc<tokio::runtime::Runtime>,
        settings: Settings,
    ) -> Self {
        let mic_devices = audio::list_input_devices();
        let form_provider = settings.provider.clone();
        let form_api_key = settings.api_key_for(&settings.provider).to_string();
        let form_model = settings.model.clone();
        let form_language = settings.language.clone();
        let form_mic = settings.mic_device.clone();
        let form_vad_mode = settings.vad_mode.clone();
        let form_snip_editor_path = settings.snip_editor_path.clone();

        // Create tray icon here (inside the event loop) so it stays alive
        let (tray_icon, tray_toggle) =
            setup_tray(state.armed.load(Ordering::SeqCst));
        println!("[tray] icon created: {}", tray_icon.is_some());

        let mut app = Self {
            state,
            event_tx,
            event_rx,
            runtime,
            settings,
            settings_open: false,
            status_text: "Ready".into(),
            status_state: "idle".into(),
            is_recording: false,
            audio_capture: None,
            should_quit: false,
            visible: true,
            mic_devices,
            _tray_icon: tray_icon,
            positioned: false,
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
            form_api_key,
            form_model,
            form_language,
            form_mic,
            form_vad_mode,
            form_snip_editor_path,
            key_check_inflight: false,
            key_check_result: None,
            last_armed: false,
            tray_toggle,
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

        self.is_recording = true;
        // Update VAD mode from settings (strict/lenient/off).
        let mode = match self.settings.vad_mode.as_str() {
            "lenient" => 1,
            "off" => 2,
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
                AppEvent::ApiKeyValidated { ok, message } => {
                    self.key_check_inflight = false;
                    self.key_check_result = Some((ok, message));
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
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(BG_COLOR)
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0)),
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

                // --- Header row: "Jarvis" on left, status on right ---
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Jarvis")
                            .size(13.0)
                            .strong()
                            .color(TEXT_COLOR),
                    );

                    // Push status to the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let is_live = self.is_recording;

                        if is_live {
                            // Draw visualizer bars
                            let fft = self
                                .state
                                .fft_data
                                .lock()
                                .map(|d| *d)
                                .unwrap_or([0.0; 50]);
                            let (bar_rect, _) =
                                ui.allocate_exact_size(vec2(160.0, 16.0), Sense::hover());
                            let painter = ui.painter();
                            let bar_count = 50;
                            let gap = 1.0;
                            let mut bar_w = (bar_rect.width() - gap * (bar_count as f32 - 1.0))
                                / bar_count as f32;
                            if bar_w < 1.0 {
                                bar_w = 1.0;
                            }
                            for i in 0..bar_count {
                                // Boost sensitivity so low-level speech shows movement.
                                // Use a nonlinear curve to lift quiet signals.
                                let boosted = (fft[i] * 50.0).min(1.0);
                                let value = boosted.sqrt().max(0.06);
                                let h = (value * bar_rect.height()).max(2.0);
                                let x = bar_rect.min.x + i as f32 * (bar_w + gap);
                                let y = bar_rect.center().y - h / 2.0;
                                painter.rect_filled(
                                    Rect::from_min_size(pos2(x, y), vec2(bar_w, h)),
                                    1.0,
                                    GREEN,
                                );
                            }
                        } else {
                            // Status text
                            ui.label(
                                egui::RichText::new(&self.status_text)
                                    .size(11.0)
                                    .color(TEXT_MUTED),
                            );
                        }

                        // Status dot
                        let dot_color = match self.status_state.as_str() {
                            "live" => GREEN,
                            "error" => RED,
                            _ => GRAY_DOT,
                        };
                        let (dot_rect, _) =
                            ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                        ui.painter()
                            .circle_filled(dot_rect.center(), 4.0, dot_color);
                    });
                });

                ui.add_space(6.0);

                // --- Button row ---
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    if record_toggle(ui, self.is_recording).clicked() {
                        if self.is_recording {
                            self.stop_recording();
                        } else {
                            self.start_recording();
                        }
                    }
                    if icon_btn(ui, "\u{2699}", "Settings").clicked() {
                        self.settings_open = !self.settings_open;
                        let new_h = if self.settings_open { 420.0 } else { 80.0 };
                        let old_h = if self.settings_open { 80.0 } else { 420.0 };
                        // Grow upward: shift window position so bottom edge stays put
                        if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                            let new_y = outer.min.y - (new_h - old_h);
                            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(
                                pos2(outer.min.x, new_y),
                            ));
                        }
                        ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(360.0, new_h)));
                    }
                    if icon_btn(ui, "\u{1F4C1}", "Open Snips Folder").clicked() {
                        let _ = snip::open_snip_folder();
                    }
                    let clip_label = if self.snip_copy_image {
                        "Clip: Image"
                    } else {
                        "Clip: Path"
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(clip_label)
                                    .size(11.0)
                                    .color(TEXT_COLOR),
                            )
                            .fill(BTN_BG)
                            .stroke(Stroke::new(1.0, BTN_BORDER))
                            .rounding(4.0)
                            .min_size(vec2(62.0, 22.0)),
                        )
                        .clicked()
                    {
                        self.snip_copy_image = !self.snip_copy_image;
                    }
                    let edit_label = if self.snip_edit_after {
                        "Edit: On"
                    } else {
                        "Edit: Off"
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(edit_label)
                                    .size(11.0)
                                    .color(TEXT_COLOR),
                            )
                            .fill(BTN_BG)
                            .stroke(Stroke::new(1.0, BTN_BORDER))
                            .rounding(4.0)
                            .min_size(vec2(62.0, 22.0)),
                        )
                        .clicked()
                    {
                        self.snip_edit_after = !self.snip_edit_after;
                    }
                    if icon_btn(ui, "\u{1F4CA}", "Task Manager").clicked() {
                        std::thread::spawn(open_task_manager);
                    }
                    if icon_btn(ui, "\u{2715}", "Minimize").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
                });

                // --- Collapsible settings panel ---
                if self.settings_open {
                    ui.add_space(8.0);
                    egui::Frame::none()
                        .fill(SETTINGS_BG)
                        .stroke(Stroke::new(1.0, BTN_BORDER))
                        .rounding(6.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;

                            ui.label(
                                egui::RichText::new("Provider")
                                    .size(11.0)
                                    .color(TEXT_MUTED),
                            );
                            let selected_label = crate::provider::PROVIDERS
                                .iter()
                                .find(|(id, _)| *id == self.form_provider)
                                .map(|(_, name)| *name)
                                .unwrap_or("OpenAI Realtime");
                            let mut provider_changed = false;
                            egui::ComboBox::from_id_salt("provider_select")
                                .selected_text(selected_label)
                                .width(ui.available_width())
                                .show_ui(ui, |ui| {
                                    for (id, name) in crate::provider::PROVIDERS {
                                        let resp = ui.selectable_value(
                                            &mut self.form_provider,
                                            id.to_string(),
                                            *name,
                                        );
                                        if resp.changed() {
                                            provider_changed = true;
                                        }
                                    }
                                });
                            if provider_changed {
                                // Save current key for old provider, load key for new provider.
                                self.settings.set_api_key(
                                    &self.settings.provider.clone(),
                                    self.form_api_key.clone(),
                                );
                                self.form_api_key = self
                                    .settings
                                    .api_key_for(&self.form_provider)
                                    .to_string();
                                self.key_check_result = None;
                            }

                            let api_resp = field(ui, "API Key", &mut self.form_api_key, true);
                            if api_resp.changed() {
                                self.key_check_result = None;
                            }
                            readonly_field(ui, "Model", &self.form_model);
                            readonly_field(ui, "Language", &self.form_language);
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new("VAD Mode")
                                    .size(11.0)
                                    .color(TEXT_MUTED),
                            );
                            egui::ComboBox::from_id_salt("vad_mode")
                                .selected_text(match self.form_vad_mode.as_str() {
                                    "lenient" => "Lenient",
                                    "off" => "Off",
                                    _ => "Strict",
                                })
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
                                    ui.selectable_value(
                                        &mut self.form_vad_mode,
                                        "off".to_string(),
                                        "Off",
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
                            field(
                                ui,
                                "Snip Editor Path (optional)",
                                &mut self.form_snip_editor_path,
                                false,
                            );

                            ui.add_space(4.0);
                            let validate_enabled = !self.form_api_key.trim().is_empty()
                                && !self.key_check_inflight;
                            let validate = ui.add_enabled(
                                validate_enabled,
                                egui::Button::new(
                                    egui::RichText::new("Validate API Key")
                                        .size(12.0)
                                        .color(TEXT_COLOR),
                                )
                                .fill(BTN_BG)
                                .stroke(Stroke::new(1.0, BTN_BORDER))
                                .min_size(vec2(ui.available_width(), 24.0)),
                            );
                            if validate.clicked() {
                                self.key_check_inflight = true;
                                self.key_check_result = None;

                                let provider =
                                    crate::provider::create_provider(&self.form_provider);
                                let provider_settings = crate::provider::ProviderSettings {
                                    api_key: self.form_api_key.clone(),
                                    model: self.form_model.clone(),
                                    transcription_model: self.settings.transcription_model.clone(),
                                    language: self.form_language.clone(),
                                };
                                let event_tx = self.event_tx.clone();

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
                                    let _ = event_tx.send(AppEvent::ApiKeyValidated { ok, message });
                                });
                            }
                            if self.key_check_inflight {
                                ui.label(
                                    egui::RichText::new("Validating...")
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                );
                            } else if let Some((ok, message)) = &self.key_check_result {
                                let color = if *ok { GREEN } else { RED };
                                ui.label(
                                    egui::RichText::new(message)
                                        .size(11.0)
                                        .color(color),
                                );
                            }

                            let save = ui.add_sized(
                                [ui.available_width(), 24.0],
                                egui::Button::new(
                                    egui::RichText::new("Save")
                                        .size(12.0)
                                        .color(TEXT_COLOR),
                                )
                                .fill(BTN_PRIMARY)
                                .stroke(Stroke::new(1.0, BTN_PRIMARY_HOVER)),
                            );
                            if save.clicked() {
                                self.settings.provider = self.form_provider.clone();
                                self.settings.set_api_key(
                                    &self.form_provider,
                                    self.form_api_key.clone(),
                                );
                                self.settings.mic_device = self.form_mic.clone();
                                self.settings.vad_mode = self.form_vad_mode.clone();
                                self.settings.snip_editor_path =
                                    self.form_snip_editor_path.clone();
                                match crate::settings::save(&self.settings) {
                                    Ok(()) => {
                                        self.set_status("Saved", "idle");
                                        // Collapse settings and return to compact height.
                                        self.settings_open = false;
                                        if let Some(outer) =
                                            ctx.input(|i| i.viewport().outer_rect)
                                        {
                                            let new_h = 80.0;
                                            let old_h = 420.0;
                                            let new_y = outer.min.y + (old_h - new_h);
                                            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(
                                                pos2(outer.min.x, new_y),
                                            ));
                                        }
                                        ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(
                                            360.0, 80.0,
                                        )));
                                    }
                                    Err(e) => {
                                        self.set_status(&format!("Save failed: {}", e), "error")
                                    }
                                }
                            }
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
        self.process_events();

        let armed = self.state.armed.load(Ordering::SeqCst);
        if armed != self.last_armed {
            self.last_armed = armed;
            self.update_tray_icon();
        }

        // Position bottom-right on first frame
        if !self.positioned {
            if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
                let win = vec2(360.0, 80.0);
                let pos = pos2(
                    monitor.x - win.x - 16.0,
                    monitor.y - win.y - 56.0, // above taskbar
                );
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
                self.positioned = true;
            }
        }

        // Auto-recover from error after 4s
        if let Some(t) = self.error_time {
            if t.elapsed() > Duration::from_secs(4) && self.status_state == "error" {
                self.set_status("Ready", "idle");
            }
        }

        // Tray menu events
        if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            let id = event.id.0.as_str();
            println!("[tray] menu event: {}", id);
            match id {
                "open" => {
                    self.visible = true;
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                }
                "toggle_armed" => {
                    let was = self.state.armed.load(Ordering::SeqCst);
                    let now_armed = !was;
                    self.state.armed.store(now_armed, Ordering::SeqCst);
                    if !now_armed {
                        // Disarming should fully stop any active session.
                        self.stop_recording();
                    }
                    println!("[tray] armed = {}", !was);
                    self.update_tray_icon();
                }
                "quit" => {
                    self.should_quit = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                _ => {}
            }
        }

        // Tray icon click → show window
        if let Ok(tray_icon::TrayIconEvent::Click { .. }) =
            tray_icon::TrayIconEvent::receiver().try_recv()
        {
            if !self.visible {
                self.visible = true;
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
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
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

// --- Helpers ---

fn icon_btn(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
    let btn = egui::Button::new(egui::RichText::new(icon).size(13.0).color(TEXT_COLOR))
        .fill(BTN_BG)
        .stroke(Stroke::new(1.0, BTN_BORDER))
        .rounding(4.0)
        .min_size(vec2(24.0, 22.0));
    ui.add(btn).on_hover_text(tooltip)
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

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, password: bool) -> egui::Response {
    ui.label(egui::RichText::new(label).size(11.0).color(TEXT_MUTED));
    // Override text edit background to match dark theme
    ui.visuals_mut().extreme_bg_color = Color32::from_rgb(0x1a, 0x1d, 0x24);
    let mut te = egui::TextEdit::singleline(value)
        .font(FontId::proportional(12.0))
        .text_color(TEXT_COLOR)
        .desired_width(f32::INFINITY);
    if password {
        te = te.password(true);
    }
    ui.add(te)
}

fn readonly_field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).size(11.0).color(TEXT_MUTED));
    ui.label(
        egui::RichText::new(value)
            .size(12.0)
            .color(TEXT_MUTED),
    );
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

fn open_task_manager() {
    use enigo::{Enigo, Key, Keyboard, Settings};
    use std::thread::sleep;
    use std::time::Duration;

    let _ = std::process::Command::new("taskmgr").spawn();
    sleep(Duration::from_millis(1500));

    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let _ = enigo.key(Key::Control, enigo::Direction::Press);
        let _ = enigo.key(Key::Unicode('f'), enigo::Direction::Click);
        let _ = enigo.key(Key::Control, enigo::Direction::Release);
        sleep(Duration::from_millis(200));
        let _ = enigo.key(Key::Home, enigo::Direction::Click);
        let _ = enigo.key(Key::Shift, enigo::Direction::Press);
        let _ = enigo.key(Key::End, enigo::Direction::Click);
        let _ = enigo.key(Key::Shift, enigo::Direction::Release);
        let _ = enigo.text("Jarvis");
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
