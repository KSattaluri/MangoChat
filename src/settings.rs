use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Per-provider API keys: {"openai": "sk-...", "deepgram": "dg-...", ...}
    #[serde(default, skip_serializing)]
    pub api_keys: HashMap<String, String>,
    /// Legacy single key - migrated to api_keys on load, not saved.
    #[serde(default, skip_serializing)]
    api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_transcription_model")]
    pub transcription_model: String,
    /// Latency knob for OpenAI `gpt-live-transcribe`.
    /// "" (server default) | minimal | low | medium | high | xhigh.
    #[serde(default)]
    pub openai_transcribe_delay: String,
    #[serde(default = "default_assemblyai_speech_model")]
    pub assemblyai_speech_model: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub mic_device: String,
    #[serde(default = "default_vad_mode")]
    pub vad_mode: String,
    #[serde(default = "default_true")]
    pub session_hotkey_enabled: bool,
    #[serde(default)]
    pub screenshot_enabled: bool,
    #[serde(default = "default_true")]
    pub screenshot_hotkey_enabled: bool,
    #[serde(default = "default_screenshot_retention_count")]
    pub screenshot_retention_count: u32,
    #[serde(default = "default_start_cue")]
    pub start_cue: String,
    #[serde(default = "default_theme")]
    pub theme: String, // dark only
    #[serde(default = "default_text_size")]
    pub text_size: String, // small | medium | large
    #[serde(default = "default_accent_color")]
    pub accent_color: String, // green | purple | blue | orange | pink
    #[serde(default)]
    pub compact_background_enabled: bool,
    #[serde(default)]
    pub auto_minimize: bool,
    #[serde(default)]
    pub update_feed_url_override: String,
    #[serde(default = "default_window_monitor_mode")]
    pub window_monitor_mode: String, // follow_cursor | fixed
    #[serde(default)]
    pub window_monitor_id: String, // Win32 monitor device id (e.g. \\.\DISPLAY1) when mode=fixed
    #[serde(default = "default_window_anchor")]
    pub window_anchor: String, // top_left | top_center | top_right | bottom_left | bottom_center | bottom_right
    #[serde(default)]
    pub snip_editor_path: String,
    #[serde(default = "default_snip_edit_revert")]
    pub snip_edit_revert: String, // stay | image | path
    #[serde(default = "default_browser")]
    pub default_browser: String, // chrome | edge | firefox
    #[serde(default = "default_chrome_path")]
    pub chrome_path: String,
    #[serde(default = "default_paint_path")]
    pub paint_path: String,
    #[serde(default = "default_provider_inactivity_timeout_secs")]
    pub provider_inactivity_timeout_secs: u64,
    #[serde(default = "default_max_session_length_minutes")]
    pub max_session_length_minutes: u64,
    #[serde(default = "default_url_commands")]
    pub url_commands: Vec<UrlCommand>,
    #[serde(default = "default_alias_commands")]
    pub alias_commands: Vec<AliasCommand>,
    #[serde(default = "default_app_shortcuts")]
    pub app_shortcuts: Vec<AppShortcut>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UrlCommand {
    pub trigger: String,
    pub url: String,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AliasCommand {
    pub trigger: String,
    pub replacement: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppShortcut {
    pub trigger: String,
    pub path: String,
    #[serde(default)]
    pub builtin: bool,
}

impl Settings {
    /// Get the API key for a given provider.
    pub fn api_key_for(&self, provider: &str) -> &str {
        self.api_keys
            .get(provider)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Model name to display and record for the active provider. OpenAI runs a
    /// realtime *transcription* session, so its transcription model is the one
    /// that matters; `model` is no longer used by any provider.
    pub fn effective_model(&self) -> String {
        match self.provider.as_str() {
            "openai" => self.transcription_model.clone(),
            "deepgram" => "nova-3".to_string(),
            "elevenlabs" => "scribe_v2_realtime".to_string(),
            "assemblyai" => self.assemblyai_speech_model.clone(),
            _ => String::new(),
        }
    }

    /// True when at least one provider key is configured.
    pub fn has_any_api_key(&self) -> bool {
        self.api_keys.values().any(|k| !k.trim().is_empty())
    }

    /// Return the browser executable path based on the selected default browser.
    /// Falls back to the custom chrome_path for "chrome", and uses known
    /// default install locations for Edge and Firefox.
    pub fn resolved_browser_path(&self) -> String {
        match self.default_browser.as_str() {
            "edge" => default_edge_path(),
            "firefox" => default_firefox_path(),
            _ => self.chrome_path.clone(), // "chrome" or unknown
        }
    }

    /// Set the API key for a given provider.
    pub fn set_api_key(&mut self, provider: &str, key: String) {
        if key.is_empty() {
            self.api_keys.remove(provider);
        } else {
            self.api_keys.insert(provider.to_string(), key);
        }
    }

    /// Defaults used by the in-app "Reset defaults" action.
    /// Provider/API-key-related fields are intentionally left to the caller.
    pub fn non_provider_reset_defaults() -> Self {
        let mut s = Self::default();
        s.session_hotkey_enabled = true;
        s.screenshot_enabled = true;
        s.screenshot_hotkey_enabled = true;
        s.compact_background_enabled = true;
        s.auto_minimize = true;
        s.window_anchor = "bottom_left".to_string();
        s.snip_edit_revert = "path".to_string();
        s.alias_commands = vec![
            AliasCommand {
                trigger: "codex".into(),
                replacement: "codex app --dangerously-bypass-approvals-and-sandbox".into(),
            },
            AliasCommand {
                trigger: "claude".into(),
                replacement: "claude --dangerously-skip-permissions".into(),
            },
            AliasCommand {
                trigger: "bombay".into(),
                replacement: "mumbai".into(),
            },
        ];
        s
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            api_keys: HashMap::new(),
            api_key: String::new(),
            model: default_model(),
            transcription_model: default_transcription_model(),
            openai_transcribe_delay: String::new(),
            assemblyai_speech_model: default_assemblyai_speech_model(),
            language: default_language(),
            mic_device: String::new(),
            vad_mode: default_vad_mode(),
            session_hotkey_enabled: true,
            screenshot_enabled: true,
            screenshot_hotkey_enabled: true,
            screenshot_retention_count: default_screenshot_retention_count(),
            start_cue: default_start_cue(),
            theme: default_theme(),
            text_size: default_text_size(),
            accent_color: default_accent_color(),
            compact_background_enabled: true,
            auto_minimize: false,
            update_feed_url_override: String::new(),
            window_monitor_mode: default_window_monitor_mode(),
            window_monitor_id: String::new(),
            window_anchor: default_window_anchor(),
            snip_editor_path: String::new(),
            snip_edit_revert: default_snip_edit_revert(),
            default_browser: default_browser(),
            chrome_path: default_chrome_path(),
            paint_path: default_paint_path(),
            provider_inactivity_timeout_secs: default_provider_inactivity_timeout_secs(),
            max_session_length_minutes: default_max_session_length_minutes(),
            url_commands: default_url_commands(),
            alias_commands: default_alias_commands(),
            app_shortcuts: default_app_shortcuts(),
        }
    }
}

fn default_provider() -> String {
    String::new()
}

/// OpenAI transcription models supported on a Realtime transcription session.
pub const OPENAI_TRANSCRIBE_MODELS: &[&str] = &["gpt-transcribe", "gpt-live-transcribe"];

/// Valid `delay` values for `gpt-live-transcribe` ("" = server default).
pub const OPENAI_TRANSCRIBE_DELAYS: &[&str] = &["minimal", "low", "medium", "high", "xhigh"];

/// AssemblyAI v3 streaming speech models.
pub const ASSEMBLYAI_SPEECH_MODELS: &[&str] = &[
    "universal-streaming-english",
    "universal-streaming-multilingual",
    "universal-3-5-pro",
];

/// OpenAI speech-to-speech realtime models that are shut down or scheduled for
/// shutdown. MangoChat no longer uses an S2S model at all, so these are cleared.
const DEAD_OPENAI_REALTIME_MODELS: &[&str] = &[
    "gpt-4o-realtime-preview",
    "gpt-4o-realtime-preview-2025-06-03",
    "gpt-4o-realtime-preview-2024-12-17",
    "gpt-4o-mini-realtime-preview",
    "gpt-realtime",
    "gpt-realtime-1.5",
    "gpt-realtime-mini",
    "gpt-4o-realtime",
    "gpt-4o-mini-realtime",
];

/// Deprecated OpenAI transcription models that are not supported on a
/// transcription session.
const LEGACY_OPENAI_TRANSCRIBE_MODELS: &[&str] = &[
    "whisper-1",
    "gpt-4o-transcribe",
    "gpt-4o-mini-transcribe",
    "gpt-4o-mini-transcribe-2025-03-20",
    "gpt-4o-mini-transcribe-2025-12-15",
    "gpt-4o-transcribe-diarize",
];

fn default_model() -> String {
    // Transcription sessions take no speech-to-speech model.
    String::new()
}
fn default_transcription_model() -> String {
    "gpt-transcribe".into()
}
fn default_assemblyai_speech_model() -> String {
    "universal-streaming-english".into()
}
fn default_language() -> String {
    "en".into()
}
fn default_vad_mode() -> String {
    "strict".into()
}
fn default_true() -> bool {
    true
}
fn default_start_cue() -> String {
    "audio1.wav".into()
}
fn default_screenshot_retention_count() -> u32 {
    10
}
fn default_theme() -> String {
    "dark".into()
}
fn default_text_size() -> String {
    "medium".into()
}
fn default_accent_color() -> String {
    "orange".into()
}
fn default_window_monitor_mode() -> String {
    "fixed".into()
}
fn default_window_anchor() -> String {
    "bottom_right".into()
}
fn default_snip_edit_revert() -> String {
    "stay".into()
}
fn default_browser() -> String {
    "chrome".into()
}
fn default_chrome_path() -> String {
    r"C:\Program Files\Google\Chrome\Application\chrome.exe".into()
}
fn default_edge_path() -> String {
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".into()
}
fn default_firefox_path() -> String {
    r"C:\Program Files\Mozilla Firefox\firefox.exe".into()
}
fn default_paint_path() -> String {
    r"C:\Windows\System32\mspaint.exe".into()
}
fn default_explorer_path() -> String {
    r"C:\".into()
}
fn default_provider_inactivity_timeout_secs() -> u64 {
    60
}
fn default_max_session_length_minutes() -> u64 {
    15
}
fn default_url_commands() -> Vec<UrlCommand> {
    vec![
        UrlCommand {
            trigger: "github".into(),
            url: "https://github.com".into(),
            builtin: true,
        },
        UrlCommand {
            trigger: "youtube".into(),
            url: "https://youtube.com".into(),
            builtin: true,
        },
        UrlCommand {
            trigger: "explorer".into(),
            url: default_explorer_path(),
            builtin: true,
        },
    ]
}
fn default_alias_commands() -> Vec<AliasCommand> {
    vec![
        AliasCommand {
            trigger: "codex".into(),
            replacement: "codex app --dangerously-bypass-approvals-and-sandbox".into(),
        },
        AliasCommand {
            trigger: "claude".into(),
            replacement: "claude --dangerously-skip-permissions".into(),
        },
        AliasCommand {
            trigger: "bombay".into(),
            replacement: "mumbai".into(),
        },
    ]
}
fn default_app_shortcuts() -> Vec<AppShortcut> {
    vec![
        AppShortcut {
            trigger: "chrome".into(),
            path: default_chrome_path(),
            builtin: true,
        },
        AppShortcut {
            trigger: "paint".into(),
            path: default_paint_path(),
            builtin: true,
        },
    ]
}

pub fn settings_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("MangoChat").join("settings.json"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".mangochat").join("settings.json"));
    }
    Err("Failed to resolve data directory".into())
}

