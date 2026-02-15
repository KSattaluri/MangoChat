use eframe::egui;
use egui::{Color32, FontId, Stroke};

use crate::ui::theme::*;
use crate::ui::MangoChatApp;

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.add_space(6.0);

            // --- Browser Commands ---
            ui.label(
                egui::RichText::new("Browser Commands")
                    .size(13.0)
                    .strong()
                    .color(TEXT_COLOR),
            );
            ui.add_space(8.0);

            let mut delete_url_idx: Option<usize> = None;
            for (i, cmd) in app.form.url_commands.iter_mut().enumerate() {
                let row_w = ui.available_width();
                let trigger_w = 100.0;
                let delete_w = 24.0;
                let spacing = ui.spacing().item_spacing.x;
                let url_w = (row_w - trigger_w - delete_w - spacing * 2.0).max(140.0);

                ui.horizontal(|ui| {
                    ui.set_width(row_w);
                    ui.visuals_mut().extreme_bg_color =
                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                    let trigger_id = egui::Id::new(("url_cmd_trigger", i));
                    ui.add_sized(
                        [trigger_w, 22.0],
                        egui::TextEdit::singleline(&mut cmd.trigger)
                            .id(trigger_id)
                            .interactive(!cmd.builtin)
                            .font(FontId::proportional(13.0))
                            .text_color(TEXT_COLOR),
                    );
                    ui.visuals_mut().extreme_bg_color =
                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                    ui.add_sized(
                        [url_w, 22.0],
                        egui::TextEdit::singleline(&mut cmd.url)
                            .font(FontId::proportional(13.0))
                            .text_color(TEXT_COLOR),
                    );
                    if !cmd.builtin {
                        if ui
                            .add_sized(
                                [delete_w, 22.0],
                                egui::Button::new(
                                    egui::RichText::new("x")
                                        .size(13.0)
                                        .color(RED),
                                )
                                .fill(BTN_BG)
                                .stroke(Stroke::new(0.5, BTN_BORDER)),
                            )
                            .clicked()
                        {
                            delete_url_idx = Some(i);
                        }
                    }
                    if cmd.builtin {
                        ui.add_sized(
                            [delete_w, 22.0],
                            egui::Label::new(""),
                        );
                    }
                });
                ui.add_space(2.0);
            }
            if let Some(idx) = delete_url_idx {
                app.form.url_commands.remove(idx);
            }

            ui.add_space(6.0);
            if ui
                .add_sized(
                    [ui.available_width(), 28.0],
                    egui::Button::new(
                        egui::RichText::new("+ Add Command")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    )
                    .fill(BTN_BG)
                    .stroke(Stroke::new(0.5, BTN_BORDER)),
                )
                .clicked()
            {
                let new_idx = app.form.url_commands.len();
                app.form.url_commands.push(crate::settings::UrlCommand {
                    trigger: String::new(),
                    url: String::new(),
                    builtin: false,
                });
                let focus_id = egui::Id::new(("url_cmd_trigger", new_idx));
                ui.memory_mut(|m| m.request_focus(focus_id));
            }

            // --- Text Aliases ---
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);

            ui.label(
                egui::RichText::new("Text Aliases")
                    .size(13.0)
                    .strong()
                    .color(TEXT_COLOR),
            );
            ui.add_space(8.0);

            let mut delete_alias_idx: Option<usize> = None;
            for (i, cmd) in app.form.alias_commands.iter_mut().enumerate() {
                let row_w = ui.available_width();
                let trigger_w = 140.0;
                let delete_w = 24.0;
                let spacing = ui.spacing().item_spacing.x;
                let replacement_w =
                    (row_w - trigger_w - delete_w - spacing * 2.0).max(180.0);

                ui.horizontal(|ui| {
                    ui.set_width(row_w);
                    ui.visuals_mut().extreme_bg_color =
                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                    let trigger_id = egui::Id::new(("alias_trigger", i));
                    ui.add_sized(
                        [trigger_w, 22.0],
                        egui::TextEdit::singleline(&mut cmd.trigger)
                            .id(trigger_id)
                            .font(FontId::proportional(13.0))
                            .text_color(TEXT_COLOR),
                    );
                    ui.visuals_mut().extreme_bg_color =
                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                    ui.add_sized(
                        [replacement_w, 22.0],
                        egui::TextEdit::singleline(&mut cmd.replacement)
                            .font(FontId::proportional(13.0))
                            .text_color(TEXT_COLOR),
                    );
                    if ui
                        .add_sized(
                            [delete_w, 22.0],
                            egui::Button::new(
                                egui::RichText::new("x")
                                    .size(13.0)
                                    .color(RED),
                            )
                            .fill(BTN_BG)
                            .stroke(Stroke::new(0.5, BTN_BORDER)),
                        )
                        .clicked()
                    {
                        delete_alias_idx = Some(i);
                    }
                });
                ui.add_space(2.0);
            }
            if let Some(idx) = delete_alias_idx {
                app.form.alias_commands.remove(idx);
            }

            ui.add_space(6.0);
            if ui
                .add_sized(
                    [ui.available_width(), 28.0],
                    egui::Button::new(
                        egui::RichText::new("+ Add Alias")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    )
                    .fill(BTN_BG)
                    .stroke(Stroke::new(0.5, BTN_BORDER)),
                )
                .clicked()
            {
                let new_idx = app.form.alias_commands.len();
                app.form
                    .alias_commands
                    .push(crate::settings::AliasCommand {
                        trigger: String::new(),
                        replacement: String::new(),
                    });
                let focus_id = egui::Id::new(("alias_trigger", new_idx));
                ui.memory_mut(|m| m.request_focus(focus_id));
            }
        });
}
