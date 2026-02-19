use super::theme::AccentPalette;

/// Mango icon PNG embedded at compile time.
const MANGO_PNG: &[u8] = include_bytes!("../../icons/mango.png");

pub fn setup_tray(_accent: AccentPalette) -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::TrayIconBuilder;

    let menu = Menu::new();
    let quit = MenuItem::with_id("quit", "Quit", true, None);

    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    let icon = match make_tray_icon() {
        Some(i) => i,
        None => return None,
    };

    let tray = match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Mango Chat")
        .with_icon(icon)
        .build()
    {
        Ok(tray) => {
            app_log!("[tray] built successfully");
            Some(tray)
        }
        Err(e) => {
            app_err!("[tray] build error: {}", e);
            None
        }
    };

    tray
}

fn make_tray_icon() -> Option<tray_icon::Icon> {
    let img = match image::load_from_memory(MANGO_PNG) {
        Ok(i) => i,
        Err(e) => {
            app_err!("[tray] failed to decode mango.png: {}", e);
            return None;
        }
    };

    // Resize to 32x32 (crisp on standard and high-DPI displays)
    let resized = img.resize(32, 32, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (w, h) = rgba.dimensions();

    match tray_icon::Icon::from_rgba(rgba.into_raw(), w, h) {
        Ok(i) => Some(i),
        Err(e) => {
            app_err!("[tray] icon error: {}", e);
            None
        }
    }
}
