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
}

#[test]
fn right_panel_source_promotes_expand_badge_and_binding_mode_controls() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(
        source.contains("text: \"Expand\""),
        "quick browser header should render an Expand affordance"
    );
    assert!(
        source.contains("connection-badge") || source.contains("sftp-panel-connection-badge"),
        "quick browser header should render the connection badge shell"
    );
    assert!(
        source.contains("binding-mode-button") || source.contains("sftp-panel-binding-mode-label"),
        "quick browser header should render a follow/locked mode toggle shell"
    );
    assert!(
        source.contains("if root.sftp-panel-path-editing")
            || source.contains("root.sftp-panel-path-editing ?"),
        "path bar should branch between breadcrumb display and inline path editing"
    );
}

#[test]
fn app_window_source_threads_quick_browser_contract_into_right_panel() {
    let source = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(
        source.contains("in-out property <string> sftp-panel-connection-badge: \"\";")
            && source.contains("in-out property <string> sftp-panel-binding-mode-label: \"Follow\";")
            && source.contains("in-out property <bool> sftp-panel-path-editing: false;"),
        "app window should own the quick browser header state contract"
    );
    assert!(
        source.contains("sftp-panel-connection-badge: root.sftp-panel-connection-badge;")
            && source.contains("sftp-panel-binding-mode-label: root.sftp-panel-binding-mode-label;")
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
