use mica_term::app::window_state::WindowPlacementKind;
use mica_term::app::windows_frame::{
    CaptionButtonGeometry, uses_native_maximize_button_hit_test,
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
