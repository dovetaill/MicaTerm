use mica_term::app::window_recovery::{
    WindowRecoveryAction, WindowRecoveryController, WindowVisibilitySnapshot,
};
use mica_term::app::window_state::WindowPlacementKind;

#[test]
fn entering_restored_from_maximized_requests_redraw() {
    let mut recovery = WindowRecoveryController::default();

    assert_eq!(
        recovery.on_placement_changed(
            WindowPlacementKind::Maximized,
            WindowPlacementKind::Restored,
            WindowVisibilitySnapshot::new(1_296_000, 1_296_000),
            1440,
            900,
        ),
        WindowRecoveryAction::RequestRedraw
    );
}

#[test]
fn entering_restored_from_snapped_can_nudge_once_when_visibility_grows() {
    let mut recovery = WindowRecoveryController::default();

    recovery.on_placement_changed(
        WindowPlacementKind::SnappedLeft,
        WindowPlacementKind::Restored,
        WindowVisibilitySnapshot::new(1_296_000, 640_000),
        1440,
        900,
    );

    assert_eq!(
        recovery.on_visibility_changed(WindowVisibilitySnapshot::new(1_296_000, 960_000), 1440, 900),
        WindowRecoveryAction::NudgeWindowSize {
            width: 1441,
            height: 900,
        }
    );
    assert_eq!(
        recovery.on_resize_ack(1441, 900),
        WindowRecoveryAction::RestoreWindowSize {
            width: 1440,
            height: 900,
        }
    );
}

#[test]
fn steady_restored_window_stays_idle() {
    let mut recovery = WindowRecoveryController::default();

    assert_eq!(
        recovery.on_placement_changed(
            WindowPlacementKind::Restored,
            WindowPlacementKind::Restored,
            WindowVisibilitySnapshot::new(1_296_000, 1_296_000),
            1440,
            900,
        ),
        WindowRecoveryAction::None
    );
}
