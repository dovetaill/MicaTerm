use std::fs;
use std::time::Duration;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::ComponentHandle;
use slint::Model;
use slint::platform::{Key, PointerEventButton, WindowEvent};

use i_slint_backend_testing::ElementHandle;

fn settle_modal_ui() {
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn element_center(element: &ElementHandle) -> slint::LogicalPosition {
    slint::LogicalPosition::new(
        element.absolute_position().x + element.size().width / 2.0,
        element.absolute_position().y + element.size().height / 2.0,
    )
}

fn descendant_by_id(element: &ElementHandle, id: &str) -> ElementHandle {
    element
        .query_descendants()
        .match_id(id)
        .find_first()
        .unwrap_or_else(|| panic!("missing descendant `{id}`"))
}

fn dispatch_pointer_click(
    app: &AppWindow,
    position: slint::LogicalPosition,
    button: PointerEventButton,
) {
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    app.window()
        .dispatch_event(WindowEvent::PointerPressed { position, button });
    settle_modal_ui();
    app.window()
        .dispatch_event(WindowEvent::PointerReleased { position, button });
    settle_modal_ui();
}

fn dispatch_modifier_pressed(app: &AppWindow, modifier: Key) {
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: modifier.into(),
    });
}

fn dispatch_modifier_released(app: &AppWindow, modifier: Key) {
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: modifier.into(),
    });
}

fn dispatch_text_key_chord(app: &AppWindow, key_text: &str, ctrl: bool, shift: bool, alt: bool) {
    if shift {
        dispatch_modifier_pressed(app, Key::Shift);
    }
    if ctrl {
        dispatch_modifier_pressed(app, Key::Control);
    }
    if alt {
        dispatch_modifier_pressed(app, Key::Alt);
    }

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: key_text.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: key_text.into(),
    });

    if alt {
        dispatch_modifier_released(app, Key::Alt);
    }
    if ctrl {
        dispatch_modifier_released(app, Key::Control);
    }
    if shift {
        dispatch_modifier_released(app, Key::Shift);
    }
    settle_modal_ui();
}

fn dispatch_text_sequence(app: &AppWindow, text: &str) {
    for ch in text.chars() {
        let key = ch.to_string();
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: key.clone().into(),
        });
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: key.into(),
        });
    }
    settle_modal_ui();
}

fn set_clipboard_text(text: &str) {
    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(text, slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("seed clipboard text");
}

fn clipboard_text() -> String {
    i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard text")
    .unwrap_or_default()
}

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
fn keychain_identity_modal_contract_uses_fluent_eye_toggle_for_password_reveal() {
    let identity_modal = fs::read_to_string("ui/components/assets-keychain-identity-modal.slint")
        .expect("read keychain identity modal");

    assert!(
        identity_modal.contains("eye-20-regular.svg"),
        "identity modal should use the shared Fluent eye icon for password reveal"
    );
    assert!(
        identity_modal.contains("eye-off-20-regular.svg"),
        "identity modal should use the shared Fluent eye-off icon for password hide"
    );
    assert!(
        identity_modal.contains(
            "trailing-icon-source: root.password-visible ? root.eye-off-icon : root.eye-icon;"
        ),
        "identity modal should drive password reveal through the shared trailing icon slot"
    );
    assert!(
        identity_modal.contains("\"password_visibility\""),
        "identity modal should keep the stable password_visibility field id when toggling reveal"
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

#[test]
fn keychain_identity_name_field_right_click_keeps_selection_and_focus() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_keychain_identity_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        if field.as_str() == "name" {
            app.set_keychain_identity_modal_name(value);
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-keychain-identity".into());
    settle_modal_ui();

    let name_field = ElementHandle::find_by_element_id(&app, "AssetsKeychainIdentityModal::name-field")
        .next()
        .expect("find identity name field");
    let name_input = descendant_by_id(&name_field, "DialogTextField::field-input");

    dispatch_pointer_click(&app, element_center(&name_input), PointerEventButton::Left);
    dispatch_text_sequence(&app, "Ops Identity");

    dispatch_text_key_chord(&app, "a", true, false, false);
    set_clipboard_text("sentinel-before-right-click");
    dispatch_pointer_click(&app, element_center(&name_input), PointerEventButton::Right);
    dispatch_text_key_chord(&app, "c", true, false, false);

    assert_eq!(
        clipboard_text(),
        "Ops Identity",
        "keychain identity name selections should survive a right-click before copy runs"
    );

    dispatch_text_sequence(&app, "Z");

    assert_eq!(
        app.get_keychain_identity_modal_name().as_str(),
        "Z",
        "after right-clicking a selected keychain identity field, the next typed character should still target the same input"
    );
}
