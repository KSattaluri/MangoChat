use eframe::egui;
use crate::ui::theme::*;
use crate::ui::widgets::section_header;
use crate::ui::{MangoChatApp, UpdateUiState};

pub fn render_about(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new("Mango Chat \u{2014} Voice Dictation")
                    .size(13.0)
                    .strong()
                    .color(TEXT_COLOR),
            );

            // --- Updates ---
            section_header(ui, "Updates");

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
                        egui::RichText::new("Latest version")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    let latest_text = match &app.update_state {
                        UpdateUiState::Available { latest, .. } => latest.version.to_string(),
                        UpdateUiState::UpToDate { current } => current.clone(),
                        UpdateUiState::Checking => "Checking...".to_string(),
                        UpdateUiState::Error(_) => "Unknown".to_string(),
                        _ => "Not checked".to_string(),
                    };
                    ui.label(
                        egui::RichText::new(latest_text)
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

                    ui.label(
                        egui::RichText::new("Update feed URL override")
                            .size(13.0)
                            .color(TEXT_COLOR),
                    );
                    let mut feed = app.form.update_feed_url_override.clone();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut feed)
                            .desired_width(360.0)
                            .hint_text(
                                "Optional: local/test URL or GitHub releases page URL",
                            ),
                    );
                    if response.changed() {
                        app.form.update_feed_url_override = feed;
                    }
                    ui.end_row();
                });

            ui.label(
                egui::RichText::new(
                    "When set, update checks use this URL. Leave empty to use MangoChat GitHub releases.",
                )
                .size(11.0)
                .color(TEXT_MUTED),
            );
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
            if let Some(last) = app.update_last_check {
                let secs = last.elapsed().as_secs();
                ui.label(
                    egui::RichText::new(format!("Last checked: {}s ago", secs))
                        .size(11.0)
                        .color(TEXT_MUTED),
                );
            }
        });
}

pub fn render_faq(app: &MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let accent = app.current_accent();
    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
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
