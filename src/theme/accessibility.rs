#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessibilityFloor {
    pub keyboard_reachable: bool,
    pub dark_light_focus_clear: bool,
    pub high_contrast_safe: bool,
}

pub fn accessibility_floor() -> AccessibilityFloor {
    AccessibilityFloor {
        keyboard_reachable: true,
        dark_light_focus_clear: true,
        high_contrast_safe: true,
    }
}
