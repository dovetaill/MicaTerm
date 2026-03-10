#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSpec {
    pub accent_name: &'static str,
    pub terminal_is_neutral: bool,
    pub panel_uses_tint: bool,
    pub supports_dark: bool,
    pub supports_light: bool,
}

pub fn theme_spec(mode: ThemeMode) -> ThemeSpec {
    match mode {
        ThemeMode::Dark => ThemeSpec {
            accent_name: "electric-blue",
            terminal_is_neutral: true,
            panel_uses_tint: true,
            supports_dark: true,
            supports_light: true,
        },
        ThemeMode::Light => ThemeSpec {
            accent_name: "electric-blue",
            terminal_is_neutral: true,
            panel_uses_tint: true,
            supports_dark: true,
            supports_light: true,
        },
    }
}
