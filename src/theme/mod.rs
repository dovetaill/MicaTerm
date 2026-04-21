//! Theme entrypoints shared by persisted preferences, Slint bindings, and native effects.

mod spec;

pub use spec::{
    AppThemeSpec, DecorationTheme, SemanticHighlightTheme, ShellChromeTheme,
    TerminalBackgroundTheme, TerminalCursorTheme, TerminalForegroundTheme, TerminalOverlayTheme,
    TerminalScrollbarTheme, TerminalTheme, ThemeMode, ThemeTerminalPaletteSpec, ThemeVariant,
    app_theme_spec, terminal_palette_spec, terminal_palette_spec_for,
};
