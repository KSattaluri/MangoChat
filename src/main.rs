#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod diagnostics;
mod audio;
mod hotkey;
mod headset;
mod provider;
mod settings;
mod secrets;
mod single_instance;
mod snip;
mod start_cue;
mod state;
mod typing;
mod ui;
mod updater;
mod usage;

use eframe::egui;
use egui::{vec2, ViewportBuilder};
use state::{AppEvent, AppState};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use usage::{load_usage, save_usage, usage_path, USAGE_SAVE_INTERVAL_SECS, load_provider_totals, save_provider_totals};

fn main() {
    let _ = diagnostics::init_session_logging();
    diagnostics::install_panic_hook();
    env_logger::init();

    let _single_instance_guard = match single_instance::acquire("MangoChat.App.Singleton") {
        Some(g) => g,
        None => {
            app_err!("[mangochat] another instance is already running; exiting");
            return;
        }
    };
    let app_state = Arc::new(AppState::new());
    let settings = settings::load();
    let (event_tx, event_rx) = std::sync::mpsc::channel::<AppEvent>();
    let runtime = Arc::new(
        tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"),
    );

    // Load usage totals from disk
    if let Ok(path) = usage_path() {
        let usage = load_usage(&path);
        if let Ok(mut guard) = app_state.usage.lock() {
            *guard = usage;
        }
    }
    // Load per-provider totals from disk
    {
        let pt = load_provider_totals();
        if let Ok(mut guard) = app_state.provider_totals.lock() {
            *guard = pt;
        }
    }

    // Populate dynamic config from settings
    if let Ok(mut p) = app_state.chrome_path.lock() {
        *p = settings.resolved_browser_path();
    }
    if let Ok(mut p) = app_state.paint_path.lock() {
        *p = settings.paint_path.clone();
    }
    if let Ok(mut v) = app_state.url_commands.lock() {
        *v = settings
            .url_commands
            .iter()
            .map(|c| (c.trigger.clone(), c.url.clone()))
            .collect();
    }
    if let Ok(mut v) = app_state.alias_commands.lock() {
        *v = settings
            .alias_commands
            .iter()
            .map(|c| (c.trigger.clone(), c.replacement.clone()))
            .collect();
    }
    if let Ok(mut v) = app_state.app_shortcuts.lock() {
        *v = settings
            .app_shortcuts
            .iter()
            .map(|c| (c.trigger.clone(), c.path.clone()))
            .collect();
    }

    // Populate feature gates from settings
    app_state
        .session_hotkey_enabled
        .store(settings.session_hotkey_enabled, Ordering::SeqCst);
    app_state
        .screenshot_enabled
        .store(settings.screenshot_enabled, Ordering::SeqCst);
    app_state
        .screenshot_hotkey_enabled
        .store(settings.screenshot_hotkey_enabled, Ordering::SeqCst);
    if let Ok(mut usage) = app_state.usage.lock() {
        if usage.provider.is_empty() {
            usage.provider = settings.provider.clone();
        }
        if usage.model.is_empty() {
            usage.model = settings.model.clone();
        }
    }

    // Start hotkey listener
    hotkey::start_listener(app_state.clone(), event_tx.clone());
    // Windows-only test hook for headset mic stem mute/unmute.
    headset::start_mute_watcher(event_tx.clone());
    app_log!("[mangochat] hotkeys active, hold Right Ctrl to dictate");

    // Periodic usage logging thread
    {
        let usage_state = app_state.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(USAGE_SAVE_INTERVAL_SECS));
            let snapshot = match usage_state.usage.lock() {
                Ok(v) => v.clone(),
                Err(_) => continue,
            };
            if let Ok(path) = usage_path() {
                let _ = save_usage(&path, &snapshot);
            }
            if let Ok(pt) = usage_state.provider_totals.lock() {
                let _ = save_provider_totals(&pt);
            }
            let hours_sent = snapshot.ms_sent as f64 / 3_600_000.0;
            let hours_suppressed = snapshot.ms_suppressed as f64 / 3_600_000.0;
            let mb_sent = snapshot.bytes_sent as f64 / (1024.0 * 1024.0);
            app_log!(
                "[usage] provider={} model={} sent={:.2}h suppressed={:.2}h bytes={:.1}MB commits={}",
                if snapshot.provider.is_empty() { "-" } else { snapshot.provider.as_str() },
                if snapshot.model.is_empty() { "-" } else { snapshot.model.as_str() },
                hours_sent, hours_suppressed, mb_sent, snapshot.commits
            );
        });
    }

    // Load mango icon for the window/taskbar
    let window_icon = {
        const MANGO_PNG: &[u8] = include_bytes!("../icons/mango.png");
        image::load_from_memory(MANGO_PNG).ok().map(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            Arc::new(egui::IconData {
                rgba: rgba.into_raw(),
                width: w,
                height: h,
            })
        })
    };

    let mut vp = ViewportBuilder::default()
        .with_title("Mango Chat")
        .with_inner_size(vec2(
            if settings.screenshot_enabled { 360.0 } else { 210.0 },
            if settings.compact_background_enabled { 92.0 } else { 80.0 },
        ))
        .with_taskbar(false)
        .with_transparent(true)
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(true);

    if let Some(icon) = window_icon {
        vp = vp.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport: vp,
        ..Default::default()
    };

    app_log!("[mangochat] starting eframe...");

    eframe::run_native(
        "Mango Chat",
        native_options,
        Box::new(move |cc| {
            if settings.theme == "light" {
                cc.egui_ctx.set_visuals(egui::Visuals::light());
            } else {
                cc.egui_ctx.set_visuals(egui::Visuals::dark());
            }
            app_log!("[mangochat] eframe app created");
            Ok(Box::new(ui::MangoChatApp::new(
                app_state,
                event_tx,
                event_rx,
                runtime,
                settings,
                cc.egui_ctx.clone(),
            )))
        }),
    )
    .expect("Failed to start eframe");
}




