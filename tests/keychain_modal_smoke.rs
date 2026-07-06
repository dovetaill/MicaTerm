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
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
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

    let name_field =
        ElementHandle::find_by_element_id(&app, "AssetsKeychainIdentityModal::name-field")
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

#[test]
fn keychain_identity_password_field_context_menu_pastes_without_exposing_copy_even_when_revealed() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_keychain_identity_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        if field.as_str() == "password" {
            app.set_keychain_identity_modal_password(value);
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-keychain-identity".into());
    app.set_keychain_identity_modal_auth_kind("password".into());
    settle_modal_ui();

    let password_field =
        ElementHandle::find_by_element_id(&app, "AssetsKeychainIdentityModal::password-field")
            .next()
            .expect("find identity password field");
    let password_input = descendant_by_id(&password_field, "DialogTextField::field-input");
    let password_position = element_center(&password_input);

    dispatch_pointer_click(&app, password_position, PointerEventButton::Left);
    dispatch_text_sequence(&app, "topsecret");
    dispatch_text_key_chord(&app, "a", true, false, false);

    set_clipboard_text("sentinel-before-secret-menu");
    dispatch_pointer_click(&app, password_position, PointerEventButton::Right);

    assert!(
        app.get_text_context_menu_open(),
        "right-clicking a secret keychain password field should still open the shared text context menu"
    );
    assert!(
        !app.get_text_context_menu_copy_enabled(),
        "secret keychain password fields should not expose the shared Copy row by default"
    );
    assert!(
        app.get_text_context_menu_paste_enabled(),
        "secret keychain password fields should still expose Paste for password manager workflows"
    );

    set_clipboard_text("replaced-secret");
    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let paste_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::paste-row");
    dispatch_pointer_click(&app, element_center(&paste_row), PointerEventButton::Left);

    assert_eq!(
        app.get_keychain_identity_modal_password().as_str(),
        "replaced-secret",
        "the shared text context menu Paste row should continue to work for secret keychain password fields"
    );

    app.set_keychain_identity_modal_password_visible(true);
    settle_modal_ui();
    dispatch_pointer_click(&app, password_position, PointerEventButton::Left);
    dispatch_text_key_chord(&app, "a", true, false, false);
    dispatch_pointer_click(&app, password_position, PointerEventButton::Right);

    assert!(
        !app.get_text_context_menu_copy_enabled(),
        "revealing a secret keychain password field should not automatically grant Copy in the shared text context menu"
    );
}

#[test]
fn keychain_ssh_key_modal_private_and_public_material_follow_context_menu_policy() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_keychain_ssh_key_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        match field.as_str() {
            "private_key" => app.set_keychain_ssh_key_modal_private_key(value),
            "public_key" => app.set_keychain_ssh_key_modal_public_key(value),
            _ => {}
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-keychain-ssh-key".into());
    settle_modal_ui();

    let ssh_key_modal = ElementHandle::find_by_element_type_name(&app, "AssetsKeychainSshKeyModal")
        .next()
        .expect("find keychain ssh key modal");
    let visible_inputs = ssh_key_modal
        .query_descendants()
        .match_id("DialogTextField::field-input")
        .find_all()
        .into_iter()
        .filter(|field| field.size().height > 0.0)
        .collect::<Vec<_>>();
    let private_key_input = visible_inputs
        .get(1)
        .cloned()
        .expect("find visible private key input");
    let private_key_position = element_center(&private_key_input);

    dispatch_pointer_click(&app, private_key_position, PointerEventButton::Left);
    dispatch_text_sequence(&app, "PRIVATE KEY");
    dispatch_text_key_chord(&app, "a", true, false, false);

    set_clipboard_text("sentinel-before-private-key-menu");
    dispatch_pointer_click(&app, private_key_position, PointerEventButton::Right);

    assert!(
        !app.get_text_context_menu_copy_enabled(),
        "secret private key material should not expose Copy through the shared text context menu"
    );
    assert!(
        app.get_text_context_menu_paste_enabled(),
        "secret private key material should still allow Paste through the shared text context menu"
    );

    set_clipboard_text("REPLACED PRIVATE KEY");
    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let paste_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::paste-row");
    dispatch_pointer_click(&app, element_center(&paste_row), PointerEventButton::Left);

    assert_eq!(
        app.get_keychain_ssh_key_modal_private_key().as_str(),
        "REPLACED PRIVATE KEY",
        "private key material should keep shared Paste support even while Copy stays disabled"
    );

    let body_scroll =
        ElementHandle::find_by_element_id(&app, "AssetsKeychainSshKeyModal::body-scroll")
            .next()
            .expect("find keychain ssh key body scroll");
    let scroll_position = element_center(&body_scroll);
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: scroll_position,
    });
    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position: scroll_position,
        delta_x: 0.0,
        delta_y: -720.0,
    });
    settle_modal_ui();

    let public_key_input = ssh_key_modal
        .query_descendants()
        .match_id("DialogTextField::field-input")
        .find_all()
        .into_iter()
        .filter(|field| field.size().height > 0.0)
        .max_by(|left, right| {
            left.size()
                .height
                .partial_cmp(&right.size().height)
                .expect("compare visible input heights")
        })
        .expect("find visible public key input");
    let public_key_position = element_center(&public_key_input);
    let public_key_text = "ssh-ed25519 AAAA public";

    dispatch_pointer_click(&app, public_key_position, PointerEventButton::Left);
    dispatch_text_sequence(&app, public_key_text);
    dispatch_text_key_chord(&app, "a", true, false, false);

    set_clipboard_text("sentinel-before-public-key-copy");
    dispatch_pointer_click(&app, public_key_position, PointerEventButton::Right);

    assert!(
        app.get_text_context_menu_copy_enabled(),
        "public key material should still expose Copy through the shared text context menu"
    );

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let copy_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::copy-row");
    dispatch_pointer_click(&app, element_center(&copy_row), PointerEventButton::Left);

    assert_eq!(
        clipboard_text(),
        public_key_text,
        "public key material should remain copyable through the shared text context menu allowlist"
    );
}
