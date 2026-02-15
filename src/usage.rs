use crate::state::{ProviderUsage, SessionUsage, UsageTotals};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub const USAGE_SAVE_INTERVAL_SECS: u64 = 60;

/// Max lines to keep in usage-session.jsonl (one line per session).
const MAX_SESSION_LOG_LINES: usize = 500;
/// Max lines to keep in usage.jsonl (periodic all-time snapshots).
const MAX_TOTALS_LOG_LINES: usize = 100;

pub fn usage_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("MangoChat").join("usage.jsonl"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".mangochat").join("usage.jsonl"));
    }
    Err("Failed to resolve data directory for usage logs".into())
}

pub fn session_usage_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("MangoChat").join("usage-session.jsonl"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".mangochat").join("usage-session.jsonl"));
    }
    Err("Failed to resolve data directory for usage logs".into())
}

pub fn load_usage(path: &PathBuf) -> UsageTotals {
    if let Ok(text) = fs::read_to_string(path) {
        if let Some(line) = text.lines().rev().find(|l| !l.trim().is_empty()) {
            if let Ok(v) = serde_json::from_str::<UsageTotals>(line) {
                return v;
            }
        }
    }
    UsageTotals::default()
}

pub fn save_usage(path: &PathBuf, usage: &UsageTotals) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create usage dir: {}", e))?;
    }
    let line = serde_json::to_string(usage)
        .map_err(|e| format!("Failed to serialize usage: {}", e))?;
    let mut text = line;
    text.push('\n');
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()))
        .map_err(|e| format!("Failed to append usage log: {}", e))?;
    truncate_log(path, MAX_TOTALS_LOG_LINES);
    Ok(())
}

pub fn append_usage_line<T: Serialize>(path: &PathBuf, usage: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create usage dir: {}", e))?;
    }
    let line = serde_json::to_string(usage).map_err(|e| format!("Failed to serialize usage: {}", e))?;
    let mut text = line;
    text.push('\n');
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()))
        .map_err(|e| format!("Failed to append usage log: {}", e))?;
    truncate_log(path, MAX_SESSION_LOG_LINES);
    Ok(())
}

/// If `path` has more than `max_lines` lines, rewrite it keeping only the last `max_lines`.
fn truncate_log(path: &PathBuf, max_lines: usize) {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return,
    };
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return;
    }
    let keep = &lines[lines.len() - max_lines..];
    let mut out = keep.join("\n");
    out.push('\n');
    let _ = fs::write(path, out.as_bytes());
}

/// Load the most recent `max` session entries from usage-session.jsonl (newest first).
pub fn load_recent_sessions(max: usize) -> Vec<SessionUsage> {
    let path = match session_usage_path() {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    text.lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .take(max)
        .collect()
}

/// Return the Mango Chat data directory path.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = dirs::data_local_dir() {
        return Some(dir.join("MangoChat"));
    }
    if let Some(home) = dirs::home_dir() {
        return Some(home.join(".mangochat"));
    }
    None
}

/// Delete the all-time totals log file.
pub fn reset_totals_file() -> Result<(), String> {
    let path = usage_path()?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to reset totals: {}", e))?;
    }
    Ok(())
}

pub fn provider_totals_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("MangoChat").join("usage-provider.json"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".mangochat").join("usage-provider.json"));
    }
    Err("Failed to resolve data directory for provider totals".into())
}

pub fn load_provider_totals() -> HashMap<String, ProviderUsage> {
    let path = match provider_totals_path() {
        Ok(p) => p,
        Err(_) => return HashMap::new(),
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return HashMap::new(),
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_provider_totals(totals: &HashMap<String, ProviderUsage>) -> Result<(), String> {
    let path = provider_totals_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create provider totals dir: {}", e))?;
    }
    let json = serde_json::to_string(totals)
        .map_err(|e| format!("Failed to serialize provider totals: {}", e))?;
    fs::write(&path, json.as_bytes())
        .map_err(|e| format!("Failed to write provider totals: {}", e))
}

pub fn reset_provider_totals_file() -> Result<(), String> {
    let path = provider_totals_path()?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to reset provider totals: {}", e))?;
    }
    Ok(())
}

pub fn reset_session_file() -> Result<(), String> {
    let path = session_usage_path()?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to reset session usage: {}", e))?;
    }
    Ok(())
}

