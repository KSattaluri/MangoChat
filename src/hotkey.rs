use crate::state::{AppEvent, AppState};
use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender as EventSender;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const SNIP_TIMEOUT_MS: u64 = 30_000;

static LISTENER_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn start_listener(state: Arc<AppState>, event_tx: EventSender<AppEvent>) {
    if LISTENER_ACTIVE.load(Ordering::SeqCst) {
        return;
    }

    LISTENER_ACTIVE.store(true, Ordering::SeqCst);

    std::thread::spawn(move || {
        let key_held = Arc::new(AtomicBool::new(false));
        let key_held_clone = key_held.clone();
        let snip_key_held = Arc::new(AtomicBool::new(false));
        let snip_key_held_clone = snip_key_held.clone();

        let callback = move |event: Event| {
            let trigger_snip = |state: &Arc<AppState>, event_tx: &EventSender<AppEvent>| {
                if !state.armed.load(Ordering::SeqCst) {
                    println!("[hotkey] Alt pressed but not armed, ignoring");
                    return;
                }
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                if state.snip_active.swap(true, Ordering::SeqCst) {
                    let since = state.snip_started_ms.load(Ordering::SeqCst);
                    if now_ms.saturating_sub(since) < SNIP_TIMEOUT_MS {
                        println!("[hotkey] Alt pressed but snip already active, ignoring");
                        return;
                    }
                    println!(
                        "[hotkey] snip_active was stale ({}s), resetting",
                        (now_ms - since) / 1000
                    );
                    if let Ok(mut img) = state.snip_image.lock() {
                        *img = None;
                    }
                }
                state.snip_started_ms.store(now_ms, Ordering::SeqCst);
                println!("[hotkey] Right Alt -> snip");
                let _ = event_tx.send(AppEvent::SnipTrigger);
            };

            match event.event_type {
                EventType::KeyPress(Key::ControlRight) => {
                    if !state.armed.load(Ordering::SeqCst) {
                        return;
                    }
                    if key_held_clone.load(Ordering::SeqCst) {
                        return;
                    }
                    key_held_clone.store(true, Ordering::SeqCst);

                    let was_recording = state.hotkey_recording.load(Ordering::SeqCst);
                    if was_recording {
                        state.hotkey_recording.store(false, Ordering::SeqCst);
                        println!("[hotkey] Right Ctrl -> stop recording");
                        let _ = event_tx.send(AppEvent::HotkeyRelease);
                    } else {
                        state.hotkey_recording.store(true, Ordering::SeqCst);
                        println!("[hotkey] Right Ctrl -> start recording");
                        let _ = event_tx.send(AppEvent::HotkeyPush);
                    }
                }
                EventType::KeyRelease(Key::ControlRight) => {
                    key_held_clone.store(false, Ordering::SeqCst);
                }
                EventType::KeyPress(Key::AltGr) | EventType::KeyPress(Key::Alt) => {
                    if snip_key_held_clone.load(Ordering::SeqCst) {
                        return;
                    }
                    snip_key_held_clone.store(true, Ordering::SeqCst);
                    trigger_snip(&state, &event_tx);
                }
                EventType::KeyRelease(Key::AltGr) | EventType::KeyRelease(Key::Alt) => {
                    snip_key_held_clone.store(false, Ordering::SeqCst);
                }
                EventType::MouseMove { x, y } => {
                    if let Ok(mut pos) = state.cursor_pos.lock() {
                        *pos = Some((x as i32, y as i32));
                    }
                }
                _ => {}
            }
        };

        if let Err(e) = listen(callback) {
            eprintln!("rdev listener error: {:?}", e);
        }

        LISTENER_ACTIVE.store(false, Ordering::SeqCst);
    });
}
