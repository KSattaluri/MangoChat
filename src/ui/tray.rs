use eframe::egui::Color32;
use super::theme::AccentPalette;

pub fn setup_tray(accent: AccentPalette) -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::TrayIconBuilder;

    let menu = Menu::new();
    let quit = MenuItem::with_id("quit", "Quit", true, None);

    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    let icon = match make_tray_icon(accent.base) {
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
            println!("[tray] built successfully");
            Some(tray)
        }
        Err(e) => {
            eprintln!("[tray] build error: {}", e);
            None
        }
    };

    tray
}

fn make_tray_icon(color: Color32) -> Option<tray_icon::Icon> {
    let mut icon_data = vec![0u8; 16 * 16 * 4];
    for pixel in icon_data.chunks_exact_mut(4) {
        pixel[0] = color.r();
        pixel[1] = color.g();
        pixel[2] = color.b();
        pixel[3] = 0xFF;
    }
    match tray_icon::Icon::from_rgba(icon_data, 16, 16) {
        Ok(i) => Some(i),
        Err(e) => {
            eprintln!("[tray] icon error: {}", e);
            None
        }
    }
}

