use mica_term::AppWindow;
use mica_term::WorkspaceTabItem;
use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    PersistedAssetPayload, PersistedAssetSocks5ProxySpec, PersistedAssetSshProxySpec,
};
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_store, bind_top_status_bar_with_store_and_effects_and_asset_repo,
};
use mica_term::app::ssh::known_hosts::{KnownHostCheck, KnownHostsService};
use mica_term::app::window_effects::default_platform_window_effects;
use russh::keys::{HashAlg, PublicKey};
use slint::Model;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, VecModel};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;
use i_slint_backend_testing::ElementHandle;

#[derive(Default)]
struct ModalAssetRepoState {
    save_attempts: Vec<PersistedAssetCatalog>,
}

struct RecordingModalAssetRepo {
    state: Rc<RefCell<ModalAssetRepoState>>,
}

impl RecordingModalAssetRepo {
    fn new(state: Rc<RefCell<ModalAssetRepoState>>) -> Self {
        Self { state }
    }
}

impl AssetCatalogRepository for RecordingModalAssetRepo {
    fn load(&self) -> Result<PersistedAssetCatalog> {
        Ok(PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        })
    }

    fn save(&self, catalog: &PersistedAssetCatalog) -> Result<()> {
        self.state.borrow_mut().save_attempts.push(catalog.clone());
        Ok(())
    }
}

fn sample_known_hosts_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mica-term-assets-modal-known-hosts-{}-{}.txt",
        label,
        std::process::id()
    ));
    path
}

fn sample_public_key() -> PublicKey {
    PublicKey::from_openssh(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti test-1@example.com",
    )
    .expect("parse public key")
}

fn settle_modal_ui() {
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn element_center(element: &ElementHandle) -> LogicalPosition {
    LogicalPosition::new(
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

fn dispatch_pointer_click(app: &AppWindow, position: LogicalPosition, button: PointerEventButton) {
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
fn folder_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-folder".into());
    app.set_asset_folder_modal_name("Infra".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "Infra");
}

#[test]
fn snippet_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-snippet".into());
    app.set_asset_snippet_modal_name("Deploy prod".into());
    app.set_asset_snippet_modal_script("kubectl rollout restart deploy/api".into());
    app.set_asset_snippet_modal_package("Operations".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-snippet");
    assert_eq!(app.get_asset_snippet_modal_name().as_str(), "Deploy prod");
    assert_eq!(
        app.get_asset_snippet_modal_script().as_str(),
        "kubectl rollout restart deploy/api"
    );
    assert_eq!(app.get_asset_snippet_modal_package().as_str(), "Operations");
}

#[test]
fn snippet_modal_callback_contract_exposes_name_script_and_package_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let changes = Rc::new(RefCell::new(Vec::<(String, String)>::new()));
    let recorded_changes = Rc::clone(&changes);

    app.on_asset_snippet_modal_draft_changed(move |field, value| {
        recorded_changes
            .borrow_mut()
            .push((field.to_string(), value.to_string()));
    });

    app.invoke_asset_snippet_modal_draft_changed("name".into(), "Deploy prod".into());
    app.invoke_asset_snippet_modal_draft_changed(
        "script".into(),
        "kubectl rollout restart deploy/api".into(),
    );
    app.invoke_asset_snippet_modal_draft_changed("package".into(), "Operations".into());

    assert_eq!(
        changes.borrow().as_slice(),
        [
            ("name".into(), "Deploy prod".into()),
            ("script".into(), "kubectl rollout restart deploy/api".into()),
            ("package".into(), "Operations".into()),
        ]
    );
}

#[test]
fn snippet_modal_contract_routes_package_picker_through_dialog_select_field() {
    let snippet_modal =
        fs::read_to_string("ui/components/assets-snippet-modal.slint").expect("read snippet modal");

    assert!(
        snippet_modal.contains("DialogSelectField"),
        "snippet modal should use the shared modal-local select trigger for package selection"
    );
    assert!(
        !snippet_modal.contains("ComboBox {"),
        "snippet modal should no longer rely on the stock ComboBox popup inside the modal body"
    );
    assert!(
        snippet_modal.contains("\"package\""),
        "snippet modal should continue emitting the stable package field id"
    );
    assert!(
        snippet_modal.contains("value == \"No Package\" ? \"\" : value"),
        "snippet modal should preserve the No Package -> empty-string mapping"
    );
}

#[test]
fn snippet_modal_shell_dismisses_local_select_overlay_before_closing_modal() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let block = app_window
        .split("if root.asset-modal-open && root.asset-modal-kind == \"new-snippet\" : asset-snippet-modal-shell := BlockingModalShell {\n")
        .nth(1)
        .expect("extract snippet shell block");
    let block = block
        .split("asset-snippet-modal-overlay := AssetsSnippetModal {")
        .next()
        .expect("truncate snippet shell block");

    assert!(
        block.contains("if !asset-snippet-modal-overlay.select-overlay-open {")
            && block.contains("main-workspace.restore-primary-focus();")
            && block.contains("root.blocking-modal-focus-restore-requested();"),
        "snippet shell should only restore workspace focus when its local select overlay is closed"
    );
    assert!(
        block.contains("consume-event => {")
            && block.contains("if asset-snippet-modal-overlay.select-overlay-open {")
            && block.contains("asset-snippet-modal-overlay.dismiss-open-select();"),
        "snippet shell should dismiss the package popup before letting backdrop clicks fall through"
    );
    assert!(
        block.contains("escape-requested => {")
            && block.contains("if asset-snippet-modal-overlay.select-overlay-open {")
            && block.contains("} else {\n                root.close-asset-modal-requested();"),
        "snippet shell should dismiss the package popup on Escape before closing the modal"
    );
}

#[test]
fn ssh_proxy_upstream_select_uses_narrower_inset_width_than_primary_proxy_type_field() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    app.show().expect("show app window");

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_name("Prod".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());
    app.set_asset_ssh_modal_user("ops".into());
    app.set_asset_ssh_modal_port("22".into());
    app.set_asset_ssh_modal_proxy_type("ssh-asset".into());
    app.set_asset_ssh_modal_proxy_ssh_selected_label("Mega".into());
    app.set_asset_ssh_modal_proxy_ssh_options(ModelRc::new(VecModel::from(vec!["Mega".into()])));

    let scroll_position = slint::LogicalPosition::new(520.0, 260.0);

    for _ in 0..8 {
        app.window().dispatch_event(WindowEvent::PointerScrolled {
            position: scroll_position,
            delta_x: 0.0,
            delta_y: -120.0,
        });
    }

    let proxy_field_stack =
        ElementHandle::find_by_element_id(&app, "AssetsSshConnectionModal::proxy-ssh-field-stack")
            .next()
            .expect("find proxy ssh field stack");
    let proxy_type_select =
        ElementHandle::find_by_element_id(&app, "AssetsSshConnectionModal::proxy-type-select")
            .next()
            .expect("find proxy type select");
    let proxy_select =
        ElementHandle::find_by_element_id(&app, "AssetsSshConnectionModal::proxy-ssh-select")
            .next()
            .expect("find proxy ssh select");

    let group_bottom = proxy_field_stack.absolute_position().y + proxy_field_stack.size().height;
    let select_bottom = proxy_select.absolute_position().y + proxy_select.size().height;

    assert!(
        select_bottom <= group_bottom,
        "upstream ssh select should fit fully inside its inset field stack, group_bottom={group_bottom}, select_bottom={select_bottom}"
    );
    assert!(
        proxy_select.absolute_position().x >= proxy_type_select.absolute_position().x + 8.0,
        "upstream ssh select should visibly inset from the primary proxy type field, proxy_x={}, primary_x={}",
        proxy_select.absolute_position().x,
        proxy_type_select.absolute_position().x,
    );
    assert!(
        proxy_select.size().width + 24.0 <= proxy_type_select.size().width,
        "upstream ssh select should read as a smaller nested field than the primary proxy type select, proxy_width={}, primary_width={}",
        proxy_select.size().width,
        proxy_type_select.size().width,
    );
}

