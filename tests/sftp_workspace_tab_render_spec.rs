use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::time::Duration;

use i_slint_backend_testing::ElementHandle;
use mica_term::{AppWindow, SftpBreadcrumbItem};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, PhysicalSize, VecModel};

fn click_element(app: &AppWindow, element: &ElementHandle) {
    let position = LogicalPosition::new(
        element.absolute_position().x + element.size().width / 2.0,
        element.absolute_position().y + element.size().height / 2.0,
    );
    let window = app.window();
    window.dispatch_event(WindowEvent::PointerMoved { position });
    window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    slint::platform::update_timers_and_animations();
}

#[test]
fn workspace_pane_source_branches_to_sftp_workspace_host() {
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");

    assert!(
        workspace_pane
            .contains("import { SftpWorkspaceHost } from \"./sftp-workspace-host.slint\";")
            || workspace_pane.contains(
                "import { SftpBreadcrumbItem, SftpWorkspaceHost } from \"./sftp-workspace-host.slint\";"
            ),
        "WorkspacePane should import the dedicated SFTP workspace host"
    );
    assert!(
        workspace_pane.contains(
            "if root.workspace-session-host-mode == \"sftp\" : sftp-host := SftpWorkspaceHost {"
        ),
        "WorkspacePane should switch to SftpWorkspaceHost when the active workspace tab is an sftp tab"
    );
    assert!(
        workspace_pane.contains("session-title: root.workspace-session-title;")
            && workspace_pane.contains("session-subtitle: root.workspace-session-subtitle;")
            && workspace_pane.contains("session-state: root.workspace-session-state;"),
        "WorkspacePane should forward the active workspace title/subtitle/state into the SFTP workspace host"
    );
    for contract in [
        "workspace-sftp-path: root.workspace-sftp-path;",
        "workspace-sftp-items: root.workspace-sftp-items;",
        "workspace-sftp-selected-entry-ids: root.workspace-sftp-selected-entry-ids;",
        "workspace-sftp-item-selected(item-id, ctrl, shift) => {",
        "workspace-sftp-back-requested => {",
        "workspace-sftp-path-submitted(path) => {",
        "workspace-sftp-item-activated(item-id, item-kind) => {",
        "workspace-sftp-context-menu-requested(item-id, item-kind, anchor-x, anchor-y) => {",
    ] {
        assert!(
            workspace_pane.contains(contract),
            "WorkspacePane should thread the live workspace SFTP contract `{contract}` into SftpWorkspaceHost"
        );
    }
}

#[test]
fn app_window_source_threads_workspace_sftp_contract_into_workspace_pane() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    for contract in [
        "in-out property <[SftpBreadcrumbItem]> workspace-sftp-breadcrumb-items: [];",
        "callback workspace-sftp-path-submitted(string);",
        "callback workspace-sftp-breadcrumb-clicked(string);",
        "callback workspace-sftp-item-selected(string, bool, bool);",
        "workspace-sftp-breadcrumb-items: root.workspace-sftp-breadcrumb-items;",
        "workspace-sftp-path-submitted(path) => {",
        "root.workspace-sftp-path-submitted(path);",
        "workspace-sftp-breadcrumb-clicked(path) => {",
        "root.workspace-sftp-breadcrumb-clicked(path);",
        "workspace-sftp-item-activated(item-id, item-kind) => {",
        "workspace-sftp-item-selected(item-id, ctrl, shift) => {",
        "root.workspace-sftp-item-selected(item-id, ctrl, shift);",
        "workspace-sftp-context-menu-requested(item-id, item-kind, anchor-x, anchor-y) => {",
    ] {
        assert!(
            app_window.contains(contract),
            "AppWindow should thread workspace SFTP contract `{contract}` into WorkspacePane"
        );
    }
}

