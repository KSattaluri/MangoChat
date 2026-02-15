use eframe::egui;
use crate::ui::theme::*;
use crate::ui::widgets::section_header;
use crate::ui::MangoChatApp;

pub fn render_about(app: &MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let accent = app.current_accent();
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

            section_header(ui, "Keyboard Shortcuts");
            for (key, desc) in [
                ("Right Ctrl (hold)", "Push-to-talk dictation"),
                ("Right Ctrl (tap)", "Quick toggle recording"),
                ("Escape", "Cancel snip overlay"),
            ] {
                ui.columns(2, |cols| {
                    cols[0].label(
                        egui::RichText::new(key)
                            .size(11.0)
                            .strong()
                            .color(TEXT_COLOR),
                    );
                    cols[1].label(
                        egui::RichText::new(desc)
                            .size(11.0)
                            .color(TEXT_MUTED),
                    );
                });
            }

            section_header(ui, "Voice Commands");
            for (cmd, desc) in [
                ("\"back\"", "Delete previous word"),
                ("\"new line\"", "Insert line break"),
                ("\"new paragraph\"", "Double line break"),
                ("\"undo\" / \"redo\"", "Undo or redo"),
                ("\"open <trigger>\"", "Open URL (see Commands tab)"),
            ] {
                ui.columns(2, |cols| {
                    cols[0].label(
                        egui::RichText::new(cmd)
                            .size(11.0)
                            .color(accent.base),
                    );
                    cols[1].label(
                        egui::RichText::new(desc)
                            .size(11.0)
                            .color(TEXT_MUTED),
                    );
                });
            }
        });
}

pub fn render_faq(_app: &MangoChatApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
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
            ui.add_space(6.0);

            for (q, a) in [
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
            ] {
                egui::CollapsingHeader::new(
                    egui::RichText::new(q)
                        .size(11.0)
                        .color(TEXT_COLOR),
                )
                .default_open(false)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(a)
                                .size(11.0)
                                .color(TEXT_MUTED),
                        )
                        .wrap(),
                    );
                });
            }
        });
}

