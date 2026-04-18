#[test]
fn right_panel_source_exposes_quick_browser_header_contract() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(
        source.contains("callback sftp-panel-expand-requested();"),
        "quick browser header should expose an expand-to-workspace callback"
    );
    assert!(
        source.contains("callback sftp-panel-binding-mode-toggle-requested();"),
        "quick browser header should expose a follow/locked toggle callback"
    );
    assert!(
        source.contains("in property <string> sftp-panel-connection-badge: \"\";"),
        "quick browser header should accept a connection badge string"
    );
    assert!(
        source.contains("in property <string> sftp-panel-binding-mode-label: \"Follow\";"),
        "quick browser header should accept a follow/locked label"
    );
    assert!(
        source.contains("in property <bool> sftp-panel-path-editing: false;"),
        "quick browser header should expose path-editing mode"
    );
    assert!(
        source.contains("import { SidebarToolbarIconButton } from \"../components/sidebar-toolbar-icon-button.slint\";"),
        "quick browser header should reuse the shared icon-button component for compact rail actions"
    );
}

#[test]
fn right_panel_source_uses_two_row_icon_first_quick_browser_header() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(
        source.contains("link-20-regular.svg")
            && source.contains("link-20-filled.svg")
            && source.contains("arrow-sync-20-regular.svg")
            && source.contains("panel-right-expand-20-regular.svg")
            && source.contains("panel-right-expand-20-filled.svg"),
        "quick browser header should load dedicated Fluent follow/refresh/expand icon assets"
    );
    assert!(
        source.contains("follow-button := SidebarToolbarIconButton")
            && source.contains("refresh-button := SidebarToolbarIconButton")
            && source.contains("expand-button := SidebarToolbarIconButton"),
        "quick browser header should render follow, refresh, and expand as icon buttons instead of equal-weight text pills"
    );
    assert!(
        source.contains("active: root.sftp-panel-binding-mode-active;")
            && source.contains("tooltip-text: \"Follow active terminal\";")
            && source.contains("tooltip-text: \"Refresh files\";")
            && source.contains("tooltip-text: \"Open in SFTP workspace\";"),
        "quick browser toolbar should expose explicit follow state and clear tooltips for every icon action"
    );
    assert!(
        source.contains("out property <string> tooltip-text <=> root.tooltip-text-value;")
            && source.contains("function schedule-tooltip(")
            && source.contains(
                "tooltip-open-requested(source-id, text, anchor-x, anchor-y, anchor-width) => {"
            )
            && source.contains(
                "root.schedule-tooltip(source-id, text, anchor-x, anchor-y, anchor-width);"
            )
            && source.contains("root.queue-tooltip-close(source-id);"),
        "quick browser should wire toolbar hover events into a reusable tooltip state machine"
    );
    assert!(
        (source.contains("connection-badge := Rectangle {")
            || source.contains("connection-badge := StatusPill {"))
            && source.contains("\"Current connection: \" + root.sftp-panel-connection-badge-text")
            && source.contains("border-color: ThemeTokens.status-pill-border;")
            && source.contains("background: ThemeTokens.status-pill-surface;"),
        "quick browser header should render the connection shell as a badge with its own tooltip instead of an input-like field"
    );
    assert!(
        source.contains("breadcrumb-row") || source.contains("breadcrumb-shell"),
        "quick browser header should render a dedicated second-row breadcrumb path"
    );
    assert!(
        source.contains("if root.sftp-panel-path-editing")
            || source.contains("root.sftp-panel-path-editing ?"),
        "path bar should branch between breadcrumb display and inline path editing"
    );
    assert!(
        !source.contains("text: \"Expand\";\n                    font-family")
            && !source.contains("label: \"Type\"")
            && !source.contains("label: \"Modified\"")
            && !source.contains("label: \"Size\""),
        "quick browser should no longer render text expand pills or multi-column table headers"
    );
    assert!(
        !source.contains("horizontal-scrollbar-policy: always-on")
            && !source.contains("viewport-x <=> root.sftp-list-viewport-x"),
        "quick browser should stop relying on horizontal table scrolling in the narrow right rail"
    );
    assert!(
        source.contains("meta-text := Text") || source.contains("secondary-meta := Text"),
        "quick browser rows should render secondary metadata inline beneath the primary file name"
    );
}

#[test]
fn app_window_source_threads_quick_browser_contract_into_right_panel() {
    let source = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(
        source.contains("in-out property <string> sftp-panel-connection-badge: \"\";")
            && source
                .contains("in-out property <string> sftp-panel-binding-mode-label: \"Follow\";")
            && source.contains("in-out property <bool> sftp-panel-path-editing: false;"),
        "app window should own the quick browser header state contract"
    );
    assert!(
        source.contains("sftp-panel-connection-badge: root.sftp-panel-connection-badge;")
            && source
                .contains("sftp-panel-binding-mode-label: root.sftp-panel-binding-mode-label;")
            && source
                .contains("sftp-panel-binding-mode-active: root.sftp-panel-binding-mode-active;")
            && source.contains("sftp-panel-path-editing: root.sftp-panel-path-editing;"),
        "app window should forward quick browser header state into the right panel"
    );
    assert!(
        source.contains("callback sftp-panel-expand-requested();")
            && source.contains("callback sftp-panel-binding-mode-toggle-requested();"),
        "app window should own quick browser header callbacks"
    );
    assert!(
        source.contains("sftp-panel-expand-requested => {")
            && source.contains("root.sftp-panel-expand-requested();")
            && source.contains("sftp-panel-binding-mode-toggle-requested => {")
            && source.contains("root.sftp-panel-binding-mode-toggle-requested();"),
        "app window should proxy quick browser header callbacks from the right panel"
    );
}

#[test]
fn app_window_source_mounts_a_dedicated_quick_browser_tooltip_overlay() {
    let source = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(
        source.contains("quick-browser-tooltip-overlay := TitlebarTooltip {")
            && source.contains("text: right-panel.tooltip-text;")
            && source.contains("anchor-x: right-panel.tooltip-anchor-x;")
            && source.contains("anchor-y: right-panel.tooltip-anchor-y;")
            && source.contains("anchor-width: right-panel.tooltip-anchor-width;")
            && source.contains("tooltip-visible: right-panel.tooltip-visible;")
            && source.contains("place-right: false;"),
        "app window should reuse the shared tooltip overlay for the quick browser toolbar and connection badge"
    );
}
