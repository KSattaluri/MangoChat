use eframe::egui;
use egui::{pos2, vec2, Align2, Color32, FontId, Sense, Stroke};

use crate::ui::theme::*;
use crate::ui::widgets;
use crate::ui::MangoChatApp;

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let accent = app.current_accent();

    // ── Sub-tab bar (pinned above scroll area) ──
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (id, label) in [
            ("browser", "Browser"),
            ("aliases", "Custom text aliases"),
            ("system", "Mango Chat aliases"),
            ("apps", "App locations"),
        ] {
            let active = app.commands_sub_tab == id;
            if widgets::sub_tab_button(ui, label, active, accent).clicked() {
                app.commands_sub_tab = id.to_string();
            }
        }
    });
    ui.add_space(10.0);

    // ── Sub-tab content inside scroll area ──
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.add_space(12.0);
            match app.commands_sub_tab.as_str() {
                "browser" => render_browser_commands(app, ui),
                "aliases" => render_text_aliases(app, ui),
                "apps" => render_app_paths(app, ui),
                "system" => render_system_placeholder(ui),
                _ => render_browser_commands(app, ui),
            }
        });
}

fn render_browser_commands(app: &mut MangoChatApp, ui: &mut egui::Ui) {
    let accent = app.current_accent();

    // ── Default browser selector (single row: icon + label + buttons) ──
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        // Globe icon
        let icon_size = 16.0;
        let (icon_rect, _) =
            ui.allocate_exact_size(vec2(icon_size, icon_size), Sense::hover());
        if ui.is_rect_visible(icon_rect) {
            draw_globe_icon(ui.painter(), icon_rect.center(), icon_size, accent.base);
        }

        ui.label(
            egui::RichText::new("Default Browser")
                .size(12.0)
                .strong()
                .color(TEXT_COLOR),
        );

        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.x = 4.0;
        for (id, label) in [
            ("chrome", "Chrome"),
            ("edge", "Edge"),
            ("firefox", "Firefox"),
        ] {
            let active = app.form.default_browser == id;
            let text_color = if active {
                Color32::BLACK
            } else {
                TEXT_COLOR
            };
            let fill = if active {
                accent.base
            } else {
                BTN_BG
            };
            let border = if active {
                accent.ring
            } else {
                BTN_BORDER
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(label)
                            .size(12.0)
                            .color(text_color),
                    )
                    .fill(fill)
                    .stroke(Stroke::new(1.0, border)),
                )
                .clicked()
            {
                app.form.default_browser = id.to_string();
            }
        }
    });

    ui.add_space(20.0);

    // ── URL command list ──
    let trigger_w = 140.0;
    let delete_w = 24.0;
    let spacing = ui.spacing().item_spacing.x;
    {
        let row_w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(row_w.max(0.0), 20.0), Sense::hover());
        let font = FontId::proportional(12.0);
        ui.painter().text(
            pos2(rect.min.x, rect.center().y),
            Align2::LEFT_CENTER,
            "Command",
            font.clone(),
            TEXT_MUTED,
        );
        ui.painter().text(
            pos2(rect.min.x + trigger_w + spacing, rect.center().y),
            Align2::LEFT_CENTER,
            "Target address",
            font,
            TEXT_MUTED,
        );
    }
    ui.add_space(4.0);

    let mut delete_url_idx: Option<usize> = None;
    for (i, cmd) in app.form.url_commands.iter_mut().enumerate() {
        let row_w = ui.available_width();
        let url_w = (row_w - trigger_w - delete_w - spacing * 2.0).max(140.0);

        ui.horizontal(|ui| {
            ui.set_width(row_w.max(0.0));
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
}

fn render_text_aliases(app: &mut MangoChatApp, ui: &mut egui::Ui) {
    let trigger_w = 140.0;
    let delete_w = 24.0;
    let spacing = ui.spacing().item_spacing.x;
    {
        let row_w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(row_w.max(0.0), 20.0), Sense::hover());
        let font = FontId::proportional(12.0);
        ui.painter().text(
            pos2(rect.min.x, rect.center().y),
            Align2::LEFT_CENTER,
            "Command",
            font.clone(),
            TEXT_MUTED,
        );
        ui.painter().text(
            pos2(rect.min.x + trigger_w + spacing, rect.center().y),
            Align2::LEFT_CENTER,
            "Text alias",
            font,
            TEXT_MUTED,
        );
    }
    ui.add_space(4.0);

    let mut delete_alias_idx: Option<usize> = None;
    for (i, cmd) in app.form.alias_commands.iter_mut().enumerate() {
        let row_w = ui.available_width();
        let replacement_w =
            (row_w - trigger_w - delete_w - spacing * 2.0).max(180.0);

        ui.horizontal(|ui| {
            ui.set_width(row_w.max(0.0));
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
}

fn render_app_paths(app: &mut MangoChatApp, ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("Use valid .exe paths for this machine; mileage may vary.")
            .size(12.0)
            .color(TEXT_MUTED),
    );
    ui.add_space(8.0);

    let trigger_w = 140.0;
    let delete_w = 24.0;
    let spacing = ui.spacing().item_spacing.x;
    {
        let row_w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(row_w.max(0.0), 20.0), Sense::hover());
        let font = FontId::proportional(12.0);
        ui.painter().text(
            pos2(rect.min.x, rect.center().y),
            Align2::LEFT_CENTER,
            "App name",
            font.clone(),
            TEXT_MUTED,
        );
        ui.painter().text(
            pos2(rect.min.x + trigger_w + spacing, rect.center().y),
            Align2::LEFT_CENTER,
            "App path",
            font,
            TEXT_MUTED,
        );
    }
    ui.add_space(4.0);

    let mut delete_idx: Option<usize> = None;
    for (i, shortcut) in app.form.app_shortcuts.iter_mut().enumerate() {
        let row_w = ui.available_width();
        let path_w = (row_w - trigger_w - delete_w - spacing * 2.0).max(180.0);

        ui.horizontal(|ui| {
            ui.set_width(row_w.max(0.0));
            ui.visuals_mut().extreme_bg_color = Color32::from_rgb(0x1a, 0x1d, 0x24);
            let trigger_id = egui::Id::new(("app_shortcut_trigger", i));
            ui.add_sized(
                [trigger_w, 22.0],
                egui::TextEdit::singleline(&mut shortcut.trigger)
                    .id(trigger_id)
                    .interactive(!shortcut.builtin)
                    .font(FontId::proportional(13.0))
                    .text_color(TEXT_COLOR),
            );
            ui.visuals_mut().extreme_bg_color = Color32::from_rgb(0x1a, 0x1d, 0x24);
            ui.add_sized(
                [path_w, 22.0],
                egui::TextEdit::singleline(&mut shortcut.path)
                    .font(FontId::proportional(13.0))
                    .text_color(TEXT_COLOR),
            );
            if !shortcut.builtin {
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
                    delete_idx = Some(i);
                }
            } else {
                ui.add_sized([delete_w, 22.0], egui::Label::new(""));
            }
        });
        ui.add_space(2.0);
    }
    if let Some(idx) = delete_idx {
        app.form.app_shortcuts.remove(idx);
    }

    ui.add_space(6.0);
    if ui
        .add_sized(
            [ui.available_width(), 28.0],
            egui::Button::new(
                egui::RichText::new("+ Add Shortcut")
                    .size(13.0)
                    .color(TEXT_COLOR),
            )
            .fill(BTN_BG)
            .stroke(Stroke::new(0.5, BTN_BORDER)),
        )
        .clicked()
    {
        let new_idx = app.form.app_shortcuts.len();
        app.form.app_shortcuts.push(crate::settings::AppShortcut {
            trigger: String::new(),
            path: String::new(),
            builtin: false,
        });
        let focus_id = egui::Id::new(("app_shortcut_trigger", new_idx));
        ui.memory_mut(|m| m.request_focus(focus_id));
    }
}

fn render_system_placeholder(ui: &mut egui::Ui) {
    let p = theme_palette(ui.visuals().dark_mode);
    ui.label(
        egui::RichText::new("Say these commands by themselves; mileage may vary.")
            .size(12.0)
            .color(TEXT_MUTED),
    );
    ui.add_space(8.0);

    let rows = [
        ("enter", "Insert a line break in the active app."),
        ("yes", "Insert a line break in the active app."),
        ("back", "Delete the previous word."),
        ("back back", "Delete the current line."),
        ("new line", "Insert a line break."),
        ("new paragraph", "Insert a double line break."),
        ("undo", "Undo the previous action (Ctrl+Z)."),
        ("copy", "Copy selected text (Ctrl+C)."),
        ("paste", "Paste from clipboard (Ctrl+V)."),
        ("cut", "Cut selected text (Ctrl+X)."),
        ("select all", "Select all text (Ctrl+A)."),
    ];

    egui::Frame::none()
        .fill(p.settings_bg)
        .stroke(Stroke::new(0.5, BTN_BORDER))
        .inner_margin(egui::Margin::same(10.0))
        .rounding(egui::Rounding::same(8.0))
        .show(ui, |ui| {
            let row_w = ui.available_width();
            let command_w = 160.0;
            let behavior_w = (row_w - command_w - 12.0).max(240.0);

            egui::Grid::new("system_commands_grid")
                .num_columns(2)
                .striped(true)
                .min_col_width(0.0)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        [command_w, 20.0].into(),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new("Command")
                                    .size(12.0)
                                    .strong()
                                    .color(p.text_muted),
                            );
                        },
                    );
                    ui.allocate_ui_with_layout(
                        [behavior_w, 20.0].into(),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new("Behavior")
                                    .size(12.0)
                                    .strong()
                                    .color(p.text_muted),
                            );
                        },
                    );
                    ui.end_row();

                    for (command, behavior) in rows {
                        ui.allocate_ui_with_layout(
                            [command_w, 22.0].into(),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(command)
                                        .size(13.0)
                                        .strong()
                                        .color(TEXT_COLOR),
                                );
                            },
                        );
                        ui.allocate_ui_with_layout(
                            [behavior_w, 22.0].into(),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(behavior)
                                        .size(12.5)
                                        .color(TEXT_COLOR),
                                );
                            },
                        );
                        ui.end_row();
                    }
                });
        });

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("These commands are built-in and cannot be edited.")
            .size(12.0)
            .color(p.text_muted),
    );
}

/// Draws a simple globe icon (circle + meridian + equator) at the given center.
fn draw_globe_icon(painter: &egui::Painter, c: egui::Pos2, s: f32, color: Color32) {
    let r = s * 0.44;
    let stroke = Stroke::new(1.2, color);
    // Outer circle
    painter.circle_stroke(c, r, stroke);
    // Horizontal equator
    painter.line_segment(
        [pos2(c.x - r, c.y), pos2(c.x + r, c.y)],
        stroke,
    );
    // Vertical meridian (ellipse approximated with a few line segments)
    let n = 12;
    let rx = r * 0.45;
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let a = std::f32::consts::TAU * (i as f32 / n as f32);
        pts.push(pos2(c.x + rx * a.cos(), c.y + r * a.sin()));
    }
    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], stroke);
    }
}
