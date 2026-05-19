use std::fs;

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
        "workspace-sftp-breadcrumb-items: root.workspace-sftp-breadcrumb-items;",
        "workspace-sftp-path-submitted(path) => {",
        "root.workspace-sftp-path-submitted(path);",
        "workspace-sftp-breadcrumb-clicked(path) => {",
        "root.workspace-sftp-breadcrumb-clicked(path);",
        "workspace-sftp-item-activated(item-id, item-kind) => {",
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
        !source.contains("Expand a Quick Browser session to bring file work into the main workspace.")
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
