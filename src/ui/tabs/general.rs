use eframe::egui;
use crate::ui::theme::*;
use crate::ui::window::*;
use crate::ui::MangoChatApp;

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let frame_overhead = 34.0;
    let content_w = ui.available_width() - frame_overhead;

    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.add_space(6.0);

            // --- Window Placement ---
            let choices = app.monitor_choices();
            egui::Grid::new("window_placement_grid")
                .num_columns(2)
                .spacing([16.0, 12.0])
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("Monitor")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    egui::ComboBox::from_id_salt("window_monitor_id_select")
                        .selected_text(
                            if app.form.window_monitor_id.trim().is_empty() {
                                "Primary monitor".to_string()
                            } else {
                                app.monitor_label_for_id(
                                    &app.form.window_monitor_id,
                                )
                            },
                        )
                        .width(content_w * 0.6)
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
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Anchor")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    egui::ComboBox::from_id_salt("window_anchor_select")
                        .selected_text(MangoChatApp::anchor_label(
                            &app.form.window_anchor,
                        ))
                        .width(content_w * 0.6)
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
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Auto-minimize on focus loss")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    let mut auto_min = app.form.auto_minimize;
                    egui::ComboBox::from_id_salt("auto_minimize_select")
                        .selected_text(if auto_min { "Yes" } else { "No" })
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut auto_min, true, "Yes");
                            ui.selectable_value(&mut auto_min, false, "No");
                        });
                    app.form.auto_minimize = auto_min;
                    ui.end_row();

                    // --- Screenshot (same grid so dropdowns align) ---
                    ui.separator();
                    ui.separator();
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Screenshot capture")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    let mut enabled = app.form.screenshot_enabled;
                    egui::ComboBox::from_id_salt("screenshot_enabled_select")
                        .selected_text(if enabled { "Yes" } else { "No" })
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut enabled, true, "Yes");
                            ui.selectable_value(&mut enabled, false, "No");
                        });
                    app.form.screenshot_enabled = enabled;
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Retention count")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            egui::DragValue::new(
                                &mut app.form.screenshot_retention_count,
                            )
                            .range(1..=200),
                        );
                        if resp.hovered() || resp.has_focus() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                        }
                        ui.label("images");
                    });
                    ui.end_row();
                });

            // --- App Paths ---
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);

            egui::Grid::new("app_paths_grid")
                .num_columns(2)
                .spacing([16.0, 12.0])
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("Chrome")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut app.form.chrome_path)
                            .desired_width(content_w * 0.7),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Paint")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut app.form.paint_path)
                            .desired_width(content_w * 0.7),
                    );
                    ui.end_row();
                });
        });
}
