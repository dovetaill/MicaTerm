//! Source-level contract coverage for modal input/select hardening.

use std::fs;
use std::path::Path;

#[test]
fn dialog_text_field_contract_exposes_icon_action_for_modal_fields() {
    let source = fs::read_to_string("ui/components/modal-chrome.slint")
        .expect("read modal chrome source");

    assert!(
        source.contains("export component DialogFieldIconAction inherits Rectangle {"),
        "modal chrome should export a reusable trailing icon action component for modal fields"
    );
}

#[test]
fn fluent_eye_assets_exist_for_modal_secret_toggle() {
    for path in [
        "assets/icons/fluent/eye-20-regular.svg",
        "assets/icons/fluent/eye-off-20-regular.svg",
    ] {
        assert!(Path::new(path).exists(), "missing {path}");
    }
}
