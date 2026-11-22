#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

use std::sync::atomic::{AtomicBool, Ordering};

static IS_LIGHT: AtomicBool = AtomicBool::new(true);

pub fn set_mode(mode: ThemeMode) {
    IS_LIGHT.store(mode == ThemeMode::Light, Ordering::Relaxed);
}

pub fn mode() -> ThemeMode {
    if IS_LIGHT.load(Ordering::Relaxed) {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}

pub fn palette() -> &'static Palette {
    Palette::of(mode())
}

pub fn rgb(hex: u32) -> gpui::Rgba {
    gpui::rgb(hex)
}

pub fn alpha(hex: u32, a: f32) -> gpui::Rgba {
    let mut color = gpui::rgb(hex);
    color.a = a;
    color
}

pub fn border() -> gpui::Rgba {
    let p = palette();
    alpha(p.border_base, p.border_a)
}

pub fn overlay() -> gpui::Rgba {
    let p = palette();
    alpha(p.border_base, p.overlay_a)
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: u32,
    pub panel: u32,
    pub card: u32,
    pub card_hi: u32,
    pub popover: u32,
    pub text: u32,
    pub text_muted: u32,
    pub text_faint: u32,
    pub border_base: u32,
    pub border_a: f32,
    pub overlay_a: f32,
    pub accent: u32,
    pub primary: u32,
    pub primary_fg: u32,
    pub danger: u32,
    pub live: u32,
    pub blackkey: u32,
    pub blackkey_a: f32,
    pub note_gloss: f32,
}

pub const LIGHT: Palette = Palette {
    bg: 0xf6f6f7,
    panel: 0xffffff,
    card: 0xfdfdfd,
    card_hi: 0xf4f4f5,
    popover: 0xffffff,
    text: 0x18181b,
    text_muted: 0x52525b,
    text_faint: 0x8e8e96,
    border_base: 0x000000,
    border_a: 0.08,
    overlay_a: 0.045,
    accent: 0x2f6fe4,
    primary: 0x18181b,
    primary_fg: 0xffffff,
    danger: 0xdc2626,
    live: 0x16a34a,
    blackkey: 0x000000,
    blackkey_a: 0.035,
    note_gloss: 0.35,
};

pub const DARK: Palette = Palette {
    bg: 0x151518,
    panel: 0x1b1b1f,
    card: 0x232329,
    card_hi: 0x2b2b32,
    popover: 0x232329,
    text: 0xf5f5f6,
    text_muted: 0xc2c2ca,
    text_faint: 0x94949e,
    border_base: 0xffffff,
    border_a: 0.10,
    overlay_a: 0.05,
    accent: 0x7fb3ff,
    primary: 0xfafafa,
    primary_fg: 0x0a0a0b,
    danger: 0xff8b8b,
    live: 0x5ee49a,
    blackkey: 0xffffff,
    blackkey_a: 0.035,
    note_gloss: 0.24,
};

pub const TRACK_COLORS: [u32; 6] = [0x3b82f6, 0x22c55e, 0xf59e0b, 0xa855f7, 0x06b6d4, 0xec4899];

pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;
pub const RADIUS_LG: f32 = 10.0;

pub const FONT_XS: f32 = 10.0;
pub const FONT_SM: f32 = 11.0;
pub const FONT_MD: f32 = 12.0;
pub const FONT_LG: f32 = 14.0;
pub const FONT_XL: f32 = 18.0;

impl Palette {
    pub fn of(mode: ThemeMode) -> &'static Palette {
        match mode {
            ThemeMode::Light => &LIGHT,
            ThemeMode::Dark => &DARK,
        }
    }

    pub fn track_color(&self, index: usize) -> u32 {
        TRACK_COLORS[index % TRACK_COLORS.len()]
    }
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Light => "亮色",
            ThemeMode::Dark => "暗色",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_toggles_both_ways() {
        assert_eq!(ThemeMode::Light.toggled(), ThemeMode::Dark);
        assert_eq!(ThemeMode::Dark.toggled(), ThemeMode::Light);
        assert_eq!(ThemeMode::Light.toggled().toggled(), ThemeMode::Light);
    }

    #[test]
    fn light_and_dark_are_visually_distinct() {
        assert_ne!(LIGHT.bg, DARK.bg);
        assert_ne!(LIGHT.text, DARK.text);
        assert_ne!(LIGHT.panel, DARK.panel);
    }

    #[test]
    fn light_theme_has_dark_text() {
        let lum = |hex: u32| {
            let r = (hex >> 16) & 0xff;
            let g = (hex >> 8) & 0xff;
            let b = hex & 0xff;
            r * 299 + g * 587 + b * 114
        };
        assert!(lum(LIGHT.bg) > lum(LIGHT.text), "亮色主题应浅底深字");
        assert!(lum(DARK.bg) < lum(DARK.text), "暗色主题应深底浅字");
    }

    #[test]
    fn contrast_is_readable_in_both_modes() {
        let lum = |hex: u32| {
            let r = ((hex >> 16) & 0xff) as f32 / 255.0;
            let g = ((hex >> 8) & 0xff) as f32 / 255.0;
            let b = (hex & 0xff) as f32 / 255.0;
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };

        for p in [&LIGHT, &DARK] {
            let bg_lum = lum(p.bg);
            let text_lum = lum(p.text);
            let ratio = (bg_lum.max(text_lum) + 0.05) / (bg_lum.min(text_lum) + 0.05);
            assert!(ratio >= 7.0, "正文对比度应达 AAA，实际 {ratio:.1}");
        }
    }

    #[test]
    fn track_colors_cycle() {
        let p = LIGHT;
        assert_eq!(p.track_color(0), TRACK_COLORS[0]);
        assert_eq!(p.track_color(TRACK_COLORS.len()), TRACK_COLORS[0]);
        assert_eq!(p.track_color(TRACK_COLORS.len() + 1), TRACK_COLORS[1]);
    }

    #[test]
    fn palette_of_matches_constants() {
        assert_eq!(Palette::of(ThemeMode::Light).bg, LIGHT.bg);
        assert_eq!(Palette::of(ThemeMode::Dark).bg, DARK.bg);
    }
}
