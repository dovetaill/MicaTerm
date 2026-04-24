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

#[test]
fn dialog_select_contract_exposes_modal_local_popup_primitives() {
    let source = fs::read_to_string("ui/components/modal-chrome.slint")
        .expect("read modal chrome source");

    assert!(
        source.contains("export component DialogSelectField inherits Rectangle {"),
        "modal chrome should export a shared select trigger primitive for modal-local overlays"
    );
    assert!(
        source.contains("export component DialogSelectPopup inherits Rectangle {"),
        "modal chrome should export a shared popup primitive for modal-local overlays"
    );
    assert!(
        source.contains("in property <bool> open: false;"),
        "shared select primitives should expose open state so modal owners can control overlay visibility"
    );
    assert!(
        source.contains("callback option-selected(string);"),
        "shared select popup should report the chosen label back to the modal owner"
    );
    assert!(
        source.contains("callback dismiss-requested();"),
        "shared select popup should expose an explicit dismiss callback for Esc and outside-click owners"
    );
    assert!(
        source.contains("callback move-highlight-requested(int);"),
        "shared select popup should expose a minimal highlight navigation callback"
    );
    assert!(
        source.contains("in property <length> popup-max-height"),
        "shared select popup should expose a max-height contract for bounded modal layouts"
    );
    assert!(
        !source.contains("ComboBox {"),
        "shared modal select primitives should not wrap the stock ComboBox popup"
    );
}

#[test]
fn ssh_modal_contract_uses_dialog_select_field_instead_of_combobox() {
    let source = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal source");

    assert!(
        source.contains("DialogSelectField"),
        "ssh modal should consume the shared modal-local select trigger"
    );
    assert!(
        !source.contains("ComboBox {"),
        "ssh modal should stop using the stock ComboBox popup inside modal scroll content"
    );
}
