use mica_term::app::window_state::{
    Rect, WindowChromeMode, WindowPlacementKind, classify_window_placement,
};

#[test]
fn restored_state_keeps_rounded_chrome() {
    assert_eq!(
        WindowPlacementKind::Restored.chrome_mode(),
        WindowChromeMode::Rounded
    );
}

#[test]
fn maximized_and_snapped_states_use_flat_chrome() {
    for placement in [
        WindowPlacementKind::Maximized,
        WindowPlacementKind::SnappedLeft,
        WindowPlacementKind::SnappedRight,
        WindowPlacementKind::SnappedTop,
        WindowPlacementKind::SnappedBottom,
    ] {
        assert_eq!(placement.chrome_mode(), WindowChromeMode::Flat);
    }
}

#[test]
fn classifier_detects_left_and_right_snap_from_work_area_halves() {
    let work_area = Rect::new(0, 0, 1920, 1080);

    assert_eq!(
        classify_window_placement(Rect::new(0, 0, 960, 1080), work_area, false),
        WindowPlacementKind::SnappedLeft
    );
    assert_eq!(
        classify_window_placement(Rect::new(960, 0, 960, 1080), work_area, false),
        WindowPlacementKind::SnappedRight
    );
}

#[test]
fn classifier_prefers_maximized_when_flag_is_true() {
    let work_area = Rect::new(0, 0, 1920, 1080);

    assert_eq!(
        classify_window_placement(Rect::new(0, 0, 1920, 1080), work_area, true),
        WindowPlacementKind::Maximized
    );
}
