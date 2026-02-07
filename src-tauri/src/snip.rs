use crate::state::AppState;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Local;
use image::codecs::jpeg::JpegEncoder;
use image::{imageops, DynamicImage, ImageEncoder, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};

/// Monitor bounds in physical pixels.
pub struct MonitorBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn capture_screen(
    cursor: Option<(i32, i32)>,
) -> Result<(RgbaImage, String, MonitorBounds), String> {
    let monitors = xcap::Monitor::all().map_err(|e| format!("xcap monitors error: {:?}", e))?;
    let mut cursor_monitor = None;
    if let Some((cx, cy)) = cursor {
        for monitor in monitors.iter() {
            let mx = match monitor.x() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let my = match monitor.y() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let mw = match monitor.width() {
                Ok(v) => v as i32,
                Err(_) => continue,
            };
            let mh = match monitor.height() {
                Ok(v) => v as i32,
                Err(_) => continue,
            };
            if cx >= mx && cx < mx + mw && cy >= my && cy < my + mh {
                cursor_monitor = Some(monitor);
                break;
            }
        }
    }

    let monitor = cursor_monitor
        .or_else(|| monitors.iter().find(|m| m.is_primary().unwrap_or(false)))
        .or_else(|| monitors.first())
        .ok_or("No monitors found")?;

    let bounds = MonitorBounds {
        x: monitor.x().unwrap_or(0),
        y: monitor.y().unwrap_or(0),
        width: monitor.width().unwrap_or(1920),
        height: monitor.height().unwrap_or(1080),
    };

    let image = monitor
        .capture_image()
        .map_err(|e| format!("xcap capture error: {:?}", e))?;

    // JPEG is ~50x faster to encode than PNG — fine for a preview overlay.
    let (w, h) = image.dimensions();
    let rgb_data: Vec<u8> = image
        .as_raw()
        .chunks_exact(4)
        .flat_map(|px| &px[..3])
        .copied()
        .collect();
    let mut jpeg_bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg_bytes, 80)
        .write_image(&rgb_data, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("JPEG encode error: {}", e))?;
    let b64 = STANDARD.encode(&jpeg_bytes);

    Ok((image, b64, bounds))
}

/// Create the overlay window once at startup (hidden).
/// WebView2 initialization is expensive (~1-2s), so we pay that cost once.
pub fn init_overlay(app: &AppHandle) -> Result<(), String> {
    let window = WebviewWindowBuilder::new(
        app,
        "snip-overlay",
        WebviewUrl::App("snip.html".into()),
    )
    .title("Snip")
    .decorations(false)
    .resizable(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()
    .map_err(|e| format!("Failed to create overlay: {}", e))?;

    let app_handle = app.clone();
    let overlay_state = app.state::<Arc<AppState>>().inner().clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Some(win) = app_handle.get_webview_window("snip-overlay") {
                let _ = win.hide();
            }
            overlay_state.snip_active.store(false, Ordering::SeqCst);
            if let Ok(mut img) = overlay_state.snip_image.lock() {
                *img = None;
            }
        }
    });

    Ok(())
}

/// Send screenshot data, position on the correct monitor, and show.
pub fn show_overlay(
    app: &AppHandle,
    b64_jpeg: &str,
    bounds: &MonitorBounds,
) -> Result<(), String> {
    let window = app
        .get_webview_window("snip-overlay")
        .ok_or("Overlay window not found — init_overlay may have failed")?;
    // Position the overlay to cover the captured monitor exactly.
    let _ = window.set_position(PhysicalPosition::new(bounds.x, bounds.y));
    let _ = window.set_size(PhysicalSize::new(bounds.width, bounds.height));
    window
        .emit("snip-screenshot", b64_jpeg)
        .map_err(|e| format!("Failed to emit screenshot: {}", e))?;
    window
        .show()
        .map_err(|e| format!("Failed to show overlay: {}", e))?;
    window
        .set_focus()
        .map_err(|e| format!("Failed to focus overlay: {}", e))?;
    Ok(())
}

/// Hide the overlay without destroying it.
pub fn hide_overlay(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("snip-overlay") {
        let _ = win.emit("snip-reset", ());
        let _ = win.hide();
    }
}

pub fn crop_and_save(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> Result<PathBuf, String> {
    let max_w = img.width();
    let max_h = img.height();
    if max_w == 0 || max_h == 0 {
        return Err("Captured image is empty".into());
    }

    let x = x.min(max_w.saturating_sub(1));
    let y = y.min(max_h.saturating_sub(1));
    let w = w.min(max_w.saturating_sub(x)).max(1);
    let h = h.min(max_h.saturating_sub(y)).max(1);

    let cropped = imageops::crop_imm(img, x, y, w, h).to_image();

    let dir = snip_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create snip dir: {}", e))?;

    let now = Local::now();
    let base = now.format("snip-%Y-%m-%d-%H%M%S").to_string();
    let mut path = dir.join(format!("{}.png", base));
    if path.exists() {
        let suffix = now.timestamp_millis() % 1000;
        path = dir.join(format!("{}-{:03}.png", base, suffix));
    }

    let dyn_img = DynamicImage::ImageRgba8(cropped);
    dyn_img
        .save(&path)
        .map_err(|e| format!("Failed to save snip: {}", e))?;

    Ok(path)
}

pub fn copy_path_to_clipboard(path: &Path) -> Result<(), String> {
    let text = path
        .to_str()
        .ok_or("Failed to convert path to string")?
        .to_string();
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| format!("Failed to init clipboard: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to copy path to clipboard: {}", e))?;
    Ok(())
}

pub(crate) fn snip_dir() -> Result<PathBuf, String> {
    if let Some(pictures) = dirs::picture_dir() {
        return Ok(pictures.join("Diction"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join("Pictures").join("Diction"));
    }
    Err("Failed to resolve Pictures directory".into())
}
