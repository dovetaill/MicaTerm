//! Experimental alacritty-backed terminal core adapter.
//!
//! This keeps the shipped runtime on the WezTerm-backed core while exercising a real
//! `alacritty_terminal` state machine behind the existing adapter boundary.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, GridCell, Scroll};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags, LineLength};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb};
use anyhow::{Result, bail};
use termwiz::input::{KeyCode, KeyCodeEncodeModes, KeyboardEncoding, Modifiers as KeyModifiers};

use crate::app::ssh::runtime::{
    TerminalCellState, TerminalCursorShape, TerminalCursorState, TerminalKeyEvent, TerminalKeyKind,
    TerminalMouseButton, TerminalMouseEventKind, TerminalMouseInput, TerminalRowState,
};
use crate::app::terminal_core::{
    SelectionState, TerminalCoreAdapter, TerminalFrameSnapshot, ViewportState,
};
use crate::app::terminal_theme::preset_for_theme;
use crate::theme::{ThemeMode, ThemeVariant, terminal_palette_spec_for};

pub struct AlacrittyTerminalCoreAdapter {
    term: Term<VoidListener>,
    parser: Processor,
    theme_mode: ThemeMode,
    theme_variant: ThemeVariant,
    sequence_number: usize,
    fallback_mouse_button: Option<TerminalMouseButton>,
}

impl AlacrittyTerminalCoreAdapter {
    pub fn new(rows: usize, cols: usize, scrollback_lines: usize) -> Self {
        let dimensions = AlacrittyDimensions::new(rows, cols);
        let config = Config {
            scrolling_history: scrollback_lines.max(1),
            ..Config::default()
        };

        Self {
            term: Term::new(config, &dimensions, VoidListener),
            parser: Processor::new(),
            theme_mode: ThemeMode::Dark,
            theme_variant: ThemeVariant::PremiumDefault,
            sequence_number: 0,
            fallback_mouse_button: None,
        }
    }

    fn visible_rows_internal(&self) -> Vec<TerminalRowState> {
        let grid = self.term.grid();
        let display_offset = grid.display_offset() as i32;
        let rows = grid.screen_lines().max(1);
        let cols = grid.columns().max(1);
        let mut projected = Vec::with_capacity(rows);

        for viewport_row in 0..rows {
            let line = Line(viewport_row as i32 - display_offset);
            let row = &grid[line];
            projected.push(TerminalRowState {
                index: viewport_row as u32,
                text: row_text(row, cols),
                wrapped: row
                    .last()
                    .is_some_and(|cell| cell.flags.contains(Flags::WRAPLINE)),
            });
        }

        projected
    }

    fn visible_cells_internal(&self) -> Vec<TerminalCellState> {
        let renderable = self.term.renderable_content();
        let display_offset = renderable.display_offset as i32;
        let rows = self.term.grid().screen_lines() as i32;
        let mut cells = Vec::new();

        for indexed in renderable.display_iter {
            let cell = indexed.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER)
                || cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
                || cell.is_empty()
            {
                continue;
            }

            let row = indexed.point.line.0 + display_offset;
            if !(0..rows).contains(&row) {
                continue;
            }

            let (fg_rgba, bg_rgba) = resolve_cell_colors(self.theme_mode, self.theme_variant, cell);
            cells.push(TerminalCellState {
                row: row as u32,
                col: indexed.point.column.0 as u32,
                width: if cell.flags.contains(Flags::WIDE_CHAR) {
                    2
                } else {
                    1
                },
                text: cell_text(cell),
                bold: cell
                    .flags
                    .intersects(Flags::BOLD | Flags::DIM_BOLD | Flags::BOLD_ITALIC),
                underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
                fg_rgba,
                bg_rgba,
            });
        }

