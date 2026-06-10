//! Source-level contract coverage for modal input/select hardening.

use std::fs;
use std::path::Path;

#[test]
fn dialog_text_field_contract_exposes_icon_action_for_modal_fields() {
    let source =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome source");

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
    let source =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome source");

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
fn dialog_text_field_contract_declares_local_right_click_pointer_handling() {
    let source =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome source");
    let dialog_text_field_block = source
        .split("export component DialogTextField inherits Rectangle {")
        .nth(1)
        .expect("extract dialog text field block");
    let dialog_text_field_block = dialog_text_field_block
        .split("export component ModalHeaderBar inherits Rectangle {")
        .next()
        .expect("truncate dialog text field block");

    assert!(
        dialog_text_field_block.contains("pointer-event(event) =>")
            && dialog_text_field_block.contains("PointerEventButton.right"),
        "dialog text fields should keep right-click detection local to the field chrome so future text context menus do not depend on a global transparent overlay"
    );
}

#[test]
fn dialog_text_field_contract_exposes_text_context_menu_bridge_metadata() {
    let source =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome source");
    let dialog_text_field_block = source
        .split("export component DialogTextField inherits Rectangle {")
        .nth(1)
        .expect("extract dialog text field block");
    let dialog_text_field_block = dialog_text_field_block
        .split("export component ModalHeaderBar inherits Rectangle {")
        .next()
        .expect("truncate dialog text field block");

    for marker in [
        "in property <string> field-id: \"\";",
        "in property <string> field-kind:",
        "in property <bool> read-only: false;",
        "in property <bool> context-menu-secret:",
        "callback text-context-menu-requested(",
        "field-right-click-hit-area := TouchArea {",
    ] {
        assert!(
            dialog_text_field_block.contains(marker),
            "dialog text fields should expose the text context-menu bridge marker `{marker}`"
        );
    }
}

#[test]
fn bare_snippet_package_input_contract_keeps_right_click_handling_local() {
    let source = fs::read_to_string("ui/components/assets-snippet-package-modal.slint")
        .expect("read snippet package modal source");

    assert!(
        source.contains("PointerEventButton.right"),
        "the bare snippet package TextInput outlier should also declare a local right-click hook instead of remaining outside the shared text-menu bridge"
    );
    assert!(
        source.contains("callback text-context-menu-requested(")
            && source.contains("name-field-right-click-hit-area := TouchArea {"),
        "the bare snippet package TextInput outlier should export the same local bridge contract as DialogTextField"
    );
}

#[test]
fn text_context_menu_overlay_contract_exposes_copy_paste_actions() {
    let source = fs::read_to_string("ui/components/text-context-menu-overlay.slint")
        .expect("read text context menu overlay source");

    for marker in [
        "export component TextContextMenuOverlay inherits Rectangle {",
        "copy-enabled",
        "paste-enabled",
        "select-all-enabled",
        "callback action-invoked(string);",
        "title: \"Copy\"",
        "title: \"Paste\"",
    ] {
        assert!(
            source.contains(marker),
            "text context menu overlay should expose `{marker}` so ordinary text fields can route copy/paste without reusing terminal semantics"
        );
    }
}

#[test]
fn ssh_modal_contract_wires_public_fields_into_text_context_menu_bridge() {
    let source = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal source");

    for marker in [
        "callback text-context-menu-requested(",
        "in property <string> text-context-menu-action-id: \"\";",
        "in property <string> text-context-menu-action-field-id: \"\";",
        "in property <int> text-context-menu-action-sequence: 0;",
        "field-id: \"ssh.name\";",
        "field-id: \"ssh.host\";",
        "field-id: \"ssh.user\";",
        "field-id: \"ssh.port\";",
        "field-id: \"ssh.remark\";",
    ] {
        assert!(
            source.contains(marker),
            "ssh modal should wire public text fields into the shared text context-menu bridge marker `{marker}`"
        );
    }
}

#[test]
fn keychain_identity_modal_contract_wires_public_and_secret_fields_into_text_context_menu_bridge()
{
    let source = fs::read_to_string("ui/components/assets-keychain-identity-modal.slint")
        .expect("read keychain identity modal source");

    for marker in [
        "callback text-context-menu-requested(",
        "in property <string> text-context-menu-action-id: \"\";",
        "in property <string> text-context-menu-action-field-id: \"\";",
        "in property <int> text-context-menu-action-sequence: 0;",
        "field-id: \"keychain-identity.name\";",
        "field-id: \"keychain-identity.username\";",
        "field-id: \"keychain-identity.password\";",
        "context-menu-secret: true;",
        "field-id: \"keychain-identity.remark\";",
        "field-kind: \"comment\";",
    ] {
        assert!(
            source.contains(marker),
            "keychain identity modal should wire text fields into the shared text context-menu bridge marker `{marker}`"
        );
    }
}

#[test]
fn keychain_ssh_key_modal_contract_wires_secret_and_public_material_into_text_context_menu_bridge()
{
    let source = fs::read_to_string("ui/components/assets-keychain-ssh-key-modal.slint")
        .expect("read keychain ssh key modal source");

    for marker in [
        "callback text-context-menu-requested(",
        "in property <string> text-context-menu-action-id: \"\";",
        "in property <string> text-context-menu-action-field-id: \"\";",
        "in property <int> text-context-menu-action-sequence: 0;",
        "field-id: \"keychain-ssh-key.name\";",
        "field-id: \"keychain-ssh-key.private-key\";",
        "field-kind: \"private-key\";",
        "context-menu-secret: true;",
        "field-id: \"keychain-ssh-key.public-key\";",
        "field-kind: \"public-key\";",
        "field-id: \"keychain-ssh-key.fingerprint\";",
        "field-kind: \"fingerprint\";",
    ] {
        assert!(
            source.contains(marker),
            "keychain SSH key modal should wire secret/public material into the shared text context-menu bridge marker `{marker}`"
        );
    }
}

#[test]
fn sync_vault_modal_contract_wires_public_and_secret_fields_into_text_context_menu_bridge() {
    let source = fs::read_to_string("ui/components/sync-vault-modal.slint")
        .expect("read sync vault modal source");

    for marker in [
        "callback text-context-menu-requested(",
        "in property <string> text-context-menu-action-id: \"\";",
        "in property <string> text-context-menu-action-field-id: \"\";",
        "in property <int> text-context-menu-action-sequence: 0;",
        "field-id: \"sync.master-password\";",
        "field-id: \"sync.git-base-url\";",
        "field-id: \"sync.git-namespace\";",
        "field-id: \"sync.git-repository\";",
        "field-id: \"sync.git-branch\";",
        "field-id: \"sync.git-root-path\";",
        "field-id: \"sync.git-https-username\";",
        "field-id: \"sync.git-pat\";",
        "field-id: \"sync.git-ssh-private-key\";",
        "field-id: \"sync.git-ssh-passphrase\";",
        "context-menu-secret: true;",
    ] {
        assert!(
            source.contains(marker),
            "sync vault modal should wire text fields into the shared text context-menu bridge marker `{marker}`"
        );
    }
}

#[test]
fn dialog_select_contract_exposes_modal_local_popup_primitives() {
    let source =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome source");

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

#[test]
fn snippet_modal_contract_uses_dialog_select_field_for_package_picker() {
    let source = fs::read_to_string("ui/components/assets-snippet-modal.slint")
        .expect("read snippet modal source");

    assert!(
        source.contains("DialogSelectField"),
        "snippet modal should consume the shared modal-local select trigger for the package picker"
    );
    assert!(
        !source.contains("ComboBox {"),
        "snippet modal should stop using the stock ComboBox popup inside modal scroll content"
    );
}
