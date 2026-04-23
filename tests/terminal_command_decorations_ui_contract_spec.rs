use std::fs;

#[test]
fn terminal_host_exposes_command_block_and_overview_marker_contracts() {
    let host = fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read host");
    let workspace =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let bootstrap = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        host.contains("export struct TerminalCommandBlockRow {")
            && host.contains("export struct TerminalOverviewMarkerRow {")
            && host.contains("in property <[TerminalCommandBlockRow]> session-command-blocks: [];")
            && host.contains(
                "in property <[TerminalOverviewMarkerRow]> session-overview-markers: [];"
            )
            && host.contains("command-block-gutter := Rectangle {")
            && host.contains("overview-ruler := Rectangle {")
            && host.contains("for block in root.session-command-blocks : Rectangle {")
            && host.contains("for marker in root.session-overview-markers : Rectangle {"),
        "terminal host should expose narrow command-block gutter and overview ruler contracts instead of leaving command status and failures trapped inside Rust-only semantic payloads"
    );

    assert!(
        workspace.contains(
            "in property <[TerminalCommandBlockRow]> workspace-session-command-blocks: [];"
        ) && workspace.contains(
            "in property <[TerminalOverviewMarkerRow]> workspace-session-overview-markers: [];"
        ) && workspace.contains("session-command-blocks: root.workspace-session-command-blocks;")
            && workspace
                .contains("session-overview-markers: root.workspace-session-overview-markers;"),
        "workspace pane should forward command block and overview marker models into TerminalSessionHost"
    );

    assert!(
        app_window.contains(
            "in-out property <[TerminalCommandBlockRow]> workspace-session-command-blocks: [];"
        ) && app_window.contains(
            "in-out property <[TerminalOverviewMarkerRow]> workspace-session-overview-markers: [];"
        ) && app_window
            .contains("workspace-session-command-blocks: root.workspace-session-command-blocks;")
            && app_window.contains(
                "workspace-session-overview-markers: root.workspace-session-overview-markers;"
            ),
        "app window should keep command decoration payloads in the same terminal projection contract as cursor, selection, and scroll state"
    );

    assert!(
        bootstrap.contains("window.get_workspace_session_command_blocks()")
            && bootstrap.contains("window.set_workspace_session_command_blocks(model)")
            && bootstrap.contains("window.get_workspace_session_overview_markers()")
            && bootstrap.contains("window.set_workspace_session_overview_markers(model)")
            && bootstrap.contains("presentable_frame.command_blocks")
            && bootstrap.contains("presentable_frame.overview_markers")
            && bootstrap.contains("clear_workspace_terminal_semantic_projection(window);")
            && bootstrap.contains("analyze_workspace_terminal_semantic_projection(")
            && bootstrap
                .matches("sync_workspace_terminal_semantic_projection(")
                .count()
                >= 2
            && bootstrap
                .contains("project_terminal_command_blocks(&presentable_frame.command_blocks)")
            && bootstrap
                .contains("project_terminal_overview_markers(&presentable_frame.overview_markers)"),
        "bootstrap should project the retained presenter payload into Slint models so shell chrome can surface running/failure cues without re-running regex in the renderer"
    );
}
