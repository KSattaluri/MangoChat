use eframe::egui;
use egui::{pos2, vec2, Color32, CursorIcon, FontId, Rect, Sense, Stroke};

use super::theme::*;

#[derive(Clone)]
pub struct ControlTooltipState {
    pub key: String,
    pub text: String,
    pub until: f64,
}

pub fn settings_toggle(
    ui: &mut egui::Ui,
    is_recording: bool,
    accent: AccentPalette,
) -> egui::Response {
    let size = 28.0;
    let radius = size / 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());

    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let hovered = response.hovered();
        let idle_ring = Color32::from_rgba_unmultiplied(255, 255, 255, 180);

        let (fill, ring, glyph) = if is_recording {
            let fill = if hovered { accent.hover } else { accent.base };
            (fill, accent.ring, Color32::WHITE)
        } else {
            let gray = Color32::from_rgb(0x3a, 0x3d, 0x45);
            let gray_hover = Color32::from_rgb(0x4a, 0x4d, 0x55);
            let fill = if hovered { gray_hover } else { gray };
            (fill, idle_ring, Color32::WHITE)
        };

        ui.painter()
            .circle_stroke(center, radius, Stroke::new(1.5, ring));
        ui.painter().circle_filled(center, radius - 2.5, fill);
        // Draw a small cog manually so color is fully controllable across fonts/platforms.
        let gear_stroke = Stroke::new(1.2, glyph);
        let r_inner = 2.0;
        let r_ring = 4.2;
        let r_tooth_outer = 6.0;
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            let dir = vec2(a.cos(), a.sin());
            let p1 = center + dir * r_ring;
            let p2 = center + dir * r_tooth_outer;
            ui.painter().line_segment([p1, p2], gear_stroke);
        }
        ui.painter()
            .circle_stroke(center, r_ring, Stroke::new(1.2, glyph));
        ui.painter().circle_filled(center, r_inner, glyph);
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn mic_unavailable_badge(ui: &mut egui::Ui, rect: Rect) -> egui::Response {
    let response = ui.interact(rect, egui::Id::new("mic_unavailable_badge"), Sense::hover());
    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let ring = Color32::from_rgb(0xef, 0x44, 0x44);
        let icon = Color32::from_rgb(0xf3, 0xf4, 0xf6);

        // Simple mic glyph (capsule + stem + base) with a diagonal strike-through.
        let capsule = Rect::from_center_size(center + vec2(0.0, -2.0), vec2(5.0, 8.0));
        ui.painter().rect_filled(capsule, 2.0, icon);
        ui.painter().line_segment(
            [center + vec2(0.0, 2.0), center + vec2(0.0, 5.5)],
            Stroke::new(1.4, icon),
        );
        ui.painter().line_segment(
            [center + vec2(-3.0, 6.0), center + vec2(3.0, 6.0)],
            Stroke::new(1.4, icon),
        );
        ui.painter().line_segment(
            [
                rect.left_top() + vec2(3.0, 3.0),
                rect.right_bottom() - vec2(3.0, 3.0),
            ],
            Stroke::new(1.8, ring),
        );
    }
    response
}