#[test]
fn sftp_workspace_host_source_exposes_core_file_table_headers() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        source.contains("export component SftpWorkspaceHost"),
        "SFTP workspace host should live in its own component"
    );
    for label in ["Name", "Type", "Size", "Modified"] {
        assert!(
            source.contains(&format!("text: \"{label}\""))
                || source.contains(&format!("label: \"{label}\"")),
            "SFTP workspace host should expose a `{label}` table header"
        );
    }
    assert!(
        source.contains("workspace-sftp-path")
            && source.contains("workspace-sftp-items")
            && source.contains("workspace-sftp-breadcrumb-clicked")
            && source.contains("workspace-sftp-item-activated")
            && source.contains("workspace-sftp-context-menu-requested"),
        "SFTP workspace host should expose a real workspace browser contract instead of a passive title shell"
    );
    assert!(
        !source
            .contains("Expand a Quick Browser session to bring file work into the main workspace.")
            && !source.contains("Open a Quick Browser"),
        "SFTP workspace host should stop rendering the old placeholder copy once the workspace becomes a real browser surface"
    );
}

#[test]
fn sftp_workspace_host_source_defines_productized_layout_markers() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    for marker in [
        "workspace-header :=",
        "workspace-toolbar :=",
        "workspace-breadcrumb-shell :=",
        "workspace-file-table :=",
        "workspace-statusbar :=",
    ] {
        assert!(
            source.contains(marker),
            "SFTP workspace host should expose the productized layout marker `{marker}` so the UI contract can freeze the compact shell structure"
        );
    }
}

#[test]
fn sftp_workspace_host_source_freezes_icon_toolbar_breadcrumb_root_and_responsive_columns() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    for contract in [
        "arrow-hook-up-left-20-regular.svg",
        "arrow-sync-20-regular.svg",
        "folder-20-regular.svg",
        "arrow-upload-20-regular.svg",
        "edit-20-regular.svg",
        "function workspace-width-tier() -> string {",
        "root.workspace-width-tier()",
        "crumb.path == \"/\"",
        "text: \"Permissions\"",
        "text: \"Owner\"",
        "text: \"Group\"",
    ] {
        assert!(
            source.contains(contract),
            "SFTP workspace host should freeze the productized contract `{contract}` for icon-first toolbar, stable root breadcrumb, and responsive optional columns"
        );
    }
}

#[test]
fn sftp_workspace_host_source_uses_runtime_shell_selection_tokens() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    for contract in [
        "in property <color> workspace-session-frame-surface: ThemeTokens.terminal-frame-background;",
        "in property <color> shell-sidebar-item-selected: ThemeTokens.sidebar-item-selected-background;",
        "in property <color> shell-sidebar-item-selected-border: ThemeTokens.sidebar-item-selected-border;",
        "background: item.selected ? root.shell-sidebar-item-selected",
        "background: root.shell-sidebar-item-selected-border;",
    ] {
        assert!(
            source.contains(contract),
            "SFTP workspace host should use runtime-projected shell/session contract `{contract}` instead of inventing a detached palette or heavy boxed selection"
        );
    }
}

#[test]
fn sftp_workspace_host_source_wires_workspace_only_interactions() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    for contract in [
        "callback workspace-sftp-path-edit-requested();",
        "callback workspace-sftp-breadcrumb-clicked(string);",
        "callback workspace-sftp-viewport-changed(length, length);",
        "callback workspace-sftp-retry-requested();",
        "callback local-action-requested(string);",
        "root.workspace-sftp-path-submitted(self.text);",
        "root.workspace-sftp-breadcrumb-clicked(crumb.path);",
        "root.workspace-sftp-viewport-changed(self.viewport-y, self.visible-height);",
        "root.workspace-sftp-context-menu-requested(",
        "root.local-action-requested(\"reconnect-sftp-workspace\");",
    ] {
        assert!(
            source.contains(contract),
            "SFTP workspace host should wire interactive contract `{contract}` instead of remaining a passive shell"
        );
    }
}

