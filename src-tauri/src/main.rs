#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod hotkey;
mod openai;
mod settings;
mod snip;
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
#[allow(unused_imports)]
use usage::{
    append_usage_line, load_usage, save_usage, session_usage_path, usage_path,
    USAGE_SAVE_INTERVAL_SECS,
};

fn main() {
    env_logger::init();

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

    // Auto-arm and start hotkey listener
    app_state.armed.store(true, Ordering::SeqCst);
    hotkey::start_listener(app_state.clone(), event_tx.clone());
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
            if let Ok(session_path) = session_usage_path() {
                if let Ok(session) = usage_state.session_usage.lock() {
                    if session.started_ms != 0 {
                        let snapshot = session.clone();
                        let _ = append_usage_line(&session_path, &snapshot);
                    }
                }
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
            .with_inner_size(vec2(280.0, 80.0))
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false),
        ..Default::default()
    };

    println!("[jarvis] starting eframe...");

    eframe::run_native(
        "Jarvis",
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            println!("[jarvis] eframe app created");
            Ok(Box::new(ui::JarvisApp::new(
                app_state,
                event_tx,
                event_rx,
                runtime,
                settings,
            )))
        }),
    )
    .expect("Failed to start eframe");
}
