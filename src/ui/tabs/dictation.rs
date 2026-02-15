use eframe::egui;
use crate::audio;
use crate::ui::theme::*;
use crate::ui::widgets::section_header;
use crate::ui::JarvisApp;

pub fn render(app: &mut JarvisApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("VAD Mode")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
            egui::ComboBox::from_id_salt("vad_mode")
                .selected_text(match app.form.vad_mode.as_str() {
                    "lenient" => "Lenient",
                    _ => "Strict",
                })
                .width(ui.available_width())
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
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Microphone")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
            ui.horizontal(|ui| {
                let combo_width = (ui.available_width() - 74.0).max(120.0);
                egui::ComboBox::from_id_salt("mic_select")
                    .selected_text(if app.form.mic.is_empty() {
                        "Default"
                    } else {
                        &app.form.mic
                    })
                    .width(combo_width)
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
                    .add_sized([68.0, 22.0], egui::Button::new("Refresh"))
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

            // --- Session Limits (from Configurations tab) ---
            ui.add_space(8.0);
            section_header(ui, "Session Limits");
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Max session length (minutes)")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
            ui.add(
                egui::Slider::new(
                    &mut app.form.max_session_length_minutes,
                    1..=120,
                )
                .text("min"),
            );
            ui.label(
                egui::RichText::new(
                    "Hard cap: recording always stops at this duration, even if activity continues.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Provider inactivity timeout (seconds)")
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
            ui.add(
                egui::Slider::new(
                    &mut app.form.provider_inactivity_timeout_secs,
                    5..=300,
                )
                .text("s"),
            );
            ui.label(
                egui::RichText::new(
                    "When no provider activity is observed, the app closes the live session and stops recording.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );
        });
}