#[test]
fn workspace_row_context_menu_maps_rows_to_sftp_target_kinds() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        source.contains("item.kind == \"parent-directory\"")
            && source.contains("\"sftp-blank\"")
            && source.contains("\"sftp-directory\"")
            && source.contains("\"sftp-file\""),
        "workspace row right-clicks should translate row kinds into the shared SFTP context-target ids instead of forwarding raw `directory`/`file` kinds into the assets menu router"
    );
}

#[test]
fn workspace_selection_modifier_contract_threads_ctrl_and_shift_across_the_workspace_chain() {
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let bootstrap = fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read sftp bootstrap");

    assert!(
        host.contains("callback workspace-sftp-item-selected(string, bool, bool);")
            && host.contains("event.modifiers.control")
            && host.contains("event.modifiers.shift")
            && host.contains("root.workspace-sftp-item-selected("),
        "workspace rows should emit ctrl/shift modifier state from the host instead of collapsing every click into an unqualified single-select callback"
    );
    assert!(
        workspace_pane.contains("callback workspace-sftp-item-selected(string, bool, bool);")
            && workspace_pane.contains("workspace-sftp-item-selected(item-id, ctrl, shift) => {")
            && workspace_pane.contains("root.workspace-sftp-item-selected(item-id, ctrl, shift);"),
        "WorkspacePane should forward ctrl/shift selection modifiers all the way through the workspace host contract"
    );
    assert!(
        app_window.contains("callback workspace-sftp-item-selected(string, bool, bool);")
            && app_window.contains("workspace-sftp-item-selected(item-id, ctrl, shift) => {")
            && app_window.contains("root.workspace-sftp-item-selected(item-id, ctrl, shift);"),
        "AppWindow should keep the same ctrl/shift selection callback shape so bootstrap can distinguish click, Ctrl+click, and Shift+click"
    );
    assert!(
        bootstrap.contains("window.on_workspace_sftp_item_selected(move |entry_id, ctrl, shift| {"),
        "bootstrap should receive the ctrl/shift selection modifiers instead of only an item id"
    );
}

#[test]
fn workspace_breadcrumb_shell_click_requests_path_edit_mode() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    app.set_workspace_session_host_mode("sftp".into());
    app.set_workspace_sftp_actions_enabled(true);
    app.set_workspace_sftp_path("/home/wwwroot".into());
    app.set_workspace_sftp_breadcrumb_items(ModelRc::new(VecModel::from(vec![
        SftpBreadcrumbItem {
            label: "/".into(),
            path: "/".into(),
            active: false,
        },
        SftpBreadcrumbItem {
            label: "home".into(),
            path: "/home".into(),
            active: false,
        },
        SftpBreadcrumbItem {
            label: "wwwroot".into(),
            path: "/home/wwwroot".into(),
            active: true,
        },
    ])));

    let app_handle = app.as_weak();
    app.on_workspace_sftp_path_edit_requested(move || {
        let app = app_handle.unwrap();
        app.set_workspace_sftp_path_editing(true);
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_workspace_sftp_focus_sequence(1);
    app.invoke_focus_workspace_primary();
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    slint::platform::update_timers_and_animations();

    let shell =
        ElementHandle::find_by_element_id(&app, "SftpWorkspaceHost::workspace-breadcrumb-shell")
            .chain(ElementHandle::find_by_element_id(
                &app,
                "workspace-breadcrumb-shell",
            ))
            .next()
            .expect("workspace breadcrumb shell");
    click_element(&app, &shell);

    assert!(
        app.get_workspace_sftp_path_editing(),
        "clicking the workspace breadcrumb shell should enter path editing instead of forcing users onto the pencil affordance"
    );
}

#[test]
fn workspace_home_breadcrumb_click_routes_to_the_home_path() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    app.set_workspace_session_host_mode("sftp".into());
    app.set_workspace_sftp_actions_enabled(true);
    app.set_workspace_sftp_path("/home/wwwroot".into());
    app.set_workspace_sftp_breadcrumb_items(ModelRc::new(VecModel::from(vec![
        SftpBreadcrumbItem {
            label: "/".into(),
            path: "/".into(),
            active: false,
        },
        SftpBreadcrumbItem {
            label: "home".into(),
            path: "/home".into(),
            active: false,
        },
        SftpBreadcrumbItem {
            label: "wwwroot".into(),
            path: "/home/wwwroot".into(),
            active: true,
        },
    ])));

    let clicked_path = Rc::new(RefCell::new(None::<String>));
    let clicked_path_ref = Rc::clone(&clicked_path);
    app.on_workspace_sftp_breadcrumb_clicked(move |path| {
        clicked_path_ref.replace(Some(path.to_string()));
    });

    app.show().expect("show app window");
    slint::platform::update_timers_and_animations();

    let mut crumb_targets = ElementHandle::find_by_element_id(
        &app,
        "SftpWorkspaceHost::workspace-breadcrumb-crumb-touch",
    )
    .chain(ElementHandle::find_by_element_id(
        &app,
        "workspace-breadcrumb-crumb-touch",
    ))
    .collect::<Vec<_>>();
    crumb_targets.sort_by(|left, right| {
        left.absolute_position()
            .x
            .partial_cmp(&right.absolute_position().x)
            .expect("workspace breadcrumb x position")
    });

    let home_crumb = crumb_targets.get(1).expect("workspace home breadcrumb");
    click_element(&app, home_crumb);

    assert_eq!(
        clicked_path.borrow().as_deref(),
        Some("/home"),
        "clicking the `home` workspace breadcrumb should route the canonical `/home` path instead of reusing the current leaf path or entering edit mode"
    );
}

