use eframe::egui;
use crate::ui::theme::*;
use crate::ui::{MangoChatApp, UpdateUiState};

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    let mut out: String = input.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

pub fn render_about(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(0.0));
            ui.horizontal(|ui| {
                // Mango icon (lazy-loaded)
                let icon_sz = 20.0;
                let tex = app.mango_texture.get_or_insert_with(|| {
                    const MANGO_PNG: &[u8] = include_bytes!("../../../icons/mango.png");
                    let img = image::load_from_memory(MANGO_PNG)
                        .expect("embedded mango.png");
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let pixels = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                    ui.ctx().load_texture("mango-logo", pixels, egui::TextureOptions::LINEAR)
                });
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                let rect = ui.allocate_space(egui::vec2(icon_sz, icon_sz)).1;
                ui.painter().image(tex.id(), rect, uv, egui::Color32::WHITE);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Mango Chat \u{2014} Voice Dictation & Productivity App")
                        .size(13.0)
                        .strong()
                        .color(TEXT_COLOR),
                );
            });

            // ── Credits ──
            let accent = app.current_accent();
            let sz = 13.0;
            ui.add_space(12.0);
            {
                let prev = ui.spacing().item_spacing.y;
                ui.spacing_mut().item_spacing.y = 6.0;

                ui.hyperlink_to(
                    egui::RichText::new("mangochat.org")
                        .size(sz)
                        .color(accent.base),
                    "https://mangochat.org",
                );
                ui.label(
                    egui::RichText::new("Made by Kalyan Sattaluri")
                        .size(sz)
                        .color(TEXT_COLOR),
                );
                ui.label(
                    egui::RichText::new("Made with Claude & Codex")
                        .size(sz)
                        .color(TEXT_MUTED),
                );

                let fmt = |color| egui::text::TextFormat {
                    font_id: egui::FontId::proportional(sz),
                    color,
                    ..Default::default()
                };
                let mut job = egui::text::LayoutJob::default();
                job.append("Made for ", 0.0, fmt(TEXT_MUTED));
                job.append("Shreya ", 0.0, fmt(TEXT_COLOR));
                job.append("\u{2665}", 0.0, fmt(accent.base));
                job.append(" & ", 0.0, fmt(TEXT_MUTED));
                job.append("Avy ", 0.0, fmt(TEXT_COLOR));
                job.append("\u{2665}", 0.0, fmt(accent.base));
                ui.label(job);

                ui.hyperlink_to(
                    egui::RichText::new("github.com/KSattaluri/MangoChat")
                        .size(sz)
                        .color(accent.base),
                    "https://github.com/KSattaluri/MangoChat",
                );

                ui.spacing_mut().item_spacing.y = prev;
            }

            // --- Updates ---
            ui.add_space(16.0);
            {
                let rect = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.min.x, rect.min.y),
                        egui::pos2(rect.max.x, rect.min.y),
                    ],
                    egui::Stroke::new(0.5, BTN_BORDER),
                );
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Updates")
                    .size(13.0)
                    .strong()
                    .color(TEXT_MUTED),
            );

            egui::Grid::new("updates_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    // Version row — compact inline with status
                    ui.label(
                        egui::RichText::new("Version")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    let version_text = match &app.update_state {
                        UpdateUiState::UpToDate => {
                            format!("{} (up to date)", env!("CARGO_PKG_VERSION"))
                        }
                        UpdateUiState::Available { latest } => {
                            format!("{} -> {} ({})", env!("CARGO_PKG_VERSION"), latest.version, latest.tag)
                        }
                        UpdateUiState::Checking => {
                            format!("{} (checking\u{2026})", env!("CARGO_PKG_VERSION"))
                        }
                        UpdateUiState::Installing => {
                            format!("{} (installing\u{2026})", env!("CARGO_PKG_VERSION"))
                        }
                        UpdateUiState::Error(e) => {
                            format!("{} (error: {})", env!("CARGO_PKG_VERSION"), e)
                        }
                        _ => env!("CARGO_PKG_VERSION").to_string(),
                    };
                    let display_version = truncate_chars(&version_text, 72);
                    ui.allocate_ui_with_layout(
                        egui::vec2(360.0, 20.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(display_version)
                                        .size(12.0)
                                        .color(TEXT_MUTED),
                                )
                                .wrap_mode(egui::TextWrapMode::Truncate),
                            );
                        },
                    );
                    ui.end_row();

                });

            ui.add_space(4.0);
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

                let install_enabled = matches!(app.update_state, UpdateUiState::Available { .. })
                    && !app.update_install_inflight;
                let install_text = if app.update_install_inflight {
                    "Installing..."
                } else {
                    "Download & Install"
                };
                let install_btn = if install_enabled {
                    egui::Button::new(
                        egui::RichText::new(install_text)
                            .size(11.0)
                            .color(egui::Color32::BLACK),
                    )
                    .fill(accent.base)
                    .stroke(egui::Stroke::new(1.0, accent.ring))
                } else {
                    egui::Button::new(
                        egui::RichText::new(install_text)
                            .size(11.0)
                            .color(TEXT_COLOR),
                    )
                };
                if ui
                    .add_enabled(install_enabled, install_btn)
                    .clicked()
                {
                    app.trigger_update_install();
                }
            });

            // --- Diagnostics ---
            ui.add_space(14.0);
            {
                let rect = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.min.x, rect.min.y),
                        egui::pos2(rect.max.x, rect.min.y),
                    ],
                    egui::Stroke::new(0.5, BTN_BORDER),
                );
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Diagnostics")
                        .size(13.0)
                        .strong()
                        .color(TEXT_MUTED),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("(API keys excluded)")
                        .size(11.5)
                        .color(TEXT_MUTED),
                );
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Open logs folder")
                                .size(11.0)
                                .color(TEXT_COLOR),
                        )
                        .stroke(egui::Stroke::new(1.0, BTN_BORDER)),
                    )
                    .clicked()
                {
                    app.open_logs_folder();
                }

                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Export diagnostics ZIP")
                                .size(11.0)
                                .color(egui::Color32::BLACK),
                        )
                        .fill(accent.base)
                        .stroke(egui::Stroke::new(1.0, accent.ring)),
                    )
                    .clicked()
                {
                    app.export_diagnostics_zip();
                }
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Need help? Email the ZIP to {}",
                        crate::diagnostics::support_email()
                    ))
                    .size(11.5)
                    .color(accent.base),
                );
            });
            ui.add_space(4.0);
            if let Some(path) = app.diagnostics_last_export_path.as_ref() {
                ui.label(
                    egui::RichText::new(format!("Find the logs at: {}", path))
                        .size(10.5)
                        .color(accent.base),
                );
            }
        });
}

