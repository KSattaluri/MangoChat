use eframe::egui;
use crate::ui::theme::*;
use crate::ui::widgets::*;
use crate::ui::window::*;
use crate::ui::MangoChatApp;

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            // --- Window Placement (from Advanced tab) ---
            section_header(ui, "Window Placement");
            ui.label(
                egui::RichText::new("Monitor")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
            let choices = app.monitor_choices();
            egui::ComboBox::from_id_salt("window_monitor_id_select")
                .selected_text(
                    if app.form.window_monitor_id.trim().is_empty() {
                        "Primary monitor".to_string()
                    } else {
                        app.monitor_label_for_id(&app.form.window_monitor_id)
                    },
                )
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.form.window_monitor_id,
                        String::new(),
                        "Primary monitor",
                    );
                    for m in choices {
                        ui.selectable_value(
                            &mut app.form.window_monitor_id,
                            m.id.clone(),
                            m.label,
                        );
                    }
                });
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Anchor")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
            egui::ComboBox::from_id_salt("window_anchor_select")
                .selected_text(MangoChatApp::anchor_label(&app.form.window_anchor))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.form.window_anchor,
                        WINDOW_ANCHOR_TOP_LEFT.to_string(),
                        "Top Left",
                    );
                    ui.selectable_value(
                        &mut app.form.window_anchor,
                        WINDOW_ANCHOR_TOP_CENTER.to_string(),
                        "Top Center",
                    );
                    ui.selectable_value(
                        &mut app.form.window_anchor,
                        WINDOW_ANCHOR_TOP_RIGHT.to_string(),
                        "Top Right",
                    );
                    ui.selectable_value(
                        &mut app.form.window_anchor,
                        WINDOW_ANCHOR_BOTTOM_LEFT.to_string(),
                        "Bottom Left",
                    );
                    ui.selectable_value(
                        &mut app.form.window_anchor,
                        WINDOW_ANCHOR_BOTTOM_CENTER.to_string(),
                        "Bottom Center",
                    );
                    ui.selectable_value(
                        &mut app.form.window_anchor,
                        WINDOW_ANCHOR_BOTTOM_RIGHT.to_string(),
                        "Bottom Right",
                    );
                });
            ui.label(
                egui::RichText::new(
                    "Choose monitor + anchor for compact mode startup/collapse placement.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );
            ui.add_space(8.0);
            ui.checkbox(
                &mut app.form.auto_minimize,
                egui::RichText::new("Auto-minimize on focus loss")
                    .size(12.0)
                    .color(TEXT_COLOR),
            );
            ui.label(
                egui::RichText::new("Collapse settings when the app loses focus.")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );

            // --- Screenshot (from Screenshot tab) ---
            ui.add_space(10.0);
            section_header(ui, "Screenshot");
            ui.add_space(4.0);
            ui.checkbox(
                &mut app.form.screenshot_enabled,
                egui::RichText::new("Enable screenshot capture (Right Alt)")
                    .size(11.0)
                    .color(TEXT_COLOR),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Retention count")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
            ui.add(
                egui::Slider::new(
                    &mut app.form.screenshot_retention_count,
                    1..=200,
                )
                .text("images"),
            );
            ui.label(
                egui::RichText::new(
                    "When enabled, P / I / E buttons are shown and Right Alt triggers screenshot capture.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new(
                    "When disabled, Right Alt behaves normally and screenshot controls are hidden.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );

            // --- App Paths (from Advanced tab) ---
            ui.add_space(10.0);
            section_header(ui, "App Paths");
            field(ui, "Chrome", &mut app.form.chrome_path, false);
            ui.add_space(2.0);
            field(ui, "Paint", &mut app.form.paint_path, false);
        });
}