fn legacy_settings_path() -> Result<PathBuf, String> {
    Err("Legacy settings path disabled".into())
}

pub fn load() -> Settings {
    let path = match settings_path() {
        Ok(p) => p,
        Err(_) => return Settings::default(),
    };
    let read_path = if path.exists() {
        path
    } else {
        match legacy_settings_path() {
            Ok(p) => p,
            Err(_) => return Settings::default(),
        }
    };
    let mut settings: Settings = match fs::read_to_string(&read_path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => return Settings::default(),
    };

    let had_plaintext_keys = !settings.api_keys.is_empty() || !settings.api_key.is_empty();

    // Migrate legacy single api_key to per-provider map.
    if !settings.api_key.is_empty() && !settings.api_keys.contains_key("openai") {
        settings
            .api_keys
            .insert("openai".into(), settings.api_key.clone());
        settings.api_key.clear();
    }

    let mut resolved_api_keys = settings.api_keys.clone();
    match crate::secrets::load_api_keys() {
        Ok(secure_keys) => {
            for (provider, key) in secure_keys {
                if !key.trim().is_empty() {
                    resolved_api_keys.insert(provider, key);
                }
            }
        }
        Err(e) => app_err!("[settings] secure key load failed: {}", e),
    }

    if had_plaintext_keys {
        match crate::secrets::save_api_keys(&resolved_api_keys) {
            Ok(()) => {
                settings.api_keys.clear();
                settings.api_key.clear();
                let _ = save_settings_without_api_keys(&settings);
            }
            Err(e) => app_err!("[settings] secure key migration failed: {}", e),
        }
    }
    settings.api_keys = resolved_api_keys;

    // Rewriting a dead provider model must survive a crash before the next
    // manual Save, so persist immediately when migrate() changed one.
    if migrate(&mut settings) {
        let _ = save_settings_without_api_keys(&settings);
    }
    settings
}

