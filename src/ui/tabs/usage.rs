use eframe::egui;
use egui::{vec2, Stroke};

use crate::state::ProviderUsage;
use crate::ui::formatting::*;
use crate::ui::theme::*;
use crate::ui::widgets::section_header;
use crate::ui::MangoChatApp;

/// A column in the metrics table.
struct MetricsCol {
    label: String,
    color: egui::Color32,
    ms_sent: u64,
    ms_suppressed: u64,
    bytes_sent: u64,
    finals: u64,
    is_live: bool,
}

impl MetricsCol {
    fn value(&self, row: usize) -> String {
        match row {
            0 => fmt_duration_ms(self.ms_sent + self.ms_suppressed),
            1 => fmt_duration_ms(self.ms_sent),
            2 => fmt_bytes(self.bytes_sent),
            3 => self.finals.to_string(),
            _ => String::new(),
        }
    }
}

/// Short display name for column headers to prevent overlap.
fn short_provider_name(name: &str) -> &str {
    match name {
        "ElevenLabs Realtime" => "ElevenLabs",
        "OpenAI Realtime" => "OpenAI",
        other => other,
    }
}

pub fn render(app: &mut MangoChatApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let accent = app.current_accent();

    egui::ScrollArea::vertical()
        .max_height(ui.available_height().max(260.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(0.0));

            // ── Build columns ──
            let mut columns: Vec<MetricsCol> = Vec::new();

            // Live session (first column if recording)
            if app.is_recording {
                if let Ok(s) = app.state.session_usage.lock() {
                    if s.started_ms != 0 {
                        columns.push(MetricsCol {
                            label: "Live".into(),
                            color: accent.base,
                            ms_sent: s.ms_sent,
                            ms_suppressed: s.ms_suppressed,
                            bytes_sent: s.bytes_sent,
                            finals: s.finals,
                            is_live: true,
                        });
                    }
                }
            }

            // Per-provider columns (sorted descending by ms_sent)
            if let Ok(pt) = app.state.provider_totals.lock() {
                let mut providers: Vec<(&String, &ProviderUsage)> = pt.iter().collect();
                providers.sort_by(|a, b| b.1.ms_sent.cmp(&a.1.ms_sent));
                for (provider_id, pu) in providers {
                    let p = theme_palette(ui.visuals().dark_mode);
                    columns.push(MetricsCol {
                        label: MangoChatApp::provider_display_name(provider_id).into(),
                        color: MangoChatApp::provider_color(provider_id, p),
                        ms_sent: pu.ms_sent,
                        ms_suppressed: pu.ms_suppressed,
                        bytes_sent: pu.bytes_sent,
                        finals: pu.finals,
                        is_live: false,
                    });
                }
            }

            // Total column
            if let Ok(u) = app.state.usage.lock() {
                columns.push(MetricsCol {
                    label: "Total".into(),
                    color: TEXT_MUTED,
                    ms_sent: u.ms_sent,
                    ms_suppressed: u.ms_suppressed,
                    bytes_sent: u.bytes_sent,
                    finals: u.finals,
                    is_live: false,
                });
            }

            let col_labels = ["Captured", "Sent", "Data", "Transcripts"];
            let now = ui.ctx().input(|i| i.time) as f32;
            let col_w = (ui.available_width() / (col_labels.len() + 1) as f32).max(60.0);

            // ── Metrics table: providers as rows, metrics as columns ──
            egui::Grid::new("usage_metrics_grid")
                .num_columns(col_labels.len() + 1)
                .min_col_width(col_w)
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    // Header row
                    ui.label("");
                    for label in &col_labels {
                        ui.label(
                            egui::RichText::new(*label)
                                .size(13.0)
                                .color(TEXT_MUTED),
                        );
                    }
                    ui.end_row();

                    // Provider rows
                    for col in &columns {
                        let is_total = col.label == "Total";

                        // Thin divider before Total row (painted inline, no extra spacer row)
                        if is_total && columns.len() > 1 {
                            let rect = ui.available_rect_before_wrap();
                            let y = rect.min.y;
                            let full_w = ui.min_rect().max.x;
                            ui.painter().line_segment(
                                [egui::pos2(rect.min.x, y), egui::pos2(full_w, y)],
                                Stroke::new(0.5, BTN_BORDER),
                            );
                        }

                        let name = short_provider_name(&col.label);

                        if col.is_live {
                            let pulse = (now * 2.2).sin() * 0.5 + 0.5;
                            let alpha = (80.0 + pulse * 175.0) as u8;
                            let live_color = egui::Color32::from_rgba_unmultiplied(
                                accent.base.r(), accent.base.g(), accent.base.b(), alpha,
                            );
                            ui.label(
                                egui::RichText::new("Live \u{00B7}\u{00B7}\u{00B7}")
                                    .size(13.0)
                                    .strong()
                                    .color(live_color),
                            );
                            for ri in 0..col_labels.len() {
                                ui.label(
                                    egui::RichText::new(&col.value(ri))
                                        .size(13.0)
                                        .strong()
                                        .color(accent.base),
                                );
                            }
                            ui.ctx().request_repaint();
                        } else {
                            ui.label(
                                egui::RichText::new(name)
                                    .size(13.0)
                                    .strong()
                                    .color(col.color),
                            );
                            for ri in 0..col_labels.len() {
                                ui.label(
                                    egui::RichText::new(&col.value(ri))
                                        .size(13.0)
                                        .strong()
                                        .color(TEXT_COLOR),
                                );
                            }
                        }
                        ui.end_row();
                    }
                });

            // ── Action buttons ──
            ui.add_space(10.0);
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

            // Reset confirmation dialog
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

            // ── Recent Sessions ──
            if !app.session_history.is_empty() {
                ui.add_space(16.0);
                section_header(ui, "Recent Sessions");
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
            } else {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("No session history yet")
                        .size(11.0)
                        .color(TEXT_MUTED),
                );
            }
        });
}
