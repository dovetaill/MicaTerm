//! SSH runtime public terminal contracts.

use uuid::Uuid;

use crate::app::terminal_core::TerminalFrameSnapshot;

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
    pub application_cursor_keys: bool,
    pub bracketed_paste_enabled: bool,
    pub shell_integration: TerminalShellIntegrationState,
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
    pub application_cursor_keys: bool,
    pub bracketed_paste_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalShellIntegrationState {
    pub has_markers: bool,
    pub input_active: bool,
    pub command_running: bool,
    pub last_command_exit_code: Option<i32>,
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
pub struct TerminalViewportSelectionRange {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalSelectionGestureMode {
    Cell,
    Word,
    Line,
}

impl TerminalSelectionGestureMode {
    pub fn from_code(value: i32) -> Self {
        match value {
            1 => Self::Word,
            2 => Self::Line,
            _ => Self::Cell,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSelectionRange {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

impl TerminalSelectionRange {
    pub const fn new(start_row: u32, start_col: u32, end_row: u32, end_col: u32) -> Self {
        Self {
            start_row,
            start_col,
            end_row,
            end_col,
        }
    }
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
    pub fn visible_top_row(&self) -> u32 {
        self.viewport_max_offset_lines
            .saturating_sub(self.viewport_offset_lines)
    }

    pub fn visible_bottom_row_exclusive(&self) -> u32 {
        self.visible_top_row().saturating_add(self.rows.max(1))
    }

    pub fn viewport_row_to_buffer_row(&self, row: u32) -> u32 {
        self.visible_top_row().saturating_add(row)
    }

    pub fn project_buffer_selection_to_viewport(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Option<TerminalViewportSelectionRange> {
        let ((start_row, start_col), (end_row, end_col)) =
            normalized_selection((start_row, start_col), (end_row, end_col));
        let visible_top = self.visible_top_row();
        let visible_bottom = self.visible_bottom_row_exclusive();
        if end_row < visible_top || start_row >= visible_bottom {
            return None;
        }

        Some(TerminalViewportSelectionRange {
            start_row: start_row.max(visible_top).saturating_sub(visible_top),
            start_col: if start_row < visible_top {
                0
            } else {
                start_col
            },
            end_row: end_row
                .min(visible_bottom.saturating_sub(1))
                .saturating_sub(visible_top),
            end_col: if end_row >= visible_bottom {
                self.cols
            } else {
                end_col
            },
        })
    }

    pub fn from_frame_snapshot(session_id: Uuid, frame: TerminalFrameSnapshot) -> Self {
        Self {
            session_id,
            seqno: frame.seqno,
            rows: frame.rows,
            cols: frame.cols,
            default_fg_rgba: frame.default_fg_rgba,
            default_bg_rgba: frame.default_bg_rgba,
            row_bg_even_rgba: frame.row_bg_even_rgba,
            row_bg_odd_rgba: frame.row_bg_odd_rgba,
            viewport_offset_lines: frame.viewport.offset_lines,
            viewport_max_offset_lines: frame.viewport.max_offset_lines,
            viewport_at_bottom: frame.viewport.at_bottom,
            visible_rows: frame.visible_rows,
            visible_lines: frame.visible_lines,
            cells: frame.cells,
            cursor: frame.cursor,
            alternate_screen_active: frame.alternate_screen_active,
            mouse_grabbed: frame.mouse_grabbed,
            application_cursor_keys: frame.application_cursor_keys,
            bracketed_paste_enabled: frame.bracketed_paste_enabled,
            shell_integration: TerminalShellIntegrationState::default(),
        }
    }

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
            application_cursor_keys: self.application_cursor_keys,
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
            application_cursor_keys: false,
            bracketed_paste_enabled: false,
            shell_integration: TerminalShellIntegrationState::default(),
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
            let row_start = if row == start_row {
                start_col.min(self.cols)
            } else {
                0
            };
            let row_end = if row == end_row {
                end_col.min(self.cols)
            } else {
                self.cols
            };
            let mut row_text = String::new();

            if row_end > row_start {
                for cell in self.cells.iter().filter(|cell| cell.row == row) {
                    let cell_start = cell.col;
                    let cell_end = cell.col.saturating_add(cell.width);
                    if cell_end <= row_start || cell_start >= row_end {
                        continue;
                    }
                    row_text.push_str(&cell.text);
                }
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

    pub fn selection_text_from_buffer_rows(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        self.project_buffer_selection_to_viewport(start_row, start_col, end_row, end_col)
            .map(|range| {
                self.selection_text(
                    range.start_row,
                    range.start_col,
                    range.end_row,
                    range.end_col,
                )
            })
            .unwrap_or_default()
    }

    pub fn normalize_hit_col(&self, row: u32, col: u32) -> u32 {
        let clamped_col = col.min(self.cols.saturating_sub(1));
        self.cells
            .iter()
            .find(|cell| {
                cell.row == row
                    && cell.width > 1
                    && clamped_col > cell.col
                    && clamped_col < cell.col.saturating_add(cell.width)
            })
            .map(|cell| cell.col)
            .unwrap_or(clamped_col)
    }

    pub fn normalize_selection_hit_col(&self, row: u32, col: u32) -> u32 {
        let clamped_col = col.min(self.cols);
        self.cells
            .iter()
            .find(|cell| {
                cell.row == row
                    && cell.width > 1
                    && clamped_col > cell.col
                    && clamped_col < cell.col.saturating_add(cell.width)
            })
            .map(|cell| cell.col.saturating_add(cell.width))
            .unwrap_or(clamped_col)
    }

    pub fn selection_gesture_range(
        &self,
        mode: TerminalSelectionGestureMode,
        anchor_row: u32,
        anchor_col: u32,
        focus_row: u32,
        focus_col: u32,
    ) -> TerminalSelectionRange {
        match mode {
            TerminalSelectionGestureMode::Cell => self.cell_selection_range(
                anchor_row,
                anchor_col,
                focus_row,
                focus_col,
            ),
            TerminalSelectionGestureMode::Word => self.word_selection_range(
                anchor_row,
                anchor_col,
                focus_row,
                focus_col,
            ),
            TerminalSelectionGestureMode::Line => self.line_selection_range(anchor_row, focus_row),
        }
    }

    pub fn expand_to_token(&self, row: u32, col: u32) -> TerminalSelectionRange {
        if self.cols == 0 {
            return TerminalSelectionRange::new(0, 0, 0, 0);
        }

        let safe_row = row.min(self.rows.saturating_sub(1));
        let buffer_row = self.viewport_row_to_buffer_row(safe_row);
        let Some((start_col, end_col)) = self.token_span_at_row_col(safe_row, col) else {
            let safe_col = col.min(self.cols.saturating_sub(1));
            return TerminalSelectionRange::new(
                buffer_row,
                safe_col,
                buffer_row,
                safe_col.saturating_add(1).min(self.cols),
            );
        };

        TerminalSelectionRange::new(buffer_row, start_col, buffer_row, end_col)
    }

    fn cell_selection_range(
        &self,
        anchor_row: u32,
        anchor_col: u32,
        focus_row: u32,
        focus_col: u32,
    ) -> TerminalSelectionRange {
        let start = (
            self.viewport_row_to_buffer_row(anchor_row.min(self.rows.saturating_sub(1))),
            anchor_col.min(self.cols),
        );
        let end = (
            self.viewport_row_to_buffer_row(focus_row.min(self.rows.saturating_sub(1))),
            focus_col.min(self.cols),
        );
        let ((start_row, start_col), (end_row, end_col)) = normalized_selection(start, end);
        TerminalSelectionRange::new(start_row, start_col, end_row, end_col)
    }

    fn word_selection_range(
        &self,
        anchor_row: u32,
        anchor_col: u32,
        focus_row: u32,
        focus_col: u32,
    ) -> TerminalSelectionRange {
        merge_selection_ranges(
            self.expand_to_token(anchor_row, anchor_col),
            self.expand_to_token(focus_row, focus_col),
        )
    }

    fn line_selection_range(&self, anchor_row: u32, focus_row: u32) -> TerminalSelectionRange {
        let start_row = self.viewport_row_to_buffer_row(anchor_row.min(focus_row));
        let end_row = self.viewport_row_to_buffer_row(anchor_row.max(focus_row));
        TerminalSelectionRange::new(start_row, 0, end_row, self.cols)
    }

    fn token_span_at_row_col(&self, row: u32, col: u32) -> Option<(u32, u32)> {
        let row_cells = self.row_cells(row);
        let hit_col = col.min(self.cols.saturating_sub(1));
        let hit_index = row_cells.iter().position(|cell| {
            hit_col >= cell.col && hit_col < cell.col.saturating_add(cell.width)
        })?;
        let hit_cell = row_cells[hit_index];

        let Some(token_class) = selection_token_class(hit_cell) else {
            return Some((
                hit_cell.col,
                hit_cell.col.saturating_add(hit_cell.width).min(self.cols),
            ));
        };

        let mut start_col = hit_cell.col;
        let mut end_col = hit_cell.col.saturating_add(hit_cell.width).min(self.cols);

        let mut run_start_index = hit_index;
        while run_start_index > 0 {
            let previous = row_cells[run_start_index - 1];
            if previous.col.saturating_add(previous.width) < start_col
                || !cell_matches_selection_class(previous, token_class)
            {
                break;
            }
            start_col = previous.col;
            run_start_index -= 1;
        }

        let mut run_end_index = hit_index;
        while run_end_index + 1 < row_cells.len() {
            let next = row_cells[run_end_index + 1];
            if next.col > end_col || !cell_matches_selection_class(next, token_class) {
                break;
            }
            end_col = next.col.saturating_add(next.width).min(self.cols);
            run_end_index += 1;
        }

        if token_class == SelectionTokenClass::NonWhitespace {
            let token_cells = &row_cells[run_start_index..=run_end_index];
            if token_cells.len() > 1 && token_cells.iter().all(|cell| cell.width > 1) {
                let logical_start = row_cells[..run_start_index].iter().count() as u32;
                return Some((logical_start, logical_start.saturating_add(token_cells.len() as u32)));
            }
        }

        Some((start_col, end_col))
    }

    fn row_cells(&self, row: u32) -> Vec<&TerminalCellState> {
        let mut row_cells = self
            .cells
            .iter()
            .filter(|cell| cell.row == row && !cell.text.is_empty())
            .collect::<Vec<_>>();
        row_cells.sort_by_key(|cell| cell.col);
        row_cells
    }
}

fn normalized_selection(start: (u32, u32), end: (u32, u32)) -> ((u32, u32), (u32, u32)) {
    if start.0 < end.0 || (start.0 == end.0 && start.1 <= end.1) {
        (start, end)
    } else {
        (end, start)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionTokenClass {
    Shellish,
    NonWhitespace,
}

fn merge_selection_ranges(
    left: TerminalSelectionRange,
    right: TerminalSelectionRange,
) -> TerminalSelectionRange {
    if (left.start_row, left.start_col) <= (right.start_row, right.start_col) {
        TerminalSelectionRange::new(
            left.start_row,
            left.start_col,
            right.end_row,
            right.end_col,
        )
    } else {
        TerminalSelectionRange::new(
            right.start_row,
            right.start_col,
            left.end_row,
            left.end_col,
        )
    }
}

fn selection_token_class(cell: &TerminalCellState) -> Option<SelectionTokenClass> {
    let ch = cell.text.chars().next()?;
    if ch.is_whitespace() {
        None
    } else if is_shellish_token_char(ch) {
        Some(SelectionTokenClass::Shellish)
    } else {
        Some(SelectionTokenClass::NonWhitespace)
    }
}

fn cell_matches_selection_class(cell: &TerminalCellState, token_class: SelectionTokenClass) -> bool {
    match (selection_token_class(cell), token_class) {
        (Some(SelectionTokenClass::Shellish), SelectionTokenClass::Shellish) => true,
        (Some(SelectionTokenClass::NonWhitespace), SelectionTokenClass::NonWhitespace) => true,
        _ => false,
    }
}

fn is_shellish_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            '_' | '.'
                | '/'
                | '\\'
                | ':'
                | '@'
                | '%'
                | '+'
                | '='
                | '~'
                | '-'
                | '#'
                | '?'
                | '&'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_surface() -> TerminalSurfaceState {
        let session_id = Uuid::nil();
        let mut surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 1, 8, vec!["A条B".into()]);
        surface.cells = vec![
            TerminalCellState {
                row: 0,
                col: 0,
                width: 1,
                text: "A".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xff_ff_ff_ff,
                bg_rgba: 0xff_00_00_00,
            },
            TerminalCellState {
                row: 0,
                col: 1,
                width: 2,
                text: "条".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xff_ff_ff_ff,
                bg_rgba: 0xff_00_00_00,
            },
            TerminalCellState {
                row: 0,
                col: 3,
                width: 1,
                text: "B".into(),
                bold: false,
                underline: false,
                fg_rgba: 0xff_ff_ff_ff,
                bg_rgba: 0xff_00_00_00,
            },
        ];
        surface
    }

    #[test]
    fn selection_text_uses_exclusive_end_column_boundaries() {
        let surface = sample_surface();

        assert_eq!(surface.selection_text(0, 0, 0, 1), "A");
        assert_eq!(surface.selection_text(0, 1, 0, 3), "条");
        assert_eq!(surface.selection_text(0, 0, 0, 4), "A条B");
    }

    #[test]
    fn wide_char_trailing_cell_hit_normalizes_back_to_leading_cell() {
        let surface = sample_surface();

        assert_eq!(surface.normalize_hit_col(0, 2), 1);
        assert_eq!(surface.normalize_hit_col(0, 3), 3);
    }

    #[test]
    fn wide_char_internal_selection_boundary_snaps_to_cluster_end() {
        let surface = sample_surface();

        assert_eq!(surface.normalize_selection_hit_col(0, 2), 3);
        assert_eq!(surface.normalize_selection_hit_col(0, 1), 1);
        assert_eq!(surface.normalize_selection_hit_col(0, 4), 4);
    }

    #[test]
    fn buffer_selection_projects_into_viewport_coordinates() {
        let mut surface = sample_surface();
        surface.rows = 4;
        surface.cols = 8;
        surface.viewport_offset_lines = 3;
        surface.viewport_max_offset_lines = 8;
        surface.viewport_at_bottom = false;

        let projected = surface
            .project_buffer_selection_to_viewport(5, 1, 5, 3)
            .expect("visible selection");
        assert_eq!(
            projected,
            TerminalViewportSelectionRange {
                start_row: 0,
                start_col: 1,
                end_row: 0,
                end_col: 3,
            }
        );

        let shifted = surface
            .project_buffer_selection_to_viewport(7, 0, 7, 2)
            .expect("selection shifted down within viewport");
        assert_eq!(shifted.start_row, 2);
        assert_eq!(shifted.end_row, 2);
    }

    #[test]
    fn buffer_selection_clips_to_visible_rows() {
        let mut surface = sample_surface();
        surface.rows = 4;
        surface.cols = 8;
        surface.viewport_offset_lines = 3;
        surface.viewport_max_offset_lines = 8;
        surface.viewport_at_bottom = false;

        let clipped = surface
            .project_buffer_selection_to_viewport(4, 2, 6, 4)
            .expect("partially visible selection");
        assert_eq!(
            clipped,
            TerminalViewportSelectionRange {
                start_row: 0,
                start_col: 0,
                end_row: 1,
                end_col: 4,
            }
        );
    }
}
