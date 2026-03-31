use std::fs;

#[test]
fn keychain_panel_contract_replaces_placeholder_copy_with_tree_projection() {
    let sidebar = fs::read_to_string("ui/shell/assets-sidebar.slint").expect("read assets sidebar");

    assert!(
        sidebar.contains("in property <[ConsoleAssetItem]> keychain-asset-items: [];"),
        "keychain panel should expose a dedicated projected row model"
    );
    assert!(
        sidebar.contains(
            "if root.active-panel == \"keychain\" && root.keychain-asset-items.length == 0"
        ),
        "keychain panel should keep an explicit empty state once explorer rows are wired"
    );
    assert!(
        sidebar.contains(
            "if root.active-panel == \"keychain\" && root.keychain-asset-items.length > 0"
        ),
        "keychain panel should render a keychain list host when rows exist"
    );
    assert!(
        !sidebar.contains("text: \"Accounts, identities, SSH keys\""),
        "task 6 should remove the keychain placeholder copy"
    );
}

#[test]
fn keychain_identity_modal_contract_exposes_identity_fields_and_auth_choices() {
    let identity_modal = fs::read_to_string("ui/components/assets-keychain-identity-modal.slint")
        .expect("read keychain identity modal");

    assert!(
        identity_modal.contains("Identity"),
        "identity modal should declare its dialog title"
    );
    assert!(
        identity_modal.contains("Name"),
        "identity modal should expose a name field"
    );
    assert!(
        identity_modal.contains("Username"),
        "identity modal should expose a username field"
    );
    assert!(
        identity_modal.contains("Password"),
        "identity modal should expose password auth"
    );
    assert!(
        identity_modal.contains("SSH Key"),
        "identity modal should expose SSH key auth"
    );
}

#[test]
fn keychain_ssh_key_modal_contract_exposes_import_generate_and_copy_actions() {
    let ssh_key_modal = fs::read_to_string("ui/components/assets-keychain-ssh-key-modal.slint")
        .expect("read keychain ssh key modal");

    for required_copy in [
        "Private Key",
        "Import Private Key",
        "Paste Private Key",
        "Public Key",
        "Import Public Key",
        "Paste Public Key",
        "Generate Key Pair",
        "Copy Public Key",
    ] {
        assert!(
            ssh_key_modal.contains(required_copy),
            "ssh key modal should include `{required_copy}`"
        );
    }
}
