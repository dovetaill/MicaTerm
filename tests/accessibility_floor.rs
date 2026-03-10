use mica_term::theme::accessibility::accessibility_floor;

#[test]
fn accessibility_floor_requires_keyboard_reachability_and_readable_high_contrast() {
    let floor = accessibility_floor();
    assert!(floor.keyboard_reachable);
    assert!(floor.dark_light_focus_clear);
    assert!(floor.high_contrast_safe);
}
