//! Runtime diagnostics snapshots for native terminal surface backends.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeTerminalSurfaceDrawCounters {
    pub background_runs: usize,
    pub monochrome_glyphs: usize,
    pub color_glyphs: usize,
    pub selection_rects: usize,
    pub underline_runs: usize,
    pub cursor_overlay_visible: bool,
    pub ime_preview_active: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeTerminalSurfaceDiagnostics {
    pub hwnd: Option<isize>,
    pub render_target_generation: u64,
    pub last_prepared_frame_token: u64,
    pub last_presented_frame_token: u64,
    pub draw_counters: NativeTerminalSurfaceDrawCounters,
}