#[test]
fn ssh_keychain_identity_flow_uses_inset_select_and_keeps_summary_visible() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    app.show().expect("show app window");

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_name("Prod".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());
    app.set_asset_ssh_modal_user("ops".into());
    app.set_asset_ssh_modal_port("22".into());
    app.set_asset_ssh_modal_auth_source("keychain-identity".into());
    app.set_asset_ssh_modal_keychain_identity_selected_label("Shared Identity".into());
    app.set_asset_ssh_modal_keychain_identity_options(ModelRc::new(VecModel::from(vec![
        "Shared Identity".into(),
    ])));

    let scroll_position = slint::LogicalPosition::new(520.0, 260.0);

    for _ in 0..3 {
        app.window().dispatch_event(WindowEvent::PointerScrolled {
            position: scroll_position,
            delta_x: 0.0,
            delta_y: -120.0,
        });
    }

    let identity_field_stack = ElementHandle::find_by_element_id(
        &app,
        "AssetsSshConnectionModal::keychain-identity-field-stack",
    )
    .next()
    .expect("find identity field stack");
    let identity_select = ElementHandle::find_by_element_id(
        &app,
        "AssetsSshConnectionModal::keychain-identity-select",
    )
    .next()
    .expect("find keychain identity select");
    let identity_summary_card =
        ElementHandle::find_by_element_id(&app, "AssetsSshConnectionModal::identity-summary-card")
            .next()
            .expect("find identity summary card");
    let identity_summary_value = ElementHandle::find_by_element_id(
        &app,
        "AssetsSshConnectionModal::identity-auth-summary-value",
    )
    .next()
    .expect("find identity auth summary value");

    let group_bottom =
        identity_field_stack.absolute_position().y + identity_field_stack.size().height;
    let select_bottom = identity_select.absolute_position().y + identity_select.size().height;
    let summary_bottom =
        identity_summary_value.absolute_position().y + identity_summary_value.size().height;
    let card_bottom =
        identity_summary_card.absolute_position().y + identity_summary_card.size().height;

    assert!(
        select_bottom <= group_bottom,
        "keychain identity select should fit fully inside its inset field stack, group_bottom={group_bottom}, select_bottom={select_bottom}"
    );
    assert!(
        identity_field_stack.absolute_position().x > 430.0,
        "keychain identity field stack should be inset from the main form column, field_x={}",
        identity_field_stack.absolute_position().x,
    );
    assert!(
        identity_select.size().width < 560.0,
        "keychain identity select should read as a smaller nested field than the primary auth source field, identity_width={}",
        identity_select.size().width,
    );
    assert!(
        summary_bottom <= card_bottom - 10.0,
        "identity authentication summary should stay fully visible inside its summary card, summary_bottom={summary_bottom}, card_bottom={card_bottom}"
    );
}

#[test]
fn ssh_modal_host_field_right_click_keeps_selection_and_typing_ownership() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_asset_ssh_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        if field.as_str() == "host" {
            app.set_asset_ssh_modal_host(value);
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    settle_modal_ui();

    let host_field = ElementHandle::find_by_element_id(&app, "AssetsSshConnectionModal::host-field")
        .next()
        .expect("find ssh host field");
    let host_input = descendant_by_id(&host_field, "DialogTextField::field-input");
    let host_input_position = element_center(&host_input);

    dispatch_pointer_click(&app, host_input_position, PointerEventButton::Left);
    dispatch_text_sequence(&app, "Alpha");

    dispatch_text_key_chord(&app, "a", true, false, false);
    set_clipboard_text("sentinel-before-right-click");
    dispatch_pointer_click(&app, host_input_position, PointerEventButton::Right);
    dispatch_text_key_chord(&app, "c", true, false, false);

    assert_eq!(
        clipboard_text(),
        "Alpha",
        "after a modal field selection is made, right-click should not clear the current selection before copy runs"
    );

    dispatch_text_sequence(&app, "Z");

    assert_eq!(
        app.get_asset_ssh_modal_host().as_str(),
        "Z",
        "after right-clicking a selected modal field, the next typed character should still replace the same field selection instead of leaking to another surface or dropping focus"
    );
}

#[test]
fn ssh_modal_host_field_padding_right_click_keeps_field_ownership() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_asset_ssh_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        if field.as_str() == "host" {
            app.set_asset_ssh_modal_host(value);
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    settle_modal_ui();

    let host_field = ElementHandle::find_by_element_id(&app, "AssetsSshConnectionModal::host-field")
        .next()
        .expect("find ssh host field");
    let host_input = descendant_by_id(&host_field, "DialogTextField::field-input");
    let right_padding = descendant_by_id(&host_field, "DialogTextField::right-padding-focus");

    dispatch_pointer_click(&app, element_center(&host_input), PointerEventButton::Left);
    dispatch_text_sequence(&app, "Alpha");

    dispatch_pointer_click(&app, element_center(&right_padding), PointerEventButton::Right);
    dispatch_text_sequence(&app, "Z");

    assert_eq!(
        app.get_asset_ssh_modal_host().as_str(),
        "AlphaZ",
        "right-clicking a dialog text field padding gutter should not steal the active field ownership away from the input"
    );
}

#[test]
fn ssh_modal_host_field_context_menu_copy_and_paste_actions_work() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_asset_ssh_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        if field.as_str() == "host" {
            app.set_asset_ssh_modal_host(value);
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    settle_modal_ui();

    let host_field = ElementHandle::find_by_element_id(&app, "AssetsSshConnectionModal::host-field")
        .next()
        .expect("find ssh host field");
    let host_input = descendant_by_id(&host_field, "DialogTextField::field-input");
    let host_input_position = element_center(&host_input);

    dispatch_pointer_click(&app, host_input_position, PointerEventButton::Left);
    dispatch_text_sequence(&app, "Alpha");
    dispatch_text_key_chord(&app, "a", true, false, false);

    set_clipboard_text("sentinel-before-copy-row");
    dispatch_pointer_click(&app, host_input_position, PointerEventButton::Right);
    assert!(
        app.get_text_context_menu_open(),
        "right-clicking a bridged SSH text field should open the shared text context menu"
    );

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let copy_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::copy-row");
    dispatch_pointer_click(&app, element_center(&copy_row), PointerEventButton::Left);

    assert!(
        !app.get_text_context_menu_open(),
        "invoking the copy row should dismiss the shared text context menu"
    );

    assert_eq!(
        clipboard_text(),
        "Alpha",
        "the shared text context menu copy row should forward to the owning SSH host field selection"
    );

    dispatch_pointer_click(&app, host_input_position, PointerEventButton::Left);
    dispatch_text_key_chord(&app, "a", true, false, false);
    set_clipboard_text("Beta");
    dispatch_pointer_click(&app, host_input_position, PointerEventButton::Right);
    assert!(
        app.get_text_context_menu_open(),
        "right-clicking again should reopen the shared text context menu for paste"
    );

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let paste_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::paste-row");
    dispatch_pointer_click(&app, element_center(&paste_row), PointerEventButton::Left);

    assert_eq!(
        app.get_asset_ssh_modal_host().as_str(),
        "Beta",
        "the shared text context menu paste row should insert clipboard text back into the owning SSH host field"
    );
}

#[test]
fn snippet_modal_script_field_context_menu_preserves_multiline_paste_without_terminal_pipeline() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_asset_snippet_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        match field.as_str() {
            "name" => app.set_asset_snippet_modal_name(value),
            "script" => app.set_asset_snippet_modal_script(value),
            "package" => app.set_asset_snippet_modal_package(value),
            _ => {}
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-snippet".into());
    settle_modal_ui();

    let script_field =
        ElementHandle::find_by_element_id(&app, "AssetsSnippetModal::script-field")
            .next()
            .expect("find snippet script field");
    let script_input = descendant_by_id(&script_field, "DialogTextField::field-input");
    let script_position = element_center(&script_input);
    let multiline_script = "echo alpha\n\tprintf 'beta'\n  gamma";

    dispatch_pointer_click(&app, script_position, PointerEventButton::Left);
    set_clipboard_text(multiline_script);
    dispatch_pointer_click(&app, script_position, PointerEventButton::Right);

    assert!(
        app.get_text_context_menu_open(),
        "right-clicking the snippet script editor should open the shared text context menu"
    );
    assert!(
        app.get_text_context_menu_paste_enabled(),
        "snippet script editors should expose Paste through the shared text context menu"
    );

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let paste_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::paste-row");
    dispatch_pointer_click(&app, element_center(&paste_row), PointerEventButton::Left);

    assert_eq!(
        app.get_asset_snippet_modal_script().as_str(),
        multiline_script,
        "snippet script editors should preserve newlines, tabs, and indentation when the shared text context menu pastes multiline text"
    );
    assert!(
        !app.get_workspace_paste_warning_modal_open(),
        "snippet script right-click paste should stay inside the text field domain instead of opening the terminal paste warning flow"
    );

    dispatch_pointer_click(&app, script_position, PointerEventButton::Left);
    dispatch_text_key_chord(&app, "a", true, false, false);
    set_clipboard_text("sentinel-before-snippet-copy");
    dispatch_pointer_click(&app, script_position, PointerEventButton::Right);

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let copy_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::copy-row");
    dispatch_pointer_click(&app, element_center(&copy_row), PointerEventButton::Left);

    assert_eq!(
        clipboard_text(),
        multiline_script,
        "snippet script editors should copy the original multiline payload without terminal paste normalization"
    );
}

