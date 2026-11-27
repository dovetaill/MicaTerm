pub mod colors;
pub mod engine;
pub mod input;
pub mod pty_backend;
pub mod semantic;
pub mod session;

pub use colors::TerminalPalette;
pub use engine::{
    AlacrittyTerminalEngine, EngineSideEffect, RenderCursor, RenderCursorShape, TerminalBgSpan,
    TerminalRenderSnapshot, TerminalRowSnapshot, TerminalStyledRun,
};
pub use input::{MouseReportKind, encode_keystroke, encode_mouse_report, encode_paste};
pub use pty_backend::{LocalPtyBackend, LocalPtyEvent, SpawnedLocalPty, normalize_shell_program_name};
pub use semantic::{
    CommandBlockStatus, OverviewMarker, OverviewMarkerKind, SemanticOptions, SemanticRole,
    VisibleCommandBlock,
};
pub use session::{
    HostKeyPrompt, SessionKind, SessionPhase, TerminalSessionEntity, format_local_shell_titles,
};
