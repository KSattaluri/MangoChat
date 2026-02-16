use eframe::egui;

use crate::ui::theme::*;
use crate::ui::window::*;
use crate::ui::MangoChatApp;

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let accent = app.current_accent();
    let frame_overhead = 34.0;
    let content_w = (ui.available_width() - frame_overhead).max(0.0);
    let label_w = 200.0;
    let control_w = (content_w - label_w - 16.0).max(160.0);

    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.add_space(6.0);

            egui::Grid::new("appearance_grid")
                .num_columns(2)
                .min_col_width(label_w)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    // ── Accent color ──
                    ui.label(
                        egui::RichText::new("Accent color")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    {
                        let options = accent_options();
                        let selected_name = options
                            .iter()
                            .find(|o| o.id == app.form.accent_color)
                            .map(|o| o.name)
                            .unwrap_or("Green");
                        egui::ComboBox::from_id_salt("accent_color_select")
                            .selected_text(
                                egui::RichText::new(selected_name)
                                    .color(accent.base),
                            )
                            .width(control_w)
                            .show_ui(ui, |ui| {
                                for choice in options {
                                    let resp = ui.selectable_value(
                                        &mut app.form.accent_color,
                                        choice.id.to_string(),
                                        egui::RichText::new(choice.name)
                                            .color(choice.base),
                                    );
                                    if resp.changed() {}
                                }
                            });
                    }
                    ui.end_row();

                    // Help text spans into the control column
                    ui.label("");
                    ui.label(
                        egui::RichText::new(
                            "Applies to visualizer, controls, and highlights.",
                        )
                        .size(11.0)
                        .color(TEXT_MUTED),
                    );
                    ui.end_row();

                    // ── Transparent background ──
                    ui.label(
                        egui::RichText::new("Transparent background")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    {
                        let mut transparent = !app.form.compact_background_enabled;
                        egui::ComboBox::from_id_salt("transparent_bg_select")
                            .selected_text(if transparent { "Yes" } else { "No" })
                            .width(control_w)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut transparent, true, "Yes");
                                ui.selectable_value(&mut transparent, false, "No");
                            });
                        app.form.compact_background_enabled = !transparent;
                    }
                    ui.end_row();

                    // ── Separator ──
                    ui.separator();
                    ui.separator();
                    ui.end_row();

                    // ── Monitor ──
                    ui.label(
                        egui::RichText::new("Monitor")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    {
                        let choices = app.monitor_choices();
                        egui::ComboBox::from_id_salt("window_monitor_id_select")
                            .selected_text(
                                if app.form.window_monitor_id.trim().is_empty() {
                                    "Primary monitor".to_string()
                                } else {
                                    app.monitor_label_for_id(&app.form.window_monitor_id)
                                },
                            )
                            .width(control_w)
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
                    }
                    ui.end_row();

                    // ── Anchor ──
                    ui.label(
                        egui::RichText::new("Anchor")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    egui::ComboBox::from_id_salt("window_anchor_select")
                        .selected_text(MangoChatApp::anchor_label(&app.form.window_anchor))
                        .width(control_w)
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

                    // ── Auto-minimize ──
                    ui.label(
                        egui::RichText::new("Auto-minimize on focus loss")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    {
                        let mut auto_min = app.form.auto_minimize;
                        egui::ComboBox::from_id_salt("auto_minimize_select")
                            .selected_text(if auto_min { "Yes" } else { "No" })
                            .width(control_w)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut auto_min, true, "Yes");
                                ui.selectable_value(&mut auto_min, false, "No");
                            });
                        app.form.auto_minimize = auto_min;
                    }
                    ui.end_row();

                    // ── Separator ──
                    ui.separator();
                    ui.separator();
                    ui.end_row();

                    // ── Screenshot capture ──
                    ui.label(
                        egui::RichText::new("Screenshot capture")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    {
                        let mut enabled = app.form.screenshot_enabled;
                        egui::ComboBox::from_id_salt("screenshot_enabled_select")
                            .selected_text(if enabled { "Yes" } else { "No" })
                            .width(control_w)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut enabled, true, "Yes");
                                ui.selectable_value(&mut enabled, false, "No");
                            });
                        app.form.screenshot_enabled = enabled;
                    }
                    ui.end_row();

                    // ── Retention count ──
                    ui.label(
                        egui::RichText::new("Retention count")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            egui::DragValue::new(&mut app.form.screenshot_retention_count)
                                .range(1..=200),
                        );
                        if resp.hovered() || resp.has_focus() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                        }
                        ui.label(
                            egui::RichText::new("images")
                                .size(12.0)
                                .color(TEXT_MUTED),
                        );
                    });
                    ui.end_row();

                    // ── After edit capture ──
                    ui.label(
                        egui::RichText::new("After edit capture")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    {
                        let revert_label = match app.form.snip_edit_revert.as_str() {
                            "image" => "Switch to copy image",
                            "path" => "Switch to copy path",
                            _ => "Stay on edit",
                        };
                        egui::ComboBox::from_id_salt("snip_edit_revert_select")
                            .selected_text(revert_label)
                            .width(control_w)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut app.form.snip_edit_revert,
                                    "stay".to_string(),
                                    "Stay on edit",
                                );
                                ui.selectable_value(
                                    &mut app.form.snip_edit_revert,
                                    "image".to_string(),
                                    "Switch to copy image",
                                );
                                ui.selectable_value(
                                    &mut app.form.snip_edit_revert,
                                    "path".to_string(),
                                    "Switch to copy path",
                                );
                            });
                    }
                    ui.end_row();
                });
        });
}