#[test]
fn snippet_package_name_field_context_menu_copy_and_paste_actions_work() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_asset_snippet_package_modal_name_changed(move |value| {
        let app = app_weak.upgrade().expect("upgrade app");
        app.set_asset_snippet_package_modal_name(value);
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-snippet-package".into());
    settle_modal_ui();

    let package_name_input =
        ElementHandle::find_by_element_id(&app, "AssetsSnippetPackageModal::name-input")
            .next()
            .expect("find snippet package name input");
    let package_name_position = element_center(&package_name_input);

    dispatch_pointer_click(&app, package_name_position, PointerEventButton::Left);
    dispatch_text_sequence(&app, "Ops");
    dispatch_text_key_chord(&app, "a", true, false, false);
    set_clipboard_text("sentinel-before-snippet-package-copy");
    dispatch_pointer_click(&app, package_name_position, PointerEventButton::Right);

    assert!(
        app.get_text_context_menu_open(),
        "right-clicking the bare snippet package input should open the shared text context menu"
    );

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let copy_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::copy-row");
    dispatch_pointer_click(&app, element_center(&copy_row), PointerEventButton::Left);

    assert_eq!(
        clipboard_text(),
        "Ops",
        "the shared text context menu Copy row should forward to the bare snippet package input selection"
    );

    dispatch_pointer_click(&app, package_name_position, PointerEventButton::Left);
    dispatch_text_key_chord(&app, "a", true, false, false);
    set_clipboard_text("Team Tools");
    dispatch_pointer_click(&app, package_name_position, PointerEventButton::Right);

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let paste_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::paste-row");
    dispatch_pointer_click(&app, element_center(&paste_row), PointerEventButton::Left);

    assert_eq!(
        app.get_asset_snippet_package_modal_name().as_str(),
        "Team Tools",
        "the shared text context menu Paste row should update the bare snippet package input without introducing a DialogTextField wrapper"
    );
}

#[test]
fn snippet_package_select_fully_fits_inside_its_layout_group() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    app.show().expect("show app window");

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-snippet".into());
    app.set_asset_snippet_modal_name("Deploy".into());
    app.set_asset_snippet_modal_script("kubectl rollout restart deploy/api".into());
    app.set_asset_snippet_modal_package_selected_label("Operations".into());
    app.set_asset_snippet_modal_package_options(ModelRc::new(VecModel::from(vec![
        "No Package".into(),
        "Operations".into(),
    ])));

    let package_group =
        ElementHandle::find_by_element_id(&app, "AssetsSnippetModal::snippet-package-select-group")
            .next()
            .expect("find snippet package select group");
    let package_select =
        ElementHandle::find_by_element_id(&app, "AssetsSnippetModal::package-select")
            .next()
            .expect("find snippet package select");

    let group_bottom = package_group.absolute_position().y + package_group.size().height;
    let select_bottom = package_select.absolute_position().y + package_select.size().height;

    assert!(
        select_bottom <= group_bottom,
        "snippet package select should fit fully inside its own layout group, group_bottom={group_bottom}, select_bottom={select_bottom}"
    );
}

#[test]
fn snippet_package_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-snippet-package".into());
    app.set_asset_snippet_package_modal_name("Operations".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-snippet-package");
    assert_eq!(
        app.get_asset_snippet_package_modal_name().as_str(),
        "Operations"
    );
}

#[test]
fn snippet_package_modal_name_callback_contract_is_exposed() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let names = Rc::new(RefCell::new(Vec::<String>::new()));
    let recorded_names = Rc::clone(&names);

    app.on_asset_snippet_package_modal_name_changed(move |value| {
        recorded_names.borrow_mut().push(value.to_string());
    });

    app.invoke_asset_snippet_package_modal_name_changed("Operations".into());

    assert_eq!(names.borrow().as_slice(), ["Operations"]);
}

#[test]
fn ssh_modal_round_trips_grouped_form_fields_without_top_level_tab_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_name("Prod Bastion".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());
    app.set_asset_ssh_modal_user("ops".into());
    app.set_asset_ssh_modal_port("22".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Prod Bastion");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "10.0.0.12");
    assert_eq!(app.get_asset_ssh_modal_user().as_str(), "ops");
    assert_eq!(app.get_asset_ssh_modal_port().as_str(), "22");
}

#[test]
fn ssh_modal_round_trips_standard_fields_and_auth_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_name("Prod Bastion".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());
    app.set_asset_ssh_modal_user("ops".into());
    app.set_asset_ssh_modal_port("2222".into());
    app.set_asset_ssh_modal_auth_method("private-key".into());
    app.set_asset_ssh_modal_private_key_source("path".into());
    app.set_asset_ssh_modal_password("secret".into());
    app.set_asset_ssh_modal_remark("Primary entry point".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Prod Bastion");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "10.0.0.12");
    assert_eq!(app.get_asset_ssh_modal_user().as_str(), "ops");
    assert_eq!(app.get_asset_ssh_modal_port().as_str(), "2222");
    assert_eq!(
        app.get_asset_ssh_modal_auth_method().as_str(),
        "private-key"
    );
    assert_eq!(
        app.get_asset_ssh_modal_private_key_source().as_str(),
        "path"
    );
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert_eq!(
        app.get_asset_ssh_modal_remark().as_str(),
        "Primary entry point"
    );
}

#[test]
fn ssh_modal_round_trips_proxy_fields_and_visibility_flags() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_proxy_type("socks5".into());
    app.set_asset_ssh_modal_proxy_socks5_host("proxy.example.net".into());
    app.set_asset_ssh_modal_proxy_socks5_port("1080".into());
    app.set_asset_ssh_modal_proxy_socks5_username("ops-proxy".into());
    app.set_asset_ssh_modal_proxy_socks5_password("proxy-secret".into());
    app.set_asset_ssh_modal_proxy_socks5_password_visible(true);
    app.set_asset_ssh_modal_proxy_ssh_asset_id("asset-bastion".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_proxy_type().as_str(), "socks5");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_host().as_str(),
        "proxy.example.net"
    );
    assert_eq!(app.get_asset_ssh_modal_proxy_socks5_port().as_str(), "1080");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_username().as_str(),
        "ops-proxy"
    );
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_password().as_str(),
        "proxy-secret"
    );
    assert!(app.get_asset_ssh_modal_proxy_socks5_password_visible());
    assert_eq!(
        app.get_asset_ssh_modal_proxy_ssh_asset_id().as_str(),
        "asset-bastion"
    );
}

#[test]
fn ssh_modal_action_callback_contract_exposes_full_connect_family() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let actions = Rc::new(RefCell::new(Vec::<String>::new()));
    let recorded_actions = Rc::clone(&actions);

    app.on_asset_ssh_modal_action_requested(move |action| {
        recorded_actions.borrow_mut().push(action.to_string());
    });

    app.invoke_asset_ssh_modal_action_requested("save".into());
    app.invoke_asset_ssh_modal_action_requested("connect".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());
    app.invoke_asset_ssh_modal_action_requested("save-and-connect".into());

    assert_eq!(
        actions.borrow().as_slice(),
        ["save", "connect", "test", "save-and-connect"]
    );
}

#[test]
fn ssh_modal_contract_round_trips_button_state_and_inline_feedback() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_modal_can_confirm(true);
    app.set_asset_modal_validation_message("Host is required.".into());
    app.set_asset_ssh_modal_connect_family_enabled(false);
    app.set_asset_ssh_modal_feedback_state("busy".into());
    app.set_asset_ssh_modal_feedback_message("Testing connection...".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert!(app.get_asset_modal_can_confirm());
    assert_eq!(
        app.get_asset_modal_validation_message().as_str(),
        "Host is required."
    );
    assert!(!app.get_asset_ssh_modal_connect_family_enabled());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "busy");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Testing connection..."
    );
}

