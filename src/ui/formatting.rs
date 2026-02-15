use eframe::egui;
use super::theme::theme_palette;

pub fn fmt_duration_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

pub fn fmt_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

pub fn fmt_relative_time(ms: u64) -> String {
    if ms == 0 {
        return "\u{2014}".into();
    }
    let now = now_ms();
    let ago = now.saturating_sub(ms) / 1000;
    if ago < 60 {
        "just now".into()
    } else if ago < 3600 {
        format!("{}m ago", ago / 60)
    } else if ago < 86400 {
        format!("{}h ago", ago / 3600)
    } else {
        format!("{}d ago", ago / 86400)
    }
}

pub fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn stat_card(ui: &mut egui::Ui, label: &str, value: &str) {
    let p = theme_palette(ui.visuals().dark_mode);
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(label).size(9.0).color(p.text_muted));
        ui.label(
            egui::RichText::new(value)
                .size(13.0)
                .strong()
                .color(p.text),
        );
    });
}

