use eframe::egui::Color32;

// Colors matching the original CSS theme
pub const TEXT_COLOR: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x9c, 0xa3, 0xaf);
pub const BTN_BG: Color32 = Color32::from_rgb(0x25, 0x28, 0x30);
pub const BTN_BORDER: Color32 = Color32::from_rgb(0x2c, 0x2f, 0x36);
pub const SETTINGS_BG: Color32 = Color32::from_rgb(0x1c, 0x1f, 0x2a);
pub const RED: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);

pub const PROVIDER_ROWS: &[(&str, &str)] = &[
    ("deepgram", "Deepgram"),
    ("openai", "OpenAI Realtime"),
    ("elevenlabs", "ElevenLabs Realtime"),
    ("assemblyai", "AssemblyAI"),
];

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub text: Color32,
    pub text_muted: Color32,
    pub btn_bg: Color32,
    pub btn_border: Color32,
    pub settings_bg: Color32,
}

#[derive(Clone, Copy)]
pub struct AccentPalette {
    pub id: &'static str,
    pub name: &'static str,
    pub base: Color32,
    pub hover: Color32,
    pub ring: Color32,
    pub tint_bg: Color32,
}

pub fn theme_palette(_dark: bool) -> ThemePalette {
    ThemePalette {
        text: TEXT_COLOR,
        text_muted: TEXT_MUTED,
        btn_bg: BTN_BG,
        btn_border: BTN_BORDER,
        settings_bg: SETTINGS_BG,
    }
}

pub fn accent_palette(id: &str) -> AccentPalette {
    match id {
        "purple" => AccentPalette {
            id: "purple",
            name: "Purple",
            base: Color32::from_rgb(0x9d, 0x6e, 0xc0),
            hover: Color32::from_rgb(0x8a, 0x5a, 0xad),
            ring: Color32::from_rgb(0x74, 0x48, 0x98),
            tint_bg: Color32::from_rgb(0xd0, 0xbf, 0xe0),
        },
        "blue" => AccentPalette {
            id: "blue",
            name: "Blue",
            base: Color32::from_rgb(0x5a, 0x8e, 0xc0),
            hover: Color32::from_rgb(0x4a, 0x7a, 0xac),
            ring: Color32::from_rgb(0x3c, 0x68, 0x98),
            tint_bg: Color32::from_rgb(0xb8, 0xd0, 0xe8),
        },
        "orange" => AccentPalette {
            id: "orange",
            name: "Orange",
            base: Color32::from_rgb(0xd4, 0x93, 0x45),
            hover: Color32::from_rgb(0xc0, 0x80, 0x30),
            ring: Color32::from_rgb(0xa5, 0x6a, 0x20),
            tint_bg: Color32::from_rgb(0xed, 0xcf, 0xa0),
        },
        "pink" => AccentPalette {
            id: "pink",
            name: "Pink",
            base: Color32::from_rgb(0xc4, 0x60, 0x8a),
            hover: Color32::from_rgb(0xb0, 0x4c, 0x78),
            ring: Color32::from_rgb(0x98, 0x3c, 0x65),
            tint_bg: Color32::from_rgb(0xe8, 0xb8, 0xcc),
        },
        _ => AccentPalette {
            id: "green",
            name: "Green",
            base: Color32::from_rgb(0x4d, 0xb8, 0x8a),
            hover: Color32::from_rgb(0x3d, 0xa0, 0x7a),
            ring: Color32::from_rgb(0x2d, 0x88, 0x68),
            tint_bg: Color32::from_rgb(0xa8, 0xdc, 0xc4),
        },
    }
}

pub fn accent_options() -> [AccentPalette; 5] {
    [
        accent_palette("green"),
        accent_palette("purple"),
        accent_palette("blue"),
        accent_palette("orange"),
        accent_palette("pink"),
    ]
}