/// Normalize and migrate loaded settings in place. Does no file I/O so it can
/// be unit tested. Returns true when a deprecated provider model was rewritten
/// and the settings file should be re-saved.
pub fn migrate(settings: &mut Settings) -> bool {
    // Migrate deprecated provider id.
    if settings.provider == "deepgram-flux" {
        settings.provider = "deepgram".into();
    }
    // Keep provider unset unless it's a known provider id.
    if settings.provider != "openai"
        && settings.provider != "deepgram"
        && settings.provider != "elevenlabs"
        && settings.provider != "assemblyai"
    {
        settings.provider.clear();
    }

    // --- Provider model migrations (2026-09 provider refresh) ---
    let mut models_migrated = false;
    // A Realtime transcription session takes no speech-to-speech model, so any
    // leftover value is dropped.
    if !settings.model.is_empty() {
        if DEAD_OPENAI_REALTIME_MODELS.contains(&settings.model.as_str()) {
            app_log!(
                "[settings] dropping retired realtime model '{}'",
                settings.model
            );
        } else {
            app_log!(
                "[settings] dropping unused realtime model '{}'",
                settings.model
            );
        }
        settings.model.clear();
        models_migrated = true;
    }
    if LEGACY_OPENAI_TRANSCRIBE_MODELS.contains(&settings.transcription_model.as_str())
        || !OPENAI_TRANSCRIBE_MODELS.contains(&settings.transcription_model.as_str())
    {
        settings.transcription_model = default_transcription_model();
        models_migrated = true;
    }
    if !settings.openai_transcribe_delay.is_empty()
        && !OPENAI_TRANSCRIBE_DELAYS.contains(&settings.openai_transcribe_delay.as_str())
    {
        settings.openai_transcribe_delay.clear();
        models_migrated = true;
    }
    if !ASSEMBLYAI_SPEECH_MODELS.contains(&settings.assemblyai_speech_model.as_str()) {
        settings.assemblyai_speech_model = default_assemblyai_speech_model();
        models_migrated = true;
    }

    // App is dark-theme only.
    settings.theme = default_theme();
    // App supports strict/lenient VAD only.
    if settings.vad_mode == "off" {
        settings.vad_mode = default_vad_mode();
    }
    if settings.vad_mode != "strict" && settings.vad_mode != "lenient" {
        settings.vad_mode = default_vad_mode();
    }
    if settings.start_cue != "audio1.wav" && settings.start_cue != "audio2.wav" {
        settings.start_cue = default_start_cue();
    }
    let mut has_explorer = false;
    for cmd in settings.url_commands.iter_mut() {
        if cmd.trigger.trim().eq_ignore_ascii_case("explorer") {
            cmd.builtin = true;
            if cmd.url.trim().is_empty() {
                cmd.url = default_explorer_path();
            }
            has_explorer = true;
            break;
        }
    }
    if !has_explorer {
        settings.url_commands.push(UrlCommand {
            trigger: "explorer".into(),
            url: default_explorer_path(),
            builtin: true,
        });
    }
    for builtin in default_app_shortcuts() {
        if let Some(existing) = settings
            .app_shortcuts
            .iter_mut()
            .find(|s| s.trigger.trim().eq_ignore_ascii_case(&builtin.trigger))
        {
            existing.builtin = true;
            if existing.path.trim().is_empty() {
                existing.path = builtin.path;
            }
        } else {
            settings.app_shortcuts.push(builtin);
        }
    }
    if let Some(chrome) = settings
        .app_shortcuts
        .iter()
        .find(|s| s.trigger.trim().eq_ignore_ascii_case("chrome"))
    {
        if !chrome.path.trim().is_empty() {
            settings.chrome_path = chrome.path.clone();
        }
    }
    if let Some(paint) = settings
        .app_shortcuts
        .iter()
        .find(|s| s.trigger.trim().eq_ignore_ascii_case("paint"))
    {
        if !paint.path.trim().is_empty() {
            settings.paint_path = paint.path.clone();
        }
    }
    if settings.default_browser != "chrome"
        && settings.default_browser != "edge"
        && settings.default_browser != "firefox"
    {
        settings.default_browser = default_browser();
    }
    settings.screenshot_retention_count = settings.screenshot_retention_count.clamp(1, 200);
    if settings.text_size != "small"
        && settings.text_size != "medium"
        && settings.text_size != "large"
    {
        settings.text_size = default_text_size();
    }
    if settings.accent_color != "green"
        && settings.accent_color != "purple"
        && settings.accent_color != "blue"
        && settings.accent_color != "orange"
        && settings.accent_color != "pink"
    {
        settings.accent_color = default_accent_color();
    }
    if settings.window_monitor_mode != "fixed" {
        settings.window_monitor_mode = default_window_monitor_mode();
    }
    if settings.window_anchor != "top_left"
        && settings.window_anchor != "top_center"
        && settings.window_anchor != "top_right"
        && settings.window_anchor != "bottom_left"
        && settings.window_anchor != "bottom_center"
        && settings.window_anchor != "bottom_right"
    {
        settings.window_anchor = default_window_anchor();
    }
    if settings.snip_edit_revert != "stay"
        && settings.snip_edit_revert != "image"
        && settings.snip_edit_revert != "path"
    {
        settings.snip_edit_revert = default_snip_edit_revert();
    }
    settings.provider_inactivity_timeout_secs =
        settings.provider_inactivity_timeout_secs.clamp(5, 300);
    settings.max_session_length_minutes = settings.max_session_length_minutes.clamp(1, 120);
    settings.update_feed_url_override = settings.update_feed_url_override.trim().to_string();
    models_migrated
}

