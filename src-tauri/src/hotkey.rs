use crate::state::AppState;
use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

static LISTENER_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn start_listener(app: AppHandle) {
    if LISTENER_ACTIVE.load(Ordering::SeqCst) {
        return;
    }

    LISTENER_ACTIVE.store(true, Ordering::SeqCst);

    let app_clone = app.clone();
    std::thread::spawn(move || {
        let recording = Arc::new(AtomicBool::new(false));
        let recording_clone = recording.clone();
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

                    // Toggle recording on each press
                    let was_recording = recording_clone.load(Ordering::SeqCst);
                    if was_recording {
                        recording_clone.store(false, Ordering::SeqCst);
                        println!("[hotkey] Right Ctrl → stop recording");
                        let _ = app_inner.emit("hotkey-release", ());
                    } else {
                        recording_clone.store(true, Ordering::SeqCst);
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
                    if state.snip_active.swap(true, Ordering::SeqCst) {
                        println!("[hotkey] AltGr pressed but snip already active, ignoring");
                        return;
                    }
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