#[test]
fn ssh_modal_round_trips_password_visibility_without_secret_retention_flags() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_password("secret".into());
    app.set_asset_ssh_modal_password_visible(false);
    app.set_asset_ssh_modal_passphrase("hunter2".into());
    app.set_asset_ssh_modal_passphrase_visible(true);

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!app.get_asset_ssh_modal_password_visible());
    assert_eq!(app.get_asset_ssh_modal_passphrase().as_str(), "hunter2");
    assert!(app.get_asset_ssh_modal_passphrase_visible());
}

#[test]
fn ssh_modal_contract_removes_saved_secret_retention_flags() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(
        !app_window.contains("asset-ssh-modal-secret-retention-message"),
        "app window should stop projecting saved-secret retention copy"
    );
    assert!(
        !app_window.contains("asset-ssh-modal-can-clear-saved-secret"),
        "app window should stop projecting clear-secret affordance state"
    );
    assert!(
        !app_window.contains("asset-ssh-modal-clear-saved-secret-requested"),
        "app window should stop projecting clear-secret request state"
    );
    assert!(
        !ssh_modal.contains("Clear Saved Secret"),
        "ssh modal should remove the clear-secret button"
    );
}

#[test]
fn ssh_modal_no_longer_renders_dead_connection_options_group() {
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(!ssh_modal.contains("text: \"Connection Options\""));
    assert!(!ssh_modal.contains("label: \"Proxy Method\""));
    assert!(!ssh_modal.contains("label: \"Session Environment\""));
    assert!(ssh_modal.contains("title: \"Proxy chain\";"));
    assert!(ssh_modal.contains("text: \"Proxy type\""));
    assert!(
        ssh_modal.contains("label: root.proxy-type == \"http\" ? \"HTTP host\" : \"SOCKS5 host\";")
    );
    assert!(
        ssh_modal.contains("label: root.proxy-type == \"http\" ? \"HTTP port\" : \"SOCKS5 port\";")
    );
    assert!(ssh_modal.contains("label: \"Username\""));
    assert!(ssh_modal.contains("label: \"Password\""));
    assert!(ssh_modal.contains("text: \"Upstream SSH connection\""));
    assert!(ssh_modal.contains("None"));
    assert!(ssh_modal.contains("SOCKS5"));
    assert!(ssh_modal.contains("Existing SSH Connection"));
}

#[test]
fn ssh_modal_exposes_private_key_import_guidance() {
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(
        ssh_modal
            .contains("Only the private key is needed here. The public key stays on the server."),
        "ssh modal should explain that users only provide the private key locally"
    );
    assert!(
        ssh_modal.contains("root.action-requested(\"import-private-key\")"),
        "ssh modal should expose an import-private-key action hook"
    );
}

#[test]
fn ssh_modal_contract_exposes_auth_source_switch_and_keychain_identity_summary() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(app_window.contains("asset-ssh-modal-auth-source"));
    assert!(app_window.contains("asset-ssh-modal-keychain-identity-options"));
    assert!(app_window.contains("asset-ssh-modal-keychain-identity-selected-label"));
    assert!(app_window.contains("asset-ssh-modal-keychain-identity-username"));
    assert!(app_window.contains("asset-ssh-modal-keychain-identity-auth-summary"));

    assert!(ssh_modal.contains("Manual"));
    assert!(ssh_modal.contains("Keychain Identity"));
    assert!(ssh_modal.contains("text: \"Identity\""));
    assert!(ssh_modal.contains("text: \"Username\""));
    assert!(ssh_modal.contains("text: \"Authentication summary\""));
    assert!(ssh_modal.contains("\"auth_source\""));
    assert!(ssh_modal.contains("\"keychain_identity_label\""));
    assert!(!ssh_modal.contains("Use Existing Keychain Identity"));
}

#[test]
fn ssh_modal_contract_routes_selects_through_dialog_select_field() {
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(
        ssh_modal.contains("DialogSelectField"),
        "ssh modal should use the shared modal-local select trigger"
    );
    assert!(
        !ssh_modal.contains("ComboBox {"),
        "ssh modal should no longer rely on the stock ComboBox popup inside the modal body"
    );
    assert!(ssh_modal.contains("\"auth_source\""));
    assert!(ssh_modal.contains("\"keychain_identity_label\""));
    assert!(ssh_modal.contains("\"proxy_type\""));
    assert!(ssh_modal.contains("\"proxy_ssh_asset_label\""));
}

#[test]
fn ssh_modal_labels_saved_path_mode_as_legacy_file_path() {
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(
        ssh_modal.contains("label: \"Legacy file path\""),
        "ssh modal should relabel saved path-mode assets as legacy file path"
    );
    assert!(
        ssh_modal
            .contains("Paste or import a fresh key below to replace the legacy path reference."),
        "ssh modal should explain how a legacy path asset migrates to imported key content"
    );
}

#[test]
fn app_window_and_create_menu_contract_wire_keychain_modal_entries() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let create_menu =
        fs::read_to_string("ui/components/assets-create-menu.slint").expect("read create menu");

    assert!(app_window.contains(
        "import { AssetsKeychainIdentityModal } from \"components/assets-keychain-identity-modal.slint\";"
    ));
    assert!(app_window.contains(
        "import { AssetsKeychainSshKeyModal } from \"components/assets-keychain-ssh-key-modal.slint\";"
    ));
    assert!(create_menu.contains("callback new-identity-selected;"));
    assert!(create_menu.contains("callback new-ssh-key-selected;"));
    assert!(create_menu.contains("label: \"New Identity\""));
    assert!(create_menu.contains("label: \"New SSH Key\""));
    assert!(app_window.contains("new-identity-selected => {"));
    assert!(app_window.contains("root.assets-create-action-selected(\"new-identity\");"));
    assert!(app_window.contains("new-ssh-key-selected => {"));
    assert!(app_window.contains("root.assets-create-action-selected(\"new-ssh-key\");"));
    assert!(app_window.contains("root.asset-modal-kind == \"new-keychain-identity\""));
    assert!(app_window.contains("root.asset-modal-kind == \"new-keychain-ssh-key\""));
}

#[test]
fn keychain_ssh_key_modal_round_trips_fields_and_action_callbacks() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let actions = Rc::new(RefCell::new(Vec::<String>::new()));
    let recorded_actions = Rc::clone(&actions);

    app.on_keychain_ssh_key_modal_action_requested(move |action| {
        recorded_actions.borrow_mut().push(action.to_string());
    });

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-keychain-ssh-key".into());
    app.set_keychain_ssh_key_modal_name("Prod Bastion Key".into());
    app.set_keychain_ssh_key_modal_private_key("PRIVATE".into());
    app.set_keychain_ssh_key_modal_public_key("ssh-ed25519 AAAATEST".into());
    app.set_keychain_ssh_key_modal_fingerprint("SHA256:test".into());

    app.invoke_keychain_ssh_key_modal_action_requested("import-private-key".into());
    app.invoke_keychain_ssh_key_modal_action_requested("import-public-key".into());
    app.invoke_keychain_ssh_key_modal_action_requested("generate-key-pair".into());
    app.invoke_keychain_ssh_key_modal_action_requested("copy-public-key".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-keychain-ssh-key");
    assert_eq!(
        app.get_keychain_ssh_key_modal_name().as_str(),
        "Prod Bastion Key"
    );
    assert_eq!(
        app.get_keychain_ssh_key_modal_private_key().as_str(),
        "PRIVATE"
    );
    assert_eq!(
        app.get_keychain_ssh_key_modal_public_key().as_str(),
        "ssh-ed25519 AAAATEST"
    );
    assert_eq!(
        app.get_keychain_ssh_key_modal_fingerprint().as_str(),
        "SHA256:test"
    );
    assert_eq!(
        actions.borrow().as_slice(),
        [
            "import-private-key",
            "import-public-key",
            "generate-key-pair",
            "copy-public-key"
        ]
    );
}

