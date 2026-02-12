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
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub mic_device: String,
    #[serde(default = "default_vad_mode")]
    pub vad_mode: String,
    #[serde(default)]
    pub screenshot_enabled: bool,
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
    pub snip_editor_path: String,
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

impl Settings {
    /// Get the API key for a given provider.
    pub fn api_key_for(&self, provider: &str) -> &str {
        self.api_keys.get(provider).map(|s| s.as_str()).unwrap_or("")
    }

    /// Set the API key for a given provider.
    pub fn set_api_key(&mut self, provider: &str, key: String) {
        if key.is_empty() {
            self.api_keys.remove(provider);
        } else {
            self.api_keys.insert(provider.to_string(), key);
        }
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
            language: default_language(),
            mic_device: String::new(),
            vad_mode: default_vad_mode(),
            screenshot_enabled: false,
            screenshot_retention_count: default_screenshot_retention_count(),
            start_cue: default_start_cue(),
            theme: default_theme(),
            text_size: default_text_size(),
            accent_color: default_accent_color(),
            snip_editor_path: String::new(),
            chrome_path: default_chrome_path(),
            paint_path: default_paint_path(),
            provider_inactivity_timeout_secs: default_provider_inactivity_timeout_secs(),
            max_session_length_minutes: default_max_session_length_minutes(),
            url_commands: default_url_commands(),
            alias_commands: default_alias_commands(),
        }
    }
}

fn default_provider() -> String {
    "openai".into()
}

fn default_model() -> String {
    "gpt-4o-realtime-preview".into()
}
fn default_transcription_model() -> String {
    "gpt-4o-mini-transcribe".into()
}
fn default_language() -> String {
    "en".into()
}
fn default_vad_mode() -> String {
    "strict".into()
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
    "green".into()
}
fn default_chrome_path() -> String {
    r"C:\Program Files\Google\Chrome\Application\chrome.exe".into()
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
        UrlCommand { trigger: "github".into(), url: "https://github.com".into(), builtin: true },
        UrlCommand { trigger: "youtube".into(), url: "https://youtube.com".into(), builtin: true },
        UrlCommand { trigger: "explorer".into(), url: default_explorer_path(), builtin: true },
    ]
}
fn default_alias_commands() -> Vec<AliasCommand> {
    vec![AliasCommand {
        trigger: "codex".into(),
        replacement: "codex app --dangerously-bypass-approvals-and-sandbox".into(),
    }]
}

pub fn settings_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("Jarvis").join("settings.json"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".jarvis").join("settings.json"));
    }
    Err("Failed to resolve data directory".into())
}

pub fn load() -> Settings {
    let path = match settings_path() {
        Ok(p) => p,
        Err(_) => return Settings::default(),
    };
    let mut settings: Settings = match fs::read_to_string(&path) {
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
        Err(e) => eprintln!("[settings] secure key load failed: {}", e),
    }

    if had_plaintext_keys {
        match crate::secrets::save_api_keys(&resolved_api_keys) {
            Ok(()) => {
                settings.api_keys.clear();
                settings.api_key.clear();
                let _ = save_settings_without_api_keys(&settings);
            }
            Err(e) => eprintln!("[settings] secure key migration failed: {}", e),
        }
    }
    settings.api_keys = resolved_api_keys;

    // Migrate deprecated provider id.
    if settings.provider == "deepgram-flux" {
        settings.provider = "deepgram".into();
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
    settings.provider_inactivity_timeout_secs =
        settings.provider_inactivity_timeout_secs.clamp(5, 300);
    settings.max_session_length_minutes = settings.max_session_length_minutes.clamp(1, 120);
    settings
}

pub fn save(settings: &Settings) -> Result<(), String> {
    crate::secrets::save_api_keys(&settings.api_keys)?;
    save_settings_without_api_keys(settings)
}

fn save_settings_without_api_keys(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings dir: {}", e))?;
    }
    let mut clean = settings.clone();
    clean.api_keys.clear();
    clean.api_key.clear();
    let json = serde_json::to_string_pretty(&clean)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}
