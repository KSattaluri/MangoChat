use eframe::egui;
use egui::{Color32, FontId, Stroke};

use crate::ui::theme::*;
use crate::ui::widgets::section_header;
use crate::ui::MangoChatApp;

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            // --- Browser Commands ---
            section_header(ui, "Browser Commands");
            ui.label(
                egui::RichText::new(
                    "URL/browser commands: say the trigger to open in Chrome. 'explorer' opens File Explorer at configured path.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );
            ui.add_space(2.0);

            let mut delete_url_idx: Option<usize> = None;
            for (i, cmd) in app.form.url_commands.iter_mut().enumerate() {
                let row_w = ui.available_width();
                let trigger_w = 84.0;
                let delete_w = 20.0;
                let spacing = ui.spacing().item_spacing.x;
                let url_w = (row_w - trigger_w - delete_w - spacing * 2.0).max(140.0);

                ui.horizontal(|ui| {
                    ui.set_width(row_w);
                    ui.visuals_mut().extreme_bg_color =
                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                    ui.add_sized(
                        [trigger_w, 18.0],
                        egui::TextEdit::singleline(&mut cmd.trigger)
                            .interactive(!cmd.builtin)
                            .font(FontId::proportional(11.0))
                            .text_color(TEXT_COLOR),
                    );
                    ui.visuals_mut().extreme_bg_color =
                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                    ui.add_sized(
                        [url_w, 18.0],
                        egui::TextEdit::singleline(&mut cmd.url)
                            .font(FontId::proportional(11.0))
                            .text_color(TEXT_COLOR),
                    );
                    if !cmd.builtin {
                        if ui
                            .add_sized(
                                [delete_w, 18.0],
                                egui::Button::new(
                                    egui::RichText::new("x")
                                        .size(11.0)
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
                            [delete_w, 18.0],
                            egui::Label::new(""),
                        );
                    }
                });
            }
            if let Some(idx) = delete_url_idx {
                app.form.url_commands.remove(idx);
            }

            if ui
                .add_sized(
                    [ui.available_width(), 20.0],
                    egui::Button::new(
                        egui::RichText::new("+ Add Command")
                            .size(11.0)
                            .color(TEXT_COLOR),
                    )
                    .fill(BTN_BG)
                    .stroke(Stroke::new(0.5, BTN_BORDER)),
                )
                .clicked()
            {
                app.form.url_commands.push(crate::settings::UrlCommand {
                    trigger: String::new(),
                    url: String::new(),
                    builtin: false,
                });
            }

            // --- Text Aliases (moved from Screenshot tab) ---
            ui.add_space(10.0);
            section_header(ui, "Text Aliases");
            ui.label(
                egui::RichText::new(
                    "Experimental aliases: when trigger is heard, type replacement text.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );
            ui.add_space(2.0);

            let mut delete_alias_idx: Option<usize> = None;
            for (i, cmd) in app.form.alias_commands.iter_mut().enumerate() {
                let row_w = ui.available_width();
                let trigger_w = 120.0;
                let delete_w = 20.0;
                let spacing = ui.spacing().item_spacing.x;
                let replacement_w =
                    (row_w - trigger_w - delete_w - spacing * 2.0).max(180.0);

                ui.horizontal(|ui| {
                    ui.set_width(row_w);
                    ui.visuals_mut().extreme_bg_color =
                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                    ui.add_sized(
                        [trigger_w, 18.0],
                        egui::TextEdit::singleline(&mut cmd.trigger)
                            .font(FontId::proportional(11.0))
                            .text_color(TEXT_COLOR),
                    );
                    ui.visuals_mut().extreme_bg_color =
                        Color32::from_rgb(0x1a, 0x1d, 0x24);
                    ui.add_sized(
                        [replacement_w, 18.0],
                        egui::TextEdit::singleline(&mut cmd.replacement)
                            .font(FontId::proportional(11.0))
                            .text_color(TEXT_COLOR),
                    );
                    if ui
                        .add_sized(
                            [delete_w, 18.0],
                            egui::Button::new(
                                egui::RichText::new("x")
                                    .size(11.0)
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
            }
            if let Some(idx) = delete_alias_idx {
                app.form.alias_commands.remove(idx);
            }

            if ui
                .add_sized(
                    [ui.available_width(), 20.0],
                    egui::Button::new(
                        egui::RichText::new("+ Add Alias")
                            .size(11.0)
                            .color(TEXT_COLOR),
                    )
                    .fill(BTN_BG)
                    .stroke(Stroke::new(0.5, BTN_BORDER)),
                )
                .clicked()
            {
                app.form
                    .alias_commands
                    .push(crate::settings::AliasCommand {
                        trigger: String::new(),
                        replacement: String::new(),
                    });
            }
        });
}