pub fn render_faq(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let accent = app.current_accent();
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(0.0));
            egui::Frame::none()
                .inner_margin(egui::Margin { left: 4.0, right: 16.0, top: 0.0, bottom: 0.0 })
                .show(ui, |ui| {
            // Title row with text size controls
            {
                let row_h = 24.0;
                let row_rect = ui.available_rect_before_wrap();
                let row_rect = egui::Rect::from_min_size(
                    row_rect.min,
                    egui::vec2(row_rect.width(), row_h),
                );
                ui.allocate_rect(row_rect, egui::Sense::hover());

                // Title on the left
                ui.painter().text(
                    egui::pos2(row_rect.min.x, row_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Frequently Asked Questions",
                    egui::FontId::proportional(13.0),
                    TEXT_COLOR,
                );

                // Controls on the right: "Text size" [-] [+]
                let btn_w = 26.0;
                let btn_h = 22.0;
                let gap = 6.0;
                let edge_pad = 8.0;

                let plus_right = row_rect.max.x - edge_pad;
                let plus_left = plus_right - btn_w;
                let minus_right = plus_left - gap;
                let minus_left = minus_right - btn_w;
                let label_right = minus_left - gap;

                let cy = row_rect.center().y;
                let btn_top = cy - btn_h * 0.5;
                let btn_bottom = cy + btn_h * 0.5;

                // "Text size" label
                ui.painter().text(
                    egui::pos2(label_right, cy),
                    egui::Align2::RIGHT_CENTER,
                    "Text size",
                    egui::FontId::proportional(13.0),
                    accent.base,
                );

                // Minus button
                let minus_rect = egui::Rect::from_min_max(
                    egui::pos2(minus_left, btn_top),
                    egui::pos2(minus_right, btn_bottom),
                );
                let minus_resp = ui.allocate_rect(minus_rect, egui::Sense::click());
                let minus_fill = if minus_resp.hovered() {
                    accent.base.gamma_multiply(0.35)
                } else {
                    accent.base.gamma_multiply(0.22)
                };
                ui.painter().rect(
                    minus_rect,
                    4.0,
                    minus_fill,
                    egui::Stroke::new(1.0, accent.base.gamma_multiply(0.85)),
                );
                ui.painter().text(
                    minus_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "\u{2212}",
                    egui::FontId::proportional(14.0),
                    TEXT_COLOR,
                );
                if minus_resp.clicked() {
                    app.faq_text_size = (app.faq_text_size - 1.0).max(9.0);
                }

                // Plus button
                let plus_rect = egui::Rect::from_min_max(
                    egui::pos2(plus_left, btn_top),
                    egui::pos2(plus_right, btn_bottom),
                );
                let plus_resp = ui.allocate_rect(plus_rect, egui::Sense::click());
                let plus_fill = if plus_resp.hovered() {
                    accent.base.gamma_multiply(0.35)
                } else {
                    accent.base.gamma_multiply(0.22)
                };
                ui.painter().rect(
                    plus_rect,
                    4.0,
                    plus_fill,
                    egui::Stroke::new(1.0, accent.base.gamma_multiply(0.85)),
                );
                ui.painter().text(
                    plus_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "+",
                    egui::FontId::proportional(14.0),
                    TEXT_COLOR,
                );
                if plus_resp.clicked() {
                    app.faq_text_size = (app.faq_text_size + 1.0).min(20.0);
                }
            }
            ui.add_space(12.0);

            let items = [
                (
                    "What happens when you start Mango Chat?",
                    "When you start recording, Mango Chat listens for audio from your device and streams it to your selected provider for transcription. Place your cursor in a text field to begin dictating.",
                ),
                (
                    "How do I quit Mango Chat?",
                    "Open the system tray and click Quit.",
                ),
                (
                    "Why do I need API keys?",
                    "API keys are required to connect Mango Chat to your speech-to-text provider. You can sign up for Deepgram and AssemblyAI to get up to $250 in trial credits with no credit card.",
                ),
                (
                    "Where are my API keys stored?",
                    "API keys are encrypted at rest and stored locally on your machine in AppData/Local/MangoChat. They are only transmitted over secure connections when authenticating with your chosen provider.",
                ),
                (
                    "Does Mango Chat collect telemetry or personal information?",
                    "Mango Chat has no built-in telemetry. During recording, audio is sent only to your selected provider for transcription.",
                ),
                (
                    "What are the hotkeys to start and stop Mango Chat?",
                    "In addition to the start/stop buttons on the UI, you can use Right Ctrl to start and stop recording when that hotkey is enabled in settings.",
                ),
                (
                    "Why do I sometimes experience delays or inaccurate transcription?",
                    "These are provider-dependent and may be caused by audio quality, speech clarity, network latency, or inherent limitations of the model.",
                ),
                (
                    "How do I take a screenshot?",
                    "When screenshot capture is enabled, move your cursor to the monitor you want, press Right Alt, then select the region.",
                ),
                (
                    "What happens after I capture a screenshot?",
                    "Based on your settings, Mango Chat can copy the image path, copy the image content, or open it in Paint for editing.",
                ),
                (
                    "Where are screenshots saved?",
                    "Use \u{201c}Open images folder\u{201d} in Settings to open the active screenshot directory.",
                ),
                (
                    "How much does transcription cost?",
                    "It depends on the chosen provider and model. Pricing is typically per second or per hour. Deepgram and AssemblyAI often provide free trial credits \u{2014} check their sites for current details.",
                ),
                (
                    "Which providers are supported?",
                    "Deepgram, OpenAI Realtime, ElevenLabs Realtime, and AssemblyAI.",
                ),
                (
                    "Can I customize commands and aliases?",
                    "Yes. You can edit browser commands, text aliases, and app locations from the Commands tab.",
                ),
            ];

            let q_size = app.faq_text_size + 2.0;
            let a_size = (app.faq_text_size - 0.5).max(9.0);
            let fmt_normal = |sz: f32| egui::text::TextFormat {
                font_id: egui::FontId::proportional(sz),
                color: TEXT_MUTED,
                ..Default::default()
            };
            let fmt_accent = |sz: f32| egui::text::TextFormat {
                font_id: egui::FontId::proportional(sz),
                color: accent.base,
                ..Default::default()
            };
            for (i, (q, a)) in items.iter().enumerate() {
                ui.label(
                    egui::RichText::new(*q)
                        .size(q_size)
                        .strong()
                        .color(accent.base),
                );
                ui.add_space(3.0);
                // Highlight "Right Ctrl" and "Right Alt" in accent color
                let parts: Vec<&str> = a.split("Right Ctrl").collect();
                if parts.len() > 1 {
                    let mut job = egui::text::LayoutJob::default();
                    job.wrap = egui::text::TextWrapping {
                        max_width: ui.available_width(),
                        ..Default::default()
                    };
                    for (j, part) in parts.iter().enumerate() {
                        job.append(part, 0.0, fmt_normal(a_size));
                        if j < parts.len() - 1 {
                            job.append("Right Ctrl", 0.0, fmt_accent(a_size));
                        }
                    }
                    ui.label(job);
                } else {
                    let parts: Vec<&str> = a.split("Right Alt").collect();
                    if parts.len() > 1 {
                        let mut job = egui::text::LayoutJob::default();
                        job.wrap = egui::text::TextWrapping {
                            max_width: ui.available_width(),
                            ..Default::default()
                        };
                        for (j, part) in parts.iter().enumerate() {
                            job.append(part, 0.0, fmt_normal(a_size));
                            if j < parts.len() - 1 {
                                job.append("Right Alt", 0.0, fmt_accent(a_size));
                            }
                        }
                        ui.label(job);
                    } else {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(*a)
                                    .size(a_size)
                                    .color(TEXT_MUTED),
                            )
                            .wrap(),
                        );
                    }
                }
                if i < items.len() - 1 {
                    ui.add_space(14.0);
                }
            }
            }); // Frame
        });
}
