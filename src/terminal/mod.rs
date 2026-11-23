pub mod colors;
pub mod engine;
pub mod input;
pub mod pty_backend;
pub mod session;

pub use colors::TerminalPalette;
pub use engine::{
    AlacrittyTerminalEngine, EngineSideEffect, RenderCursor, RenderCursorShape, TerminalBgSpan,
    TerminalRenderSnapshot, TerminalRowSnapshot, TerminalStyledRun,
};
pub use input::{MouseReportKind, encode_keystroke, encode_mouse_report, encode_paste};
pub use pty_backend::{LocalPtyBackend, LocalPtyEvent};
pub use session::{HostKeyPrompt, SessionKind, SessionPhase, TerminalSessionEntity};
