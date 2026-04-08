//! Windows frame helper coverage for hit-testing and reserved resize bands.

use mica_term::app::terminal_renderer::NativeTerminalSurfaceDiagnostics;
use mica_term::app::window_state::WindowPlacementKind;
use mica_term::app::windows_frame::{
    CaptionButtonGeometry, native_surface_text_renderer_path, point_hits_outer_resize_band,
    uses_native_maximize_button_hit_test,
};

#[test]
fn maximize_button_geometry_detects_points_inside_exported_rect() {
    let geometry = CaptionButtonGeometry {
        x: 100,
        y: 8,
        width: 36,
        height: 36,
    };

    assert!(geometry.contains_window_point(100, 8));
    assert!(geometry.contains_window_point(135, 43));
    assert!(!geometry.contains_window_point(99, 8));
    assert!(!geometry.contains_window_point(136, 43));
    assert!(!geometry.contains_window_point(120, 44));
}

#[test]
fn native_maximize_hit_test_is_disabled_for_all_window_states() {
    for placement in [
        WindowPlacementKind::Restored,
        WindowPlacementKind::Maximized,
        WindowPlacementKind::SnappedLeft,
        WindowPlacementKind::SnappedRight,
        WindowPlacementKind::SnappedTop,
        WindowPlacementKind::SnappedBottom,
        WindowPlacementKind::Unknown,
    ] {
        assert!(!uses_native_maximize_button_hit_test(placement));
    }
}

#[test]
fn frame_adapter_treats_outer_resize_band_as_reserved() {
    assert!(point_hits_outer_resize_band(2, 2, 1200, 800, 10));
    assert!(point_hits_outer_resize_band(1198, 798, 1200, 800, 10));
    assert!(!point_hits_outer_resize_band(80, 24, 1200, 800, 10));
}

#[test]
fn windows_frame_helper_exposes_native_surface_text_renderer_path() {
    let diagnostics = NativeTerminalSurfaceDiagnostics {
        text_renderer_path: Some("directwrite-d2d"),
        ..Default::default()
    };

    assert_eq!(
        native_surface_text_renderer_path(&diagnostics),
        Some("directwrite-d2d")
    );
}
