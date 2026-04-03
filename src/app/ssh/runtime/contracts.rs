//! SSH runtime public terminal contracts.

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSurfaceState {
    pub session_id: Uuid,
    pub seqno: usize,
    pub rows: u32,
    pub cols: u32,
    pub default_fg_rgba: u32,
    pub default_bg_rgba: u32,
    pub row_bg_even_rgba: u32,
    pub row_bg_odd_rgba: u32,
    pub viewport_offset_lines: u32,
    pub viewport_max_offset_lines: u32,
    pub viewport_at_bottom: bool,
    pub visible_rows: Vec<TerminalRowState>,
    pub visible_lines: Vec<String>,
    pub cells: Vec<TerminalCellState>,
    pub cursor: TerminalCursorState,
    pub alternate_screen_active: bool,
    pub mouse_grabbed: bool,
    pub bracketed_paste_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSurfaceSignature {
    pub session_id: Uuid,
    pub seqno: usize,
    pub rows: u32,
    pub cols: u32,
    pub default_fg_rgba: u32,
    pub default_bg_rgba: u32,
    pub row_bg_even_rgba: u32,
    pub row_bg_odd_rgba: u32,
    pub viewport_offset_lines: u32,
    pub viewport_max_offset_lines: u32,
    pub viewport_at_bottom: bool,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub cursor_visible: bool,
    pub cursor_blinking: bool,
    pub cursor_shape: TerminalCursorShape,
    pub cursor_fg_rgba: u32,
    pub cursor_bg_rgba: u32,
    pub alternate_screen_active: bool,
    pub mouse_grabbed: bool,
    pub bracketed_paste_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRowState {
    pub index: u32,
    pub text: String,
    pub wrapped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCellState {
    pub row: u32,
    pub col: u32,
    pub width: u32,
    pub text: String,
    pub bold: bool,
    pub underline: bool,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCursorShape {
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCursorState {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub blinking: bool,
    pub shape: TerminalCursorShape,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMouseEventKind {
    Down,
    Up,
    Move,
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalMouseInput {
    pub kind: TerminalMouseEventKind,
    pub button: TerminalMouseButton,
    pub row: u32,
    pub col: u32,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKeyKind {
    Named(&'static str),
    Function(u8),
    Char(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalKeyEvent {
    pub key: TerminalKeyKind,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl TerminalKeyEvent {
    pub fn named(key_name: &'static str, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Named(key_name),
            alt,
            ctrl,
            shift,
        }
    }

    pub fn function(number: u8, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Function(number),
            alt,
            ctrl,
            shift,
        }
    }

    pub fn character(ch: char, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Char(ch),
            alt,
            ctrl,
            shift,
        }
    }
}

impl TerminalSurfaceState {
    pub fn signature(&self) -> TerminalSurfaceSignature {
        TerminalSurfaceSignature {
            session_id: self.session_id,
            seqno: self.seqno,
            rows: self.rows,
            cols: self.cols,
            default_fg_rgba: self.default_fg_rgba,
            default_bg_rgba: self.default_bg_rgba,
            row_bg_even_rgba: self.row_bg_even_rgba,
            row_bg_odd_rgba: self.row_bg_odd_rgba,
            viewport_offset_lines: self.viewport_offset_lines,
            viewport_max_offset_lines: self.viewport_max_offset_lines,
            viewport_at_bottom: self.viewport_at_bottom,
            cursor_row: self.cursor.row,
            cursor_col: self.cursor.col,
            cursor_visible: self.cursor.visible,
            cursor_blinking: self.cursor.blinking,
            cursor_shape: self.cursor.shape,
            cursor_fg_rgba: self.cursor.fg_rgba,
            cursor_bg_rgba: self.cursor.bg_rgba,
            alternate_screen_active: self.alternate_screen_active,
            mouse_grabbed: self.mouse_grabbed,
            bracketed_paste_enabled: self.bracketed_paste_enabled,
        }
    }

    pub fn from_visible_lines(
        session_id: Uuid,
        seqno: usize,
        rows: u32,
        cols: u32,
        visible_lines: Vec<String>,
    ) -> Self {
        Self {
            session_id,
            seqno,
            rows,
            cols,
            default_fg_rgba: 0xff00_0000,
            default_bg_rgba: 0xffff_ffff,
            row_bg_even_rgba: 0xffff_ffff,
            row_bg_odd_rgba: 0xffff_ffff,
            viewport_offset_lines: 0,
            viewport_max_offset_lines: 0,
            viewport_at_bottom: true,
            visible_rows: visible_lines
                .iter()
                .enumerate()
                .map(|(index, text)| TerminalRowState {
                    index: index as u32,
                    text: text.clone(),
                    wrapped: false,
                })
                .collect(),
            visible_lines,
            cells: Vec::new(),
            cursor: TerminalCursorState {
                row: 0,
                col: 0,
                visible: false,
                blinking: false,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0xff00_0000,
                bg_rgba: 0xff52_ad70,
            },
            alternate_screen_active: false,
            mouse_grabbed: false,
            bracketed_paste_enabled: false,
        }
    }

    pub fn selection_text(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        let ((start_row, start_col), (end_row, end_col)) =
            normalized_selection((start_row, start_col), (end_row, end_col));
        let mut text = String::new();

        for row in start_row..=end_row {
            let row_start = if row == start_row { start_col } else { 0 };
            let row_end = if row == end_row {
                end_col
            } else {
                self.cols.saturating_sub(1)
            };
            let mut row_text = String::new();

            for cell in self.cells.iter().filter(|cell| cell.row == row) {
                let cell_start = cell.col;
                let cell_end = cell.col.saturating_add(cell.width.saturating_sub(1));
                if cell_end < row_start || cell_start > row_end {
                    continue;
                }
                row_text.push_str(&cell.text);
            }

            text.push_str(row_text.trim_end_matches(' '));
            let wrapped = self
                .visible_rows
                .iter()
                .find(|visible_row| visible_row.index == row)
                .map(|visible_row| visible_row.wrapped)
                .unwrap_or(false);
            if row < end_row && !wrapped {
                text.push('\n');
            }
        }

        text
    }
}

fn normalized_selection(start: (u32, u32), end: (u32, u32)) -> ((u32, u32), (u32, u32)) {
    if start.0 < end.0 || (start.0 == end.0 && start.1 <= end.1) {
        (start, end)
    } else {
        (end, start)
    }
}
