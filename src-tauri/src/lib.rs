mod commands;
mod hotkey;
mod openai;
mod state;
mod typing;

use state::AppState;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
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
        ])
        .setup(|app| {
            let handle = app.handle().clone();

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
                        let state = tray.app_handle().state::<Arc<AppState>>();
                        let mut armed = state.armed.lock().unwrap();
                        *armed = !*armed;
                        let is_armed = *armed;
                        drop(armed);

                        if let Some(tray) = tray.app_handle().tray_by_id("main") {
                            let icon = Image::new(&ICON_RGBA, 1, 1);
                            let _ = tray.set_icon(Some(icon));
                            let tooltip = if is_armed {
                                "Diction - Armed"
                            } else {
                                "Diction - Disarmed"
                            };
                            let _ = tray.set_tooltip(Some(tooltip));
                        }
                    }
                })
                .build(app)?;

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

            // Close to tray instead of quitting
            if let Some(window) = app.get_webview_window("main") {
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