#[test]
fn workspace_ctrl_l_requests_path_edit_mode() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");

    assert!(
        app_window.contains("public function focus-workspace-primary()")
            && app_window.contains("main-workspace.restore-primary-focus();")
            && workspace_pane.contains("workspace-sftp-shortcut-anchor := TextInput {")
            && workspace_pane.contains("root.workspace-sftp-path-edit-requested();"),
        "Ctrl+L should have a dedicated workspace shortcut-focus handoff and callback route instead of depending on the hidden terminal-only input path"
    );
}

#[test]
fn workspace_ctrl_a_routes_to_select_all_sftp_local_action() {
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");

    assert!(
        workspace_pane
            .contains("event.modifiers.control && !event.modifiers.shift && !event.modifiers.alt")
            && workspace_pane.contains(
                "(event.text == \"a\" || event.text == \"A\" || event.text == \"\\u{1}\")"
            )
            && workspace_pane.contains("root.local-action-requested(\"select-all-sftp\");"),
        "workspace SFTP should claim Ctrl+A on the hidden workspace shortcut anchor and route it through the existing local-action pipeline instead of leaving select-all trapped in the context menu only"
    );
}

#[test]
fn workspace_blank_area_left_click_routes_to_clear_selection_local_action() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        source.contains(
            "if event.kind == PointerEventKind.down && event.button == PointerEventButton.left {"
        ) && source.contains("root.local-action-requested(\"clear-selection-sftp\");"),
        "clicking the blank workspace file area should clear the current SFTP selection through the shared local-action channel instead of leaving stale rows selected until another file is clicked"
    );
}

#[test]
fn workspace_path_escape_is_a_cancel_instead_of_a_hidden_resubmit() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        !source.contains("root.workspace-sftp-path-submitted(root.workspace-sftp-path);"),
        "Esc in the workspace path editor should cancel editing and restore the canonical path instead of routing a hidden submit of the current path"
    );
    assert!(
        source.contains("callback workspace-sftp-path-cancelled();")
            && workspace_pane.contains("callback workspace-sftp-path-cancelled();")
            && workspace_pane.contains("workspace-sftp-path-cancelled => {")
            && workspace_pane.contains("root.workspace-sftp-path-cancelled();")
            && app_window.contains("callback workspace-sftp-path-cancelled();")
            && app_window.contains("workspace-sftp-path-cancelled => {")
            && app_window.contains("root.workspace-sftp-path-cancelled();"),
        "Esc cancel should travel through an explicit workspace path-cancel contract instead of smuggling a submit through the host-only text field"
    );
}

