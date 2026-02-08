use chrono::Local;
use image::{imageops, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Monitor bounds in physical pixels.
pub struct MonitorBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

pub fn capture_screen(
    cursor: Option<(i32, i32)>,
) -> Result<(RgbaImage, MonitorBounds), String> {
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

    let scale_factor = monitor.scale_factor().unwrap_or(1.0);
    let bounds = MonitorBounds {
        x: monitor.x().unwrap_or(0),
        y: monitor.y().unwrap_or(0),
        width: monitor.width().unwrap_or(1920),
        height: monitor.height().unwrap_or(1080),
        scale_factor,
    };

    let image = monitor
        .capture_image()
        .map_err(|e| format!("xcap capture error: {:?}", e))?;

    Ok((image, bounds))
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
    let mut path = dir.join(format!("{}.jpg", base));
    if path.exists() {
        let suffix = now.timestamp_millis() % 1000;
        path = dir.join(format!("{}-{:03}.jpg", base, suffix));
    }

    let (w, h) = cropped.dimensions();
    let rgb_data: Vec<u8> = cropped
        .as_raw()
        .chunks_exact(4)
        .flat_map(|px| &px[..3])
        .copied()
        .collect();

    use image::codecs::jpeg::JpegEncoder;
    use image::ImageEncoder;
    let mut jpeg_bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg_bytes, 90)
        .write_image(&rgb_data, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("JPEG encode error: {}", e))?;
    fs::write(&path, jpeg_bytes).map_err(|e| format!("Failed to save snip: {}", e))?;

    let _ = prune_old_snips(&dir, 5);

    Ok(path)
}

pub fn copy_path_to_clipboard(path: &Path) -> Result<(), String> {
    let text = path
        .to_str()
        .ok_or("Failed to convert path to string")?
        .to_string();
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Failed to init clipboard: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to copy path to clipboard: {}", e))?;
    Ok(())
}

pub fn snip_dir() -> Result<PathBuf, String> {
    if let Some(pictures) = dirs::picture_dir() {
        return Ok(pictures.join("Jarvis"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join("Pictures").join("Jarvis"));
    }
    Err("Failed to resolve Pictures directory".into())
}

pub fn open_snip_folder() -> Result<(), String> {
    let dir = snip_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {}", e))?;
    std::process::Command::new("explorer")
        .arg(dir.as_os_str())
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;
    Ok(())
}

fn prune_old_snips(dir: &Path, keep: usize) -> Result<(), String> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read snip dir: {}", e))?;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "jpg" && ext != "jpeg" {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        files.push((path, modified));
    }

    files.sort_by(|a, b| b.1.cmp(&a.1));
    for (idx, (path, _)) in files.into_iter().enumerate() {
        if idx >= keep {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}