#[test]
fn app_window_round_trips_workspace_tab_items_and_active_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    app.set_workspace_tab_items(ModelRc::new(VecModel::from(vec![
        WorkspaceTabItem {
            tab_id: "session-1".into(),
            title: "Prod Bastion".into(),
            subtitle: "ops@example.com:22".into(),
            state: "connected".into(),
            enhanced_session_state: "enhanced".into(),
            active: false,
        },
        WorkspaceTabItem {
            tab_id: "session-2".into(),
            title: "Staging Bastion".into(),
            subtitle: "ops@staging.example.com:22".into(),
            state: "error".into(),
            enhanced_session_state: "fallback".into(),
            active: true,
        },
    ])));
    app.set_active_workspace_session_id("session-2".into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_eq!(
        app.get_workspace_tab_items()
            .row_data(1)
            .expect("workspace tab item")
            .title
            .as_str(),
        "Staging Bastion"
    );
    assert_eq!(app.get_active_workspace_session_id().as_str(), "session-2");
}

#[test]
fn host_key_confirm_modal_round_trips_target_host_and_fingerprint() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_ssh_host_key_modal_open(true);
    app.set_ssh_host_key_modal_host("example.com".into());
    app.set_ssh_host_key_modal_fingerprint("SHA256:abc123".into());

    assert!(app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_ssh_host_key_modal_host().as_str(), "example.com");
    assert_eq!(
        app.get_ssh_host_key_modal_fingerprint().as_str(),
        "SHA256:abc123"
    );
}

#[test]
fn unknown_host_key_prompts_once_then_reconnect_uses_trusted_key() {
    i_slint_backend_testing::init_no_event_loop();

    let path = sample_known_hosts_path("trusted-once");
    let _ = fs::remove_file(&path);
    let service = KnownHostsService::new(&path);
    let key = sample_public_key();

    let first_check = service
        .check("example.com", 22, &key)
        .expect("check unknown host");
    let fingerprint = match first_check {
        KnownHostCheck::Unknown { fingerprint } => fingerprint,
        other => panic!("expected unknown host result, got {other:?}"),
    };

    let app = AppWindow::new().unwrap();
    app.set_ssh_host_key_modal_open(true);
    app.set_ssh_host_key_modal_host("example.com".into());
    app.set_ssh_host_key_modal_fingerprint(fingerprint.clone().into());

    assert!(app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_ssh_host_key_modal_host().as_str(), "example.com");
    assert_eq!(
        app.get_ssh_host_key_modal_fingerprint().as_str(),
        key.fingerprint(HashAlg::Sha256).to_string()
    );

    service
        .accept_unknown("example.com", 22, &key)
        .expect("accept trusted host key");

    let second_check = service
        .check("example.com", 22, &key)
        .expect("recheck trusted host");
    assert!(matches!(second_check, KnownHostCheck::Trusted));

    let _ = fs::remove_file(&path);
}

#[test]
fn ssh_modal_reopens_with_default_authentication_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("auth_method".into(), "private-key".into());
    app.invoke_close_asset_modal_requested();
    app.invoke_assets_create_action_selected("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_auth_method().as_str(), "password");
    assert_eq!(
        app.get_asset_ssh_modal_dialog_title().as_str(),
        "New SSH Connection"
    );
}

#[test]
fn rename_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_rename_modal_open(true);
    app.set_asset_rename_modal_name("Prod".into());
    app.set_asset_rename_modal_validation_message("Duplicate name".into());
    app.set_asset_rename_modal_can_confirm(false);

    assert!(app.get_asset_rename_modal_open());
    assert_eq!(app.get_asset_rename_modal_name().as_str(), "Prod");
    assert_eq!(
        app.get_asset_rename_modal_validation_message().as_str(),
        "Duplicate name"
    );
    assert!(!app.get_asset_rename_modal_can_confirm());
}

#[test]
fn delete_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_delete_confirm_modal_open(true);
    app.set_asset_delete_confirm_target_label("Prod".into());
    app.set_asset_delete_confirm_descendant_count(3);

    assert!(app.get_asset_delete_confirm_modal_open());
    assert_eq!(app.get_asset_delete_confirm_target_label().as_str(), "Prod");
    assert_eq!(app.get_asset_delete_confirm_descendant_count(), 3);
}

#[test]
fn blocking_modal_children_own_shared_asset_modal_chrome_contract() {
    let shell = fs::read_to_string("ui/components/blocking-modal-shell.slint")
        .expect("read blocking modal shell");
    let folder = fs::read_to_string("ui/components/assets-folder-create-modal.slint")
        .expect("read folder modal");
    let ssh = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");
    let rename =
        fs::read_to_string("ui/components/assets-rename-modal.slint").expect("read rename modal");
    let delete = fs::read_to_string("ui/components/assets-delete-confirm-modal.slint")
        .expect("read delete modal");
    let host_key = fs::read_to_string("ui/components/ssh-host-key-confirm-modal.slint")
        .expect("read host key modal");

    assert!(!shell.contains("in property <string> dialog-title"));
    assert!(!shell.contains("callback close-requested();"));
    assert!(!shell.contains("header := Rectangle {"));
    assert!(!shell.contains("close-button := Rectangle {"));
    assert!(folder.contains("in property <string> dialog-title: \"New Folder\";"));
    assert!(!folder.contains("DialogSectionCard"));
    assert!(folder.contains("DialogTextField"));
    assert!(ssh.contains("in property <string> dialog-title: \"New SSH Connection\";"));
    assert!(ssh.contains("DialogFormSection"));
    assert!(ssh.contains("header := ModalHeaderBar {"));
    assert!(ssh.contains("footer := ModalFooterBar {"));
    assert!(rename.contains("header := ModalHeaderBar {"));
    assert!(rename.contains("footer := ModalFooterBar {"));
    assert!(
        !rename.contains("DialogSectionCard"),
        "rename should stay a simple one-field dialog instead of using the heavier asset form card"
    );
    assert!(rename.contains("DialogTextField"));
    assert!(delete.contains("header := ModalHeaderBar {"));
    assert!(delete.contains("footer := ModalFooterBar {"));
    assert!(delete.contains("DialogSectionCard"));
    assert!(host_key.contains("header := ModalHeaderBar {"));
    assert!(host_key.contains("footer := ModalFooterBar {"));
    assert!(host_key.contains("DialogSectionCard"));
}

#[test]
fn blocking_modal_shell_exposes_a_full_frame_for_child_owned_chrome() {
    let shell = fs::read_to_string("ui/components/blocking-modal-shell.slint")
        .expect("read blocking modal shell");

    assert!(
        shell.contains("modal-event-scope := FocusScope {")
            && shell.contains(
                "content-host := Rectangle {\n                x: 0px;\n                y: 0px;"
            ),
        "blocking modal shell content host must expose the full frame so child modals can own header and footer geometry"
    );
    assert!(
        shell.contains("height: parent.height;"),
        "blocking modal shell content host must keep the full modal height available to child layouts"
    );
}

#[test]
fn blocking_modal_shell_clamps_dragged_frames_inside_the_viewport() {
    let shell = fs::read_to_string("ui/components/blocking-modal-shell.slint")
        .expect("read blocking modal shell");

    assert!(
        shell.contains("root.width - root.viewport-margin - root.resolved-modal-width"),
        "blocking modal shell should cap drag offsets at the right edge"
    );
    assert!(
        shell.contains("root.height - root.viewport-margin - root.resolved-modal-height"),
        "blocking modal shell should cap drag offsets at the bottom edge"
    );
}

#[test]
fn blocking_modal_shell_claims_focus_and_captures_escape_for_the_topmost_dialog() {
    let shell = fs::read_to_string("ui/components/blocking-modal-shell.slint")
        .expect("read blocking modal shell");

    assert!(
        shell.contains("in property <int> focus-sequence: 0;"),
        "blocking modal shell should expose a shared focus sequence so every dialog can claim keyboard focus when it opens"
    );
    assert!(
        shell.contains("callback escape-requested();"),
        "blocking modal shell should expose a shared escape callback so AppWindow can route close requests consistently"
    );
    assert!(
        shell.contains("capture-key-pressed(event) => {")
            && shell.contains("event.text == Key.Escape")
            && shell.contains("root.focus-restore-requested();")
            && shell.contains("root.escape-requested();"),
        "blocking modal shell should capture Escape before it falls through to the underlying terminal or workspace"
    );
    assert!(
        shell.contains("changed focus-sequence => {")
            && shell.contains("modal-event-scope.focus();"),
        "blocking modal shell should actively focus its keyboard host whenever the shared focus sequence changes"
    );
}

