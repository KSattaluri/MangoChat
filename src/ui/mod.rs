pub mod theme;
pub mod formatting;
pub mod window;
pub mod tray;
pub mod widgets;
pub mod form_state;
pub mod snip_overlay;
pub mod tabs;

use crate::audio;
use crate::settings::Settings;
use crate::usage::{append_usage_line, session_usage_path};
use crate::state::{AppEvent, AppState, SessionUsage};
use crate::updater::{self, CheckOutcome, ReleaseInfo, WorkerMessage};
use eframe::egui;
use egui::{
    pos2, vec2, Color32, Pos2, Rect, Sense, Stroke, TextureHandle,
    ViewportBuilder, ViewportCommand, ViewportId,
};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver as EventReceiver, Sender as EventSender};
use std::sync::Arc;
use std::time::Duration;

use theme::*;
use formatting::*;
use window::*;
use widgets::*;
use tray::*;
use form_state::FormState;

#[derive(Debug, Clone)]
pub enum UpdateUiState {
    NotChecked,
    Checking,
    UpToDate { current: String },
    Available { current: String, latest: ReleaseInfo },
    Installing,
    InstallLaunched { path: String },
    Error(String),
}

pub struct MangoChatApp {
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
    pub snip_bounds: Option<crate::snip::MonitorBounds>,
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
    pub form: FormState,
    pub key_check_inflight: HashSet<String>,
    pub key_check_result: HashMap<String, (bool, String)>,
    pub last_validated_provider: Option<String>,
    pub session_history: Vec<SessionUsage>,
    control_tooltip: Option<ControlTooltipState>,
    recording_limit_token: u64,
    pub confirm_reset_totals: bool,
    pub confirm_reset_include_sessions: bool,
    pub selected_mic_unavailable: bool,
    pub update_state: UpdateUiState,
    pub update_worker_tx: mpsc::Sender<WorkerMessage>,
    pub update_worker_rx: mpsc::Receiver<WorkerMessage>,
    pub update_last_check: Option<std::time::Instant>,
    pub update_check_inflight: bool,
    pub update_install_inflight: bool,
}

impl MangoChatApp {
    pub fn current_accent(&self) -> AccentPalette {
        if self.settings_open {
            accent_palette(&self.form.accent_color)
        } else {
            accent_palette(&self.settings.accent_color)
        }
    }

    fn persist_accent_if_changed(&mut self) {
        if self.settings.accent_color == self.form.accent_color {
            return;
        }
        self.settings.accent_color = self.form.accent_color.clone();
        match crate::settings::save(&self.settings) {
            Ok(()) => {
                self._tray_icon = setup_tray(accent_palette(&self.settings.accent_color));
            }
            Err(e) => {
                self.set_status(&format!("Save failed: {}", e), "error");
            }
        }
    }

    pub fn provider_form_dirty(&self) -> bool {
        if self.form.provider != self.settings.provider {
            return true;
        }
        for (provider_id, _) in PROVIDER_ROWS {
            let form_val = self
                .form
                .api_keys
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
        let base = if self.settings.screenshot_enabled {
            COMPACT_WINDOW_W_WITH_SNIP
        } else {
            COMPACT_WINDOW_W_NO_SNIP
        };
        if self.settings.compact_background_enabled {
            base + COMPACT_BG_EXTRA_W
        } else {
            base
        }
    }

    fn compact_window_height(&self) -> f32 {
        let base = if self.settings.screenshot_enabled {
            COMPACT_WINDOW_H_WITH_SNIP
        } else {
            COMPACT_WINDOW_H
        };
        if self.settings.compact_background_enabled {
            base + COMPACT_BG_EXTRA_H
        } else {
            base
        }
    }

    pub fn monitor_choices(&self) -> Vec<MonitorChoice> {
        available_monitor_choices()
    }

    pub fn monitor_label_for_id(&self, id: &str) -> String {
        if id.trim().is_empty() {
            return "Auto (cursor monitor)".into();
        }
        self.monitor_choices()
            .into_iter()
            .find(|m| m.id == id)
            .map(|m| m.label)
            .unwrap_or_else(|| format!("{} (disconnected)", id))
    }

    pub fn anchor_label(anchor: &str) -> &'static str {
        match anchor {
            WINDOW_ANCHOR_TOP_LEFT => "Top Left",
            WINDOW_ANCHOR_TOP_CENTER => "Top Center",
            WINDOW_ANCHOR_TOP_RIGHT => "Top Right",
            WINDOW_ANCHOR_BOTTOM_LEFT => "Bottom Left",
            WINDOW_ANCHOR_BOTTOM_CENTER => "Bottom Center",
            _ => "Bottom Right",
        }
    }

