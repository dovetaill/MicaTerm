//! Headless terminal engine built on `alacritty_terminal::Term` + `vte::ansi::Processor`.
//!
//! Handles ANSI/VT state, scrollback, selection, regex search, OSC-7 directory tracking,
//! and extracts compact render snapshots for GPU painting in GPUI.

use std::sync::Arc;

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::RegexSearch;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config as TermConfig, TermMode, point_to_viewport, viewport_to_point};
use alacritty_terminal::vte::ansi::{CursorShape, Processor};
use gpui_kit::Hsla;
use parking_lot::Mutex;

use crate::terminal::colors::{TerminalPalette, rgb_to_hsla};

#[derive(Debug, Clone)]
pub enum EngineSideEffect {
    PtyReply(Vec<u8>),
    TitleChanged(String),
    ClipboardStore(String),
    CurrentDirChanged(String),
    Bell,
}

#[derive(Clone, Default)]
pub struct TerminalEventProxy {
    queue: Arc<Mutex<Vec<EngineSideEffect>>>,
}

impl TerminalEventProxy {
    pub fn drain(&self) -> Vec<EngineSideEffect> {
        std::mem::take(&mut *self.queue.lock())
    }
}

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: Event) {
        let mut q = self.queue.lock();
        match event {
            Event::PtyWrite(text) => q.push(EngineSideEffect::PtyReply(text.into_bytes())),
            Event::Title(title) => q.push(EngineSideEffect::TitleChanged(title)),
            Event::ClipboardStore(_, text) => q.push(EngineSideEffect::ClipboardStore(text)),
            Event::Bell => q.push(EngineSideEffect::Bell),
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalBgSpan {
    pub start_col: usize,
    pub end_col: usize,
    pub color: Hsla,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalStyledRun {
    pub start_col: usize,
    pub col_len: usize,
    pub text: String,
    pub fg: Hsla,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TerminalRowSnapshot {
    pub bg_spans: Vec<TerminalBgSpan>,
    pub text_runs: Vec<TerminalStyledRun>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderCursorShape {
    Block,
    HollowBlock,
    Beam,
    Underline,
    Hidden,
}

#[derive(Debug, Clone)]
pub struct RenderCursor {
    pub row: usize,
    pub col: usize,
    pub shape: RenderCursorShape,
    pub color: Hsla,
    pub text_color: Hsla,
    pub wide: bool,
    pub ch: char,
}

#[derive(Debug, Clone)]
pub struct TerminalRenderSnapshot {
    pub rows: usize,
    pub cols: usize,
    pub background: Hsla,
    pub foreground: Hsla,
    pub lines: Vec<TerminalRowSnapshot>,
    pub selection_spans: Vec<(usize, usize, usize, Hsla)>,
    pub search_spans: Vec<(usize, usize, usize, Hsla)>,
    pub cursor: Option<RenderCursor>,
    pub display_offset: usize,
    pub history_size: usize,
    pub mode: TermMode,
    pub search_match_count: usize,
}

pub struct AlacrittyTerminalEngine {
    term: Term<TerminalEventProxy>,
    processor: Processor,
    proxy: TerminalEventProxy,
    rows: usize,
    cols: usize,
    osc7_tail: String,
    last_cwd: Option<String>,
    search_query: String,
    search_dfa: Option<RegexSearch>,
}

impl AlacrittyTerminalEngine {
    pub fn new(rows: usize, cols: usize, scrollback_lines: usize) -> Self {
        let rows = rows.max(2);
        let cols = cols.max(4);
        let size = TermSize::new(cols, rows);
        let mut config = TermConfig::default();
        config.scrolling_history = scrollback_lines.max(100);
        let proxy = TerminalEventProxy::default();
        let term = Term::new(config, &size, proxy.clone());

        Self {
            term,
            processor: Processor::new(),
            proxy,
            rows,
            cols,
            osc7_tail: String::new(),
            last_cwd: None,
            search_query: String::new(),
            search_dfa: None,
        }
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    pub fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Vec<EngineSideEffect> {
        self.processor.advance(&mut self.term, bytes);
        let mut effects = self.proxy.drain();
        if let Some(cwd) = self.scan_osc7_cwd(bytes) {
            if self.last_cwd.as_deref() != Some(cwd.as_str()) {
                self.last_cwd = Some(cwd.clone());
                effects.push(EngineSideEffect::CurrentDirChanged(cwd));
            }
        }
        effects
    }

    pub fn resize(&mut self, rows: usize, cols: usize) -> bool {
        let rows = rows.max(2);
        let cols = cols.max(4);
        if self.rows == rows && self.cols == cols {
            return false;
        }
        self.rows = rows;
        self.cols = cols;
        self.term.resize(TermSize::new(cols, rows));
        true
    }

    pub fn scroll(&mut self, scroll: Scroll) {
        self.term.scroll_display(scroll);
    }

    pub fn scroll_to_ratio(&mut self, ratio_from_top: f32) {
        let history = self.term.grid().history_size();
        if history == 0 {
            return;
        }
        let clamped = ratio_from_top.clamp(0.0, 1.0);
        // ratio 0.0 = top of history (display_offset = history), ratio 1.0 = live bottom (offset = 0)
        let target_offset = ((1.0 - clamped) * (history as f32)).round() as usize;
        let current = self.term.grid().display_offset();
        let delta = target_offset as i32 - current as i32;
        if delta != 0 {
            self.term.scroll_display(Scroll::Delta(delta));
        }
    }

    pub fn start_selection(&mut self, row: usize, col: usize, click_count: usize) {
        let sel_type = match click_count {
            2 => SelectionType::Semantic,
            3.. => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        let point = viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(row.min(self.rows.saturating_sub(1)), Column(col.min(self.cols.saturating_sub(1)))),
        );
        self.term.selection = Some(Selection::new(sel_type, point, Side::Left));
    }

    pub fn update_selection(&mut self, row: usize, col: usize, side: Side) {
        let point = viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(row.min(self.rows.saturating_sub(1)), Column(col.min(self.cols.saturating_sub(1)))),
        );
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    pub fn select_all(&mut self) {
        let top = self.term.topmost_line();
        let bottom = self.term.bottommost_line();
        let mut sel = Selection::new(SelectionType::Lines, Point::new(top, Column(0)), Side::Left);
        sel.update(
            Point::new(bottom, Column(self.cols.saturating_sub(1))),
            Side::Right,
        );
        self.term.selection = Some(sel);
    }

    pub fn selected_text(&self) -> Option<String> {
        self.term
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }

    pub fn set_search_query(&mut self, query: &str) {
        let trimmed = query.trim();
        if self.search_query == trimmed {
            return;
        }
        self.search_query = trimmed.to_string();
        if trimmed.is_empty() {
            self.search_dfa = None;
        } else {
            let escaped = escape_regex_literal(trimmed);
            self.search_dfa = RegexSearch::new(&escaped).ok();
        }
    }

    pub fn jump_search(&mut self, forward: bool) -> bool {
        let Some(mut dfa) = self.search_dfa.clone() else {
            return false;
        };
        let direction = if forward {
            Direction::Right
        } else {
            Direction::Left
        };
        let origin = if let Some(range) = self.term.selection.as_ref().and_then(|s| s.to_range(&self.term)) {
            if forward {
                range.end.add(&self.term, alacritty_terminal::index::Boundary::None, 1)
            } else {
                range.start.sub(&self.term, alacritty_terminal::index::Boundary::None, 1)
            }
        } else {
            viewport_to_point(self.term.grid().display_offset(), Point::new(0, Column(0)))
        };

        let found = self.term.search_next(&mut dfa, origin, direction, Side::Left, None);
        self.search_dfa = Some(dfa);
        if let Some(matched) = found {
            let start = *matched.start();
            let end = *matched.end();
            let mut sel = Selection::new(SelectionType::Simple, start, Side::Left);
            sel.update(end, Side::Right);
            self.term.selection = Some(sel);

            // Scroll viewport if match is outside current view
            let current_offset = self.term.grid().display_offset() as i32;
            let view_top = -current_offset;
            let view_bottom = view_top + self.rows as i32 - 1;
            if start.line.0 < view_top || start.line.0 > view_bottom {
                let desired_top = (start.line.0 - (self.rows as i32 / 2)).min(0);
                let desired_offset = (-desired_top).max(0);
                let delta = desired_offset - current_offset;
                if delta != 0 {
                    self.term.scroll_display(Scroll::Delta(delta));
                }
            }
            true
        } else {
            false
        }
    }

    pub fn snapshot(&mut self, palette: &TerminalPalette, focused: bool) -> TerminalRenderSnapshot {
        let rows = self.rows;
        let cols = self.cols;
        let mut row_snapshots = vec![TerminalRowSnapshot::default(); rows];

        {
            let content = self.term.renderable_content();
            let display_offset = content.display_offset;
            let term_colors = content.colors;

            for indexed in content.display_iter {
                let Some(vp) = point_to_viewport(display_offset, indexed.point) else {
                    continue;
                };
                let r = vp.line;
                let c = vp.column.0;
                if r >= rows || c >= cols {
                    continue;
                }

                let cell = indexed.cell;
                let flags = cell.flags;

                if flags.contains(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }

                let col_width = if flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
                let (fg, bg_opt) =
                    palette.resolve_cell_colors(cell.fg, cell.bg, flags, term_colors);

                let row_snap = &mut row_snapshots[r];

                // Coalesce contiguous background quads on the row
                if let Some(bg_color) = bg_opt {
                    let end_col = (c + col_width).min(cols);
                    if let Some(last_bg) = row_snap.bg_spans.last_mut()
                        && last_bg.end_col == c
                        && last_bg.color == bg_color
                    {
                        last_bg.end_col = end_col;
                    } else {
                        row_snap.bg_spans.push(TerminalBgSpan {
                            start_col: c,
                            end_col,
                            color: bg_color,
                        });
                    }
                }

                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                let bold = flags.contains(Flags::BOLD);
                let italic = flags.contains(Flags::ITALIC);
                let underline = flags.intersects(Flags::ALL_UNDERLINES);
                let strikethrough = flags.contains(Flags::STRIKEOUT);

                // Skip empty unstyled space cells so we don't shape blank whitespace unnecessarily
                if ch == ' ' && !underline && !strikethrough {
                    continue;
                }

                if let Some(last_run) = row_snap.text_runs.last_mut()
                    && last_run.start_col + last_run.col_len == c
                    && last_run.fg == fg
                    && last_run.bold == bold
                    && last_run.italic == italic
                    && last_run.underline == underline
                    && last_run.strikethrough == strikethrough
                    // Wide characters are placed in dedicated 2-cell runs so column alignment stays exact
                    && col_width == 1
                {
                    last_run.text.push(ch);
                    for zw in cell.zerowidth().into_iter().flatten() {
                        last_run.text.push(*zw);
                    }
                    last_run.col_len += 1;
                } else {
                    let mut text = String::with_capacity(4);
                    text.push(ch);
                    for zw in cell.zerowidth().into_iter().flatten() {
                        text.push(*zw);
                    }
                    row_snapshots[r].text_runs.push(TerminalStyledRun {
                        start_col: c,
                        col_len: col_width,
                        text,
                        fg,
                        bold,
                        italic,
                        underline,
                        strikethrough,
                    });
                }
            }
        }

        let display_offset = self.term.grid().display_offset();
        let history_size = self.term.grid().history_size();
        let mode = *self.term.mode();

        // Selection highlight spans
        let mut selection_spans = Vec::new();
        if let Some(sel_range) = self.term.selection.as_ref().and_then(|s| s.to_range(&self.term)) {
            for r in 0..rows {
                let grid_line = Line(r as i32 - display_offset as i32);
                if grid_line < sel_range.start.line || grid_line > sel_range.end.line {
                    continue;
                }
                let start_col = if sel_range.is_block || grid_line == sel_range.start.line {
                    sel_range.start.column.0
                } else {
                    0
                };
                let end_col = if sel_range.is_block || grid_line == sel_range.end.line {
                    (sel_range.end.column.0 + 1).min(cols)
                } else {
                    cols
                };
                if start_col < end_col {
                    selection_spans.push((r, start_col, end_col, palette.selection_bg));
                }
            }
        }

        // Visible search highlight spans
        let mut search_spans = Vec::new();
        let mut search_match_count = 0usize;
        if !self.search_query.is_empty() {
            let needle = self.search_query.to_lowercase();
            for r in 0..rows {
                let grid_line = Line(r as i32 - display_offset as i32);
                let mut line_chars: Vec<(usize, char)> = Vec::with_capacity(cols);
                for c in 0..cols {
                    let cell = &self.term.grid()[grid_line][Column(c)];
                    if !cell
                        .flags
                        .contains(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                    {
                        line_chars.push((c, if cell.c == '\0' { ' ' } else { cell.c }));
                    }
                }
                let rendered: String = line_chars.iter().map(|(_, ch)| *ch).collect();
                let lowered = rendered.to_lowercase();
                let mut byte_from = 0usize;
                while let Some(pos) = lowered[byte_from..].find(&needle) {
                    let match_byte_start = byte_from + pos;
                    let match_byte_end = match_byte_start + needle.len();
                    let char_start = lowered[..match_byte_start].chars().count();
                    let char_end = lowered[..match_byte_end].chars().count();
                    if let Some(&(sc, _)) = line_chars.get(char_start) {
                        let ec = line_chars
                            .get(char_end)
                            .map(|(col, _)| *col)
                            .unwrap_or(cols);
                        if sc < ec {
                            search_spans.push((r, sc, ec, palette.search_match_bg));
                            search_match_count += 1;
                        }
                    }
                    byte_from = match_byte_end.max(byte_from + 1);
                    if byte_from >= lowered.len() {
                        break;
                    }
                }
            }
        }

        // Renderable cursor
        let content = self.term.renderable_content();
        let cursor = point_to_viewport(display_offset, content.cursor.point).and_then(|vp| {
            if vp.line >= rows || vp.column.0 >= cols {
                return None;
            }
            let shape = match content.cursor.shape {
                CursorShape::Hidden => return None,
                CursorShape::Block if !focused => RenderCursorShape::HollowBlock,
                CursorShape::Block => RenderCursorShape::Block,
                CursorShape::HollowBlock => RenderCursorShape::HollowBlock,
                CursorShape::Beam => RenderCursorShape::Beam,
                CursorShape::Underline => RenderCursorShape::Underline,
            };
            let cell = &self.term.grid()[content.cursor.point];
            let wide = cell.flags.contains(Flags::WIDE_CHAR);
            let ch = if cell.c == '\0' { ' ' } else { cell.c };
            Some(RenderCursor {
                row: vp.line,
                col: vp.column.0,
                shape,
                color: rgb_to_hsla(palette.cursor, 0.92),
                text_color: rgb_to_hsla(palette.cursor_text, 1.0),
                wide,
                ch,
            })
        });

        TerminalRenderSnapshot {
            rows,
            cols,
            background: rgb_to_hsla(palette.background, 1.0),
            foreground: rgb_to_hsla(palette.foreground, 1.0),
            lines: row_snapshots,
            selection_spans,
            search_spans,
            cursor,
            display_offset,
            history_size,
            mode,
            search_match_count,
        }
    }

    fn scan_osc7_cwd(&mut self, bytes: &[u8]) -> Option<String> {
        let chunk = String::from_utf8_lossy(bytes);
        self.osc7_tail.push_str(&chunk);
        if self.osc7_tail.len() > 4096 {
            let drain_to = self.osc7_tail.len() - 2048;
            let boundary = self.osc7_tail.ceil_char_boundary(drain_to);
            self.osc7_tail.drain(..boundary);
        }

        let mut latest_dir = None;
        while let Some(start) = self.osc7_tail.find("\x1b]7;") {
            let after = &self.osc7_tail[start + 4..];
            let end_rel = after
                .find('\x07')
                .map(|i| (i, 1))
                .or_else(|| after.find("\x1b\\").map(|i| (i, 2)));
            let Some((end_idx, term_len)) = end_rel else {
                // Incomplete sequence; keep from `start`
                if start > 0 {
                    self.osc7_tail.drain(..start);
                }
                break;
            };
            let uri = &after[..end_idx];
            if let Some(parsed) = parse_osc7_file_uri(uri) {
                latest_dir = Some(parsed);
            }
            let consume = start + 4 + end_idx + term_len;
            self.osc7_tail.drain(..consume);
        }
        latest_dir
    }
}

pub fn parse_osc7_file_uri(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let slash_idx = rest.find('/')?;
    let raw_path = &rest[slash_idx..];
    let decoded = percent_decode(raw_path);
    (!decoded.is_empty()).then_some(decoded)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex_str) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(val) = u8::from_str_radix(hex_str, 16)
        {
            out.push(val);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn escape_regex_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_renders_ansi_text_and_parses_osc7() {
        let mut engine = AlacrittyTerminalEngine::new(24, 80, 1000);
        let effects = engine.feed_bytes(
            b"\x1b]7;file://prod-host/var/log/nginx\x07\x1b[1;32mREADY\x1b[0m world\r\n",
        );
        assert!(effects.iter().any(|e| matches!(
            e,
            EngineSideEffect::CurrentDirChanged(dir) if dir == "/var/log/nginx"
        )));

        let snap = engine.snapshot(&TerminalPalette::dark(), true);
        assert_eq!(snap.lines[0].text_runs.len(), 2);
        assert_eq!(snap.lines[0].text_runs[0].start_col, 0);
        assert_eq!(snap.lines[0].text_runs[0].text, "READY");
        assert!(snap.lines[0].text_runs[0].bold);
        assert_eq!(snap.lines[0].text_runs[1].start_col, 6);
        assert_eq!(snap.lines[0].text_runs[1].text, "world");
    }
}
