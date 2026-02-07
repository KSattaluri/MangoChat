mod commands;
mod hotkey;
mod openai;
mod snip;
mod state;
mod typing;

use state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Listener, Manager, WindowEvent,
};

// Minimal 1x1 transparent RGBA icon to avoid image decoding features.
const ICON_RGBA: [u8; 4] = [0, 0, 0, 0];

pub fn run() {
    let app_state = Arc::new(AppState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::send_audio,
            commands::commit_audio,
            commands::start_session,
            commands::stop_session,
            commands::get_setting,
            commands::set_setting,
            commands::finish_snip,
            commands::cancel_snip,
            commands::open_snip_folder,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let snip_state = handle.state::<Arc<AppState>>().inner().clone();

            // Build tray menu
            let toggle_armed =
                MenuItemBuilder::with_id("toggle_armed", "Arm Dictation").build(app)?;
            let toggle_armed_setup = toggle_armed.clone();
            let copy_last =
                MenuItemBuilder::with_id("copy_last", "Copy Last Transcript").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&toggle_armed)
                .separator()
                .item(&copy_last)
                .separator()
                .item(&quit)
                .build()?;

            // Build tray icon
            let icon = Image::new(&ICON_RGBA, 1, 1);
            let _tray = TrayIconBuilder::with_id("main")
                .icon(icon)
                .tooltip("Diction - Disarmed")
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    let id = event.id().as_ref();
                    match id {
                        "toggle_armed" => {
                            let state = app.state::<Arc<AppState>>();
                            let mut armed = state.armed.lock().unwrap();
                            *armed = !*armed;
                            let is_armed = *armed;
                            drop(armed);

                            // Update tray icon and tooltip
                            if let Some(tray) = app.tray_by_id("main") {
                                let icon = Image::new(&ICON_RGBA, 1, 1);
                                let _ = tray.set_icon(Some(icon));
                                let tooltip = if is_armed {
                                    "Diction - Armed"
                                } else {
                                    "Diction - Disarmed"
                                };
                                let _ = tray.set_tooltip(Some(tooltip));
                            }

                            // Update menu item text
                            let text = if is_armed {
                                "Disarm Dictation"
                            } else {
                                "Arm Dictation"
                            };
                            let _ = toggle_armed.set_text(text);

                            // Start hotkey listener once; it gates on armed state
                            if is_armed {
                                hotkey::start_listener(app.clone());
                            }
                        }
                        "copy_last" => {
                            let state = app.state::<Arc<AppState>>();
                            let last = state.last_transcript.lock().unwrap().clone();
                            if !last.is_empty() {
                                typing::copy_to_clipboard(&last);
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        if let Some(win) = tray.app_handle().get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Pre-create snip overlay (hidden) — avoids WebView2 init cost on each trigger
            if let Err(e) = snip::init_overlay(&handle) {
                log::error!("[snip] failed to pre-create overlay: {}", e);
            }

            // Auto-arm and start hotkey listener
            {
                let state = handle.state::<Arc<AppState>>();
                *state.armed.lock().unwrap() = true;
                if let Some(tray) = handle.tray_by_id("main") {
                    let _ = tray.set_tooltip(Some("Diction - Armed"));
                }
                let _ = toggle_armed_setup.set_text("Disarm Dictation");
                hotkey::start_listener(handle.clone());
                println!("[diction] auto-armed, hold Right Ctrl to dictate");
            }

            // Snip trigger listener
            {
                let snip_handle = handle.clone();
                app.listen("snip-trigger", move |_| {
                    let app_handle = snip_handle.clone();
                    let state = snip_state.clone();
                    let cursor = state.cursor_pos.lock().ok().and_then(|v| *v);
                    std::thread::spawn(move || match snip::capture_screen(cursor) {
                        Ok((img, b64, bounds)) => {
                            if let Ok(mut guard) = state.snip_image.lock() {
                                *guard = Some(img);
                            }
                            if let Err(e) = snip::show_overlay(&app_handle, &b64, &bounds) {
                                log::error!("[snip] overlay error: {}", e);
                                state.snip_active.store(false, Ordering::SeqCst);
                                if let Ok(mut guard) = state.snip_image.lock() {
                                    *guard = None;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("[snip] capture error: {}", e);
                            state.snip_active.store(false, Ordering::SeqCst);
                        }
                    });
                });
            }

            // Close to tray instead of quitting + position bottom-right
            if let Some(window) = app.get_webview_window("main") {
                // Anchor above the taskbar, bottom-right
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let scale = monitor.scale_factor();
                    let screen_w = monitor.size().width as f64 / scale;
                    let screen_h = monitor.size().height as f64 / scale;
                    let win_w = 260.0;
                    let win_h = 72.0;
                    let taskbar_h = 48.0;
                    let margin = 12.0;
                    let x = screen_w - win_w - margin;
                    let y = screen_h - win_h - taskbar_h - margin;
                    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
                }

                let handle_clone = handle.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(win) = handle_clone.get_webview_window("main") {
                            let _ = win.hide();
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