        cells
    }

    fn cursor_state_internal(&self) -> TerminalCursorState {
        let renderable = self.term.renderable_content();
        let cursor = renderable.cursor;
        let row = cursor.point.line.0 + renderable.display_offset as i32;
        let visible = row >= 0
            && row < self.term.grid().screen_lines() as i32
            && !matches!(cursor.shape, CursorShape::Hidden);
        let preset = preset_for_theme(self.theme_mode, self.theme_variant);

        TerminalCursorState {
            row: row.max(0) as u32,
            col: cursor.point.column.0 as u32,
            visible,
            blinking: cursor_shape_blinks(cursor.shape),
            shape: project_cursor_shape(cursor.shape),
            fg_rgba: pack_rgb_hex(preset.cursor_fg),
            bg_rgba: pack_rgb_hex(preset.cursor_bg),
        }
    }

    fn selection_state_internal(&self) -> SelectionState {
        let renderable = self.term.renderable_content();
        let Some(selection) = renderable.selection else {
            return SelectionState::default();
        };

        SelectionState {
            active: true,
            start_row: viewport_row(selection.start.line, renderable.display_offset),
            start_col: selection.start.column.0 as u32,
            end_row: viewport_row(selection.end.line, renderable.display_offset),
            end_col: selection.end.column.0 as u32,
        }
    }

    fn snap_viewport_to_bottom(&mut self) {
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        if self.term.grid().display_offset() > 0 {
            self.term.scroll_display(Scroll::Bottom);
            self.sequence_number = self.sequence_number.saturating_add(1);
        }
    }

    fn mouse_grabbed(&self) -> bool {
        self.term.mode().intersects(TermMode::MOUSE_MODE)
    }

    fn resolve_fallback_mouse_button(&mut self, event: TerminalMouseInput) -> TerminalMouseButton {
        match event.kind {
            TerminalMouseEventKind::Down | TerminalMouseEventKind::Move => {
                if event.button != TerminalMouseButton::None {
                    self.fallback_mouse_button = Some(event.button);
                    event.button
                } else {
                    self.fallback_mouse_button
                        .unwrap_or(TerminalMouseButton::None)
                }
            }
            TerminalMouseEventKind::Up => {
                let button = if event.button != TerminalMouseButton::None {
                    event.button
                } else {
                    self.fallback_mouse_button
                        .unwrap_or(TerminalMouseButton::None)
                };
                self.fallback_mouse_button = None;
                button
            }
            TerminalMouseEventKind::Scroll => event.button,
        }
    }
}

impl TerminalCoreAdapter for AlacrittyTerminalCoreAdapter {
    fn sequence_number(&self) -> usize {
        self.sequence_number
    }

