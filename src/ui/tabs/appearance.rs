use eframe::egui;
use egui::{Stroke, vec2};

use crate::ui::theme::*;
use crate::ui::widgets::provider_default_button;
use crate::ui::MangoChatApp;

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let accent = app.current_accent();

    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            // --- Accent Color Picker (from Color tab) ---
            let frame_overhead = 34.0;
            let total_w = ui.available_width() - frame_overhead;
            let select_w = 56.0;
            let color_w = (total_w - select_w - 16.0).max(120.0);

            ui.horizontal(|ui| {
                ui.add_sized(
                    [select_w, 20.0],
                    egui::Label::new(
                        egui::RichText::new("Select")
                            .size(13.0)
                            .strong()
                            .color(TEXT_MUTED),
                    ),
                );
                ui.add_sized(
                    [color_w, 20.0],
                    egui::Label::new(
                        egui::RichText::new("Color")
                            .size(13.0)
                            .strong()
                            .color(TEXT_MUTED),
                    ),
                );
            });
            ui.add_space(2.0);

            for choice in accent_options() {
                let is_selected = app.form.accent_color == choice.id;
                egui::Frame::none()
                    .fill(BTN_BG)
                    .stroke(Stroke::new(1.0, BTN_BORDER))
                    .rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_width(total_w);
                        ui.horizontal(|ui| {
                            let selector = ui
                                .allocate_ui_with_layout(
                                    vec2(select_w, 24.0),
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |ui| {
                                        provider_default_button(
                                            ui,
                                            true,
                                            is_selected,
                                            accent,
                                        )
                                    },
                                )
                                .inner;
                            if selector.clicked() {
                                app.form.accent_color = choice.id.to_string();
                            }
                            ui.add_sized(
                                [color_w, 24.0],
                                egui::Label::new(
                                    egui::RichText::new(choice.name)
                                        .size(13.0)
                                        .strong()
                                        .color(if is_selected {
                                            accent.base
                                        } else {
                                            TEXT_COLOR
                                        }),
                                ),
                            );
                        });
                    });
                ui.add_space(2.0);
            }
            ui.label(
                egui::RichText::new(
                    "Applies to visualizer, start/settings controls, and accent highlights.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );

            // --- Transparent background ---
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Transparent background")
                        .size(13.0)
                        .color(TEXT_COLOR),
                );
                ui.add_space(32.0);
                let mut transparent = !app.form.compact_background_enabled;
                egui::ComboBox::from_id_salt("transparent_bg_select")
                    .selected_text(if transparent { "Yes" } else { "No" })
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut transparent, true, "Yes");
                        ui.selectable_value(&mut transparent, false, "No");
                    });
                app.form.compact_background_enabled = !transparent;
            });
        });
}

