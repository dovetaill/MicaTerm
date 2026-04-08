use std::fs;

use mica_term::app::terminal_renderer::NativeTerminalSurfaceDiagnostics;
use mica_term::app::terminal_renderer::diagnostics::{
    NativeTerminalSurfaceGlyphBoundsTrace, NativeTerminalSurfaceWindowsTextDiagnostics,
};
use mica_term::app::windows_frame::{
    native_surface_baseline_px, native_surface_font_chain, native_surface_glyph_bounds_trace,
    native_surface_pixel_alignment, native_surface_render_target_alpha_mode,
    native_surface_scale_factor_percent, native_surface_text_antialias_mode,
};

#[test]
fn diagnostics_contract_exposes_windows_text_rendering_trace_fields() {
    let diagnostics_source = fs::read_to_string("src/app/terminal_renderer/diagnostics.rs")
        .expect("read diagnostics source");

    for expected in [
        "pub struct NativeTerminalSurfaceWindowsTextDiagnostics",
        "pub struct NativeTerminalSurfaceGlyphBoundsTrace",
        "pub windows_text: Option<NativeTerminalSurfaceWindowsTextDiagnostics>",
        "pub scheduled_present_count: u64",
        "pub host_redraw_request_count: u64",
        "pub host_redraw_replay_count: u64",
        "pub text_antialias_mode: Option<&'static str>",
        "pub render_target_alpha_mode: Option<&'static str>",
        "pub font_chain: Vec<String>",
        "pub baseline_px: Option<i32>",
        "pub pixel_alignment: Option<&'static str>",
        "pub dpi_x: Option<u32>",
        "pub dpi_y: Option<u32>",
        "pub scale_factor_percent: Option<u32>",
        "pub glyph_bounds: Vec<NativeTerminalSurfaceGlyphBoundsTrace>",
        "pub screen_left_px: i32",
        "pub visible_width_px: u32",
    ] {
        assert!(
            diagnostics_source.contains(expected),
            "Task 6 diagnostics contract should expose `{expected}` so Windows text rendering regressions can be inspected without reverse-engineering backend state"
        );
    }
}

#[test]
fn windows_backend_source_publishes_windows_text_rendering_snapshot() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows backend");

    for expected in [
        "GetDpiForWindow",
        "windows_text: Some(",
        "text_antialias_mode:",
        "render_target_alpha_mode:",
        "font_chain:",
        "baseline_px:",
        "pixel_alignment:",
        "scale_factor_percent:",
        "glyph_bounds:",
    ] {
        assert!(
            windows_backend_source.contains(expected),
            "windows backend should publish `{expected}` in the diagnostics snapshot so text AA, alpha, font, and bounds traces stay observable during native rendering regressions"
        );
    }
}

#[test]
fn windows_frame_helpers_project_windows_text_rendering_diagnostics() {
    let diagnostics = NativeTerminalSurfaceDiagnostics {
        windows_text: Some(NativeTerminalSurfaceWindowsTextDiagnostics {
            text_antialias_mode: Some("cleartype"),
            render_target_alpha_mode: Some("ignore"),
            font_chain: vec!["Cascadia Mono".into(), "Sarasa Term SC".into()],
            baseline_px: Some(14),
            pixel_alignment: Some("pixel-snapped"),
            dpi_x: Some(144),
            dpi_y: Some(144),
            scale_factor_percent: Some(150),
            glyph_bounds: vec![NativeTerminalSurfaceGlyphBoundsTrace {
                glyph_id: 87,
                row: 1,
                start_col: 2,
                end_col: 3,
                atlas_slot: 9,
                screen_left_px: 48,
                screen_top_px: 24,
                screen_width_px: 17,
                screen_height_px: 20,
                visible_left_px: -1,
                visible_top_px: -2,
                visible_width_px: 18,
                visible_height_px: 21,
            }],
        }),
        ..Default::default()
    };

    assert_eq!(
        native_surface_text_antialias_mode(&diagnostics),
        Some("cleartype")
    );
    assert_eq!(
        native_surface_render_target_alpha_mode(&diagnostics),
        Some("ignore")
    );
    assert_eq!(native_surface_baseline_px(&diagnostics), Some(14));
    assert_eq!(
        native_surface_pixel_alignment(&diagnostics),
        Some("pixel-snapped")
    );
    assert_eq!(native_surface_scale_factor_percent(&diagnostics), Some(150));
    assert_eq!(
        native_surface_font_chain(&diagnostics).map(|chain| chain.to_vec()),
        Some(vec![
            "Cascadia Mono".to_string(),
            "Sarasa Term SC".to_string()
        ])
    );
    assert_eq!(
        native_surface_glyph_bounds_trace(&diagnostics)
            .expect("glyph bounds trace")
            .first()
            .map(|trace| trace.screen_left_px),
        Some(48)
    );
}

#[test]
fn bootstrap_source_installs_native_terminal_diagnostics_trace_hook() {
    let bootstrap_source =
        fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    for expected in [
        "fn trace_workspace_native_terminal_diagnostics(",
        "native_surface_text_antialias_mode(&diagnostics)",
        "native_surface_render_target_alpha_mode(&diagnostics)",
        "native_surface_font_chain(&diagnostics)",
        "native_surface_glyph_bounds_trace(&diagnostics)",
        "scheduled_present_count = diagnostics.scheduled_present_count",
        "host_redraw_request_count = diagnostics.host_redraw_request_count",
        "host_redraw_replay_count = diagnostics.host_redraw_replay_count",
        "window_scale_factor(window)",
    ] {
        assert!(
            bootstrap_source.contains(expected),
            "bootstrap should wire `{expected}` into the native terminal diagnostics hook so Task 6 observability is reachable through the existing workspace bridge"
        );
    }
}