    fn apply_remote_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        self.parser.advance(&mut self.term, bytes);
        self.sequence_number = self.sequence_number.saturating_add(1);
    }

    fn screen_text(&self) -> String {
        self.visible_lines().join("\n")
    }

    fn visible_rows(&self) -> Vec<TerminalRowState> {
        self.visible_rows_internal()
    }

    fn visible_lines(&self) -> Vec<String> {
        visible_lines_from_rows(&self.visible_rows_internal())
    }

    fn selection_text_from_buffer_rows(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        let grid = self.term.grid();
        let total_rows = grid.history_size().saturating_add(grid.screen_lines());
        if total_rows == 0 {
            return String::new();
        }

        let cols = grid.columns().max(1) as u32;
        let history_size = grid.history_size();
        let ((mut start_row, start_col), (mut end_row, end_col)) =
            normalized_selection_bounds((start_row, start_col), (end_row, end_col));
        let last_row = total_rows.saturating_sub(1) as u32;
        if start_row > last_row {
            return String::new();
        }
        end_row = end_row.min(last_row);
        start_row = start_row.min(end_row);

        let mut text = String::new();
        for row in start_row..=end_row {
            let row_start = if row == start_row {
                start_col.min(cols)
            } else {
                0
            };
            let row_end = if row == end_row {
                end_col.min(cols)
            } else {
                cols
            };
            let line = buffer_row_to_grid_line(row, history_size);
            let grid_row = &grid[line];
            text.push_str(&row_text_in_column_range(
                grid_row,
                row_start as usize,
                row_end as usize,
            ));
            let wrapped = grid_row
                .last()
                .is_some_and(|cell| cell.flags.contains(Flags::WRAPLINE));
            if row < end_row && !wrapped {
                text.push('\n');
            }
        }

        text
    }

    fn frame_snapshot(&self) -> TerminalFrameSnapshot {
        let rows = self.visible_rows_internal();
        let lines = visible_lines_from_rows(&rows);
        let grid = self.term.grid();
        let preset = preset_for_theme(self.theme_mode, self.theme_variant);
        let viewport_bg_top_rgba = pack_rgb_hex(preset.viewport_bg_top);
        let viewport_bg_bottom_rgba = pack_rgb_hex(preset.viewport_bg_bottom);
        let alternate_screen_active = self.term.mode().contains(TermMode::ALT_SCREEN);
        let viewport = if alternate_screen_active {
            ViewportState {
                offset_lines: 0,
                max_offset_lines: 0,
                at_bottom: true,
            }
        } else {
            ViewportState {
                offset_lines: grid.display_offset() as u32,
                max_offset_lines: grid.history_size() as u32,
                at_bottom: grid.display_offset() == 0,
            }
        };

        TerminalFrameSnapshot {
            seqno: self.sequence_number,
            rows: grid.screen_lines() as u32,
            cols: grid.columns() as u32,
            default_fg_rgba: pack_rgb_hex(preset.foreground),
            default_bg_rgba: pack_rgb_hex(preset.background),
            row_bg_even_rgba: viewport_bg_top_rgba,
            row_bg_odd_rgba: viewport_bg_bottom_rgba,
            viewport,
            visible_rows: rows,
            visible_lines: lines,
            cells: self.visible_cells_internal(),
            cursor: self.cursor_state_internal(),
            selection: self.selection_state_internal(),
            alternate_screen_active,
            mouse_grabbed: self.mouse_grabbed(),
            application_cursor_keys: self.term.mode().contains(TermMode::APP_CURSOR),
            bracketed_paste_enabled: self.term.mode().contains(TermMode::BRACKETED_PASTE),
        }
    }

    fn resize(&mut self, rows: usize, cols: usize) {
        self.term.resize(AlacrittyDimensions::new(rows, cols));
        self.sequence_number = self.sequence_number.saturating_add(1);
    }

    fn set_theme(&mut self, mode: ThemeMode, variant: ThemeVariant) {
        if self.theme_mode != mode || self.theme_variant != variant {
            self.theme_mode = mode;
            self.theme_variant = variant;
            self.sequence_number = self.sequence_number.saturating_add(1);
        }
    }

    fn scroll_viewport_lines(&mut self, delta: i32) {
        if delta == 0 || self.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }

        self.term.scroll_display(Scroll::Delta(delta));
        self.sequence_number = self.sequence_number.saturating_add(1);
    }

    fn send_key_down(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>> {
        self.snap_viewport_to_bottom();
        Ok(key
            .encode(
                modifiers,
                KeyCodeEncodeModes {
                    encoding: KeyboardEncoding::Xterm,
                    newline_mode: false,
                    application_cursor_keys: self.term.mode().contains(TermMode::APP_CURSOR),
                    modify_other_keys: None,
                },
                true,
            )?
            .into_bytes())
    }

    fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>> {
        let key = match event.key {
            TerminalKeyKind::Named(name) => match named_key_code(name) {
                Some(key) => key,
                None => bail!("unsupported named terminal key `{name}`"),
            },
            TerminalKeyKind::Function(number) => KeyCode::Function(number),
            TerminalKeyKind::Char(ch) => KeyCode::Char(ch),
        };

        self.send_key_down(key, key_modifiers(event.alt, event.ctrl, event.shift))
    }

    fn send_mouse_input(&mut self, event: TerminalMouseInput) -> Result<Vec<u8>> {
        if !self.mouse_grabbed() {
            return Ok(Vec::new());
        }

        Ok(encode_sgr_mouse_fallback(
            event,
            self.resolve_fallback_mouse_button(event),
        ))
    }

    fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>> {
        self.snap_viewport_to_bottom();
        if self.term.mode().contains(TermMode::BRACKETED_PASTE) {
            return Ok(format!("\x1b[200~{text}\x1b[201~").into_bytes());
        }

        Ok(text.as_bytes().to_vec())
    }
}

#[derive(Clone, Copy)]
struct AlacrittyDimensions {
    rows: usize,
    cols: usize,
}

impl AlacrittyDimensions {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows: rows.max(1),
            cols: cols.max(2),
        }
    }
}

impl Dimensions for AlacrittyDimensions {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

fn row_text(row: &alacritty_terminal::grid::Row<Cell>, cols: usize) -> String {
    let line_length = row.line_length().0.min(cols);
    let mut text = String::new();

    for index in 0..line_length {
        let cell = &row[Column(index)];
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER)
            || cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }

        text.push(cell.c);
        if let Some(zerowidth) = cell.zerowidth() {
            for ch in zerowidth {
                text.push(*ch);
            }
        }
    }

    text.trim_end().to_string()
}

