use mica_term::app::ssh::runtime::TerminalSurfaceState;
use mica_term::app::terminal_model::TerminalModelFrame;
use mica_term::app::terminal_semantic::{
    CommandBlockStatus, OverviewMarkerKind, command_blocks_from_frame, command_blocks_from_lines,
};
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
