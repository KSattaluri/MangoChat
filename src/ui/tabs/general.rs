use eframe::egui;
use crate::ui::theme::*;
use crate::ui::window::*;
use crate::ui::{MangoChatApp, UpdateUiState};

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
                                app.monitor_label_for_id(&app.form.window_monitor_id)
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
                        .selected_text(MangoChatApp::anchor_label(&app.form.window_anchor))
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

                    // --- Screenshot ---
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
                            egui::DragValue::new(&mut app.form.screenshot_retention_count)
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

            // --- Updates ---
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);

            egui::Grid::new("updates_grid")
                .num_columns(2)
                .spacing([16.0, 12.0])
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("Current version")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.label(
                        egui::RichText::new(env!("CARGO_PKG_VERSION"))
                            .size(12.0)
                            .color(TEXT_MUTED),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Auto-check for updates")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    let mut auto_update = app.form.auto_update_enabled;
                    egui::ComboBox::from_id_salt("auto_update_enabled_select")
                        .selected_text(if auto_update { "Yes" } else { "No" })
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut auto_update, true, "Yes");
                            ui.selectable_value(&mut auto_update, false, "No");
                        });
                    app.form.auto_update_enabled = auto_update;
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Include pre-release builds")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    let mut include_pre = app.form.update_include_prerelease;
                    egui::ComboBox::from_id_salt("update_include_prerelease_select")
                        .selected_text(if include_pre { "Yes" } else { "No" })
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut include_pre, true, "Yes");
                            ui.selectable_value(&mut include_pre, false, "No");
                        });
                    app.form.update_include_prerelease = include_pre;
                    ui.end_row();
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !app.update_check_inflight && !app.update_install_inflight,
                        egui::Button::new(
                            egui::RichText::new("Check now")
                                .size(11.0)
                                .color(TEXT_COLOR),
                        ),
                    )
                    .clicked()
                {
                    app.trigger_update_check();
                }

                if ui
                    .add_enabled(
                        matches!(app.update_state, UpdateUiState::Available { .. })
                            && !app.update_install_inflight,
                        egui::Button::new(
                            egui::RichText::new("Download & Install")
                                .size(11.0)
                                .color(TEXT_COLOR),
                        ),
                    )
                    .clicked()
                {
                    app.trigger_update_install();
                }

                if ui
                    .add_enabled(
                        matches!(app.update_state, UpdateUiState::Available { .. }),
                        egui::Button::new(
                            egui::RichText::new("Open release page")
                                .size(11.0)
                                .color(TEXT_COLOR),
                        ),
                    )
                    .clicked()
                {
                    app.open_update_release_page();
                }
            });

            let status_text = match &app.update_state {
                UpdateUiState::NotChecked => "Update status: not checked".to_string(),
                UpdateUiState::Checking => "Update status: checking...".to_string(),
                UpdateUiState::UpToDate { current } => {
                    format!("Update status: up to date ({current})")
                }
                UpdateUiState::Available { current, latest } => format!(
                    "Update available: {} -> {} (tag {}){}",
                    current,
                    latest.version,
                    latest.tag,
                    if latest.prerelease { " (pre-release)" } else { "" }
                ),
                UpdateUiState::Installing => {
                    "Update status: downloading installer...".to_string()
                }
                UpdateUiState::InstallLaunched { path } => format!(
                    "Installer launched: {} (app will close)",
                    path
                ),
                UpdateUiState::Error(e) => format!("Update status: error ({e})"),
            };
            ui.label(
                egui::RichText::new(status_text)
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
        });
}
