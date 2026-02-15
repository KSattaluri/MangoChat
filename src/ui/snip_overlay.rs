use eframe::egui;
use egui::{pos2, vec2, Color32, CursorIcon, FontId, Rect, Sense, Stroke, ViewportCommand};
use std::sync::atomic::Ordering;

use super::theme::TEXT_COLOR;
use super::JarvisApp;

impl JarvisApp {
    pub fn trigger_snip(&mut self) {
        if !self.state.screenshot_enabled.load(Ordering::SeqCst) {
            return;
        }
        let cursor = self.state.cursor_pos.lock().ok().and_then(|v| *v);
        let state = self.state.clone();

        match crate::snip::capture_screen(cursor) {
            Ok((img, bounds)) => {
                if let Ok(mut guard) = state.snip_image.lock() {
                    *guard = Some(img);
                }
                self.snip_bounds = Some(bounds);
                self.snip_overlay_active = true;
                self.snip_texture = None;
                self.snip_drag_start = None;
                self.snip_drag_current = None;
                self.snip_focus_pending = true;
            }
            Err(e) => {
                eprintln!("[ui] capture error: {}", e);
                state.snip_active.store(false, Ordering::SeqCst);
            }
        }
    }

    pub fn finish_snip(&mut self, x: u32, y: u32, w: u32, h: u32) {
        let img = {
            let mut guard = self.state.snip_image.lock().unwrap();
            guard.take()
        };
        if let Some(img) = img {
            match crate::snip::crop_and_save(
                &img,
                x,
                y,
                w,
                h,
                self.settings.screenshot_retention_count as usize,
            ) {
                Ok((path, cropped)) => {
                    if self.snip_copy_image {
                        let _ = crate::snip::copy_image_to_clipboard(&cropped);
                    } else {
                        let _ = crate::snip::copy_path_to_clipboard(&path);
                    }
                    if self.snip_edit_after {
                        if let Err(e) = crate::snip::open_in_editor(
                            &path,
                            Some(self.settings.snip_editor_path.as_str()),
                        ) {
                            eprintln!("[snip] editor error: {}", e);
                        }
                        self.snip_edit_after = false;
                    }
                    println!("[snip] saved to {}", path.to_string_lossy());
                }
                Err(e) => eprintln!("[snip] save error: {}", e),
            }
        }
        self.close_snip();
    }

    pub fn cancel_snip(&mut self) {
        if let Ok(mut guard) = self.state.snip_image.lock() {
            *guard = None;
        }
        self.close_snip();
        println!("[snip] cancelled");
    }

    pub fn close_snip(&mut self) {
        self.snip_overlay_active = false;
        self.snip_texture = None;
        self.snip_drag_start = None;
        self.snip_drag_current = None;
        self.snip_bounds = None;
        self.state.snip_active.store(false, Ordering::SeqCst);
    }

    pub fn render_snip_overlay(&mut self, ctx: &egui::Context) {
        if self.snip_focus_pending {
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.snip_focus_pending = false;
        }
        // Load texture on first render
        if self.snip_texture.is_none() {
            if let Ok(guard) = self.state.snip_image.lock() {
                if let Some(ref img) = *guard {
                    let size = [img.width() as usize, img.height() as usize];
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                    self.snip_texture = Some(ctx.load_texture(
                        "snip-screenshot",
                        color_image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }

        ctx.set_cursor_icon(CursorIcon::Crosshair);

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_snip();
            return;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::BLACK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, Sense::drag());

                if response.drag_started() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.snip_drag_start = Some(pos);
                        self.snip_drag_current = Some(pos);
                    }
                }
                if response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.snip_drag_current = Some(pos);
                    }
                }

                let painter = ui.painter();

                // Screenshot background
                if let Some(ref tex) = self.snip_texture {
                    painter.image(
                        tex.id(),
                        rect,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                // Dark tint
                painter.rect_filled(rect, 0.0, Color32::from_black_alpha(100));

                // Selection rectangle
                if let (Some(start), Some(current)) =
                    (self.snip_drag_start, self.snip_drag_current)
                {
                    let sel = Rect::from_two_pos(start, current);
                    if sel.width() > 0.0 && sel.height() > 0.0 {
                        // Bright region (no tint)
                        if let Some(ref tex) = self.snip_texture {
                            let uv = Rect::from_min_max(
                                pos2(
                                    sel.min.x / rect.width(),
                                    sel.min.y / rect.height(),
                                ),
                                pos2(
                                    sel.max.x / rect.width(),
                                    sel.max.y / rect.height(),
                                ),
                            );
                            painter.image(tex.id(), sel, uv, Color32::WHITE);
                        }
                        painter.rect_stroke(
                            sel,
                            0.0,
                            Stroke::new(1.0, Color32::from_white_alpha(230)),
                        );

                        // Dimension label
                        let label =
                            format!("{}x{}", sel.width() as u32, sel.height() as u32);
                        let lpos =
                            pos2(sel.min.x + 8.0, (sel.min.y - 28.0).max(8.0));
                        let galley = painter.layout_no_wrap(
                            label,
                            FontId::proportional(13.0),
                            TEXT_COLOR,
                        );
                        let bg = Rect::from_min_size(
                            lpos,
                            galley.size() + vec2(12.0, 6.0),
                        );
                        painter.rect_filled(
                            bg,
                            3.0,
                            Color32::from_black_alpha(150),
                        );
                        painter.galley(lpos + vec2(6.0, 3.0), galley, TEXT_COLOR);
                    }
                }

                // Hint
                painter.text(
                    pos2(rect.center().x, 24.0),
                    egui::Align2::CENTER_CENTER,
                    "Drag to select. Escape to cancel.",
                    FontId::proportional(14.0),
                    Color32::from_white_alpha(200),
                );

                // Drag end → finish/cancel
                if response.drag_stopped() {
                    if let (Some(s), Some(c)) =
                        (self.snip_drag_start, self.snip_drag_current)
                    {
                        let sel = Rect::from_two_pos(s, c);
                        if sel.width() >= 5.0 && sel.height() >= 5.0 {
                            let sx = self
                                .snip_texture
                                .as_ref()
                                .map(|t| t.size()[0] as f32 / rect.width())
                                .unwrap_or(1.0);
                            let sy = self
                                .snip_texture
                                .as_ref()
                                .map(|t| t.size()[1] as f32 / rect.height())
                                .unwrap_or(1.0);
                            self.finish_snip(
                                (sel.min.x * sx) as u32,
                                (sel.min.y * sy) as u32,
                                (sel.width() * sx) as u32,
                                (sel.height() * sy) as u32,
                            );
                        } else {
                            self.cancel_snip();
                        }
                    } else {
                        self.cancel_snip();
                    }
                }
            });
    }
}
