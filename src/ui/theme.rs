use eframe::egui::Color32;

// Colors matching the original CSS theme
pub const TEXT_COLOR: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x9c, 0xa3, 0xaf);
pub const BTN_BG: Color32 = Color32::from_rgb(0x25, 0x28, 0x30);
pub const BTN_BORDER: Color32 = Color32::from_rgb(0x2c, 0x2f, 0x36);
pub const SETTINGS_BG: Color32 = Color32::from_rgb(0x15, 0x18, 0x21);
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
            base: Color32::from_rgb(0xa8, 0x55, 0xf7),
            hover: Color32::from_rgb(0x93, 0x3d, 0xe8),
            ring: Color32::from_rgb(0x7e, 0x22, 0xce),
            tint_bg: Color32::from_rgb(0xdb, 0xbf, 0xff),
        },
        "blue" => AccentPalette {
            id: "blue",
            name: "Blue",
            base: Color32::from_rgb(0x3b, 0x82, 0xf6),
            hover: Color32::from_rgb(0x25, 0x63, 0xeb),
            ring: Color32::from_rgb(0x1d, 0x4e, 0xd8),
            tint_bg: Color32::from_rgb(0xbf, 0xdb, 0xfe),
        },
        "orange" => AccentPalette {
            id: "orange",
            name: "Orange",
            base: Color32::from_rgb(0xf5, 0x9e, 0x0b),
            hover: Color32::from_rgb(0xea, 0x8a, 0x00),
            ring: Color32::from_rgb(0xc2, 0x41, 0x0c),
            tint_bg: Color32::from_rgb(0xfe, 0xd7, 0xaa),
        },
        "pink" => AccentPalette {
            id: "pink",
            name: "Pink",
            base: Color32::from_rgb(0xec, 0x48, 0x99),
            hover: Color32::from_rgb(0xdb, 0x27, 0x7d),
            ring: Color32::from_rgb(0xbe, 0x18, 0x5d),
            tint_bg: Color32::from_rgb(0xfb, 0xbf, 0xdc),
        },
        _ => AccentPalette {
            id: "green",
            name: "Green",
            base: Color32::from_rgb(0x36, 0xd3, 0x99),
            hover: Color32::from_rgb(0x16, 0xa3, 0x4a),
            ring: Color32::from_rgb(0x16, 0xa3, 0x4a),
            tint_bg: Color32::from_rgb(0x9f, 0xef, 0xcd),
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

