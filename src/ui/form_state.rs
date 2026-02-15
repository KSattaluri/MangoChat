use crate::settings::Settings;
use std::collections::HashMap;

use super::theme::PROVIDER_ROWS;
use super::window::WINDOW_MONITOR_MODE_FIXED;

#[allow(dead_code)]
pub struct FormState {
    pub provider: String,
    pub api_keys: HashMap<String, String>,
    pub model: String,
    pub language: String,
    pub mic: String,
    pub vad_mode: String,
    pub screenshot_enabled: bool,
    pub screenshot_retention_count: u32,
    pub start_cue: String,
    pub text_size: String,
    pub accent_color: String,
    pub compact_background_enabled: bool,
    pub auto_minimize: bool,
    pub window_monitor_mode: String,
    pub window_monitor_id: String,
    pub window_anchor: String,
    pub snip_editor_path: String,
    pub chrome_path: String,
    pub paint_path: String,
    pub provider_inactivity_timeout_secs: u64,
    pub max_session_length_minutes: u64,
    pub url_commands: Vec<crate::settings::UrlCommand>,
    pub alias_commands: Vec<crate::settings::AliasCommand>,
}

impl FormState {
    pub fn from_settings(settings: &Settings) -> Self {
        let mut api_keys = settings.api_keys.clone();
        for (id, _) in PROVIDER_ROWS {
            api_keys.entry((*id).to_string()).or_default();
        }
        Self {
            provider: settings.provider.clone(),
            api_keys,
            model: settings.model.clone(),
            language: settings.language.clone(),
            mic: settings.mic_device.clone(),
            vad_mode: settings.vad_mode.clone(),
            screenshot_enabled: settings.screenshot_enabled,
            screenshot_retention_count: settings.screenshot_retention_count,
            start_cue: settings.start_cue.clone(),
            text_size: settings.text_size.clone(),
            accent_color: settings.accent_color.clone(),
            compact_background_enabled: settings.compact_background_enabled,
            auto_minimize: settings.auto_minimize,
            window_monitor_mode: WINDOW_MONITOR_MODE_FIXED.to_string(),
            window_monitor_id: settings.window_monitor_id.clone(),
            window_anchor: settings.window_anchor.clone(),
            snip_editor_path: settings.snip_editor_path.clone(),
            chrome_path: settings.chrome_path.clone(),
            paint_path: settings.paint_path.clone(),
            provider_inactivity_timeout_secs: settings.provider_inactivity_timeout_secs,
            max_session_length_minutes: settings.max_session_length_minutes,
            url_commands: settings.url_commands.clone(),
            alias_commands: settings.alias_commands.clone(),
        }
    }

    pub fn apply_to_settings(&self, settings: &mut Settings) {
        settings.provider = self.provider.clone();
        for (provider_id, _) in PROVIDER_ROWS {
            let value = self
                .api_keys
                .get(*provider_id)
                .cloned()
                .unwrap_or_default();
            settings.set_api_key(provider_id, value);
        }
        settings.mic_device = self.mic.clone();
        settings.vad_mode = self.vad_mode.clone();
        settings.screenshot_enabled = self.screenshot_enabled;
        settings.screenshot_retention_count = self.screenshot_retention_count.clamp(1, 200);
        settings.start_cue = self.start_cue.clone();
        settings.theme = "dark".to_string();
        settings.text_size = self.text_size.clone();
        settings.accent_color = self.accent_color.clone();
        settings.compact_background_enabled = self.compact_background_enabled;
        settings.auto_minimize = self.auto_minimize;
        settings.window_monitor_mode = WINDOW_MONITOR_MODE_FIXED.to_string();
        settings.window_monitor_id = self.window_monitor_id.clone();
        settings.window_anchor = self.window_anchor.clone();
        settings.snip_editor_path = self.snip_editor_path.clone();
        settings.chrome_path = self.chrome_path.clone();
        settings.paint_path = self.paint_path.clone();
        settings.provider_inactivity_timeout_secs =
            self.provider_inactivity_timeout_secs.clamp(5, 300);
        settings.max_session_length_minutes = self.max_session_length_minutes.clamp(1, 120);
        settings.url_commands = self.url_commands.clone();
        settings.alias_commands = self.alias_commands.clone();
    }
}