#[test]
fn workspace_path_edit_mode_focuses_and_selects_the_full_canonical_path() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        source.contains("init => {")
            && source.contains("self.focus();")
            && source.contains("self.select-all();"),
        "entering workspace path edit mode should immediately focus the input and select the full canonical path so Ctrl+L and shell clicks behave like a real location bar"
    );
}

#[test]
fn workspace_toolbar_tooltips_must_route_through_the_shared_shell_overlay() {
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        host.contains("callback tooltip-open-requested(")
            && host.contains("callback tooltip-close-requested(")
            && host.contains("tooltip-source-id: \"workspace-sftp-refresh\";")
            && host.contains("tooltip-source-id: \"workspace-sftp-upload\";")
            && host.contains("tooltip-source-id: \"workspace-sftp-new-folder\";")
            && host.contains("tooltip-source-id: \"workspace-sftp-transfer-center\";")
            && workspace_pane.contains("callback workspace-sftp-tooltip-open-requested(string, string, length, length, length);")
            && workspace_pane.contains("callback workspace-sftp-tooltip-close-requested(string);")
            && workspace_pane.contains("tooltip-open-requested(source-id, text, anchor-x, anchor-y, anchor-width) => {")
            && workspace_pane.contains("root.workspace-sftp-tooltip-open-requested(source-id, text, anchor-x, anchor-y, anchor-width);")
            && workspace_pane.contains("tooltip-close-requested(source-id) => {")
            && workspace_pane.contains("root.workspace-sftp-tooltip-close-requested(source-id);")
            && app_window.contains("in-out property <bool> workspace-sftp-tooltip-visible: false;")
            && app_window.contains("in-out property <string> workspace-sftp-tooltip-text: \"\";")
            && app_window.contains("workspace-sftp-tooltip-overlay := TitlebarTooltip {")
            && app_window.contains("text: root.workspace-sftp-tooltip-text;")
            && app_window.contains("tooltip-visible: root.workspace-sftp-tooltip-visible;"),
        "workspace toolbar actions should use the shared AppWindow tooltip overlay contract instead of local tooltip text that never owns a real overlay"
    );
}

#[test]
fn workspace_toolbar_disabled_reason_must_keep_hover_tooltips_alive() {
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        host.contains("function effective-tooltip-text() -> string {")
            && host.contains("disabled-tooltip-text")
            && host.contains("out property <bool> tooltip-active:")
            && host.contains("if root.enabled {")
            && app_window
                .contains("in-out property <string> workspace-sftp-toolbar-disabled-reason: \"\";")
            && !host.contains("out property <bool> tooltip-active: root.enabled &&"),
        "workspace toolbar buttons should separate click-disable from hover/focus tooltip ownership so disconnected actions can still explain themselves"
    );
}

#[test]
fn workspace_toolbar_action_width_tiers_switch_between_full_labels_and_icon_only() {
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        host.contains("function workspace-toolbar-action-labels-visible() -> bool {")
            && host.contains("return root.workspace-width-tier() == \"wide\";")
            && host.contains("transfer-center-button := WorkspaceActionButton {")
            && host
                .contains("label: root.workspace-toolbar-action-labels-visible() ? \"Upload\" : \"\";")
            && host.contains(
                "label: root.workspace-toolbar-action-labels-visible() ? \"New Folder\" : \"\";",
            )
            && host.contains(
                "label: root.workspace-toolbar-action-labels-visible() ? \"Transfer Center\" : \"\";",
            )
            && !host.contains("return root.width < 1120px;"),
        "workspace toolbar actions should align with the shared width tiers so medium-width workspaces fall back to icon-only affordances before labels overflow or clip"
    );
}

