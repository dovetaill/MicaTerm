use mica_term::app::ssh::runtime::TerminalSurfaceState;
use mica_term::app::terminal_model::TerminalModelFrame;
use mica_term::app::terminal_semantic::{
    CommandBlockStatus, OverviewMarkerKind, command_blocks_from_frame, command_blocks_from_lines,
};
use mica_term::theme::{ThemeMode, ThemeVariant, app_theme_spec};
use std::fs;
use uuid::Uuid;

fn decoration_surface(lines: &[&str]) -> TerminalSurfaceState {
    TerminalSurfaceState::from_visible_lines(
        Uuid::new_v4(),
        1,
        lines.len() as u32,
        120,
        lines.iter().map(|line| (*line).to_string()).collect(),
    )
}

fn decoration_frame(lines: &[&str]) -> TerminalModelFrame {
    let surface = decoration_surface(lines);
    TerminalModelFrame::from_surface(&surface, None)
}

#[test]
fn command_ledger_emits_running_failure_and_success_blocks() {
    let ledger = command_blocks_from_lines(&[
        "$ cargo test",
        "running...",
        "$ false",
        "command exited with 1",
        "$ true",
        "command exited with 0",
    ]);

    assert!(
        ledger
            .blocks
            .iter()
            .any(|block| block.status == CommandBlockStatus::Running)
    );
    assert!(
        ledger
            .blocks
            .iter()
            .any(|block| block.status == CommandBlockStatus::Failure)
    );
    assert!(
        ledger
            .blocks
            .iter()
            .any(|block| block.status == CommandBlockStatus::Success)
    );
}

#[test]
fn presenter_threads_overview_markers_with_command_failures() {
    let frame = decoration_frame(&["$ false", "command exited with 1"]);
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");

    let ledger = command_blocks_from_frame(&frame);

    assert!(
        ledger
            .overview_markers
            .iter()
            .any(|marker| marker.kind == OverviewMarkerKind::CommandFailure)
    );
    assert!(
        presenter_source.contains("pub command_blocks: Vec<CommandBlock>"),
        "presentable native frames should carry command block payloads"
    );
    assert!(
        presenter_source.contains("pub overview_markers: Vec<OverviewMarker>"),
        "presentable native frames should carry overview marker payloads"
    );
}

#[test]
fn command_decorations_use_v2_running_success_and_failure_tones() {
    let spec = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert_eq!(spec.decoration.running, 0x7d_97_b8);
    assert_eq!(spec.decoration.success, 0x7f_b0_8d);
    assert_eq!(spec.decoration.failure, 0xc9_7d_88);
    assert!(
        app_window.contains("workspace-session-command-blocks"),
        "AppWindow should carry workspace command block decoration state instead of dropping it before the session host",
    );
    assert!(
        app_window.contains("workspace-session-overview-markers"),
        "AppWindow should carry workspace overview marker decoration state instead of dropping it before the session host",
    );
    assert!(
        workspace_pane.contains("session-command-blocks: root.workspace-session-command-blocks;"),
        "WorkspacePane should forward command block decorations into TerminalSessionHost",
    );
    assert!(
        workspace_pane.contains(
            "session-overview-markers: root.workspace-session-overview-markers;"
        ),
        "WorkspacePane should forward overview markers into TerminalSessionHost",
    );
    assert!(
        host_source.contains("ThemeTokens.terminal-command-decoration-running"),
        "TerminalSessionHost should render command block rails with the calmer Premium Default v2 running tone",
    );
    assert!(
        host_source.contains("ThemeTokens.terminal-overview-marker-failure"),
        "TerminalSessionHost should render overview markers with the calmer Premium Default v2 failure tone",
    );
    assert!(
        bootstrap_source.contains("set_workspace_session_command_blocks"),
        "bootstrap should sync command block decorations into the workspace session host state",
    );
    assert!(
        bootstrap_source.contains("set_workspace_session_overview_markers"),
        "bootstrap should sync overview marker decorations into the workspace session host state",
    );
}