fn row_text_in_column_range(
    row: &alacritty_terminal::grid::Row<Cell>,
    start_col: usize,
    end_col: usize,
) -> String {
    if end_col <= start_col {
        return String::new();
    }

    let line_length = row.line_length().0.min(end_col);
    let mut text = String::new();

    for index in start_col..line_length {
        let cell = &row[Column(index)];
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER)
            || cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }

        text.push(cell.c);
        if let Some(zerowidth) = cell.zerowidth() {
            for ch in zerowidth {
                text.push(*ch);
            }
        }
    }

    text.trim_end_matches(' ').to_string()
}

fn visible_lines_from_rows(rows: &[TerminalRowState]) -> Vec<String> {
    let mut lines = rows.iter().map(|row| row.text.clone()).collect::<Vec<_>>();
    while lines.first().is_some_and(String::is_empty) {
        let _ = lines.remove(0);
    }
    while lines.last().is_some_and(String::is_empty) {
        let _ = lines.pop();
    }
    lines
}

fn cell_text(cell: &Cell) -> String {
    let mut text = String::from(cell.c);
    if let Some(zerowidth) = cell.zerowidth() {
        for ch in zerowidth {
            text.push(*ch);
        }
    }
    text
}

fn viewport_row(line: Line, display_offset: usize) -> u32 {
    line.0.saturating_add(display_offset as i32).max(0) as u32
}

fn buffer_row_to_grid_line(buffer_row: u32, history_size: usize) -> Line {
    Line(buffer_row as i32 - history_size as i32)
}

fn normalized_selection_bounds(start: (u32, u32), end: (u32, u32)) -> ((u32, u32), (u32, u32)) {
    if start.0 < end.0 || (start.0 == end.0 && start.1 <= end.1) {
        (start, end)
    } else {
        (end, start)
    }
}

fn named_key_code(key_name: &str) -> Option<KeyCode> {
    match key_name {
        "enter" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "escape" => Some(KeyCode::Escape),
        "backspace" => Some(KeyCode::Backspace),
        "insert" => Some(KeyCode::Insert),
        "delete" => Some(KeyCode::Delete),
        "up" => Some(KeyCode::UpArrow),
        "down" => Some(KeyCode::DownArrow),
        "left" => Some(KeyCode::LeftArrow),
        "right" => Some(KeyCode::RightArrow),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "page-up" => Some(KeyCode::PageUp),
        "page-down" => Some(KeyCode::PageDown),
        _ => None,
    }
}

fn key_modifiers(alt: bool, ctrl: bool, shift: bool) -> KeyModifiers {
    let mut modifiers = KeyModifiers::NONE;
    if alt {
        modifiers |= KeyModifiers::ALT;
    }
    if ctrl {
        modifiers |= KeyModifiers::CTRL;
    }
    if shift {
        modifiers |= KeyModifiers::SHIFT;
    }
    modifiers
}

fn resolve_cell_colors(
    theme_mode: ThemeMode,
    theme_variant: ThemeVariant,
    cell: &Cell,
) -> (u32, u32) {
    let mut fg = resolve_color(theme_mode, theme_variant, cell.fg, false);
    let mut bg = resolve_color(theme_mode, theme_variant, cell.bg, true);
    if cell.flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if cell.flags.contains(Flags::HIDDEN) {
        fg = bg;
    }
    (fg, bg)
}

fn resolve_color(
    theme_mode: ThemeMode,
    theme_variant: ThemeVariant,
    color: Color,
    background: bool,
) -> u32 {
    match color {
        Color::Spec(rgb) => pack_rgb(rgb),
        Color::Indexed(index) => resolve_indexed_color(theme_mode, theme_variant, index),
        Color::Named(named) => resolve_named_color(theme_mode, theme_variant, named, background),
    }
}

