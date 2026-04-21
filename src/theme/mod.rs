//! Theme entrypoints shared by persisted preferences, Slint bindings, and native effects.

mod spec;

pub use spec::{
    AppThemeSpec, DecorationTheme, SemanticHighlightTheme, SemanticInkTheme, ShellChromeTheme,
    TerminalBackgroundTheme, TerminalCursorTheme, TerminalForegroundTheme, TerminalOverlayTheme,
    TerminalScrollbarTheme, TerminalTheme, ThemeMode, ThemeTerminalPaletteSpec, ThemeVariant,
    app_theme_spec, app_theme_spec_from_terminal_background, terminal_palette_spec,
    terminal_palette_spec_for,
};
