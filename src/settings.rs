use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Per-provider API keys: {"openai": "sk-...", "deepgram": "dg-...", ...}
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    /// Legacy single key — migrated to api_keys on load, not saved.
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
    pub snip_editor_path: String,
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
            snip_editor_path: String::new(),
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
    // Migrate legacy single api_key → per-provider map.
    if !settings.api_key.is_empty() && !settings.api_keys.contains_key("openai") {
        settings
            .api_keys
            .insert("openai".into(), settings.api_key.clone());
        settings.api_key.clear();
    }
    // Migrate deprecated provider id.
    if settings.provider == "deepgram-flux" {
        settings.provider = "deepgram".into();
    }
    settings
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}
