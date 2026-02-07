use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub api_key: String,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_model(),
            transcription_model: default_transcription_model(),
            language: default_language(),
            mic_device: String::new(),
            vad_mode: default_vad_mode(),
        }
    }
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
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
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
