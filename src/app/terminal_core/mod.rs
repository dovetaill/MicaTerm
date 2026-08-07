pub mod types;
pub mod wezterm_adapter;

pub use types::{
    LocalTerminalImage, SelectionState, TERMINAL_IMAGE_UV_SCALE, TerminalCoreAdapter,
    TerminalFrameSnapshot, TerminalImagePlacement, TerminalImageResource, TerminalImageUvRect,
    TerminalViewportMetrics, ViewportState,
};
pub use wezterm_adapter::WeztermTerminalCoreAdapter;

pub fn create_terminal_core_adapter(
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

pub fn create_terminal_core_adapter_with_viewport(
    rows: usize,
    cols: usize,
    scrollback_lines: usize,
    viewport: TerminalViewportMetrics,
) -> Box<dyn TerminalCoreAdapter> {
    Box::new(WeztermTerminalCoreAdapter::new_with_viewport(
        rows,
        cols,
        scrollback_lines,
        viewport,
    ))
}
