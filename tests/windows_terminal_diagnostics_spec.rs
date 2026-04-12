use std::fs;

use mica_term::app::terminal_renderer::NativeTerminalSurfaceDiagnostics;
use mica_term::app::terminal_renderer::diagnostics::{
    NativeTerminalSurfaceGlyphBoundsTrace, NativeTerminalSurfaceWindowsTextDiagnostics,
};
use mica_term::app::windows_frame::{
    native_surface_baseline_px, native_surface_clear_type_level_per_mille,
    native_surface_enhanced_contrast_per_mille, native_surface_font_chain,
    native_surface_gamma_per_mille, native_surface_glyph_bounds_trace, native_surface_host_hwnd,
    native_surface_pixel_alignment, native_surface_pixel_geometry,
    native_surface_render_target_alpha_mode, native_surface_render_target_ready,
    native_surface_rendering_mode, native_surface_rendering_params_source,
    native_surface_scale_factor_percent, native_surface_surface_hwnd,
    native_surface_surface_visible, native_surface_text_antialias_mode,
    native_surface_text_fallback_reason,
};

#[test]
fn diagnostics_contract_exposes_windows_text_rendering_trace_fields() {
    let diagnostics_source = fs::read_to_string("src/app/terminal_renderer/diagnostics.rs")
        .expect("read diagnostics source");

    for expected in [
        "pub struct NativeTerminalSurfaceWindowsTextDiagnostics",
        "pub struct NativeTerminalSurfaceGlyphBoundsTrace",
        "pub windows_text: Option<NativeTerminalSurfaceWindowsTextDiagnostics>",
        "pub host_hwnd: Option<isize>",
        "pub surface_hwnd: Option<isize>",
        "pub surface_visible: Option<bool>",
        "pub render_target_ready: Option<bool>",
        "pub scheduled_present_count: u64",
        "pub host_redraw_request_count: u64",
        "pub host_redraw_replay_count: u64",
        "pub text_antialias_mode: Option<&'static str>",
        "pub render_target_alpha_mode: Option<&'static str>",
        "pub rendering_params_source: Option<&'static str>",
        "pub rendering_mode: Option<&'static str>",
        "pub pixel_geometry: Option<&'static str>",
        "pub gamma_per_mille: Option<u32>",
        "pub enhanced_contrast_per_mille: Option<u32>",
        "pub clear_type_level_per_mille: Option<u32>",
        "pub fallback_reason: Option<&'static str>",
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
        "host_hwnd:",
        "surface_hwnd:",
        "surface_visible:",
        "render_target_ready:",
        "text_antialias_mode:",
        "render_target_alpha_mode:",
        "rendering_params_source:",
        "rendering_mode:",
        "pixel_geometry:",
        "gamma_per_mille:",
        "enhanced_contrast_per_mille:",
        "clear_type_level_per_mille:",
        "fallback_reason:",
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
fn windows_backend_source_projects_child_local_glyph_bounds_into_host_coordinates() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows backend");

    assert!(
        windows_backend_source.contains("self.window_rect.x.saturating_add(draw.dest_x_px)")
            && windows_backend_source.contains("self.window_rect.y.saturating_add(draw.dest_y_px)"),
        "windows backend diagnostics should project child-local glyph draws back into host-window coordinates so child-HWND rendering stays debuggable from the shell frame"
    );
}

#[test]
fn windows_frame_helpers_project_windows_text_rendering_diagnostics() {
    let diagnostics = NativeTerminalSurfaceDiagnostics {
        host_hwnd: Some(0x1234),
        surface_hwnd: Some(0x5678),
        surface_visible: Some(true),
        render_target_ready: Some(true),
        windows_text: Some(NativeTerminalSurfaceWindowsTextDiagnostics {
            text_antialias_mode: Some("cleartype"),
            render_target_alpha_mode: Some("ignore"),
            rendering_params_source: Some("monitor-custom"),
            rendering_mode: Some("cleartype-natural"),
            pixel_geometry: Some("rgb"),
            gamma_per_mille: Some(2200),
            enhanced_contrast_per_mille: Some(750),
            clear_type_level_per_mille: Some(1000),
            fallback_reason: Some("font-face-unresolved"),
            font_chain: vec!["JetBrains Mono".into(), "Sarasa Term SC".into()],
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

    assert_eq!(native_surface_host_hwnd(&diagnostics), Some(0x1234));
    assert_eq!(native_surface_surface_hwnd(&diagnostics), Some(0x5678));
    assert_eq!(native_surface_surface_visible(&diagnostics), Some(true));
    assert_eq!(native_surface_render_target_ready(&diagnostics), Some(true));
    assert_eq!(
        native_surface_text_antialias_mode(&diagnostics),
        Some("cleartype")
    );
    assert_eq!(
        native_surface_render_target_alpha_mode(&diagnostics),
        Some("ignore")
    );
    assert_eq!(
        native_surface_rendering_params_source(&diagnostics),
        Some("monitor-custom")
    );
    assert_eq!(
        native_surface_rendering_mode(&diagnostics),
        Some("cleartype-natural")
    );
    assert_eq!(native_surface_pixel_geometry(&diagnostics), Some("rgb"));
    assert_eq!(native_surface_gamma_per_mille(&diagnostics), Some(2200));
    assert_eq!(
        native_surface_enhanced_contrast_per_mille(&diagnostics),
        Some(750)
    );
    assert_eq!(
        native_surface_clear_type_level_per_mille(&diagnostics),
        Some(1000)
    );
    assert_eq!(
        native_surface_text_fallback_reason(&diagnostics),
        Some("font-face-unresolved")
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
            "JetBrains Mono".to_string(),
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
fn bootstrap_source_no_longer_installs_native_terminal_diagnostics_debug_hook() {
    let bootstrap_source =
        fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    for unexpected in [
        "fn trace_workspace_native_terminal_diagnostics(",
        "workspace native terminal diagnostics snapshot",
        "native_surface_text_antialias_mode(&diagnostics)",
        "native_surface_render_target_alpha_mode(&diagnostics)",
        "native_surface_rendering_params_source(&diagnostics)",
        "native_surface_rendering_mode(&diagnostics)",
        "native_surface_pixel_geometry(&diagnostics)",
        "native_surface_gamma_per_mille(&diagnostics)",
        "native_surface_enhanced_contrast_per_mille(&diagnostics)",
        "native_surface_clear_type_level_per_mille(&diagnostics)",
        "native_surface_font_chain(&diagnostics)",
        "native_surface_glyph_bounds_trace(&diagnostics)",
        "modal_blocking_native_surface = workspace_blocks_native_terminal_surface(window)",
        "host_hwnd = diagnostics.host_hwnd.unwrap_or_default()",
        "surface_hwnd = diagnostics.surface_hwnd.unwrap_or_default()",
        "surface_visible = diagnostics.surface_visible.unwrap_or(false)",
        "render_target_ready = diagnostics.render_target_ready.unwrap_or(false)",
        "scheduled_present_count = diagnostics.scheduled_present_count",
        "host_redraw_request_count = diagnostics.host_redraw_request_count",
        "host_redraw_replay_count = diagnostics.host_redraw_replay_count",
    ] {
        assert!(
            !bootstrap_source.contains(unexpected),
            "bootstrap should no longer keep the noisy retained-native diagnostics debug hook now that the transparency issue is fixed and normal launches should not spam per-frame diagnostics"
        );
    }
}

#[test]
fn bootstrap_source_hides_retained_native_child_surface_while_blocking_modals_are_open() {
    let bootstrap_source =
        fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    for unexpected in [
        "fn workspace_blocks_native_terminal_surface(window: &AppWindow) -> bool",
        "window.get_sync_modal_open()",
        "window.get_settings_modal_open()",
        "window.get_asset_modal_open()",
        "window.get_asset_rename_modal_open()",
        "window.get_asset_delete_confirm_modal_open()",
        "window.get_ssh_host_key_modal_open()",
        "window.get_workspace_paste_warning_modal_open()",
        "window.get_open_saved_ssh_modal_open()",
        "window.get_sftp_remote_file_modal_open()",
        "if workspace_blocks_native_terminal_surface(window) {",
        "return NativeTerminalSurfaceRect::default();",
    ] {
        assert!(
            bootstrap_source.contains(unexpected),
            "bootstrap should keep `{unexpected}` so retained-native child HWNDs are hidden whenever a blocking modal takes the workspace input plane"
        );
    }
}

#[test]
fn modal_state_sync_paths_force_immediate_native_surface_geometry_refresh() {
    let bootstrap_source =
        fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");
    let assets_keychain_source = fs::read_to_string("src/app/bootstrap/assets_keychain.rs")
        .expect("read assets keychain source");
    let shell_chrome_source =
        fs::read_to_string("src/app/bootstrap/shell_chrome.rs").expect("read shell chrome source");
    let sftp_source = fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read sftp source");
    let windowing_source =
        fs::read_to_string("src/app/bootstrap/windowing.rs").expect("read windowing source");

    assert!(
        bootstrap_source
            .contains("window.set_open_saved_ssh_modal_open(state.saved_ssh_picker_open());")
            && bootstrap_source
                .contains("sync_workspace_native_terminal_surface_geometry(window);"),
        "bootstrap should refresh retained-native child HWND geometry immediately after syncing the open-saved-ssh modal so a stale child surface cannot stay above the blocking overlay until the next terminal frame"
    );

    for (source, expected, label) in [
        (
            &assets_keychain_source,
            "super::sync_workspace_native_terminal_surface_geometry(window);",
            "asset modal sync",
        ),
        (
            &shell_chrome_source,
            "super::sync_workspace_native_terminal_surface_geometry(window);",
            "settings modal sync",
        ),
        (
            &sftp_source,
            "super::sync_workspace_native_terminal_surface_geometry(window);",
            "sftp remote file modal sync",
        ),
        (
            &windowing_source,
            "super::sync_workspace_native_terminal_surface_geometry(window);",
            "windowing modal sync",
        ),
    ] {
        assert!(
            source.contains(expected),
            "{label} should force an immediate retained-native geometry refresh when blocking modal state changes so child HWND visibility does not wait for a later layout or terminal-present tick"
        );
    }
}
