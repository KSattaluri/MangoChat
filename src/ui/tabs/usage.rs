use eframe::egui;
use egui::{vec2, Stroke};

use crate::ui::formatting::*;
use crate::ui::theme::*;
use crate::ui::widgets::section_header;
use crate::ui::JarvisApp;

pub fn render(app: &mut JarvisApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let accent = app.current_accent();

    // 4-column stat cards (full width)
    if let Ok(u) = app.state.usage.lock() {
        ui.columns(4, |cols| {
            stat_card(&mut cols[0], "Sent", &fmt_duration_ms(u.ms_sent));
            stat_card(
                &mut cols[1],
                "Suppressed",
                &fmt_duration_ms(u.ms_suppressed),
            );
            stat_card(&mut cols[2], "Data", &fmt_bytes(u.bytes_sent));
            stat_card(&mut cols[3], "Transcripts", &u.finals.to_string());
        });
    }
    // Per-provider breakdown
    if let Ok(pt) = app.state.provider_totals.lock() {
        if !pt.is_empty() {
            ui.add_space(4.0);
            let mut providers: Vec<_> = pt.iter().collect();
            providers.sort_by(|a, b| b.1.ms_sent.cmp(&a.1.ms_sent));
            for (provider_id, pu) in &providers {
                let color = JarvisApp::provider_color(
                    provider_id,
                    theme_palette(ui.visuals().dark_mode),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {} sent | {} suppressed | {} | {} transcripts",
                        JarvisApp::provider_display_name(provider_id),
                        fmt_duration_ms(pu.ms_sent),
                        fmt_duration_ms(pu.ms_suppressed),
                        fmt_bytes(pu.bytes_sent),
                        pu.finals,
                    ))
                    .size(10.0)
                    .color(color),
                );
            }
        }
    }
    // Live session
    if app.is_recording {
        if let Ok(s) = app.state.session_usage.lock() {
            if s.started_ms != 0 {
                let elapsed = now_ms().saturating_sub(s.started_ms);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Live: {} | {} | {} transcripts",
                        fmt_duration_ms(elapsed),
                        fmt_bytes(s.bytes_sent),
                        s.finals,
                    ))
                    .size(11.0)
                    .color(accent.base),
                );
            }
        }
    }
    // Action buttons
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("Reset Totals")
                        .size(11.0)
                        .color(TEXT_COLOR),
                )
                .fill(BTN_BG)
                .stroke(Stroke::new(1.0, BTN_BORDER))
                .rounding(4.0),
            )
            .clicked()
        {
            app.confirm_reset_totals = true;
            app.confirm_reset_include_sessions = false;
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("Open Log Folder")
                        .size(11.0)
                        .color(TEXT_COLOR),
                )
                .fill(BTN_BG)
                .stroke(Stroke::new(1.0, BTN_BORDER))
                .rounding(4.0),
            )
            .clicked()
        {
            if let Some(dir) = crate::usage::data_dir() {
                let _ = std::process::Command::new("explorer")
                    .arg(&dir)
                    .spawn();
            }
        }
    });
    if app.confirm_reset_totals {
        let mut close_dialog = false;
        egui::Window::new("Reset Totals?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "This deletes usage totals files and clears current totals. Continue?",
                    )
                    .size(11.0)
                    .color(TEXT_COLOR),
                );
                ui.add_space(4.0);
                ui.checkbox(
                    &mut app.confirm_reset_include_sessions,
                    egui::RichText::new(
                        "Also clear recent sessions (usage-session.jsonl)",
                    )
                    .size(11.0)
                    .color(TEXT_COLOR),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        close_dialog = true;
                    }
                    if ui
                        .add(
                            egui::Button::new("Yes, Reset")
                                .fill(RED)
                                .stroke(Stroke::new(1.0, RED)),
                        )
                        .clicked()
                    {
                        if let Ok(mut u) = app.state.usage.lock() {
                            *u = crate::state::UsageTotals::default();
                        }
                        if let Ok(mut pt) = app.state.provider_totals.lock() {
                            pt.clear();
                        }
                        let _ = crate::usage::reset_totals_file();
                        let _ = crate::usage::reset_provider_totals_file();
                        if app.confirm_reset_include_sessions {
                            let _ = crate::usage::reset_session_file();
                            app.session_history.clear();
                        }
                        app.set_status("Totals reset", "idle");
                        close_dialog = true;
                    }
                });
            });
        if close_dialog {
            app.confirm_reset_totals = false;
            app.confirm_reset_include_sessions = false;
        }
    }
    // Session history table
    if !app.session_history.is_empty() {
        section_header(ui, "Recent Sessions");
        egui::ScrollArea::vertical()
            .max_height(ui.available_height().max(260.0))
            .show(ui, |ui| {
                egui::Grid::new("session_table")
                    .striped(true)
                    .num_columns(6)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        for h in [
                            "When",
                            "Provider",
                            "Duration",
                            "Audio",
                            "Data",
                            "Transcripts",
                        ] {
                            ui.label(
                                egui::RichText::new(h)
                                    .size(10.0)
                                    .strong()
                                    .color(TEXT_MUTED),
                            );
                        }
                        ui.end_row();
                        for s in &app.session_history {
                            let dur = s.updated_ms.saturating_sub(s.started_ms);
                            ui.label(
                                egui::RichText::new(fmt_relative_time(s.started_ms))
                                    .size(10.0)
                                    .color(TEXT_MUTED),
                            );
                            ui.label(
                                egui::RichText::new(&s.provider)
                                    .size(10.0)
                                    .color(TEXT_COLOR),
                            );
                            ui.label(
                                egui::RichText::new(fmt_duration_ms(dur))
                                    .size(10.0)
                                    .color(TEXT_COLOR),
                            );
                            ui.label(
                                egui::RichText::new(fmt_duration_ms(s.ms_sent))
                                    .size(10.0)
                                    .color(TEXT_COLOR),
                            );
                            ui.label(
                                egui::RichText::new(fmt_bytes(s.bytes_sent))
                                    .size(10.0)
                                    .color(TEXT_COLOR),
                            );
                            ui.label(
                                egui::RichText::new(s.finals.to_string())
                                    .size(10.0)
                                    .color(TEXT_COLOR),
                            );
                            ui.end_row();
                        }
                    });
            });
    } else {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("No session history yet")
                .size(11.0)
                .color(TEXT_MUTED),
        );
    }
}
