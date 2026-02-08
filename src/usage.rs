use crate::state::UsageTotals;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

pub const USAGE_SAVE_INTERVAL_SECS: u64 = 60;

/// Max lines to keep in usage-session.jsonl (one line per session).
const MAX_SESSION_LOG_LINES: usize = 500;
/// Max lines to keep in usage.jsonl (periodic all-time snapshots).
const MAX_TOTALS_LOG_LINES: usize = 100;

pub fn usage_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("Jarvis").join("usage.jsonl"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".jarvis").join("usage.jsonl"));
    }
    Err("Failed to resolve data directory for usage logs".into())
}

pub fn session_usage_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("Jarvis").join("usage-session.jsonl"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".jarvis").join("usage-session.jsonl"));
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

