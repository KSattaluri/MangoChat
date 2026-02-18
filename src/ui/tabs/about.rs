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
                            let pre = if latest.prerelease { " pre-release" } else { "" };
                            format!("{} \u{2192} {} ({}{})", env!("CARGO_PKG_VERSION"), latest.version, latest.tag, pre)
                        }
                        UpdateUiState::Checking => {
                            format!("{} (checking\u{2026})", env!("CARGO_PKG_VERSION"))
                        }
                        UpdateUiState::Installing => {
                            format!("{} (installing\u{2026})", env!("CARGO_PKG_VERSION"))
                        }
                        UpdateUiState::InstallLaunched { path } => {
                            format!("{} (launched {})", env!("CARGO_PKG_VERSION"), path)
                        }
                        UpdateUiState::Error(e) => {
                            format!("{} (error: {})", env!("CARGO_PKG_VERSION"), e)
                        }
                        _ => env!("CARGO_PKG_VERSION").to_string(),
                    };
                    let display_version = truncate_chars(&version_text, 72);
                    ui.add_sized(
                        [360.0, 20.0],
                        egui::Label::new(
                            egui::RichText::new(display_version)
                                .size(12.0)
                                .color(TEXT_MUTED),
                        )
                        .wrap_mode(egui::TextWrapMode::Truncate),
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
                let install_btn = if install_enabled {
                    egui::Button::new(
                        egui::RichText::new("Download & Install")
                            .size(11.0)
                            .color(egui::Color32::BLACK),
                    )
                    .fill(accent.base)
                    .stroke(egui::Stroke::new(1.0, accent.ring))
                } else {
                    egui::Button::new(
                        egui::RichText::new("Download & Install")
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

                if ui
                    .add_enabled(
                        true,
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
        });
}

pub fn render_faq(app: &MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let accent = app.current_accent();
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(0.0));
            ui.label(
                egui::RichText::new("Frequently Asked Questions")
                    .size(13.0)
                    .strong()
                    .color(TEXT_COLOR),
            );
            ui.add_space(12.0);

            let items = [
                (
                    "How do I start dictating?",
                    "Hold Right Ctrl and speak. Release to commit the transcription to the active text field.",
                ),
                (
                    "What providers are supported?",
                    "OpenAI Realtime, Deepgram, ElevenLabs Realtime, and AssemblyAI. Select your provider in the Provider tab.",
                ),
                (
                    "How does VAD mode work?",
                    "Strict: only sends audio during speech. Lenient: lower threshold. Off: streams all audio.",
                ),
                (
                    "Where are settings stored?",
                    "In AppData/Local/MangoChat/settings.json on Windows. Usage logs are in the same folder.",
                ),
                (
                    "Can I use this with any app?",
                    "Yes \u{2014} Mango Chat types into whatever window has focus when you release the hotkey.",
                ),
                (
                    "How do I change the hotkey?",
                    "The hotkey is currently Right Ctrl. Custom hotkeys are planned for a future release.",
                ),
            ];

            for (i, (q, a)) in items.iter().enumerate() {
                ui.label(
                    egui::RichText::new(*q)
                        .size(12.0)
                        .strong()
                        .color(accent.base),
                );
                ui.add_space(3.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(*a)
                            .size(11.5)
                            .color(TEXT_MUTED),
                    )
                    .wrap(),
                );
                if i < items.len() - 1 {
                    ui.add_space(14.0);
                }
            }
        });
}
