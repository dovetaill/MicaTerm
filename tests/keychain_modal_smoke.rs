use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::Model;

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

#[test]
fn keychain_asset_rows_map_identity_and_ssh_key_to_dedicated_icons() {
    let asset_row =
        fs::read_to_string("ui/components/asset-node-row.slint").expect("read asset node row");

    assert!(
        asset_row.contains("key-multiple-20-regular.svg"),
        "keychain rows should use a dedicated key icon asset"
    );
    assert!(
        asset_row.contains("root.item-kind == \"identity\""),
        "identity rows should get a dedicated icon mapping"
    );
    assert!(
        asset_row.contains("root.item-kind == \"ssh-key\""),
        "ssh-key rows should get a dedicated icon mapping"
    );
    assert!(
        !asset_row.contains(
            ": root.item-kind == \"snippet\"\n                ? root.snippet-icon\n                : root.ssh-icon;"
        ),
        "identity and ssh-key rows should no longer fall through to the console SSH icon"
    );
}

#[test]
fn new_identity_create_action_opens_modal_before_creating_a_keychain_node() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_sidebar_destination_selected("keychain".into());
    app.invoke_assets_create_action_selected("new-identity".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-keychain-identity");
    assert_eq!(app.get_keychain_asset_items().row_count(), 0);
}