pub fn save(settings: &Settings) -> Result<(), String> {
    crate::secrets::save_api_keys(&settings.api_keys)?;
    save_settings_without_api_keys(settings)
}

fn save_settings_without_api_keys(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create settings dir: {}", e))?;
    }
    let mut clean = settings.clone();
    clean.api_keys.clear();
    clean.api_key.clear();
    let json = serde_json::to_string_pretty(&clean)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A settings.json as written by v0.1.x.
    fn legacy_settings_json() -> &'static str {
        r#"{
            "provider": "deepgram-flux",
            "model": "gpt-4o-realtime-preview",
            "transcription_model": "gpt-4o-mini-transcribe",
            "language": "en",
            "vad_mode": "off",
            "theme": "light",
            "start_cue": "audio9.wav"
        }"#
    }

    fn parse(json: &str) -> Settings {
        serde_json::from_str(json).expect("settings should deserialize")
    }

    #[test]
    fn defaults_use_transcription_session_models() {
        let settings = Settings::default();
        assert_eq!(settings.model, "");
        assert_eq!(settings.transcription_model, "gpt-transcribe");
        assert_eq!(settings.assemblyai_speech_model, "universal-streaming-english");
        assert_eq!(settings.openai_transcribe_delay, "");
    }

    #[test]
    fn migrate_rewrites_legacy_openai_models_and_provider_id() {
        let mut settings = parse(legacy_settings_json());
        assert!(migrate(&mut settings), "migration should request a re-save");
        assert_eq!(settings.provider, "deepgram");
        assert_eq!(settings.model, "");
        assert_eq!(settings.transcription_model, "gpt-transcribe");
        // Unrelated normalizations still run.
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.vad_mode, "strict");
        assert_eq!(settings.start_cue, "audio1.wav");
    }

    #[test]
    fn migrate_clears_every_dead_realtime_model() {
        for model in DEAD_OPENAI_REALTIME_MODELS {
            let mut settings = Settings::default();
            settings.model = (*model).to_string();
            assert!(migrate(&mut settings), "{} should migrate", model);
            assert_eq!(settings.model, "", "{} should be cleared", model);
        }
    }

    #[test]
    fn migrate_replaces_every_legacy_transcription_model() {
        for model in LEGACY_OPENAI_TRANSCRIBE_MODELS {
            let mut settings = Settings::default();
            settings.transcription_model = (*model).to_string();
            assert!(migrate(&mut settings), "{} should migrate", model);
            assert_eq!(settings.transcription_model, "gpt-transcribe");
        }
    }

    #[test]
    fn migrate_whitelists_transcription_model() {
        let mut settings = Settings::default();
        settings.transcription_model = "some-future-model".into();
        assert!(migrate(&mut settings));
        assert_eq!(settings.transcription_model, "gpt-transcribe");

        let mut settings = Settings::default();
        settings.transcription_model = "gpt-live-transcribe".into();
        assert!(!migrate(&mut settings), "supported model must be kept as-is");
        assert_eq!(settings.transcription_model, "gpt-live-transcribe");
    }

    #[test]
    fn migrate_whitelists_transcribe_delay() {
        let mut settings = Settings::default();
        settings.openai_transcribe_delay = "instant".into();
        assert!(migrate(&mut settings));
        assert_eq!(settings.openai_transcribe_delay, "");

        for delay in OPENAI_TRANSCRIBE_DELAYS {
            let mut settings = Settings::default();
            settings.openai_transcribe_delay = (*delay).to_string();
            assert!(!migrate(&mut settings), "{} should be kept", delay);
            assert_eq!(settings.openai_transcribe_delay, *delay);
        }
    }

    #[test]
    fn migrate_whitelists_assemblyai_speech_model() {
        let mut settings = Settings::default();
        settings.assemblyai_speech_model = "universal-2".into();
        assert!(migrate(&mut settings));
        assert_eq!(settings.assemblyai_speech_model, "universal-streaming-english");

        for model in ASSEMBLYAI_SPEECH_MODELS {
            let mut settings = Settings::default();
            settings.assemblyai_speech_model = (*model).to_string();
            assert!(!migrate(&mut settings), "{} should be kept", model);
            assert_eq!(settings.assemblyai_speech_model, *model);
        }
    }

    #[test]
    fn migrate_is_idempotent_and_quiet_on_current_settings() {
        let mut settings = Settings::default();
        settings.provider = "openai".into();
        assert!(!migrate(&mut settings));
        assert!(!migrate(&mut settings));
    }

    #[test]
    fn migrate_clears_unknown_provider_id() {
        let mut settings = Settings::default();
        settings.provider = "nuance".into();
        migrate(&mut settings);
        assert_eq!(settings.provider, "");
    }
}
