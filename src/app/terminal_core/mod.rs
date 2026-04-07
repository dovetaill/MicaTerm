pub mod alacritty_adapter;
pub mod types;
pub mod wezterm_adapter;

pub use alacritty_adapter::AlacrittyTerminalCoreAdapter;
pub use types::{
    SelectionState, TerminalCoreAdapter, TerminalCoreKind, TerminalFrameSnapshot, ViewportState,
};
pub use wezterm_adapter::WeztermTerminalCoreAdapter;

pub fn create_terminal_core_adapter(
    kind: TerminalCoreKind,
    rows: usize,
    cols: usize,
) -> Box<dyn TerminalCoreAdapter> {
    match kind {
        TerminalCoreKind::Wezterm => Box::new(WeztermTerminalCoreAdapter::new(rows, cols)),
        TerminalCoreKind::AlacrittyExperimental => {
            Box::new(AlacrittyTerminalCoreAdapter::new(rows, cols))
        }
    }
}
