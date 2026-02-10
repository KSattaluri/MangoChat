#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod hotkey;
mod headset;
mod provider;
mod settings;
mod single_instance;
mod snip;
mod start_cue;
mod state;
mod typing;
mod ui;
mod usage;

use eframe::egui;
use egui::{vec2, ViewportBuilder};
use state::{AppEvent, AppState};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use usage::{load_usage, save_usage, usage_path, USAGE_SAVE_INTERVAL_SECS};

fn main() {
    env_logger::init();

    let _single_instance_guard = match single_instance::acquire("Jarvis.App.Singleton") {
        Some(g) => g,
        None => {
            eprintln!("[jarvis] another instance is already running; exiting");
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

    // Populate dynamic config from settings
    if let Ok(mut p) = app_state.chrome_path.lock() {
        *p = settings.chrome_path.clone();
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

    // Auto-arm and start hotkey listener
    app_state.armed.store(true, Ordering::SeqCst);
    hotkey::start_listener(app_state.clone(), event_tx.clone());
    // Windows-only test hook for headset mic stem mute/unmute.
    headset::start_mute_watcher(app_state.clone(), event_tx.clone());
    println!("[jarvis] auto-armed, hold Right Ctrl to dictate");

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
            let hours_sent = snapshot.ms_sent as f64 / 3_600_000.0;
            let hours_suppressed = snapshot.ms_suppressed as f64 / 3_600_000.0;
            let mb_sent = snapshot.bytes_sent as f64 / (1024.0 * 1024.0);
            println!(
                "[usage] sent={:.2}h suppressed={:.2}h bytes={:.1}MB commits={}",
                hours_sent, hours_suppressed, mb_sent, snapshot.commits
            );
        });
    }

    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Jarvis")
            .with_inner_size(vec2(360.0, 80.0))
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(true),
        ..Default::default()
    };

    println!("[jarvis] starting eframe...");

    eframe::run_native(
        "Jarvis",
        native_options,
        Box::new(move |cc| {
            if settings.theme == "light" {
                cc.egui_ctx.set_visuals(egui::Visuals::light());
            } else {
                cc.egui_ctx.set_visuals(egui::Visuals::dark());
            }
            println!("[jarvis] eframe app created");
            Ok(Box::new(ui::JarvisApp::new(
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



