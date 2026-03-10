use mica_term::status::{ConnectionState, motion_spec, status_spec};

#[test]
fn connecting_uses_low_noise_feedback() {
    let status = status_spec(ConnectionState::Connecting);
    assert!(status.animated);
    assert!(!status.escalates_to_page_overlay);
}

#[test]
fn right_panel_motion_matches_the_design_duration() {
    let motion = motion_spec();
    assert_eq!(motion.drawer_open_ms, 220);
    assert_eq!(motion.welcome_transition_ms, 160);
}
