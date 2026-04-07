//! Source-level guards for terminal runtime performance-sensitive paths.

use std::fs;

fn block_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source.find(start).expect("start marker");
    let rest = &source[start_index..];
    let end_index = rest.find(end).expect("end marker");
    &rest[..end_index]
}

#[test]
fn runtime_visible_projection_limits_iteration_to_visible_phys_range() {
    let runtime_source =
        fs::read_to_string("src/app/ssh/runtime/terminal.rs").expect("read runtime terminal source");
    let visible_rows_block = block_between(
        &runtime_source,
        "    pub fn visible_rows(&self) -> Vec<TerminalRowState> {",
        "    pub fn visible_lines(&self) -> Vec<String> {",
    );
    let visible_cells_block = block_between(
        &runtime_source,
        "    fn visible_cells(&self, palette: &ColorPalette) -> Vec<TerminalCellState> {",
        "    fn cursor_state(&self, palette: &ColorPalette) -> TerminalCursorState {",
    );

    assert!(
        visible_rows_block.contains("lines_in_phys_range(visible_start..visible_end"),
        "visible row projection should iterate only the currently visible phys range so large scrollback histories do not get scanned on every local scroll update"
    );
    assert!(
        !visible_rows_block.contains("for_each_phys_line"),
        "visible row projection should not walk the full scrollback when only the visible viewport needs to be projected"
    );
    assert!(
        visible_cells_block.contains("lines_in_phys_range(visible_start..visible_end"),
        "visible cell projection should iterate only the visible phys range so bitmap/native presenters do not rebuild from a full scrollback scan during scrollbar drags"
    );
    assert!(
        !visible_cells_block.contains("for_each_phys_line"),
        "visible cell projection should not walk the full scrollback when projecting the current viewport"
    );
    assert!(
        runtime_source.contains("let visible_end = visible_start.saturating_add(visible_rows).min(total_rows);")
            && runtime_source.contains("let visible_start = visible_end.saturating_sub(visible_rows);"),
        "visible phys bounds should clamp against the live screen length so wrapped scrollback layouts cannot feed out-of-range phys indexes into the viewport projection path"
    );
}

#[test]
fn runtime_dirty_notifications_expose_a_fast_input_active_flush_contract() {
    let runtime_source =
        fs::read_to_string("src/app/ssh/runtime.rs").expect("read runtime source");
    let pump_source =
        fs::read_to_string("src/app/ssh/runtime/pump.rs").expect("read runtime pump source");

    assert!(
        runtime_source.contains("const FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL: Duration = Duration::from_millis("),
        "runtime should expose a dedicated fast dirty-notification cadence so local key repeat does not wait for the conservative idle 40ms output flush budget"
    );
    assert!(
        pump_source.contains("note_local_input("),
        "runtime pump should record recent local input activity so echoed output can request the fast dirty-notification cadence"
    );
    assert!(
        pump_source.contains("preferred_interval("),
        "surface dirty coalescing should choose between idle and input-active flush intervals instead of hard-coding one timer budget for every workload"
    );
}

#[test]
fn bootstrap_exposes_a_coalesced_fast_input_projection_refresh_path() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let workspace_terminal_source = fs::read_to_string("src/app/bootstrap/workspace_terminal.rs")
        .expect("read workspace terminal source");

    assert!(
        bootstrap_source.contains("input_projection_refresh_timer")
            && bootstrap_source.contains("input_projection_refresh_gate"),
        "bootstrap should keep a dedicated timer and gate for local-input projection refreshes so repeated key input can repaint faster than the idle 50ms polling loop without forcing a full synchronous refresh on every keystroke"
    );
    assert!(
        workspace_terminal_source.contains("schedule_workspace_input_projection_refresh("),
        "workspace terminal helpers should centralize the coalesced fast input-refresh scheduler instead of inlining ad-hoc immediate projection polls in each key/text callback"
    );
    assert!(
        workspace_terminal_source.contains("refresh_active_workspace_surface_projection("),
        "the fast input-refresh path should use a surface-only projection helper so held-key repaint stays off the heavier full tab/SFTP projection walk"
    );
    assert!(
        !workspace_terminal_source.contains(
            "schedule_workspace_input_projection_refresh(\n    window: &AppWindow,\n    state: Rc<RefCell<ShellViewModel>>,\n    bridge: Option<Rc<ShellSessionBridge>>,\n    follow_tracker: Rc<RefCell<WorkspaceFollowTracker>>,\n    timer: Rc<Timer>,\n    gate: Rc<RefCell<DeferredWorkspaceProjectionRefreshGate>>,\n) {\n    {\n        let mut gate = gate.borrow_mut();\n        if !gate.mark_scheduled() {\n            return;\n        }\n    }\n\n    let window_handle = window.as_weak();\n    timer.start(\n        TimerMode::SingleShot,\n        Duration::from_millis(WORKSPACE_INPUT_PROJECTION_DEBOUNCE_MS),\n        move || {\n            gate.borrow_mut().clear();\n            let Some(window) = window_handle.upgrade() else {\n                return;\n            };\n            let mut state = state.borrow_mut();\n            refresh_active_workspace_projection("
        ),
        "the coalesced fast input timer should no longer call the full workspace projection walker because that keeps held-key input tied to the slower global projection workload"
    );
}
