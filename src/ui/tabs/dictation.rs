use eframe::egui;
use crate::audio;
use crate::ui::theme::*;
use crate::ui::MangoChatApp;

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let frame_overhead = 34.0;
    let content_w = ui.available_width() - frame_overhead;

    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.add_space(6.0);

            // --- All dictation settings in one aligned grid ---
            egui::Grid::new("dictation_grid")
                .num_columns(2)
                .spacing([16.0, 12.0])
                .show(ui, |ui| {
                    // Microphone
                    ui.label(
                        egui::RichText::new("Microphone")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    ui.horizontal(|ui| {
                        let combo_w = (content_w - 170.0).max(120.0);
                        egui::ComboBox::from_id_salt("mic_select")
                            .selected_text(if app.form.mic.is_empty() {
                                "Default"
                            } else {
                                &app.form.mic
                            })
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
                        if ui
                            .add_sized(
                                [68.0, 22.0],
                                egui::Button::new("Refresh"),
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
                    });
                    ui.end_row();

                    // VAD Mode
                    ui.label(
                        egui::RichText::new("VAD Mode")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    egui::ComboBox::from_id_salt("vad_mode")
                        .selected_text(match app.form.vad_mode.as_str() {
                            "lenient" => "Lenient",
                            _ => "Strict",
                        })
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.form.vad_mode,
                                "strict".to_string(),
                                "Strict",
                            );
                            ui.selectable_value(
                                &mut app.form.vad_mode,
                                "lenient".to_string(),
                                "Lenient",
                            );
                        });
                    ui.end_row();

                    // Spacer row between dropdowns and session limits
                    ui.allocate_space(egui::vec2(0.0, 8.0));
                    ui.allocate_space(egui::vec2(0.0, 8.0));
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
                });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Suggest to leave defaults")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
        });
}