#[test]
fn shared_modal_chrome_exports_unified_dialog_controls_for_forms_and_action_rows() {
    let chrome =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read shared modal chrome");

    assert!(
        chrome.contains("export component DialogActionButton")
            && chrome.contains("export component DialogSegmentButton")
            && chrome.contains("export component DialogSectionCard")
            && chrome.contains("export component DialogTextField"),
        "modal chrome should export reusable action buttons, segmented controls, section cards, and text fields so forms stop reimplementing their own chrome"
    );
}

#[test]
fn app_window_routes_shell_escape_requests_to_the_same_close_paths_as_the_x_button() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    for marker in [
        "open-saved-ssh-modal-shell := BlockingModalShell {\n",
        "sync-modal-shell := BlockingModalShell {\n",
        "settings-modal-shell := BlockingModalShell {\n",
        "asset-folder-modal-shell := BlockingModalShell {\n",
        "asset-ssh-modal-shell := BlockingModalShell {\n",
        "keychain-identity-modal-shell := BlockingModalShell {\n",
        "keychain-ssh-key-modal-shell := BlockingModalShell {\n",
    ] {
        let block = app_window
            .split(marker)
            .nth(1)
            .expect("extract shell block");
        let block = block
            .split("\n    }\n")
            .next()
            .expect("truncate shell block");

        assert!(
            block.contains("escape-requested => {"),
            "{marker} should route shell Escape handling through the same close path as the dialog close affordances"
        );
    }
}

#[test]
fn simple_asset_modals_anchor_header_and_footer_to_the_frame_edges() {
    let folder = fs::read_to_string("ui/components/assets-folder-create-modal.slint")
        .expect("read folder modal");
    let rename =
        fs::read_to_string("ui/components/assets-rename-modal.slint").expect("read rename modal");
    let delete = fs::read_to_string("ui/components/assets-delete-confirm-modal.slint")
        .expect("read delete modal");
    let host_key = fs::read_to_string("ui/components/ssh-host-key-confirm-modal.slint")
        .expect("read host key modal");

    assert!(
        folder.contains("header := ModalHeaderBar {\n            x: 0px;\n            y: 0px;"),
        "folder modal header should be pinned to the frame origin via the shared header bar"
    );
    assert!(
        folder.contains("footer := ModalFooterBar {\n            x: 0px;\n            y: parent.height - root.footer-height;"),
        "folder modal footer should be pinned to the bottom edge via the shared footer bar"
    );

    for modal in [&rename, &delete, &host_key] {
        assert!(
            modal.contains("header := ModalHeaderBar {\n            x: 0px;\n            y: 0px;"),
            "remaining simple asset modals should delegate their header chrome to the shared header bar"
        );
        assert!(
            modal.contains("footer := ModalFooterBar {\n            x: 0px;\n            y: parent.height - root.footer-height;"),
            "remaining simple asset modals should pin their shared footer bar to the bottom edge"
        );
    }
}

#[test]
fn remaining_old_dialogs_adopt_shared_modal_chrome_contract() {
    let rename =
        fs::read_to_string("ui/components/assets-rename-modal.slint").expect("read rename modal");
    let delete = fs::read_to_string("ui/components/assets-delete-confirm-modal.slint")
        .expect("read delete modal");
    let host_key = fs::read_to_string("ui/components/ssh-host-key-confirm-modal.slint")
        .expect("read host key modal");
    let remote_file = fs::read_to_string("ui/components/sftp-remote-file-modal.slint")
        .expect("read remote file modal");

    for modal in [&rename, &delete, &host_key, &remote_file] {
        assert!(
            modal.contains("ModalHeaderBar") && modal.contains("ModalFooterBar"),
            "remaining blocking dialogs should move to the shared modal header/footer primitives"
        );
        assert!(
            !modal.contains("component DialogButton inherits Rectangle {"),
            "remaining dialogs should stop defining bespoke local dialog buttons once shared modal chrome is adopted"
        );
    }

    assert!(rename.contains("DialogTextField"));
    assert!(
        !rename.contains("DialogSectionCard"),
        "rename should stay a simple one-field dialog instead of using the heavier asset form card"
    );
    assert!(delete.contains("DialogSectionCard"));
    assert!(host_key.contains("DialogSectionCard"));
    assert!(
        remote_file.contains("DialogInlineBanner") || remote_file.contains("DialogSectionCard"),
        "the remote-file dialog should reuse shared inline status or card surfaces instead of bespoke flat status rows"
    );
}

#[test]
fn ssh_modal_header_body_and_footer_are_explicitly_anchored() {
    let ssh = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(
        ssh.contains("header := ModalHeaderBar {\n            x: 0px;\n            y: 0px;"),
        "ssh modal should delegate its header chrome to the shared modal header"
    );
    assert!(
        ssh.contains("body-scroll := ModalBodyScrollArea {\n            x: 0px;\n            y: header.height;"),
        "ssh modal body scroll host must start directly below the shared header"
    );
    assert!(
        !ssh.contains("tabs-host := Rectangle {"),
        "ssh modal must not keep the legacy top-level tabs host"
    );
    assert!(
        ssh.contains("footer := ModalFooterBar {\n            x: 0px;\n            y: parent.height - root.footer-height;"),
        "ssh modal footer must be pinned to the bottom edge via the shared footer bar"
    );
}

#[test]
fn sync_modal_header_body_and_footer_are_explicitly_anchored_and_scrollable() {
    let sync = fs::read_to_string("ui/components/sync-vault-modal.slint").expect("read sync modal");
    let modal_chrome =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome");

    assert!(
        sync.contains("header := ModalHeaderBar {\n            x: 0px;\n            y: 0px;"),
        "sync modal should delegate its header chrome to the shared modal header"
    );
    assert!(
        sync.contains(
            "body := ModalBodyScrollArea {\n            x: 0px;\n            y: header.height;"
        ),
        "sync modal body must scroll from directly below the title bar"
    );
    assert!(
        modal_chrome.contains("mouse-drag-pan-enabled: false;"),
        "shared modal scroll host should require wheel or scrollbar input instead of direct drag scrolling"
    );
    assert!(
        modal_chrome.contains("horizontal-scrollbar-policy: always-off;")
            && modal_chrome.contains("scroll-body := Rectangle {")
            && modal_chrome.contains(
                "body-content-host := Rectangle {\n                x: root.resolved-frame-padding + root.resolved-content-padding-horizontal;"
            )
            && modal_chrome.contains("width: root.content-column-width;"),
        "shared modal scroll host should derive content widths from explicit shared measurements even after the extra body panel shell is removed"
    );
    assert!(
        modal_chrome.contains("private property <length> resolved-content-padding-bottom:")
            && modal_chrome.contains("body-content-host := Rectangle {")
            && modal_chrome.contains("background: root.viewport-surface;"),
        "shared modal scroll body should keep an explicit viewport surface, content host, and bottom breathing room for long forms"
    );
    assert!(
        sync.contains("footer := ModalFooterBar {\n            x: 0px;\n            y: parent.height - root.footer-height;"),
        "sync modal footer must stay pinned to the bottom edge"
    );
}

#[test]
fn complex_modal_bodies_wrap_content_in_an_explicit_shared_content_column() {
    let modal_chrome =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome");

    assert!(
        modal_chrome.contains("out property <length> content-column-width:")
            && modal_chrome.contains("body-content := VerticalLayout {"),
        "shared modal chrome must expose an explicit content-column-width and keep a dedicated body content layout"
    );

    for (path, marker) in [
        (
            "ui/components/sync-vault-modal.slint",
            "content-column := Rectangle {\n                width: body.content-column-width;",
        ),
        (
            "ui/components/assets-ssh-connection-modal.slint",
            "content-column := Rectangle {\n                width: body-scroll.content-column-width;",
        ),
        (
            "ui/components/assets-keychain-identity-modal.slint",
            "content-column := Rectangle {\n                width: body-scroll.content-column-width;",
        ),
        (
            "ui/components/assets-keychain-ssh-key-modal.slint",
            "content-column := Rectangle {\n                width: body-scroll.content-column-width;",
        ),
        (
            "ui/components/assets-snippet-modal.slint",
            "content-column := Rectangle {\n                width: body.content-column-width;",
        ),
    ] {
        let source = fs::read_to_string(path).unwrap_or_else(|_| panic!("read {path}"));
        assert!(
            source.contains(marker),
            "{path} must wrap modal body controls in an explicit content-column container"
        );
    }
}