#[test]
fn workspace_compact_width_hides_optional_size_column_before_name_column_is_clipped() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    app.set_workspace_session_host_mode("sftp".into());
    app.set_workspace_sftp_actions_enabled(true);
    app.window().set_size(PhysicalSize::new(860, 720));
    app.show().expect("show app window");
    slint::platform::update_timers_and_animations();

    let name_header =
        ElementHandle::find_by_element_id(&app, "SftpWorkspaceHost::workspace-table-header-name")
            .chain(ElementHandle::find_by_element_id(
                &app,
                "workspace-table-header-name",
            ))
            .next()
            .expect("workspace name header");

    assert!(
        name_header.size().width >= 160.0,
        "compact workspace widths should protect the primary Name column instead of letting optional columns squeeze it down to an unreadable stub"
    );

    let size_header =
        ElementHandle::find_by_element_id(&app, "SftpWorkspaceHost::workspace-table-header-size")
            .chain(ElementHandle::find_by_element_id(
                &app,
                "workspace-table-header-size",
            ))
            .next();

    assert!(
        size_header.is_none(),
        "compact workspace widths should collapse the optional Size column entirely so Name remains the stable primary column"
    );
}

#[test]
fn workspace_viewport_contract_is_projected_from_the_vm_into_the_host() {
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        app_window.contains("in-out property <length> workspace-sftp-viewport-y: 0px;")
            && app_window.contains("in-out property <length> workspace-sftp-row-height: 40px;")
            && app_window.contains("in-out property <int> workspace-sftp-total-row-count: 0;")
            && app_window.contains("workspace-sftp-viewport-y <=> root.workspace-sftp-viewport-y;")
            && app_window.contains("workspace-sftp-row-height: root.workspace-sftp-row-height;")
            && app_window
                .contains("workspace-sftp-total-row-count: root.workspace-sftp-total-row-count;"),
        "AppWindow should own the projected workspace viewport, row-height, and total-row-count contract so tab restore and status summaries are driven by the view-model instead of host-private state"
    );
    assert!(
        workspace_pane.contains("in-out property <length> workspace-sftp-viewport-y: 0px;")
            && workspace_pane.contains("in property <length> workspace-sftp-row-height: 40px;")
            && workspace_pane.contains("in property <int> workspace-sftp-total-row-count: 0;")
            && workspace_pane
                .contains("workspace-sftp-viewport-y <=> root.workspace-sftp-viewport-y;")
            && workspace_pane
                .contains("workspace-sftp-row-height: root.workspace-sftp-row-height;")
            && workspace_pane
                .contains("workspace-sftp-total-row-count: root.workspace-sftp-total-row-count;"),
        "WorkspacePane should thread the controlled viewport and full-count contract straight through to SftpWorkspaceHost"
    );
    assert!(
        host.contains("in-out property <length> workspace-sftp-viewport-y: 0px;")
            && host.contains("in property <length> workspace-sftp-row-height: 40px;")
            && host.contains("in property <int> workspace-sftp-total-row-count: 0;")
            && host.contains("viewport-y <=> root.workspace-sftp-viewport-y;")
            && host.contains("height: root.workspace-sftp-row-height;")
            && !host.contains("private property <length> list-viewport-y: 0px;"),
        "SftpWorkspaceHost should consume a Rust-controlled viewport and row-height contract instead of hiding a private viewport cache that drifts away from session restore/reset policy"
    );
}

#[test]
fn workspace_statusbar_item_count_must_not_use_the_visible_row_slice_length() {
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        host.contains("return \"\" + root.workspace-sftp-total-row-count + \" items\";")
            && !host.contains("return \"\" + root.workspace-sftp-items.length + \" items\";"),
        "workspace status summaries should use the projected full row count instead of the currently visible virtual window length"
    );
}

#[test]
fn workspace_statusbar_contract_keeps_connection_counts_path_binding_and_transfer_entry_visible() {
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    for contract in [
        "text: root.status-label();",
        "text: root.statusbar-item-count();",
        "text: root.statusbar-selection-count();",
        "text: root.workspace-sftp-path == \"\" ? \"/\" : root.workspace-sftp-path;",
        "text: root.statusbar-binding-summary();",
        "transfer-entry := Rectangle {",
        "text: root.transfer-entry-label();",
    ] {
        assert!(
            host.contains(contract),
            "workspace status bar should keep the full contract `{contract}` visible instead of collapsing semantics into a single vague footer label"
        );
    }
}

