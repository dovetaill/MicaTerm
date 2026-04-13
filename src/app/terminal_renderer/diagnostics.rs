//! Runtime diagnostics snapshots for native terminal surface backends.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeTerminalSurfaceGlyphBoundsTrace {
    pub glyph_id: u32,
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub atlas_slot: u32,
    pub screen_left_px: i32,
    pub screen_top_px: i32,
    pub screen_width_px: u32,
    pub screen_height_px: u32,
    pub visible_left_px: i32,
    pub visible_top_px: i32,
    pub visible_width_px: u32,
    pub visible_height_px: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeTerminalSurfaceWindowsTextDiagnostics {
    pub text_antialias_mode: Option<&'static str>,
    pub render_target_alpha_mode: Option<&'static str>,
    pub rendering_params_source: Option<&'static str>,
    pub rendering_mode: Option<&'static str>,
    pub pixel_geometry: Option<&'static str>,
    pub gamma_per_mille: Option<u32>,
    pub enhanced_contrast_per_mille: Option<u32>,
    pub clear_type_level_per_mille: Option<u32>,
    pub fallback_reason: Option<&'static str>,
    pub font_chain: Vec<String>,
    pub baseline_px: Option<i32>,
    pub pixel_alignment: Option<&'static str>,
    pub dpi_x: Option<u32>,
    pub dpi_y: Option<u32>,
    pub scale_factor_percent: Option<u32>,
    pub glyph_bounds: Vec<NativeTerminalSurfaceGlyphBoundsTrace>,
}

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
    pub host_hwnd: Option<isize>,
    pub host_surface_hwnd: Option<isize>,
    pub host_surface_visible: Option<bool>,
    pub host_surface_ready: Option<bool>,
    pub text_renderer_path: Option<&'static str>,
    pub windows_text: Option<NativeTerminalSurfaceWindowsTextDiagnostics>,
    pub render_target_generation: u64,
    pub last_prepared_frame_token: u64,
    pub last_presented_frame_token: u64,
    pub scheduled_present_count: u64,
    pub host_redraw_request_count: u64,
    pub host_redraw_replay_count: u64,
    pub draw_counters: NativeTerminalSurfaceDrawCounters,
}