#[test]
fn shared_modal_scroll_body_binds_explicit_viewport_dimensions() {
    let modal_chrome =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome");

    assert!(
        modal_chrome.contains("viewport-width: scroll-body.width;")
            && modal_chrome.contains("viewport-height: scroll-body.height;")
            && modal_chrome.contains("height: max(")
            && modal_chrome.contains("body-scroll.visible-height,"),
        "shared modal scroll body should follow the explicit ScrollView viewport sizing pattern so short viewports stay scrollable"
    );
}

#[test]
fn sync_modal_shell_uses_viewport_constrained_height_instead_of_a_fixed_620px_frame() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let blocking_shell =
        fs::read_to_string("ui/components/blocking-modal-shell.slint").expect("read modal shell");
    let sync_shell = app_window
        .split("if root.sync-modal-open : sync-modal-shell := BlockingModalShell {")
        .nth(1)
        .and_then(|section| section.split("if root.asset-modal-open").next())
        .expect("extract sync modal shell block");

    assert!(
        sync_shell.contains("modal-height: 680px;"),
        "sync modal shell should request a taller preferred height so desktop-sized windows keep the primary form fields reachable"
    );
    assert!(
        blocking_shell.contains("root.available-height > root.modal-height")
            && blocking_shell.contains("root.available-height - root.modal-height <= 40px")
            && blocking_shell.contains("? root.available-height"),
        "blocking modal shell should expand near-constrained forms to the full available viewport height"
    );
}

#[test]
fn sync_modal_uses_distinct_header_body_and_footer_surfaces() {
    let sync = fs::read_to_string("ui/components/sync-vault-modal.slint").expect("read sync modal");

    assert!(
        sync.contains("prominent: true;"),
        "sync modal should request the stronger shared header chrome"
    );
    assert!(
        sync.contains("viewport-surface: ThemeTokens.modal-body-surface;")
            && sync.contains("panel-surface: ThemeTokens.modal-body-surface;"),
        "sync modal should render the body content inside the shared elevated modal body surface"
    );
    assert!(
        sync.contains("footer := ModalFooterBar {"),
        "sync modal footer should stay on the shared footer scaffold"
    );
}

#[test]
fn long_form_modals_share_common_modal_chrome_primitives() {
    let modal_chrome =
        fs::read_to_string("ui/components/modal-chrome.slint").expect("read shared modal chrome");
    let sync = fs::read_to_string("ui/components/sync-vault-modal.slint").expect("read sync");
    let snippet =
        fs::read_to_string("ui/components/assets-snippet-modal.slint").expect("read snippet");
    let ssh =
        fs::read_to_string("ui/components/assets-ssh-connection-modal.slint").expect("read ssh");
    let keychain_identity =
        fs::read_to_string("ui/components/assets-keychain-identity-modal.slint")
            .expect("read keychain identity");
    let keychain_ssh_key = fs::read_to_string("ui/components/assets-keychain-ssh-key-modal.slint")
        .expect("read keychain ssh key");
    let open_saved =
        fs::read_to_string("ui/components/open-saved-ssh-modal.slint").expect("read open saved");

    assert!(
        modal_chrome.contains("export component ModalHeaderBar"),
        "shared modal chrome should export a reusable header bar"
    );
    assert!(
        modal_chrome.contains("export component ModalFooterBar"),
        "shared modal chrome should export a reusable footer bar"
    );
    assert!(
        modal_chrome.contains("export component ModalBodyScrollArea"),
        "shared modal chrome should export a reusable scroll body wrapper"
    );

    for modal in [
        ("sync", &sync),
        ("snippet", &snippet),
        ("ssh", &ssh),
        ("keychain identity", &keychain_identity),
        ("keychain ssh key", &keychain_ssh_key),
        ("open saved ssh", &open_saved),
    ] {
        assert!(
            modal.1.contains("modal-chrome.slint")
                && modal.1.contains("ModalHeaderBar")
                && modal.1.contains("ModalFooterBar"),
            "{} modal should reuse the shared modal header/footer primitives",
            modal.0
        );
    }

    for modal in [
        ("sync", &sync),
        ("snippet", &snippet),
        ("ssh", &ssh),
        ("keychain identity", &keychain_identity),
        ("keychain ssh key", &keychain_ssh_key),
    ] {
        assert!(
            modal.1.contains("ModalBodyScrollArea {"),
            "{} modal should render its long form content inside the shared scroll body wrapper",
            modal.0
        );
    }
}

#[test]
fn keychain_identity_modal_scrolls_body_inside_the_shared_scaffold() {
    let identity_modal = fs::read_to_string("ui/components/assets-keychain-identity-modal.slint")
        .expect("read keychain identity modal");

    assert!(
        identity_modal.contains("ModalBodyScrollArea {"),
        "keychain identity modal should use the shared scroll body wrapper"
    );
    assert!(
        identity_modal.contains("footer := ModalFooterBar {"),
        "keychain identity modal should pin its actions inside the shared footer bar"
    );
}

#[test]
fn keychain_identity_modal_omits_dead_create_new_key_button() {
    let identity_modal = fs::read_to_string("ui/components/assets-keychain-identity-modal.slint")
        .expect("read keychain identity modal");

    assert!(identity_modal.contains("Use Existing Key"));
    assert!(!identity_modal.contains("Create New Key"));
    assert!(!identity_modal.contains("\"create-ssh-key\""));
}

#[test]
fn long_form_modal_shells_use_viewport_constrained_heights_instead_of_fixed_frames() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let blocking_shell =
        fs::read_to_string("ui/components/blocking-modal-shell.slint").expect("read modal shell");

    for expected_height in [
        "modal-height: 600px;",
        "modal-height: 680px;",
        "modal-height: 720px;",
    ] {
        assert!(
            app_window.contains(expected_height),
            "form modal shells should request taller preferred frames using `{expected_height}` so desktop windows surface the next form field before scrolling"
        );
    }

    assert!(
        blocking_shell.contains(
            "viewport-margin: root.width < 960px || root.height - root.host-titlebar-height < 720px ? 8px : 24px;"
        ),
        "blocking modal shell should tighten outer margins on constrained viewports"
    );
    assert!(
        blocking_shell.contains("root.available-height > root.modal-height")
            && blocking_shell.contains("? root.available-height"),
        "blocking modal shell should let near-full-height forms use the remaining viewport instead of preserving a too-small preferred height"
    );
}

#[test]
fn blocking_modal_children_bind_overlay_parent_dimensions() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(app_window.contains(
        "asset-folder-modal-overlay := AssetsFolderCreateModal {\n            x: 0px;\n            y: 0px;\n            width: asset-folder-modal-shell.content-width;"
    ));
    assert!(app_window.contains(
        "asset-ssh-modal-overlay := AssetsSshConnectionModal {\n            x: 0px;\n            y: 0px;\n            width: asset-ssh-modal-shell.content-width;"
    ));
    assert!(app_window.contains(
        "asset-rename-modal-overlay := AssetsRenameModal {\n            x: 0px;\n            y: 0px;\n            width: asset-rename-modal-shell.content-width;"
    ));
    assert!(app_window.contains(
        "asset-delete-confirm-modal-overlay := AssetsDeleteConfirmModal {\n            x: 0px;\n            y: 0px;\n            width: asset-delete-confirm-modal-shell.content-width;"
    ));
    assert!(app_window.contains(
        "ssh-host-key-modal-overlay := SshHostKeyConfirmModal {\n            x: 0px;\n            y: 0px;\n            width: ssh-host-key-modal-shell.content-width;"
    ));
}

