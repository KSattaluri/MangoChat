use eframe::egui;
use egui::{Color32, FontId, Stroke, vec2};

use crate::ui::theme::*;
use crate::ui::widgets::*;
use crate::ui::MangoChatApp;

fn provider_model_label(app: &MangoChatApp, provider_id: &str) -> String {
    match provider_id {
        "openai" => app.form.model.clone(),
        "deepgram" => "nova-3".to_string(),
        "elevenlabs" => "scribe_v2_realtime".to_string(),
        "assemblyai" => "Universal Streaming v3".to_string(),
        _ => "-".to_string(),
    }
}

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let p = theme_palette(true);
    let accent = app.current_accent();

    let current_provider_name = PROVIDER_ROWS
        .iter()
        .find(|(id, _)| *id == app.settings.provider.as_str())
        .map(|(_, name)| *name)
        .unwrap_or("Unknown");
    let current_provider_color = MangoChatApp::provider_color(&app.settings.provider, p);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Current Provider:")
                .size(14.0)
                .strong()
                .color(p.text_muted),
        );
        ui.label(
            egui::RichText::new(current_provider_name)
                .size(14.0)
                .strong()
                .color(current_provider_color),
        );
    });
    ui.add_space(8.0);

    // Subtract frame overhead so rows have even left/right margins.
    let frame_overhead = 34.0;
    let total_w = ui.available_width() - frame_overhead;
    let provider_w = 220.0;
    let validate_w = 92.0;
    let default_w = 72.0;
    let row_pad_x = 8.0;
    let spacing_w = 32.0;
    let api_w = (total_w - provider_w - validate_w - default_w - row_pad_x * 2.0 - spacing_w)
        .max(160.0);

    ui.horizontal(|ui| {
        ui.set_width((total_w - row_pad_x * 2.0).max(0.0));
        ui.add_space(row_pad_x);
        ui.add_sized(
            [default_w, 20.0],
            egui::Label::new(
                egui::RichText::new("Default")
                    .size(13.0)
                    .strong()
                    .color(p.text_muted),
            ),
        );
        ui.add_sized(
            [provider_w, 20.0],
            egui::Label::new(
                egui::RichText::new("Provider")
                    .size(13.0)
                    .strong()
                    .color(p.text_muted),
            ),
        );
        ui.add_sized(
            [api_w, 20.0],
            egui::Label::new(
                egui::RichText::new("API Key")
                    .size(13.0)
                    .strong()
                    .color(p.text_muted),
            ),
        );
        ui.add_sized(
            [validate_w, 20.0],
            egui::Label::new(
                egui::RichText::new("Validate")
                    .size(13.0)
                    .strong()
                    .color(p.text_muted),
            ),
        );
    });
    ui.add_space(2.0);

    for (provider_id, provider_name) in PROVIDER_ROWS {
        let provider_id = (*provider_id).to_string();
        egui::Frame::none()
            .fill(p.btn_bg)
            .stroke(Stroke::new(1.0, p.btn_border))
            .rounding(6.0)
            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
            .show(ui, |ui| {
                ui.set_width(total_w.max(0.0));
                ui.horizontal(|ui| {
                    let model_label = provider_model_label(app, &provider_id);
                    let key_value = app
                        .form
                        .api_keys
                        .entry(provider_id.clone())
                        .or_default();
                    let can_default = !key_value.trim().is_empty();
                    let is_default = app.form.provider == provider_id;
                    let default_resp = ui
                        .allocate_ui_with_layout(
                            vec2(default_w, 24.0),
                            egui::Layout::centered_and_justified(
                                egui::Direction::LeftToRight,
                            ),
                            |ui| {
                                provider_default_button(
                                    ui,
                                    can_default,
                                    is_default,
                                    accent,
                                )
                            },
                        )
                        .inner;
                    if default_resp.clicked() && can_default {
                        app.form.provider = provider_id.clone();
                    }

                    let provider_color = MangoChatApp::provider_color(&provider_id, p);
                    ui.allocate_ui_with_layout(
                        vec2(provider_w, 34.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.add_sized(
                                [provider_w, 16.0],
                                egui::Label::new(
                                    egui::RichText::new(*provider_name)
                                        .size(13.0)
                                        .strong()
                                        .color(provider_color),
                                )
                                .wrap_mode(egui::TextWrapMode::Truncate),
                            );
                            ui.add_sized(
                                [provider_w, 14.0],
                                egui::Label::new(
                                    egui::RichText::new(model_label)
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                )
                                .wrap_mode(egui::TextWrapMode::Truncate),
                            );
                        },
                    );

                    let key_resp = ui
                        .scope(|ui| {
                            let dark = ui.visuals().dark_mode;
                            let input_bg = if dark {
                                Color32::from_rgb(0x1a, 0x1d, 0x24)
                            } else {
                                Color32::from_rgb(0xff, 0xff, 0xff)
                            };
                            let input_stroke = if dark {
                                Color32::from_rgb(0x2c, 0x2f, 0x36)
                            } else {
                                Color32::from_rgb(0xd1, 0xd5, 0xdb)
                            };
                            let visuals = ui.visuals_mut();
                            visuals.extreme_bg_color = input_bg;
                            visuals.widgets.inactive.bg_fill = input_bg;
                            visuals.widgets.hovered.bg_fill = input_bg;
                            visuals.widgets.active.bg_fill = input_bg;
                            visuals.widgets.inactive.bg_stroke =
                                Stroke::new(1.0, input_stroke);
                            visuals.widgets.hovered.bg_stroke =
                                Stroke::new(1.0, input_stroke);
                            visuals.widgets.active.bg_stroke =
                                Stroke::new(1.0, input_stroke);
                            ui.add_sized(
                                [api_w, 34.0],
                                egui::TextEdit::singleline(key_value)
                                    .password(true)
                                    .font(FontId::proportional(12.5)),
                            )
                        })
                        .inner;
                    if key_resp.changed() {
                        app.key_check_result.remove(&provider_id);
                        if app
                            .last_validated_provider
                            .as_deref()
                            == Some(provider_id.as_str())
                        {
                            app.last_validated_provider = None;
                        }
                    }

                    let key_present = !key_value.trim().is_empty();
                    let inflight = app.key_check_inflight.contains(&provider_id);
                    let result = app.key_check_result.get(&provider_id).cloned();
                    let validate_resp = ui
                        .allocate_ui_with_layout(
                            vec2(validate_w, 34.0),
                            egui::Layout::centered_and_justified(
                                egui::Direction::LeftToRight,
                            ),
                            |ui| {
                                provider_validate_button(
                                    ui,
                                    key_present,
                                    inflight,
                                    result.as_ref().map(|(ok, _)| *ok),
                                    accent,
                                )
                            },
                        )
                        .inner;
                    if validate_resp.clicked() && key_present && !inflight {
                        app.key_check_inflight.insert(provider_id.clone());
                        app.key_check_result.remove(&provider_id);
                        app.last_validated_provider = Some(provider_id.clone());
                        let provider_name = PROVIDER_ROWS
                            .iter()
                            .find(|(id, _)| *id == provider_id.as_str())
                            .map(|(_, name)| (*name).to_string())
                            .unwrap_or_else(|| provider_id.clone());
                        let provider =
                            crate::provider::create_provider(&provider_id);
                        let provider_settings = crate::provider::ProviderSettings {
                            api_key: key_value.clone(),
                            model: app.form.model.clone(),
                            transcription_model: app
                                .settings
                                .transcription_model
                                .clone(),
                            language: app.form.language.clone(),
                        };
                        let event_tx = app.event_tx.clone();
                        let validated_provider_id = provider_id.clone();
                        app.runtime.spawn(async move {
                            let result =
                                crate::provider::session::validate_key(
                                    provider,
                                    provider_settings,
                                )
                                .await;
                            let (ok, message) = match result {
                                Ok(()) => (
                                    true,
                                    format!(
                                        "{} API key is valid",
                                        provider_name
                                    ),
                                ),
                                Err(e) => (
                                    false,
                                    format!(
                                        "{} validation failed: {}",
                                        provider_name, e
                                    ),
                                ),
                            };
                            let _ = event_tx.send(
                                crate::state::AppEvent::ApiKeyValidated {
                                    provider: validated_provider_id,
                                    ok,
                                    message,
                                },
                            );
                        });
                    }
                    validate_resp.on_hover_text(if inflight {
                        "Validating..."
                    } else if let Some((ok, msg)) = &result {
                        if *ok {
                            "Validated"
                        } else {
                            msg.as_str()
                        }
                    } else if key_present {
                        "Validate key"
                    } else {
                        "Enter API key first"
                    });
                    default_resp.on_hover_text(if can_default {
                        if is_default {
                            "Default provider"
                        } else {
                            "Set as default provider"
                        }
                    } else {
                        "Enter API key first"
                    });
                });
            });
        ui.add_space(6.0);
    }

    if let Some(provider_id) = app.last_validated_provider.as_ref() {
        if let Some((ok, msg)) = app.key_check_result.get(provider_id) {
            let color = if *ok { accent.base } else { RED };
            ui.add_space(4.0);
            ui.label(egui::RichText::new(msg).size(11.0).color(color));
        }
    }
    if app
        .form
        .api_keys
        .get(&app.form.provider)
        .map(|k| k.trim().is_empty())
        .unwrap_or(true)
    {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("Default provider must have an API key.")
                .size(11.0)
                .color(TEXT_MUTED),
        );
    }
}