    pub fn provider_color(provider_id: &str, p: ThemePalette) -> Color32 {
        match provider_id {
            "openai" => Color32::from_rgb(0x10, 0xb9, 0x81),
            "deepgram" => Color32::from_rgb(0x3b, 0x82, 0xf6),
            "elevenlabs" => Color32::from_rgb(0xf5, 0x9e, 0x0b),
            "assemblyai" => Color32::from_rgb(0xa8, 0x55, 0xf7),
            _ => p.text,
        }
    }

    pub fn provider_display_name(provider_id: &str) -> &str {
        PROVIDER_ROWS
            .iter()
            .find(|(id, _)| *id == provider_id)
            .map(|(_, name)| *name)
            .unwrap_or(provider_id)
    }

    fn sync_form_from_settings(&mut self) {
        self.form = FormState::from_settings(&self.settings);
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
        let form = FormState::from_settings(&settings);

        let (update_worker_tx, update_worker_rx) = mpsc::channel::<WorkerMessage>();

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

        Self {
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
            form,
            key_check_inflight: HashSet::new(),
            key_check_result: HashMap::new(),
            last_validated_provider: None,
            session_history: vec![],
            control_tooltip: None,
            recording_limit_token: 0,
            confirm_reset_totals: false,
            confirm_reset_include_sessions: false,
            selected_mic_unavailable: false,
            update_state: UpdateUiState::NotChecked,
            update_worker_tx,
            update_worker_rx,
            update_last_check: None,
            update_check_inflight: false,
            update_install_inflight: false,
        }
    }

    pub fn trigger_update_check(&mut self) {
        if self.update_check_inflight {
            return;
        }
        self.update_check_inflight = true;
        self.update_state = UpdateUiState::Checking;
        updater::spawn_check(
            self.update_worker_tx.clone(),
            self.form.update_include_prerelease,
        );
    }

    pub fn trigger_update_install(&mut self) {
        if self.update_install_inflight {
            return;
        }
        let release = match &self.update_state {
            UpdateUiState::Available { latest, .. } => latest.clone(),
            _ => return,
        };
        self.update_install_inflight = true;
        self.update_state = UpdateUiState::Installing;
        updater::spawn_install(self.update_worker_tx.clone(), release);
    }