#[test]
fn rename_modal_is_a_simple_name_input_for_assets_and_sftp() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let rename_modal =
        fs::read_to_string("ui/components/assets-rename-modal.slint").expect("read rename modal");
    let bootstrap =
        fs::read_to_string("src/app/bootstrap/assets_keychain.rs").expect("read assets bootstrap");

    for contract in [
        "in-out property <string> asset-rename-modal-dialog-title: \"Rename\";",
        "dialog-title: root.asset-rename-modal-dialog-title;",
        "item-name: root.asset-rename-modal-name;",
        "validation-message: root.asset-rename-modal-validation-message;",
    ] {
        assert!(
            app_window.contains(contract),
            "AppWindow should keep the shared rename modal contract minimal through `{contract}`"
        );
    }

    for removed_contract in [
        "asset-rename-modal-subtitle",
        "asset-rename-modal-field-helper",
        "asset-rename-modal-input-helper",
        "subtitle: root.asset-rename-modal-subtitle;",
        "field-helper: root.asset-rename-modal-field-helper;",
        "input-helper: root.asset-rename-modal-input-helper;",
    ] {
        assert!(
            !app_window.contains(removed_contract),
            "AppWindow should not project over-explained rename copy through `{removed_contract}`"
        );
    }

    for contract in [
        "in property <string> field-label:",
        "label: root.field-label;",
        "helper-text: root.validation-message;",
        "name-field.select-all();",
    ] {
        assert!(
            rename_modal.contains(contract),
            "AssetsRenameModal should keep only the editable name field behavior through `{contract}`"
        );
    }

    for removed_contract in [
        "DialogSectionCard",
        "in property <string> subtitle:",
        "in property <string> field-helper:",
        "in property <string> input-helper:",
        "text: root.field-label;",
        "text: root.field-helper;",
        ": root.input-helper;",
    ] {
        assert!(
            !rename_modal.contains(removed_contract),
            "AssetsRenameModal should not render over-explained rename chrome through `{removed_contract}`"
        );
    }

    for removed_copy in [
        "Rename Remote Item",
        "Remote name",
        "Rename the selected remote file or folder on the SFTP host.",
        "Use a valid filename for the current remote directory.",
        "Applied to the selected remote file or folder.",
        "Rename the selected asset without breaking the surrounding workspace flow.",
        "Use a stable name so the tree, search results, and quick actions stay easy to scan.",
        "Shown in the asset tree and related context menus.",
    ] {
        assert!(
            !bootstrap.contains(removed_copy),
            "rename sync should not project verbose or surface-specific copy `{removed_copy}`"
        );
    }

    assert!(
        bootstrap.contains("window.set_asset_rename_modal_dialog_title(\"Rename\".into());")
            && bootstrap.contains("window.set_asset_rename_modal_field_label(\"Name\".into());"),
        "rename sync should use the same simple title and field label for asset and SFTP rename"
    );
}

#[test]
fn ssh_form_field_contract_allows_horizontal_rows_to_shrink_without_overflow() {
    let chrome = fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome");

    assert!(
        chrome.contains("export component DialogTextField inherits Rectangle {")
            && chrome.contains("min-width: 0px;")
            && chrome.contains("preferred-width: 0px;"),
        "shared dialog text fields should opt into shrinking so SSH modal horizontal rows do not steal width from siblings"
    );
}

#[test]
fn modal_refinement_regression_contract() {
    let chrome = fs::read_to_string("ui/components/modal-chrome.slint").expect("read modal chrome");
    let folder_modal = fs::read_to_string("ui/components/assets-folder-create-modal.slint")
        .expect("read folder modal");
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");
    let snippet_modal =
        fs::read_to_string("ui/components/assets-snippet-modal.slint").expect("read snippet modal");

    assert!(
        !chrome.contains("action-rail := Rectangle {"),
        "the shared footer should stop rendering the heavy inner action rail once the modal chrome is slimmed down"
    );
    assert!(
        !folder_modal.contains("DialogSectionCard"),
        "the single-field folder modal should not keep an unnecessary card wrapper around its only form control"
    );
    assert!(
        !ssh_modal.contains(
            "Desktop-native SSH profiles with calmer sections, consistent actions, and predictable dismissal."
        ),
        "ssh modal should replace the current slogan-style subtitle with product copy that fits a real header"
    );
    assert!(
        !snippet_modal
            .contains("Keep command snippets compact, searchable, and ready for quick execution."),
        "snippet modal should replace the current slogan-style subtitle with product copy that fits a real header"
    );
}

#[test]
fn create_modals_project_inline_validation_message_and_confirm_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "Folder 1");
    assert_eq!(app.get_asset_modal_validation_message().as_str(), "");
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_confirm_asset_modal_requested();

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "SSH Connection 1");
    assert_eq!(app.get_asset_modal_validation_message().as_str(), "");
    assert!(!app.get_asset_modal_can_confirm());

    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Folder 1".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());

    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Folder 1");
    assert_eq!(
        app.get_asset_modal_validation_message().as_str(),
        "Name already exists in this folder."
    );
    assert!(!app.get_asset_modal_can_confirm());
}

#[test]
fn keychain_identity_modal_blocks_confirm_until_required_fields_are_present() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_sidebar_destination_selected("keychain".into());
    app.invoke_assets_create_action_selected("new-identity".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-keychain-identity");
    assert_eq!(app.get_keychain_asset_items().row_count(), 0);
    assert!(!app.get_asset_modal_can_confirm());

    app.invoke_confirm_asset_modal_requested();
    assert_eq!(app.get_keychain_asset_items().row_count(), 0);

    app.invoke_keychain_identity_modal_draft_changed("username".into(), "ops".into());
    assert!(!app.get_asset_modal_can_confirm());

    app.invoke_keychain_identity_modal_draft_changed("password".into(), "secret".into());
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_confirm_asset_modal_requested();

    let rows = app.get_keychain_asset_items();
    assert_eq!(rows.row_count(), 1);
    let row = rows.row_data(0).expect("created keychain identity row");
    assert_eq!(row.kind.as_str(), "identity");
    assert_eq!(row.label.as_str(), "Identity 1");
}

#[test]
fn ssh_modal_confirm_updates_runtime_tree_and_persists_ssh_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(ModalAssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> =
        Rc::new(RecordingModalAssetRepo::new(Rc::clone(&repo_state)));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("port".into(), "2022".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_draft_changed("environment".into(), "prod".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_type".into(), "socks5".into());
    app.invoke_asset_ssh_modal_draft_changed(
        "proxy_socks5_host".into(),
        "proxy.example.net".into(),
    );
    app.invoke_asset_ssh_modal_draft_changed("proxy_socks5_port".into(), "1080".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_socks5_username".into(), "ops-proxy".into());
    app.invoke_confirm_asset_modal_requested();

    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(
        app.get_console_asset_items()
            .row_data(0)
            .unwrap()
            .label
            .as_str(),
        "Prod Bastion"
    );

    let save_attempts = &repo_state.borrow().save_attempts;
    assert_eq!(save_attempts.len(), 1);
    assert_eq!(save_attempts[0].root_ids.len(), 1);
    let node = save_attempts[0]
        .nodes
        .get(save_attempts[0].root_ids[0].as_str())
        .unwrap();
    match &node.payload {
        PersistedAssetPayload::SshConnection(spec) => {
            assert_eq!(spec.host, "10.0.0.12");
            assert_eq!(spec.user, "ops");
            assert_eq!(spec.port, "2022");
            assert_eq!(spec.environment, "prod");
            assert_eq!(
                spec.proxy,
                PersistedAssetSshProxySpec::Socks5(PersistedAssetSocks5ProxySpec {
                    host: "proxy.example.net".into(),
                    port: "1080".into(),
                    username: "ops-proxy".into(),
                    password_credential_ref: None,
                })
            );
        }
        PersistedAssetPayload::Folder
        | PersistedAssetPayload::SnippetPackage
        | PersistedAssetPayload::Snippet(_) => panic!("expected ssh payload"),
    }
}

#[test]
fn ssh_modal_confirm_persists_existing_ssh_connection_proxy_selection() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(ModalAssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> =
        Rc::new(RecordingModalAssetRepo::new(Rc::clone(&repo_state)));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Upstream Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.10".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    let upstream_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved upstream ssh asset")
        .id
        .to_string();

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Target Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.11".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_type".into(), "ssh-asset".into());
    app.invoke_asset_ssh_modal_draft_changed(
        "proxy_ssh_asset_id".into(),
        upstream_id.clone().into(),
    );
    app.invoke_confirm_asset_modal_requested();

    let save_attempts = &repo_state.borrow().save_attempts;
    assert_eq!(save_attempts.len(), 2);
    let target_id = save_attempts[1]
        .root_ids
        .iter()
        .find(|id| id.as_str() != upstream_id.as_str())
        .expect("target asset id");
    let node = save_attempts[1].nodes.get(target_id.as_str()).unwrap();
    match &node.payload {
        PersistedAssetPayload::SshConnection(spec) => {
            assert_eq!(
                spec.proxy,
                PersistedAssetSshProxySpec::SshAsset {
                    asset_id: upstream_id,
                }
            );
        }
        PersistedAssetPayload::Folder
        | PersistedAssetPayload::SnippetPackage
        | PersistedAssetPayload::Snippet(_) => panic!("expected ssh payload"),
    }
}