pub fn collapse_toggle(ui: &mut egui::Ui, accent: AccentPalette) -> egui::Response {
    let size = vec2(30.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            Color32::from_rgb(0x2d, 0x31, 0x3c)
        } else {
            BTN_BG
        };
        ui.painter().rect(
            rect,
            6.0,
            fill,
            Stroke::new(1.0, BTN_BORDER),
        );

        // Draw a larger inverted triangle (font-independent).
        let c = rect.center();
        let w = 14.0;
        let h = 9.8;
        let points = vec![
            pos2(c.x - w * 0.5, c.y - h * 0.4),
            pos2(c.x + w * 0.5, c.y - h * 0.4),
            pos2(c.x, c.y + h * 0.6),
        ];
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            accent.base,
            Stroke::NONE,
        ));
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn record_toggle(
    ui: &mut egui::Ui,
    is_recording: bool,
    accent: AccentPalette,
) -> egui::Response {
    let size = 28.0;
    let radius = size / 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());

    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let hovered = response.hovered();

        let (fill, ring) = if is_recording {
            // Active: accent color with brighter hover
            if hovered {
                (accent.hover, accent.base)
            } else {
                (accent.base, accent.ring)
            }
        } else {
            // Idle: muted gray with white ring
            let gray = Color32::from_rgb(0x3a, 0x3d, 0x45);
            let gray_hover = Color32::from_rgb(0x4a, 0x4d, 0x55);
            let idle_ring = Color32::from_rgba_unmultiplied(255, 255, 255, 180);
            if hovered {
                (gray_hover, idle_ring)
            } else {
                (gray, idle_ring)
            }
        };

        // Outer ring
        ui.painter()
            .circle_stroke(center, radius, Stroke::new(1.5, ring));
        // Filled circle
        ui.painter().circle_filled(center, radius - 2.5, fill);

        // Inner icon: square (stop) when recording, circle (record) when idle
        if is_recording {
            let sq = 7.0;
            let sq_rect = egui::Rect::from_center_size(center, vec2(sq, sq));
            ui.painter().rect_filled(sq_rect, 1.5, Color32::WHITE);
        } else {
            ui.painter().circle_filled(center, 5.0, Color32::WHITE);
        }
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn provider_default_button(
    ui: &mut egui::Ui,
    enabled: bool,
    is_default: bool,
    accent: AccentPalette,
) -> egui::Response {
    let size = vec2(22.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();

    let (fill, stroke, dot) = if !enabled {
        (
            Color32::from_rgb(0x42, 0x46, 0x52),
            Color32::from_rgb(0x36, 0x3b, 0x48),
            Color32::from_rgb(0x6b, 0x72, 0x80),
        )
    } else if is_default {
        (
            if hovered {
                accent.hover
            } else {
                accent.base
            },
            accent.ring,
            Color32::WHITE,
        )
    } else if hovered {
        (
            Color32::from_rgb(0x58, 0x62, 0x76),
            Color32::from_rgb(0x8b, 0x96, 0xab),
            Color32::from_rgb(0xc7, 0xcf, 0xde),
        )
    } else {
        (
            Color32::from_rgb(0x4a, 0x50, 0x60),
            Color32::from_rgb(0x6f, 0x7a, 0x92),
            Color32::from_rgb(0x9c, 0xa3, 0xaf),
        )
    };

    ui.painter()
        .circle_filled(rect.center(), rect.width() * 0.46, fill);
    ui.painter().circle_stroke(
        rect.center(),
        rect.width() * 0.46,
        Stroke::new(1.2, stroke),
    );
    ui.painter()
        .circle_filled(rect.center(), rect.width() * 0.16, dot);

    response
}

pub fn provider_validate_button(
    ui: &mut egui::Ui,
    enabled: bool,
    inflight: bool,
    result_ok: Option<bool>,
    accent: AccentPalette,
) -> egui::Response {
    let size = vec2(24.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();

    let (fill, stroke, glyph, glyph_color) = if !enabled {
        (
            Color32::from_rgb(0x4f, 0x55, 0x63),
            Color32::from_rgb(0x3a, 0x3f, 0x4a),
            "",
            TEXT_COLOR,
        )
    } else if inflight {
        (
            if hovered {
                Color32::from_rgb(0x2d, 0x73, 0xff)
            } else {
                Color32::from_rgb(0x25, 0x63, 0xeb)
            },
            Color32::from_rgb(0x1d, 0x4e, 0xd8),
            "...",
            Color32::WHITE,
        )
    } else if result_ok == Some(true) {
        (
            if hovered {
                accent.hover
            } else {
                accent.base
            },
            accent.ring,
            "\u{2713}",
            Color32::WHITE,
        )
    } else if result_ok == Some(false) {
        (
            if hovered {
                Color32::from_rgb(0xe8, 0x34, 0x34)
            } else {
                Color32::from_rgb(0xdc, 0x26, 0x26)
            },
            Color32::from_rgb(0xb9, 0x1c, 0x1c),
            "!",
            Color32::WHITE,
        )
    } else {
        (
            if hovered {
                Color32::from_rgb(0x74, 0x7f, 0x94)
            } else {
                Color32::from_rgb(0x66, 0x6f, 0x80)
            },
            Color32::from_rgb(0x8b, 0x96, 0xab),
            "?",
            Color32::WHITE,
        )
    };

    ui.painter()
        .circle_filled(rect.center(), rect.width() * 0.45, fill);
    ui.painter().circle_stroke(
        rect.center(),
        rect.width() * 0.45,
        Stroke::new(1.0, stroke),
    );
    if result_ok == Some(true) {
        let c = rect.center();
        let w = rect.width() * 0.16;
        let check = Stroke::new(2.0, Color32::WHITE);
        ui.painter().line_segment(
            [
                pos2(c.x - w, c.y),
                pos2(c.x - w * 0.2, c.y + w * 0.8),
            ],
            check,
        );
        ui.painter().line_segment(
            [
                pos2(c.x - w * 0.2, c.y + w * 0.8),
                pos2(c.x + w * 1.2, c.y - w * 0.7),
            ],
            check,
        );
    } else if result_ok == Some(false) {
        let c = rect.center();
        let w = rect.width() * 0.18;
        let cross = Stroke::new(2.0, Color32::WHITE);
        ui.painter()
            .line_segment([pos2(c.x - w, c.y - w), pos2(c.x + w, c.y + w)], cross);
        ui.painter()
            .line_segment([pos2(c.x + w, c.y - w), pos2(c.x - w, c.y + w)], cross);
    } else if !glyph.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(12.0),
            glyph_color,
        );
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn draw_dancing_strings(
    painter: &egui::Painter,
    rect: Rect,
    t: f32,
    live_fft: Option<&[f32; 50]>,
    accent: AccentPalette,
) {
    let is_live = live_fft.is_some();
    let lines = 4usize;
    let samples = 72usize;
    let width = rect.width().max(1.0);
    let base_y = rect.center().y;
    let amp_base = if is_live { 2.4 } else { 1.8 };

    for line in 0..lines {
        let phase = t * (1.6 + line as f32 * 0.28) + line as f32 * 0.9;
        let amp = amp_base + (t * 1.2 + line as f32).sin() * 0.35;
        let color = if is_live {
            Color32::from_rgba_unmultiplied(
                accent.base.r(),
                accent.base.g(),
                accent.base.b(),
                110 + line as u8 * 24,
            )
        } else {
            Color32::from_rgba_unmultiplied(184, 192, 204, 90 + line as u8 * 20)
        };

        let mut points = Vec::with_capacity(samples);
        for i in 0..samples {
            let x = rect.min.x + width * (i as f32 / (samples - 1) as f32);
            let nx = (x - rect.min.x) / width;
            // Pin all strings to the same start/end point.
            let envelope = (std::f32::consts::PI * nx).sin().powf(1.15);
            let wave =
                (nx * std::f32::consts::TAU * (1.2 + line as f32 * 0.25) - phase).sin();
            let y = base_y + wave * amp * envelope;
            points.push(pos2(x, y));
        }
        painter.add(egui::Shape::line(points, Stroke::new(1.0, color)));
    }

    if let Some(fft) = live_fft {
        // Overlay wide equalizer when speaking, tapered near the edges.
        let bar_count = 42usize;
        let gap = 1.1;
        let overlay_w = rect.width() * 0.94;
        let left = rect.center().x - overlay_w * 0.5;
        let bar_w =
            ((overlay_w - gap * (bar_count as f32 - 1.0)) / bar_count as f32).max(1.0);
        for i in 0..bar_count {
            let idx = ((i as f32 / (bar_count - 1) as f32) * 49.0) as usize;
            let boosted = (fft[idx] * 50.0).min(1.0);
            let value = boosted.sqrt().max(0.05);
            let nx = i as f32 / (bar_count - 1) as f32;
            let envelope = (std::f32::consts::PI * nx).sin().powf(0.8);
            let h = (value * rect.height() * (0.45 + envelope * 0.75)).max(1.5);
            let x = left + i as f32 * (bar_w + gap);
            let y = rect.center().y - h * 0.5;
            painter.rect_filled(
                Rect::from_min_size(pos2(x, y), vec2(bar_w, h)),
                1.0,
                Color32::from_rgba_unmultiplied(
                    accent.base.r(),
                    accent.base.g(),
                    accent.base.b(),
                    195,
                ),
            );
        }
    }
}


pub fn section_header(ui: &mut egui::Ui, text: &str) {
    let p = theme_palette(ui.visuals().dark_mode);
    ui.add_space(4.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [
            pos2(rect.min.x, rect.min.y),
            pos2(rect.max.x, rect.min.y),
        ],
        Stroke::new(0.5, p.btn_border),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .strong()
            .color(p.text_muted),
    );
}

pub fn draw_tab_icon(
    painter: &egui::Painter,
    tab_id: &str,
    c: egui::Pos2,
    s: f32,
    color: Color32,
) {
    let stroke = Stroke::new(1.3, color);
    let thin = Stroke::new(1.0, color);

    match tab_id {
        // ── Cloud (provider / API services) ──
        "provider" => {
            let base_y = c.y + s * 0.08;
            painter.rect_filled(
                Rect::from_center_size(pos2(c.x, base_y), vec2(s * 0.72, s * 0.30)),
                s * 0.15,
                color,
            );
            painter.circle_filled(
                pos2(c.x - s * 0.14, base_y - s * 0.15),
                s * 0.20,
                color,
            );
            painter.circle_filled(
                pos2(c.x + s * 0.10, base_y - s * 0.24),
                s * 0.24,
                color,
            );
        }

        // ── Microphone (dictation / voice input) ──
        "dictation" => {
            let top = c.y - s * 0.32;
            let cap_h = s * 0.40;
            let cap_w = s * 0.24;
            let cap_center = pos2(c.x, top + cap_h * 0.5);
            painter.rect_filled(
                Rect::from_center_size(cap_center, vec2(cap_w, cap_h)),
                cap_w * 0.5,
                color,
            );
            let bowl_half = s * 0.22;
            let bowl_top = cap_center.y + cap_h * 0.1;
            let bowl_bot = cap_center.y + cap_h * 0.5 + s * 0.06;
            painter.line_segment(
                [pos2(c.x - bowl_half, bowl_top), pos2(c.x - bowl_half, bowl_bot)],
                thin,
            );
            painter.line_segment(
                [pos2(c.x + bowl_half, bowl_top), pos2(c.x + bowl_half, bowl_bot)],
                thin,
            );
            let stem_top = bowl_bot + s * 0.04;
            painter.line_segment(
                [pos2(c.x - bowl_half, bowl_bot), pos2(c.x, stem_top)],
                thin,
            );
            painter.line_segment(
                [pos2(c.x + bowl_half, bowl_bot), pos2(c.x, stem_top)],
                thin,
            );
            let base_y = c.y + s * 0.38;
            painter.line_segment([pos2(c.x, stem_top), pos2(c.x, base_y)], thin);
            painter.line_segment(
                [pos2(c.x - s * 0.14, base_y), pos2(c.x + s * 0.14, base_y)],
                stroke,
            );
        }

        // ── Terminal prompt >_ (commands) ──
        "commands" => {
            let cx = c.x - s * 0.08;
            let chev_h = s * 0.22;
            let chev_w = s * 0.22;
            painter.line_segment(
                [pos2(cx - chev_w, c.y - chev_h), pos2(cx, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(cx, c.y), pos2(cx - chev_w, c.y + chev_h)],
                stroke,
            );
            painter.line_segment(
                [pos2(cx + s * 0.10, c.y + chev_h), pos2(cx + s * 0.34, c.y + chev_h)],
                stroke,
            );
        }

        // ── Contrast / theme circle (appearance) ──
        "appearance" => {
            let r = s * 0.36;
            painter.circle_stroke(c, r, stroke);
            let n = 16;
            let mut pts = Vec::with_capacity(n + 1);
            for i in 0..=n {
                let t = i as f32 / n as f32;
                let a = -std::f32::consts::FRAC_PI_2 - std::f32::consts::PI * t;
                pts.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
            }
            painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        }

        // ── Bar chart (usage / statistics) ──
        "usage" => {
            let bar_w = s * 0.16;
            let gap = s * 0.06;
            let base_y = c.y + s * 0.32;
            let heights = [s * 0.32, s * 0.52, s * 0.40];
            let total_w = bar_w * 3.0 + gap * 2.0;
            let start_x = c.x - total_w * 0.5;
            for (i, &h) in heights.iter().enumerate() {
                let bx = start_x + i as f32 * (bar_w + gap);
                painter.rect_filled(
                    Rect::from_min_size(pos2(bx, base_y - h), vec2(bar_w, h)),
                    1.0,
                    color,
                );
            }
            painter.line_segment(
                [
                    pos2(start_x - s * 0.04, base_y),
                    pos2(start_x + total_w + s * 0.04, base_y),
                ],
                thin,
            );
        }

        // ── Question mark in circle (FAQ) ──
        "faq" => {
            painter.circle_stroke(c, s * 0.36, stroke);
            painter.text(
                pos2(c.x, c.y + s * 0.01),
                egui::Align2::CENTER_CENTER,
                "?",
                FontId::proportional(s * 0.42),
                color,
            );
        }

        // ── Info "i" in circle (about) ──
        "about" => {
            painter.circle_stroke(c, s * 0.36, stroke);
            painter.text(
                pos2(c.x, c.y + s * 0.02),
                egui::Align2::CENTER_CENTER,
                "i",
                FontId::proportional(s * 0.40),
                color,
            );
        }

        _ => {}
    }
}

/// Renders a settings-tab button with a leading icon and label.
pub fn tab_button(
    ui: &mut egui::Ui,
    tab_id: &str,
    label: &str,
    active: bool,
    accent: AccentPalette,
    width: f32,
) -> egui::Response {
    let p = theme_palette(ui.visuals().dark_mode);
    let height = 32.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, height), Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();

        // ── Background ──
        let fill = if active {
            accent.base
        } else if hovered {
            Color32::from_rgb(0x1e, 0x21, 0x2a)
        } else {
            Color32::TRANSPARENT
        };
        let border = if active {
            accent.ring
        } else if hovered {
            Color32::from_rgb(0x36, 0x3a, 0x44)
        } else {
            p.btn_border
        };
        ui.painter()
            .rect(rect, 6.0, fill, Stroke::new(1.0, border));

        // ── Icon ──
        let icon_center = pos2(rect.min.x + 20.0, rect.center().y);
        let icon_color = if active {
            Color32::from_rgba_unmultiplied(0, 0, 0, 210)
        } else if hovered {
            TEXT_COLOR
        } else {
            p.text_muted
        };
        draw_tab_icon(ui.painter(), tab_id, icon_center, 18.0, icon_color);

        // ── Label ──
        let text_color = if active {
            Color32::BLACK
        } else if hovered {
            TEXT_COLOR
        } else {
            p.text_muted
        };
        let font_size = if active { 13.5 } else { 12.0 };
        let galley = ui.painter().layout_no_wrap(
            label.to_string(),
            FontId::proportional(font_size),
            text_color,
        );
        let text_pos = pos2(
            rect.min.x + 38.0,
            rect.center().y - galley.size().y * 0.5,
        );
        ui.painter().galley(text_pos, galley, text_color);
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// Draws a preset-mode icon (P/I/E) at center `c` within a logical `s`-sized box.
pub fn draw_preset_icon(
    painter: &egui::Painter,
    preset: &str,
    c: egui::Pos2,
    s: f32,
    color: Color32,
) {
    let stroke = Stroke::new(1.4, color);

    match preset {
        // ── Path: chevron prompt  ❯  ──
        "path" => {
            let chev_h = s * 0.30;
            let chev_w = s * 0.26;
            let cx = c.x - chev_w * 0.15;
            painter.line_segment(
                [pos2(cx - chev_w, c.y - chev_h), pos2(cx, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(cx, c.y), pos2(cx - chev_w, c.y + chev_h)],
                stroke,
            );
            // underscore cursor
            painter.line_segment(
                [pos2(cx + s * 0.08, c.y + chev_h), pos2(cx + s * 0.30, c.y + chev_h)],
                stroke,
            );
        }

        // ── Image-in-memory: clipboard with image ──
        "image" => {
            // Clipboard outline
            let board_w = s * 0.54;
            let board_h = s * 0.64;
            let board = Rect::from_center_size(
                pos2(c.x, c.y + s * 0.04),
                vec2(board_w, board_h),
            );
            painter.rect_stroke(board, 2.0, stroke);

            // Clip tab on top
            let tab_w = s * 0.24;
            let tab_h = s * 0.12;
            let tab = Rect::from_center_size(
                pos2(c.x, board.min.y),
                vec2(tab_w, tab_h),
            );
            painter.rect_filled(tab, 1.0, color);

            // Small mountain/landscape inside (image indicator)
            let inner_b = board.min.y + board_h * 0.75;
            let inner_l = board.min.x + s * 0.08;
            let inner_r = board.max.x - s * 0.08;
            let peak1 = pos2(inner_l + (inner_r - inner_l) * 0.35, board.min.y + board_h * 0.42);
            let peak2 = pos2(inner_l + (inner_r - inner_l) * 0.70, board.min.y + board_h * 0.55);
            let thin = Stroke::new(1.0, color);
            painter.line_segment([pos2(inner_l, inner_b), peak1], thin);
            painter.line_segment([peak1, peak2], thin);
            painter.line_segment([peak2, pos2(inner_r, inner_b)], thin);
        }

        // ── Edit: app window with pencil ──
        "edit" => {
            // App window frame
            let win_w = s * 0.58;
            let win_h = s * 0.52;
            let win = Rect::from_center_size(
                pos2(c.x - s * 0.04, c.y + s * 0.06),
                vec2(win_w, win_h),
            );
            painter.rect_stroke(win, 2.0, stroke);

            // Title bar dots
            let dot_r = s * 0.032;
            let dot_y = win.min.y + s * 0.07;
            for i in 0..3 {
                painter.circle_filled(
                    pos2(win.min.x + s * 0.08 + i as f32 * s * 0.07, dot_y),
                    dot_r,
                    color,
                );
            }

            // Small pencil in the bottom-right corner
            let pen_tip = pos2(win.max.x + s * 0.02, win.max.y + s * 0.02);
            let pen_top = pos2(pen_tip.x - s * 0.26, pen_tip.y - s * 0.26);
            painter.line_segment([pen_top, pen_tip], Stroke::new(1.6, color));
            // Pencil nib
            let nib_dir = vec2(0.707, 0.707); // normalized diagonal
            let nib_perp = vec2(-0.707, 0.707);
            let nib_base = pos2(
                pen_tip.x - nib_dir.x * s * 0.08,
                pen_tip.y - nib_dir.y * s * 0.08,
            );
            let nib_l = pos2(
                nib_base.x + nib_perp.x * s * 0.04,
                nib_base.y + nib_perp.y * s * 0.04,
            );
            let nib_r = pos2(
                nib_base.x - nib_perp.x * s * 0.04,
                nib_base.y - nib_perp.y * s * 0.04,
            );
            painter.add(egui::Shape::convex_polygon(
                vec![nib_l, pen_tip, nib_r],
                color,
                Stroke::NONE,
            ));
        }

        _ => {}
    }
}

/// Renders a compact icon-only button for screenshot presets (P/I/E).
pub fn preset_icon_button(
    ui: &mut egui::Ui,
    preset: &str,
    active: bool,
    accent: AccentPalette,
) -> egui::Response {
    let size = vec2(28.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let p = theme_palette(ui.visuals().dark_mode);

        let fill = if active {
            accent.base
        } else if hovered {
            Color32::from_rgb(0x2d, 0x31, 0x3c)
        } else {
            p.btn_bg
        };
        let border = if active {
            accent.ring
        } else if hovered {
            Color32::from_rgb(0x44, 0x48, 0x54)
        } else {
            p.btn_border
        };
        ui.painter()
            .rect(rect, 4.0, fill, Stroke::new(1.0, border));

        let icon_color = if active {
            Color32::WHITE
        } else if hovered {
            TEXT_COLOR
        } else {
            p.text
        };
        draw_preset_icon(ui.painter(), preset, rect.center(), 18.0, icon_color);
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

