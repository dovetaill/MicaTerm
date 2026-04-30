pub mod types;
pub mod wezterm_adapter;

pub use types::{
    SelectionState, TerminalCoreAdapter, TerminalCoreKind, TerminalFrameSnapshot, ViewportState,
};
pub use wezterm_adapter::WeztermTerminalCoreAdapter;

pub fn create_terminal_core_adapter(
    _kind: TerminalCoreKind,
    rows: usize,
    cols: usize,
    scrollback_lines: usize,
) -> Box<dyn TerminalCoreAdapter> {
    Box::new(WeztermTerminalCoreAdapter::new(
        rows,
        cols,
        scrollback_lines,
    ))
}
