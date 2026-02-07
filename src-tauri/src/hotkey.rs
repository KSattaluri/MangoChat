use crate::state::AppState;
use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const SNIP_TIMEOUT_MS: u64 = 30_000;

static LISTENER_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn start_listener(app: AppHandle) {
    if LISTENER_ACTIVE.load(Ordering::SeqCst) {
        return;
    }

    LISTENER_ACTIVE.store(true, Ordering::SeqCst);

    let app_clone = app.clone();
    std::thread::spawn(move || {
        let key_held = Arc::new(AtomicBool::new(false));
        let key_held_clone = key_held.clone();
        let app_inner = app_clone.clone();

        let callback = move |event: Event| {
            let state = app_inner.state::<Arc<AppState>>();
            match event.event_type {
                EventType::KeyPress(Key::ControlRight) => {
                    let armed = state.armed.lock().map(|v| *v).unwrap_or(false);
                    if !armed {
                        return;
                    }
                    // Ignore auto-repeat (key already held)
                    if key_held_clone.load(Ordering::SeqCst) {
                        return;
                    }
                    key_held_clone.store(true, Ordering::SeqCst);

                    // Toggle recording — uses shared state so stop_session can reset it.
                    let was_recording = state.hotkey_recording.load(Ordering::SeqCst);
                    if was_recording {
                        state.hotkey_recording.store(false, Ordering::SeqCst);
                        println!("[hotkey] Right Ctrl → stop recording");
                        let _ = app_inner.emit("hotkey-release", ());
                    } else {
                        state.hotkey_recording.store(true, Ordering::SeqCst);
                        println!("[hotkey] Right Ctrl → start recording");
                        let _ = app_inner.emit("hotkey-push", ());
                    }
                }
                EventType::KeyRelease(Key::ControlRight) => {
                    key_held_clone.store(false, Ordering::SeqCst);
                }
                EventType::KeyPress(Key::AltGr) => {
                    let armed = state.armed.lock().map(|v| *v).unwrap_or(false);
                    if !armed {
                        println!("[hotkey] AltGr pressed but not armed, ignoring");
                        return;
                    }
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    if state.snip_active.swap(true, Ordering::SeqCst) {
                        // Already active — check if stale (>30s).
                        let since = state.snip_started_ms.load(Ordering::SeqCst);
                        if now_ms.saturating_sub(since) < SNIP_TIMEOUT_MS {
                            println!("[hotkey] AltGr pressed but snip already active, ignoring");
                            return;
                        }
                        println!("[hotkey] snip_active was stale ({}s), resetting", (now_ms - since) / 1000);
                        // Clean up stale snip image.
                        if let Ok(mut img) = state.snip_image.lock() {
                            *img = None;
                        }
                    }
                    state.snip_started_ms.store(now_ms, Ordering::SeqCst);
                    println!("[hotkey] Right Alt → snip");
                    let _ = app_inner.emit("snip-trigger", ());
                }
                EventType::MouseMove { x, y } => {
                    if let Ok(mut pos) = state.cursor_pos.lock() {
                        *pos = Some((x as i32, y as i32));
                    }
                }
                _ => {}
            }
        };

        // rdev::listen blocks the thread until an error occurs
        if let Err(e) = listen(callback) {
            log::error!("rdev listener error: {:?}", e);
        }

        LISTENER_ACTIVE.store(false, Ordering::SeqCst);
    });
}