fn resolve_named_color(
    theme_mode: ThemeMode,
    theme_variant: ThemeVariant,
    named: NamedColor,
    background: bool,
) -> u32 {
    let spec = terminal_palette_spec_for(theme_mode, theme_variant);
    let dim = |value| dim_rgb(value);

    let rgb = match named {
        NamedColor::Black => spec.ansi[0],
        NamedColor::Red => spec.ansi[1],
        NamedColor::Green => spec.ansi[2],
        NamedColor::Yellow => spec.ansi[3],
        NamedColor::Blue => spec.ansi[4],
        NamedColor::Magenta => spec.ansi[5],
        NamedColor::Cyan => spec.ansi[6],
        NamedColor::White => spec.ansi[7],
        NamedColor::BrightBlack => spec.ansi[8],
        NamedColor::BrightRed => spec.ansi[9],
        NamedColor::BrightGreen => spec.ansi[10],
        NamedColor::BrightYellow => spec.ansi[11],
        NamedColor::BrightBlue => spec.ansi[12],
        NamedColor::BrightMagenta => spec.ansi[13],
        NamedColor::BrightCyan => spec.ansi[14],
        NamedColor::BrightWhite => spec.ansi[15],
        NamedColor::Foreground => spec.default_fg,
        NamedColor::Background => spec.default_bg,
        NamedColor::Cursor => {
            if background {
                spec.cursor_bg
            } else {
                spec.cursor_fg
            }
        }
        NamedColor::DimBlack => dim(spec.ansi[0]),
        NamedColor::DimRed => dim(spec.ansi[1]),
        NamedColor::DimGreen => dim(spec.ansi[2]),
        NamedColor::DimYellow => dim(spec.ansi[3]),
        NamedColor::DimBlue => dim(spec.ansi[4]),
        NamedColor::DimMagenta => dim(spec.ansi[5]),
        NamedColor::DimCyan => dim(spec.ansi[6]),
        NamedColor::DimWhite => dim(spec.ansi[7]),
        NamedColor::BrightForeground => spec.default_fg,
        NamedColor::DimForeground => dim(spec.default_fg),
    };

    pack_rgb_hex(rgb)
}

fn resolve_indexed_color(theme_mode: ThemeMode, theme_variant: ThemeVariant, index: u8) -> u32 {
    match index {
        0..=15 => {
            pack_rgb_hex(terminal_palette_spec_for(theme_mode, theme_variant).ansi[index as usize])
        }
        16..=231 => {
            let index = index - 16;
            let r = index / 36;
            let g = (index % 36) / 6;
            let b = index % 6;
            let channel = |component: u8| match component {
                0 => 0,
                value => 55 + u32::from(value) * 40,
            };
            pack_rgb_hex((channel(r) << 16) | (channel(g) << 8) | channel(b))
        }
        232..=255 => {
            let gray = 8 + (u32::from(index) - 232) * 10;
            pack_rgb_hex((gray << 16) | (gray << 8) | gray)
        }
    }
}

fn dim_rgb(rgb: u32) -> u32 {
    let red = ((rgb >> 16) & 0xff) * 2 / 3;
    let green = ((rgb >> 8) & 0xff) * 2 / 3;
    let blue = (rgb & 0xff) * 2 / 3;
    (red << 16) | (green << 8) | blue
}

fn pack_rgb(rgb: Rgb) -> u32 {
    0xff00_0000 | (u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b)
}

fn pack_rgb_hex(rgb: u32) -> u32 {
    0xff00_0000 | rgb
}

fn project_cursor_shape(shape: CursorShape) -> TerminalCursorShape {
    match shape {
        CursorShape::Underline => TerminalCursorShape::Underline,
        CursorShape::Beam => TerminalCursorShape::Bar,
        CursorShape::Block | CursorShape::HollowBlock | CursorShape::Hidden => {
            TerminalCursorShape::Block
        }
    }
}

fn cursor_shape_blinks(shape: CursorShape) -> bool {
    !matches!(
        shape,
        CursorShape::Underline | CursorShape::Beam | CursorShape::HollowBlock
    )
}

fn encode_sgr_mouse_fallback(event: TerminalMouseInput, button: TerminalMouseButton) -> Vec<u8> {
    let mut code = match button {
        TerminalMouseButton::Left => 0,
        TerminalMouseButton::Middle => 1,
        TerminalMouseButton::Right => 2,
        TerminalMouseButton::WheelUp => 64,
        TerminalMouseButton::WheelDown => 65,
        TerminalMouseButton::None => 3,
    };
    if event.shift {
        code += 4;
    }
    if event.alt {
        code += 8;
    }
    if event.ctrl {
        code += 16;
    }
    if matches!(event.kind, TerminalMouseEventKind::Move) {
        code += 32;
    }

    format!(
        "\x1b[<{};{};{}{}",
        code,
        event.col + 1,
        event.row + 1,
        if matches!(event.kind, TerminalMouseEventKind::Up) {
            "m"
        } else {
            "M"
        }
    )
    .into_bytes()
}