#[test]
fn workspace_statusbar_compacts_idle_transfer_copy_instead_of_repeating_toolbar_ctas() {
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        host.contains("function statusbar-binding-summary() -> string {")
            && host.contains("root.workspace-sftp-binding-label == \"Follow / Linked\"")
            && host.contains(
                "root.workspace-width-tier() == \"wide\" ? \"Follow / Linked\" : \"Follow\""
            )
            && host.contains("root.workspace-sftp-binding-label == \"Locked / Manual\"")
            && host.contains(
                "root.workspace-width-tier() == \"wide\" ? \"Locked / Manual\" : \"Locked\""
            )
            && host.contains("function transfer-entry-width() -> length {")
            && host.contains(
                "return root.workspace-width-tier() == \"wide\" ? \"No transfers\" : \"\";"
            )
            && !host.contains("return \"Transfer Center\";"),
        "workspace status chrome should collapse verbose binding copy and stop repeating the toolbar's `Transfer Center` CTA in the idle footer"
    );
}

#[test]
fn workspace_transfer_center_contract_threads_from_app_window_into_sftp_workspace_host() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");

    assert!(
        app_window.contains(
            "workspace-sftp-items: root.workspace-sftp-items;\n                        transfer-center-open: root.transfer-center-open;\n                        transfer-queue-active: root.transfer-queue-active;\n                        transfer-queue-failed: root.transfer-queue-failed;\n                        transfer-queue-current-session: root.transfer-queue-current-session;"
        ),
        "AppWindow should thread transfer-center visibility and queue summary state into WorkspacePane so the productized SFTP workspace can expose a lightweight transfer entry even while the duplicate quick browser stays policy-hidden"
    );
    assert!(
        app_window.contains(
            "workspace-sftp-retry-requested => {\n                            root.workspace-sftp-retry-requested();\n                        }\n\n                        open-transfer-center-requested => {\n                            root.open-transfer-center-requested();\n                        }"
        ),
        "AppWindow should forward the workspace transfer-entry callback into the existing global Transfer Center toggle instead of inventing a second transfer surface"
    );

    for contract in [
        "in property <bool> transfer-center-open: false;",
        "in property <int> transfer-queue-active: 0;",
        "in property <int> transfer-queue-failed: 0;",
        "in property <int> transfer-queue-current-session: 0;",
        "callback open-transfer-center-requested();",
        "transfer-center-open: root.transfer-center-open;",
        "transfer-queue-active: root.transfer-queue-active;",
        "transfer-queue-failed: root.transfer-queue-failed;",
        "transfer-queue-current-session: root.transfer-queue-current-session;",
        "open-transfer-center-requested => {",
        "root.open-transfer-center-requested();",
    ] {
        assert!(
            workspace_pane.contains(contract),
            "WorkspacePane should thread workspace transfer contract `{contract}` into SftpWorkspaceHost so the host can expose a lightweight global queue entry"
        );
    }
}

#[test]
fn sftp_workspace_host_source_exposes_a_lightweight_transfer_center_entry() {
    let source =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    for contract in [
        "in property <bool> transfer-center-open: false;",
        "in property <int> transfer-queue-active: 0;",
        "in property <int> transfer-queue-failed: 0;",
        "in property <int> transfer-queue-current-session: 0;",
        "callback open-transfer-center-requested();",
        "transfer-entry :=",
        "root.open-transfer-center-requested();",
    ] {
        assert!(
            source.contains(contract),
            "SFTP workspace host should expose transfer-entry contract `{contract}` so upload/download activity can stay reachable from the workspace surface after the right-side duplicate browser is policy-hidden"
        );
    }
}
