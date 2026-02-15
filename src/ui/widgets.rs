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

pub fn field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    password: bool,
) -> egui::Response {
    let p = theme_palette(ui.visuals().dark_mode);
    ui.label(egui::RichText::new(label).size(11.0).color(p.text_muted));
    // Match text input surface to active theme.
    ui.visuals_mut().extreme_bg_color = if ui.visuals().dark_mode {
        Color32::from_rgb(0x1a, 0x1d, 0x24)
    } else {
        Color32::from_rgb(0xff, 0xff, 0xff)
    };
    let mut te = egui::TextEdit::singleline(value)
        .font(FontId::proportional(12.0))
        .text_color(p.text)
        .desired_width(f32::INFINITY);
    if password {
        te = te.password(true);
    }
    ui.add(te)
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