    pub fn open_update_release_page(&mut self) {
        if let UpdateUiState::Available { latest, .. } = &self.update_state {
            if let Err(e) = updater::open_release_page(&latest.html_url) {
                self.set_status(&e, "error");
            }
        }
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
        if ctx.viewport_id() != ViewportId::ROOT {
            return;
        }

        let mut style = egui::Style::default();
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(8.0, 5.0);
        style.spacing.interact_size.y = 24.0;
        ctx.set_visuals(egui::Visuals::dark());
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
            let h = (monitor_h * 0.72).max(520.0).min(max_h.min(760.0));
            return vec2(w, h);
        }
        vec2(980.0, 720.0)
    }

    fn apply_window_mode(&mut self, ctx: &egui::Context, settings_open: bool) {
        let target = if settings_open {
            self.expanded_window_size(ctx)
        } else {
            vec2(self.compact_window_width(), self.compact_window_height())
        };
        if settings_open {
            if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                self.compact_anchor_pos = Some(outer.min);
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
            let pos = clamp_window_pos(
                ctx,
                anchor,
                target,
                &self.settings.window_monitor_mode,
                &self.settings.window_monitor_id,
            );
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
        } else if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
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

    pub fn set_status(&mut self, text: &str, state: &str) {
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

        if current_key.is_empty() {
            self.set_status("Listening (no API key)", "live");
            return;
        }

        let gen = self.state.session_gen.fetch_add(1, Ordering::SeqCst) + 1;
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

        while let Ok(msg) = self.update_worker_rx.try_recv() {
            match msg {
                WorkerMessage::CheckFinished(result) => {
                    self.update_check_inflight = false;
                    self.update_last_check = Some(std::time::Instant::now());
                    match result {
                        Ok(CheckOutcome::UpToDate { current }) => {
                            self.update_state = UpdateUiState::UpToDate {
                                current: current.to_string(),
                            };
                        }
                        Ok(CheckOutcome::UpdateAvailable { current, latest }) => {
                            self.update_state = UpdateUiState::Available {
                                current: current.to_string(),
                                latest,
                            };
                        }
                        Err(e) => {
                            self.update_state = UpdateUiState::Error(e.clone());
                            self.set_status(&e, "error");
                        }
                    }
                }
                WorkerMessage::InstallFinished(result) => {
                    self.update_install_inflight = false;
                    match result {
                        Ok(path) => {
                            self.update_state = UpdateUiState::InstallLaunched {
                                path: path.clone(),
                            };
                            self.set_status(
                                "Installer launched. Closing app for upgrade...",
                                "idle",
                            );
                            self.should_quit = true;
                        }
                        Err(e) => {
                            self.update_state = UpdateUiState::Error(e.clone());
                            self.set_status(&e, "error");
                        }
                    }
                }
            }
        }
    }

    fn paint_control_tooltip(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        key: &str,
        text: &str,
        persist_on_click: bool,
        tooltip_pos: Option<Pos2>,
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

        let pos =
            tooltip_pos.unwrap_or(pos2(response.rect.center().x, response.rect.min.y - 6.0));
        egui::Area::new(egui::Id::new(format!("control_tooltip_{key}")))
            .order(egui::Order::Foreground)
            .interactable(false)
            .constrain(false)
            .pivot(egui::Align2::CENTER_BOTTOM)
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
        let show_screenshot_controls = self.settings.screenshot_enabled;
        let preset_btn =
            |ui: &mut egui::Ui,
             label: &str,
             active: bool,
             p: ThemePalette,
             accent: AccentPalette| {
                ui.add(
                    egui::Button::new(
                        egui::RichText::new(label)
                            .size(11.0)
                            .strong()
                            .color(if active { Color32::WHITE } else { p.text }),
                    )
                    .fill(if active { accent.base } else { p.btn_bg })
                    .stroke(Stroke::new(
                        1.0,
                        if active { accent.ring } else { p.btn_border },
                    ))
                    .rounding(4.0)
                    .min_size(vec2(20.0, 22.0)),
                )
            };
        let compact_mode = !self.settings_open;
        let compact_bg = compact_mode && self.settings.compact_background_enabled;
        let panel_fill = if self.settings_open {
            p.settings_bg
        } else {
            Color32::TRANSPARENT
        };
        let panel_margin = if compact_bg { 16.0 } else { 12.0 };
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(panel_fill)
                    .inner_margin(egui::Margin::symmetric(panel_margin, panel_margin)),
            )
            .show(ctx, |ui| {
                if compact_bg {
                    let bg_rect = ui.max_rect().expand2(vec2(12.0, 8.0));
                    ui.painter().rect(
                        bg_rect,
                        12.0,
                        p.settings_bg,
                        Stroke::new(1.0, p.btn_border),
                    );
                }
                // --- Top control row ---
                if self.settings_open {
                    ui.add_space(8.0);
                }
                let viz_center = ui
                    .horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if self.settings_open {
                            ui.add_space(16.0);
                        }

                        let record_resp = record_toggle(ui, self.is_recording, accent);
                        if record_resp.clicked() {
                            if self.is_recording {
                                self.stop_recording();
                            } else {
                                self.start_recording();
                            }
                        }
                        let settings_w = 28.0;
                        let right_edge_pad = 6.0;
                        let right_controls_w = settings_w + right_edge_pad;
                        let min_viz_w = 56.0;
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
                        let viz_center = viz_rect.center();
                        let record_tip = if self.is_recording { "Stop" } else { "Start" };
                        self.paint_control_tooltip(
                            ctx,
                            &record_resp,
                            "record",
                            record_tip,
                            true,
                            Some(viz_center),
                        );
                        if self.selected_mic_unavailable {
                            let icon_size = vec2(20.0, 22.0);
                            let icon_rect =
                                Rect::from_center_size(viz_rect.center(), icon_size);
                            let mic_resp = mic_unavailable_badge(ui, icon_rect);
                            self.paint_control_tooltip(
                                ctx,
                                &mic_resp,
                                "mic_unavailable",
                                "Device unavailable. Open Settings.",
                                false,
                                Some(viz_center),
                            );
                        }

                        if self.settings_open {
                            if collapse_toggle(ui, accent).clicked() {
                                self.persist_accent_if_changed();
                                self.settings_open = false;
                                self.apply_window_mode(ctx, false);
                            }
                        } else {
                            let settings_resp =
                                settings_toggle(ui, self.is_recording, accent);
                            self.paint_control_tooltip(
                                ctx,
                                &settings_resp,
                                "settings",
                                "Settings",
                                false,
                                Some(viz_center),
                            );
                            if settings_resp.clicked() {
                                self.settings_open = true;
                                self.sync_form_from_settings();
                                self.session_history =
                                    crate::usage::load_recent_sessions(20);
                                self.apply_window_mode(ctx, true);
                            }
                        }
                        ui.add_space(right_edge_pad);
                        viz_center
                    })
                    .inner;

                if show_screenshot_controls && !self.settings_open {
                    ui.add_space(1.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let btns_w = 3.0 * 28.0 + 2.0 * 6.0;
                        let pad = ((ui.available_width() - btns_w) * 0.5).max(0.0);
                        ui.add_space(pad);
                        let p_resp =
                            preset_btn(ui, "P", !self.snip_copy_image, p, accent);
                        self.paint_control_tooltip(
                            ctx,
                            &p_resp,
                            "preset_path",
                            "Preset: Path (copy file path)",
                            true,
                            Some(viz_center),
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
                            accent,
                        );
                        self.paint_control_tooltip(
                            ctx,
                            &i_resp,
                            "preset_image",
                            "Preset: Image (copy image)",
                            true,
                            Some(viz_center),
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
                            accent,
                        );
                        self.paint_control_tooltip(
                            ctx,
                            &e_resp,
                            "preset_edit",
                            "Preset: Image + Edit",
                            true,
                            Some(viz_center),
                        );
                        if e_resp.clicked() {
                            self.snip_copy_image = true;
                            self.snip_edit_after = true;
                        }
                    });
                }

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
                                            ("dictation", "Dictation"),
                                            ("commands", "Commands"),
                                            ("appearance", "Appearance"),
                                            ("usage", "Usage"),
                                            ("faq", "FAQ"),
                                            ("about", "About"),
                                        ] {
                                            let active = self.settings_tab == id;
                                            if widgets::tab_button(
                                                ui, id, label, active, accent,
                                                nav_w - 8.0,
                                            )
                                            .clicked()
                                            {
                                                self.settings_tab = id.to_string();
                                            }
                                        }
                                    },
                                );

                                ui.separator();
                                ui.add_space(8.0);
                                ui.vertical(|ui| {
                                    if self.settings_tab == "usage"
                                        && prev_tab != "usage"
                                    {
                                        self.session_history =
                                            crate::usage::load_recent_sessions(20);
                                    }
                                    ui.add_space(2.0);

                                    // Reserve vertical space for the Save button so
                                    // tab content doesn't push it off-screen on
                                    // smaller monitors.
                                    let has_save = matches!(
                                        self.settings_tab.as_str(),
                                        "provider"
                                            | "dictation"
                                            | "commands"
                                            | "appearance"
                                            | "about"
                                    );
                                    let save_reserve =
                                        if has_save { 38.0 } else { 0.0 };
                                    let content_size = vec2(
                                        ui.available_width(),
                                        (ui.available_height() - save_reserve)
                                            .max(200.0),
                                    );

                                    // ── Tab content ──
                                    ui.allocate_ui(content_size, |ui| {
                                        match self.settings_tab.as_str() {
                                            "provider" => {
                                                tabs::provider::render(
                                                    self, ui, ctx,
                                                );
                                            }
                                            "dictation" => {
                                                tabs::dictation::render(
                                                    self, ui, ctx,
                                                );
                                            }
                                            "commands" => {
                                                tabs::commands::render(
                                                    self, ui, ctx,
                                                );
                                            }
                                            "appearance" => {
                                                tabs::appearance::render(
                                                    self, ui, ctx,
                                                );
                                            }
                                            "usage" => {
                                                tabs::usage::render(
                                                    self, ui, ctx,
                                                );
                                            }
                                            "about" => {
                                                tabs::about::render_about(
                                                    self, ui, ctx,
                                                );
                                            }
                                            "faq" => {
                                                tabs::about::render_faq(
                                                    self, ui, ctx,
                                                );
                                            }
                                            _ => {}
                                        }
                                    });

                                    // Save button (only on settings tabs)
                                    ui.add_space(10.0);
                                    if matches!(
                                        self.settings_tab.as_str(),
                                        "provider"
                                            | "dictation"
                                            | "commands"
                                            | "appearance"
                                            | "about"
                                    ) {
                                        ui.add_space(6.0);
                                        let provider_dirty = self.settings_tab
                                            == "provider"
                                            && self.provider_form_dirty();
                                        let show_exit = self.settings_tab
                                            == "provider"
                                            && !provider_dirty;
                                        let save_label =
                                            if show_exit { "Exit" } else { "Save" };
                                        let save_w = ui.available_width() - 16.0;
                                        let save = ui.add_sized(
                                            [save_w, 24.0],
                                            egui::Button::new(
                                                egui::RichText::new(save_label)
                                                    .size(15.0)
                                                    .strong()
                                                    .color(if show_exit {
                                                        TEXT_COLOR
                                                    } else {
                                                        Color32::BLACK
                                                    }),
                                            )
                                            .fill(if show_exit {
                                                BTN_BG
                                            } else {
                                                accent.base
                                            })
                                            .stroke(Stroke::new(
                                                1.0,
                                                if show_exit {
                                                    BTN_BORDER
                                                } else {
                                                    accent.ring
                                                },
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
                                                .form
                                                .api_keys
                                                .get(&self.form.provider)
                                                .map(|k| !k.trim().is_empty())
                                                .unwrap_or(false);
                                            if self.settings_tab == "provider"
                                                && !default_key_present
                                            {
                                                self.set_status(
                                                    "Select a default provider with an API key",
                                                    "error",
                                                );
                                            } else {
                                                self.form
                                                    .apply_to_settings(&mut self.settings);
                                                self.selected_mic_unavailable =
                                                    self.selected_mic_unavailable_now();
                                                match crate::settings::save(
                                                    &self.settings,
                                                ) {
                                                    Ok(()) => {
                                                        if let Ok(mut p) =
                                                            self.state.chrome_path.lock()
                                                        {
                                                            *p = self
                                                                .settings
                                                                .chrome_path
                                                                .clone();
                                                        }
                                                        if let Ok(mut p) =
                                                            self.state.paint_path.lock()
                                                        {
                                                            *p = self
                                                                .settings
                                                                .paint_path
                                                                .clone();
                                                        }
                                                        if let Ok(mut v) = self
                                                            .state
                                                            .url_commands
                                                            .lock()
                                                        {
                                                            *v = self
                                                                .settings
                                                                .url_commands
                                                                .iter()
                                                                .map(|c| {
                                                                    (
                                                                        c.trigger.clone(),
                                                                        c.url.clone(),
                                                                    )
                                                                })
                                                                .collect();
                                                        }
                                                        if let Ok(mut v) = self
                                                            .state
                                                            .alias_commands
                                                            .lock()
                                                        {
                                                            *v = self
                                                                .settings
                                                                .alias_commands
                                                                .iter()
                                                                .map(|c| {
                                                                    (
                                                                        c.trigger.clone(),
                                                                        c.replacement
                                                                            .clone(),
                                                                    )
                                                                })
                                                                .collect();
                                                        }
                                                        self._tray_icon = setup_tray(
                                                            self.current_accent(),
                                                        );
                                                        self.state
                                                            .screenshot_enabled
                                                            .store(
                                                                self.settings
                                                                    .screenshot_enabled,
                                                                Ordering::SeqCst,
                                                            );
                                                        if self.settings_tab
                                                            == "provider"
                                                        {
                                                            let was_recording =
                                                                self.is_recording;
                                                            if was_recording {
                                                                self.stop_recording();
                                                                self.start_recording();
                                                            }
                                                            self.compact_anchor_pos =
                                                                None;
                                                            self.set_status(
                                                                "Saved", "idle",
                                                            );
                                                            self.settings_open = false;
                                                            self.apply_window_mode(
                                                                ctx, false,
                                                            );
                                                        } else {
                                                            self.apply_appearance(ctx);
                                                            self.compact_anchor_pos =
                                                                None;
                                                            self.set_status(
                                                                "Saved", "idle",
                                                            );
                                                            self.settings_open = false;
                                                            self.apply_window_mode(
                                                                ctx, false,
                                                            );
                                                        }
                                                    }
                                                    Err(e) => self.set_status(
                                                        &format!("Save failed: {}", e),
                                                        "error",
                                                    ),
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
}

impl eframe::App for MangoChatApp {
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

        if self.settings.auto_update_enabled
            && !self.update_check_inflight
            && !self.update_install_inflight
        {
            let should_check = self
                .update_last_check
                .map(|t| t.elapsed() >= std::time::Duration::from_secs(6 * 60 * 60))
                .unwrap_or(true);
            if should_check {
                self.trigger_update_check();
            }
        }

        // Position bottom-right on first frame
        if !self.positioned {
            let compact_size = vec2(self.compact_window_width(), self.compact_window_height());
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(compact_size));
            if self.settings.window_monitor_mode == WINDOW_MONITOR_MODE_FIXED {
                let placed = place_compact_fixed_native(
                    compact_size,
                    &self.settings.window_monitor_id,
                    &self.settings.window_anchor,
                );
                self.positioned = placed;
                self.initial_position_corrected = placed;
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
                if (clamped.x - outer.min.x).abs() > 0.5
                    || (clamped.y - outer.min.y).abs() > 0.5
                {
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
        if self.should_quit {
            std::process::exit(0);
        }

        if self.settings_open && self.settings.auto_minimize {
            let focused = ctx.input(|i| i.viewport().focused);
            if focused == Some(false) {
                self.persist_accent_if_changed();
                self.settings_open = false;
                self.apply_window_mode(ctx, false);
            }
        }

        self.render_main_ui(ctx);

        // Snip overlay viewport
        if self.snip_overlay_active {
            let vp = if let Some(b) = &self.snip_bounds {
                let scale = if b.scale_factor > 0.0 {
                    b.scale_factor
                } else {
                    1.0
                };
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
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
}

