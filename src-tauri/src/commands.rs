use crate::openai;
use crate::snip;
use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use tokio::sync::mpsc;

#[tauri::command]
pub async fn send_audio(state: State<'_, Arc<AppState>>, data: Vec<u8>) -> Result<(), String> {
    let sender = {
        let tx = state.audio_tx.lock().map_err(|e| e.to_string())?;
        tx.clone()
    };
    if let Some(sender) = sender {
        sender
            .send(data)
            .await
            .map_err(|e| format!("Failed to send audio: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn commit_audio(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let sender = {
        let tx = state.audio_tx.lock().map_err(|e| e.to_string())?;
        tx.clone()
    };
    if let Some(sender) = sender {
        // Empty Vec<u8> is used as a commit signal.
        sender
            .send(Vec::new())
            .await
            .map_err(|e| format!("Failed to send commit: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Check if already active
    {
        let active = state.session_active.lock().map_err(|e| e.to_string())?;
        if *active {
            return Ok(());
        }
    }

    // Read settings from store
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let api_key = store
        .get("api_key")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    if api_key.is_empty() {
        return Err("API key not configured. Open Settings to add your OpenAI API key.".into());
    }

    let model = store
        .get("model")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "gpt-4o-realtime-preview".into());

    let transcription_model = store
        .get("transcription_model")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "gpt-4o-mini-transcribe".into());

    let language = store
        .get("language")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "en".into());

    // Create audio channel
    let (tx, rx) = mpsc::channel::<Vec<u8>>(256);
    {
        let mut audio_tx = state.audio_tx.lock().map_err(|e| e.to_string())?;
        *audio_tx = Some(tx);
    }
    {
        let mut active = state.session_active.lock().map_err(|e| e.to_string())?;
        *active = true;
    }

    // Bump generation so any prior session's cleanup becomes a no-op.
    let gen = state.session_gen.fetch_add(1, Ordering::SeqCst) + 1;

    // Spawn WebSocket task
    let app_clone = app.clone();
    let state_clone = app.state::<Arc<AppState>>().inner().clone();
    tokio::spawn(async move {
        openai::run_session(app_clone, api_key, model, transcription_model, language, rx).await;
        // Only clean up if no newer session has started since ours.
        if state_clone.session_gen.load(Ordering::SeqCst) == gen {
            let mut active = state_clone.session_active.lock().unwrap();
            *active = false;
            let mut audio_tx = state_clone.audio_tx.lock().unwrap();
            *audio_tx = None;
            state_clone
                .hotkey_recording
                .store(false, Ordering::SeqCst);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_session(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // Drop the sender to close the channel, which signals the WebSocket task to stop
    let mut audio_tx = state.audio_tx.lock().map_err(|e| e.to_string())?;
    *audio_tx = None;

    let mut active = state.session_active.lock().map_err(|e| e.to_string())?;
    *active = false;

    // Reset hotkey recording state so next press starts fresh.
    state.hotkey_recording.store(false, Ordering::SeqCst);

    Ok(())
}

#[tauri::command]
pub async fn get_setting(app: AppHandle, key: String) -> Result<serde_json::Value, String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    Ok(store.get(&key).unwrap_or(serde_json::Value::Null))
}

#[tauri::command]
pub async fn set_setting(
    app: AppHandle,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;

    store.set(&key, value);
    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn finish_snip(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let img = {
        let mut guard = state.snip_image.lock().map_err(|e| e.to_string())?;
        guard.take()
    };

    let Some(img) = img else {
        state.snip_active.store(false, Ordering::SeqCst);
        snip::hide_overlay(&app);
        return Err("No snip image available".into());
    };

    let task = tokio::task::spawn_blocking(move || {
        let path = snip::crop_and_save(&img, x, y, width, height)?;
        snip::copy_path_to_clipboard(&path)?;
        Ok(path)
    });

    let result = match task.await {
        Ok(Ok(path)) => {
            let path_str = path.to_string_lossy().to_string();
            let _ = app.emit("snip-complete", path_str.clone());
            log::info!("[snip] saved to {}", path_str);
            Ok(())
        }
        Ok(Err(err)) => Err(err),
        Err(err) => Err(format!("Snip task failed: {}", err)),
    };

    state.snip_active.store(false, Ordering::SeqCst);
    snip::hide_overlay(&app);
    result
}

#[tauri::command]
pub async fn open_snip_folder() -> Result<(), String> {
    let dir = snip::snip_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {}", e))?;
    std::process::Command::new("explorer")
        .arg(dir.as_os_str())
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_snip(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if let Ok(mut guard) = state.snip_image.lock() {
        *guard = None;
    }
    state.snip_active.store(false, Ordering::SeqCst);
    snip::hide_overlay(&app);
    log::info!("[snip] cancelled");
    Ok(())
}
