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
