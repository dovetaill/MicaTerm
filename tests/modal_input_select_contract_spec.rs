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

#[test]
fn dialog_text_field_contract_only_uses_focus_helpers_outside_text_viewport() {
    let source = fs::read_to_string("ui/components/modal-chrome.slint")
        .expect("read modal chrome source");

    assert!(
        !source.contains(
            "field-touch := TouchArea {\n            width: parent.width;\n            height: parent.height;"
        ),
        "dialog text fields should not restore a full-surface touch overlay above the editable viewport"
    );
    assert!(
        source.contains("trailing-icon-action"),
        "dialog text fields should expose a trailing icon slot for secret reveal affordances"
    );
}
