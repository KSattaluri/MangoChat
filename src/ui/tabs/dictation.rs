use eframe::egui;
use crate::audio;
use crate::snip;
use crate::ui::theme::*;
use crate::ui::MangoChatApp;

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    let mut out: String = input.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let accent = app.current_accent();
    let frame_overhead = 34.0;
    let content_w = ui.available_width() - frame_overhead;

    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.add_space(4.0);

            // --- All dictation settings in one aligned grid ---
            egui::Grid::new("dictation_grid")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    // Microphone
                    ui.label(
                        egui::RichText::new("Microphone")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 26.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let combo_w = (content_w - 170.0).max(120.0);
                            let selected_mic = if app.form.mic.is_empty() {
                                "Default".to_string()
                            } else {
                                truncate_chars(&app.form.mic, 38)
                            };
                            egui::ComboBox::from_id_salt("mic_select")
                                .selected_text(selected_mic)
                                .width(combo_w)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut app.form.mic,
                                        String::new(),
                                        "Default",
                                    );
                                    for dev in &app.mic_devices {
                                        ui.selectable_value(
                                            &mut app.form.mic,
                                            dev.clone(),
                                            dev,
                                        );
                                    }
                                });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_sized(
                                            [72.0, 22.0],
                                            egui::Button::new(
                                                egui::RichText::new("Refresh")
                                                    .color(TEXT_COLOR),
                                            )
                                            .fill(accent.base.gamma_multiply(0.22))
                                            .stroke(egui::Stroke::new(
                                                1.0,
                                                accent.base.gamma_multiply(0.85),
                                            )),
                                        )
                                        .clicked()
                                    {
                                        app.mic_devices = audio::list_input_devices();
                                        if !app.form.mic.is_empty()
                                            && !app.mic_devices.contains(&app.form.mic)
                                        {
                                            app.form.mic.clear();
                                        }
                                    }
                                },
                            );
                        },
                    );
                    ui.end_row();

                    // Session hotkey
                    ui.label(
                        egui::RichText::new("Session hotkey")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Right Ctrl")
                                .size(13.0)
                                .strong()
                                .color(accent.base),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("(outside this window: start/stop recording)")
                                .size(12.0)
                                .color(TEXT_MUTED),
                        );
                    });
                    ui.end_row();

                    // Noise suppression
                    ui.label(
                        egui::RichText::new("Noise suppression")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    egui::ComboBox::from_id_salt("vad_mode")
                        .selected_text(match app.form.vad_mode.as_str() {
                            "lenient" => "Low",
                            _ => "High (recommended)",
                        })
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.form.vad_mode,
                                "strict".to_string(),
                                "High (recommended)",
                            );
                            ui.selectable_value(
                                &mut app.form.vad_mode,
                                "lenient".to_string(),
                                "Low",
                            );
                        });
                    ui.end_row();

                    // Max session length
                    ui.label(
                        egui::RichText::new("Max session length")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            egui::DragValue::new(
                                &mut app.form.max_session_length_minutes,
                            )
                            .range(1..=120),
                        );
                        if resp.hovered() || resp.has_focus() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                        }
                        ui.label(
                            egui::RichText::new("min")
                                .size(12.0)
                                .color(TEXT_MUTED),
                        );
                    });
                    ui.end_row();

                    // Inactivity timeout
                    ui.label(
                        egui::RichText::new("Inactivity timeout")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            egui::DragValue::new(
                                &mut app.form.provider_inactivity_timeout_secs,
                            )
                            .range(5..=300),
                        );
                        if resp.hovered() || resp.has_focus() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                        }
                        ui.label(
                            egui::RichText::new("sec")
                                .size(12.0)
                                .color(TEXT_MUTED),
                        );
                    });
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
                        let control_w = (content_w - 216.0).max(160.0);
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

                    // ── Screenshot hotkey ──
                    ui.label(
                        egui::RichText::new("Screenshot hotkey")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Right Alt")
                                .size(13.0)
                                .strong()
                                .color(accent.base),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("(outside this window: screenshot on current monitor)")
                                .size(12.0)
                                .color(TEXT_MUTED),
                        );
                    });
                    ui.end_row();

                    // ── Retention count ──
                    ui.label(
                        egui::RichText::new("Retention count")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal(|ui| {
                        let control_w = (content_w - 216.0).max(160.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(control_w, 24.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
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
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add_sized(
                                                [148.0, 22.0],
                                                egui::Button::new(
                                                    egui::RichText::new("Open images folder")
                                                        .color(TEXT_COLOR),
                                                )
                                                .fill(accent.base.gamma_multiply(0.22))
                                                .stroke(egui::Stroke::new(
                                                    1.0,
                                                    accent.base.gamma_multiply(0.85),
                                                )),
                                            )
                                            .clicked()
                                        {
                                            if let Err(e) = snip::open_snip_folder() {
                                                app.set_status(
                                                    &format!("Failed to open folder: {}", e),
                                                    "error",
                                                );
                                            }
                                        }
                                    },
                                );
                            },
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
                        let control_w = (content_w - 216.0).max(160.0);
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

                    ui.label(
                        egui::RichText::new("Reset defaults")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let btn = egui::Button::new(
                            egui::RichText::new("Reset")
                                .size(12.0)
                                .strong()
                                .color(egui::Color32::BLACK),
                        )
                        .fill(accent.base)
                        .stroke(egui::Stroke::new(1.0, accent.ring));
                        if ui.add(btn).clicked() {
                            app.form.reset_non_provider_defaults();
                            app.mic_devices = audio::list_input_devices();
                            if !app.form.mic.is_empty()
                                && !app.mic_devices.contains(&app.form.mic)
                            {
                                app.form.mic.clear();
                            }
                            app.set_status(
                                "Defaults restored. Click Save to apply.",
                                "idle",
                            );
                        }
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(
                                "(does not reset provider/API keys or usage logs)",
                            )
                            .size(12.0)
                            .color(TEXT_MUTED),
                        );
                    });
                    ui.end_row();
                });
        });
}
