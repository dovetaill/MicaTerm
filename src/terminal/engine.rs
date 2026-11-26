//! Headless terminal engine built on `alacritty_terminal::Term` + `vte::ansi::Processor`.
//!
//! Handles ANSI/VT state, strict CJK wide-cell run splitting, OSC 7 / OSC 1337 / Title CWD tracking,
//! OSC 133 & prompt-based command blocks, semantic output/input highlighting, regex search, and
//! scrollbar overview markers.

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::{Flags, LineLength};
use alacritty_terminal::term::search::RegexSearch;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{
    Config as TermConfig, TermMode, point_to_viewport, viewport_to_point,
};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor};
use gpui_kit::Hsla;
use parking_lot::Mutex;
use regex::Regex;

use syntect::parsing::{ParseState, ScopeStack};

use crate::syntax::{CodeTokenRole, SyntaxPack, SyntaxRegistry};
use crate::terminal::colors::{TerminalPalette, rgb_to_hsla};
use crate::terminal::semantic::{
    CommandBlockStatus, CommandSyntaxTarget, OverviewMarker, OverviewMarkerKind, RowTextMap,
    SemanticOptions, SemanticRole, VisibleCommandBlock, detect_command_syntax_target,
    detect_input_spans_for_prompt_row, detect_output_spans_for_row,
    detect_prompt_command_byte_offset, extract_cwd_from_prompt_line, extract_cwd_from_window_title,
    extract_http_url_byte_ranges, line_has_error_signature, openable_http_url,
    validate_openable_http_url,
};

const MAX_OSC133_RECORDS: usize = 512;
const MAX_SYNTAX_BLOCK_LINES: usize = 32_000;
const MAX_SYNTAX_LINE_BYTES: usize = 2_048;

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
    pub wide: bool,
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
    pub block_divider: Hsla,
    pub lines: Vec<TerminalRowSnapshot>,
    pub selection_spans: Vec<(usize, usize, usize, Hsla)>,
    pub search_spans: Vec<(usize, usize, usize, Hsla)>,
    pub command_blocks: Vec<VisibleCommandBlock>,
    pub overview_markers: Vec<OverviewMarker>,
    pub cursor: Option<RenderCursor>,
    pub display_offset: usize,
    pub history_size: usize,
    pub mode: TermMode,
    pub search_match_count: usize,
}

#[derive(Debug, Clone)]
struct Osc133Record {
    /// Monotonic line index from top of scrollback buffer (`history_size + grid_line`).
    abs_start_line: usize,
    abs_end_line: Option<usize>,
    status: CommandBlockStatus,
}

struct CachedLogicalLineSpans {
    hash: u64,
    end_gl: i32,
    segments: Vec<(i32, usize, usize)>,
    spans: Vec<(Range<usize>, CodeTokenRole)>,
}

struct CachedBlockCover {
    covered_scan_end_gl: i32,
    out_start_gl: i32,
    out_end_gl: i32,
    conpty_white: bool,
    tail_spans: Option<(i32, i32, Vec<(i32, usize, usize)>, Vec<(Range<usize>, CodeTokenRole)>)>,
}

struct BlockSyntaxCache {
    dirty_generation: u64,
    syntax_revision: u64,
    cols: usize,
    prompt_gl: i32,
    prompt_cmd: String,
    syntax_name: String,
    completed_spans: Vec<CachedLogicalLineSpans>,
    checkpoint: (ParseState, ScopeStack),
    cover: Option<CachedBlockCover>,
}

struct StitchedLogicalLine {
    start_gl: i32,
    end_gl: i32,
    /// `(grid_line, byte_start_in_logical, byte_end_in_logical)` for each physical row segment.
    segments: Vec<(i32, usize, usize)>,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoveredLinkMatch {
    pub url: String,
    pub osc8_host_hint: Option<String>,
    pub viewport_spans: Vec<(usize, usize, usize)>,
}

#[derive(Default)]
struct PromptScanCache {
    // Absolute physical rows already classified, including gaps between prompts.
    checked: Range<usize>,
    anchors: Vec<usize>,
    #[cfg(test)]
    scanned_lines: usize,
}

pub struct AlacrittyTerminalEngine {
    term: Term<TerminalEventProxy>,
    processor: Processor,
    proxy: TerminalEventProxy,
    rows: usize,
    cols: usize,
    scrollback_limit: usize,
    osc_tail: String,
    last_cwd: Option<String>,
    remote_home_dir: Option<String>,
    osc133_records: VecDeque<Osc133Record>,
    search_query: String,
    search_dfa: Option<RegexSearch>,
    row_search: Option<Regex>,
    dirty_generation: u64,
    cached_overview: Option<(u64, String, Vec<OverviewMarker>, usize)>,
    pub(super) is_windows_local_pty: bool,
    prompt_scan: PromptScanCache,
    block_syntax_cache: Option<BlockSyntaxCache>,
}

impl AlacrittyTerminalEngine {
    pub fn new(rows: usize, cols: usize, scrollback_lines: usize) -> Self {
        let rows = rows.max(2);
        let cols = cols.max(4);
        let scrollback_limit = scrollback_lines.max(100);
        let size = TermSize::new(cols, rows);
        let config = TermConfig {
            scrolling_history: scrollback_limit,
            ..Default::default()
        };
        let proxy = TerminalEventProxy::default();
        let term = Term::new(config, &size, proxy.clone());

        Self {
            term,
            processor: Processor::new(),
            proxy,
            rows,
            cols,
            scrollback_limit,
            osc_tail: String::new(),
            last_cwd: None,
            remote_home_dir: None,
            osc133_records: VecDeque::new(),
            search_query: String::new(),
            search_dfa: None,
            row_search: None,
            dirty_generation: 1,
            cached_overview: None,
            is_windows_local_pty: false,
            prompt_scan: PromptScanCache::default(),
            block_syntax_cache: None,
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

    pub fn set_remote_home_dir(&mut self, home: Option<String>) {
        self.remote_home_dir = home.filter(|s| !s.is_empty());
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Vec<EngineSideEffect> {
        let mut detected_cwd = None;
        // Observe each OSC at its actual cursor position, even in a multi-command PTY read.
        for chunk in bytes.split_inclusive(|&b| b == 7 || b == b'\\') {
            self.advance_grid(chunk);
            if let Some(dir) = self.scan_osc_sequences(chunk) {
                detected_cwd = Some(dir);
            }
        }
        self.dirty_generation = self.dirty_generation.wrapping_add(1);
        let mut effects = self.proxy.drain();

        // Fallback CWD extraction from standard OSC 0/2 shell prompt titles (`user@host:/path`)
        if detected_cwd.is_none() {
            for eff in &effects {
                if let EngineSideEffect::TitleChanged(title) = eff
                    && let Some(cwd) =
                        extract_cwd_from_window_title(title, self.remote_home_dir.as_deref())
                {
                    detected_cwd = Some(cwd);
                }
            }
        }

        if detected_cwd.is_none() && !self.mode().contains(TermMode::ALT_SCREEN) {
            let row = self.term.grid().cursor.point.line;
            if !self.row_has_wrapline(row) {
                let start = *self.logical_rows(row.0).start();
                let line = if start == row.0 {
                    self.extract_plain_grid_line(row)
                } else {
                    self.stitch_logical_line_at(start, row.0, row.0, &[]).text
                };
                detected_cwd = extract_cwd_from_prompt_line(&line, self.remote_home_dir.as_deref());
            }
        }
        if let Some(cwd) = detected_cwd
            && self.last_cwd.as_deref() != Some(cwd.as_str())
        {
            self.last_cwd = Some(cwd.clone());
            effects.push(EngineSideEffect::CurrentDirChanged(cwd));
        }

        effects
    }

    fn advance_grid(&mut self, chunk: &[u8]) {
        let step_cap = if chunk.contains(&0x1b) {
            (self.scrollback_limit.saturating_sub(1) / self.rows.max(1)).max(1)
        } else {
            self.scrollback_limit
                .saturating_sub(self.rows.saturating_add(1))
                .max(1)
        };
        for sub in chunk.chunks(step_cap) {
            self.advance_grid_step(sub);
        }
    }

    fn advance_grid_step(&mut self, chunk: &[u8]) {
        let history = self.term.grid().history_size();
        // Grid scrolling rotates row allocations. Locate the former last history row
        // when the history length alone cannot tell us how many rows were evicted.
        let tail = (history > 0).then(|| self.term.grid()[Line(-1)][..].as_ptr());
        let alt = self.mode().contains(TermMode::ALT_SCREEN);
        let mut stable = history;
        if self.prompt_scan.checked.end >= history {
            while stable > 0 && self.row_has_wrapline(Line(stable as i32 - history as i32 - 1)) {
                stable -= 1;
            }
        }
        self.processor.advance(&mut self.term, chunk);
        let now = self.term.grid().history_size();
        let dropped = if now < history || alt != self.mode().contains(TermMode::ALT_SCREEN) {
            None
        } else if now < self.scrollback_limit {
            // A clear followed by fresh output can finish at the old history length.
            tail.is_none_or(|ptr| {
                self.term.grid()[Line(-((now - history) as i32) - 1)][..].as_ptr() == ptr
            })
            .then_some(0)
        } else {
            // A control sequence can scroll a screenful. If a full rotation was possible
            // the old allocation may already contain unrelated text.
            let max_scroll = if chunk.contains(&0x1b) {
                chunk.len().saturating_mul(self.rows)
            } else {
                chunk.len().saturating_add(self.rows)
            };
            tail.filter(|_| max_scroll < history).and_then(|ptr| {
                (0..=max_scroll)
                    .find(|&i| self.term.grid()[Line(-(i as i32) - 1)][..].as_ptr() == ptr)
                    .map(|i| (history + i).saturating_sub(now))
            })
        };
        self.prompt_scan.checked.end = self.prompt_scan.checked.end.min(stable);
        self.prompt_scan.checked.start = self.prompt_scan.checked.start.min(stable);
        self.prompt_scan
            .anchors
            .truncate(self.prompt_scan.anchors.partition_point(|&a| a < stable));
        let dropped = dropped.unwrap_or(history + self.rows);
        if dropped != 0 {
            self.prompt_scan.checked.start = self.prompt_scan.checked.start.saturating_sub(dropped);
            self.prompt_scan.checked.end = self.prompt_scan.checked.end.saturating_sub(dropped);
            self.prompt_scan.anchors.retain_mut(|a| {
                if *a < dropped {
                    return false;
                }
                *a -= dropped;
                true
            });
            self.osc133_records.retain_mut(|rec| {
                if rec.abs_start_line < dropped {
                    return false;
                }
                rec.abs_start_line -= dropped;
                rec.abs_end_line = rec.abs_end_line.map(|end| end.saturating_sub(dropped));
                true
            });
        }
    }

    pub fn resize(&mut self, rows: usize, cols: usize) -> bool {
        let rows = rows.max(2);
        let cols = cols.max(4);
        if self.rows == rows && self.cols == cols {
            return false;
        }
        self.rows = rows;
        self.cols = cols;
        self.prompt_scan = PromptScanCache::default();
        self.osc133_records.clear();
        self.term.resize(TermSize::new(cols, rows));
        self.dirty_generation = self.dirty_generation.wrapping_add(1);
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
        let target_offset = ((1.0 - clamped) * (history as f32)).round() as usize;
        let current = self.term.grid().display_offset();
        let delta = target_offset as i32 - current as i32;
        if delta != 0 {
            self.term.scroll_display(Scroll::Delta(delta));
        }
    }

    /// Jumps viewport to the previous (`previous = true`) or next (`previous = false`)
    /// command prompt block in scrollback history.
    pub fn jump_command_block(&mut self, previous: bool) -> bool {
        let history = self.term.grid().history_size() as i32;
        let current_offset = self.term.grid().display_offset() as i32;
        let anchor_grid_line = -current_offset + 2;

        let search_range: Box<dyn Iterator<Item = i32>> = if previous {
            Box::new(((-history)..anchor_grid_line).rev())
        } else {
            Box::new((anchor_grid_line + 1)..(self.rows as i32))
        };

        let max_gl = (self.rows as i32) - 1;
        for gl in search_range {
            if gl > -history && self.row_has_wrapline(Line(gl - 1)) {
                continue;
            }
            let line = self.stitch_logical_line_at(gl, max_gl, 0, &[]);
            if !self.is_inside_osc133_output(gl, history as usize)
                && let Some((_, true)) = detect_prompt_command_byte_offset(&line.text)
            {
                let desired_top = (gl - 1).clamp(-history, 0);
                let desired_offset = (-desired_top).max(0);
                let delta = desired_offset - current_offset;
                if delta != 0 {
                    self.term.scroll_display(Scroll::Delta(delta));
                    return true;
                }
            }
        }
        false
    }

    pub fn link_at(&self, vp_row: usize, col: usize) -> Option<HoveredLinkMatch> {
        if vp_row >= self.rows || col >= self.cols {
            return None;
        }
        let view_top = -(self.display_offset() as i32);
        let gl = view_top + vp_row as i32;
        let grid = self.term.grid();
        let top = grid.topmost_line().0;
        let bottom = grid.bottommost_line().0;
        if grid[Line(gl)][Column(col)]
            .flags
            .contains(Flags::LEADING_WIDE_CHAR_SPACER)
        {
            return None;
        }
        let cell_at = |line: i32, col: usize| {
            let row = &grid[Line(line)];
            let col = if row[Column(col)].flags.contains(Flags::WIDE_CHAR_SPACER) {
                col.saturating_sub(1)
            } else {
                col
            };
            &row[Column(col)]
        };
        if let Some(link) = cell_at(gl, col).hyperlink() {
            let parsed = openable_http_url(link.uri())?;
            let index = (gl - top) as usize * self.cols + col;
            let len = (bottom - top + 1) as usize * self.cols;
            let same = |i: usize| {
                cell_at(top + (i / self.cols) as i32, i % self.cols)
                    .hyperlink()
                    .as_ref()
                    == Some(&link)
            };
            let mut start = index;
            let mut end = index + 1;
            while start > 0 && same(start - 1) {
                start -= 1;
            }
            while end < len && same(end) {
                end += 1;
            }
            let mut label = String::new();
            let viewport_spans = self.viewport_spans(top, start, end).collect();
            while start < end {
                let line = top + (start / self.cols) as i32;
                let left = start % self.cols;
                let width = (end - start).min(self.cols - left);
                label.extend(
                    self.row_text_cells(Line(line))
                        .filter(|(c, _, _)| (left..left + width).contains(c))
                        .map(|(_, _, ch)| ch),
                );
                start += width;
            }
            let host = parsed.host_str()?;
            let osc8_host_hint = (!label.to_ascii_lowercase().contains(host)
                && label != link.uri())
            .then(|| host.to_owned());
            return Some(HoveredLinkMatch {
                url: link.uri().into(),
                viewport_spans,
                osc8_host_hint,
            });
        }

        let lines = self.logical_rows(gl);
        let start = *lines.start();
        let map = self.logical_text_map(lines);
        let offset = map.byte_at_col((gl - start) as usize * self.cols + col)?;
        let (left, right) = extract_http_url_byte_ranges(&map.text)
            .into_iter()
            .find(|&(a, b)| (a..b).contains(&offset))?;
        let (sc, ec) = map.byte_range_to_cols(left, right)?;
        Some(HoveredLinkMatch {
            url: map.text[left..right].into(),
            viewport_spans: self.viewport_spans(start, sc, ec).collect(),
            osc8_host_hint: None,
        })
    }

    fn logical_rows(&self, gl: i32) -> std::ops::RangeInclusive<i32> {
        let mut start = gl;
        let mut end = gl;
        while start > self.term.topmost_line().0 && self.row_has_wrapline(Line(start - 1)) {
            start -= 1;
        }
        while end < self.term.bottommost_line().0 && self.row_has_wrapline(Line(end)) {
            end += 1;
        }
        start..=end
    }

    fn logical_text_map(&self, lines: std::ops::RangeInclusive<i32>) -> RowTextMap {
        let start = *lines.start();
        RowTextMap::from_cells(lines.flat_map(|gl| {
            self.row_text_cells(Line(gl))
                .map(move |(col, width, ch)| ((gl - start) as usize * self.cols + col, width, ch))
        }))
    }

    fn viewport_spans(
        &self,
        start: i32,
        sc: usize,
        ec: usize,
    ) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        let top = -(self.display_offset() as i32);
        let first = (start + (sc / self.cols) as i32).max(top);
        let last = (start + ((ec - 1) / self.cols) as i32).min(top + self.rows as i32 - 1);
        (first..=last).filter_map(move |gl| {
            let base = (gl - start) as usize * self.cols;
            let last_is_spacer = self.term.grid()[Line(gl)][Column(self.cols - 1)]
                .flags
                .contains(Flags::LEADING_WIDE_CHAR_SPACER);
            let left = sc.saturating_sub(base);
            let right = (ec - base).min(self.cols - usize::from(last_is_spacer));
            (left < right).then_some(((gl - top) as usize, left, right))
        })
    }

    pub fn start_selection(&mut self, row: usize, col: usize, click_count: usize) {
        let sel_type = match click_count {
            2 => SelectionType::Semantic,
            3.. => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        let point = viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(
                row.min(self.rows.saturating_sub(1)),
                Column(col.min(self.cols.saturating_sub(1))),
            ),
        );
        self.term.selection = Some(Selection::new(sel_type, point, Side::Left));
    }

    pub fn update_selection(&mut self, row: usize, col: usize, side: Side) {
        let point = viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(
                row.min(self.rows.saturating_sub(1)),
                Column(col.min(self.cols.saturating_sub(1))),
            ),
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
        let mut sel =
            Selection::new(SelectionType::Lines, Point::new(top, Column(0)), Side::Left);
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
        self.cached_overview = None;
        if trimmed.is_empty() {
            self.search_dfa = None;
            self.row_search = None;
        } else {
            // Case-insensitive literal search so Enter/Next matches visual highlights
            let escaped = format!("(?i){}", regex::escape(trimmed));
            self.search_dfa = RegexSearch::new(&escaped).ok();
            self.row_search = Regex::new(&escaped).ok();
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
        let origin = if let Some(range) = self
            .term
            .selection
            .as_ref()
            .and_then(|s| s.to_range(&self.term))
        {
            if forward {
                range
                    .end
                    .add(&self.term, alacritty_terminal::index::Boundary::None, 1)
            } else {
                range
                    .start
                    .sub(&self.term, alacritty_terminal::index::Boundary::None, 1)
            }
        } else {
            viewport_to_point(self.term.grid().display_offset(), Point::new(0, Column(0)))
        };

        let found = self
            .term
            .search_next(&mut dfa, origin, direction, Side::Left, None);
        self.search_dfa = Some(dfa);
        if let Some(matched) = found {
            let start = *matched.start();
            let end = *matched.end();
            let mut sel = Selection::new(SelectionType::Simple, start, Side::Left);
            sel.update(end, Side::Right);
            self.term.selection = Some(sel);

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

    pub fn snapshot(
        &mut self,
        palette: &TerminalPalette,
        focused: bool,
        semantic_opts: SemanticOptions,
    ) -> TerminalRenderSnapshot {
        let rows = self.rows;
        let cols = self.cols;
        let display_offset = self.term.grid().display_offset();
        let history_size = self.term.grid().history_size();
        let mode = *self.term.mode();
        let is_tui = mode.contains(TermMode::ALT_SCREEN) || mode.intersects(TermMode::MOUSE_MODE);

        // Precompute per-row semantic & syntax role overrides for unstyled default-fg cells
        let mut semantic_cell_overrides: Vec<Vec<Option<(Hsla, bool, bool)>>> = Vec::new();
        let mut white_rows = vec![false; rows];
        let mut row_maps: Vec<RowTextMap> = Vec::with_capacity(rows);
        if self.row_search.is_some()
            || (!is_tui
                && (semantic_opts.semantic_highlighting
                    || semantic_opts.code_highlighting
                    || semantic_opts.command_blocks))
        {
            for r in 0..rows {
                let gl = Line(r as i32 - display_offset as i32);
                row_maps.push(self.build_row_text_map(gl));
            }
        }

        if !is_tui && (semantic_opts.code_highlighting || semantic_opts.semantic_highlighting) {
            semantic_cell_overrides.resize_with(rows, || vec![None; cols]);

            // 1. Base layer: multi-language syntax coloring on recognized command output blocks
            if semantic_opts.code_highlighting {
                self.populate_code_syntax_overrides(
                    &row_maps,
                    display_offset,
                    history_size,
                    palette,
                    &mut semantic_cell_overrides,
                    &mut white_rows,
                );
            }

            // 2. Top layer: input command tokens on prompt rows & URLs / file:line / diagnostics on output rows
            if semantic_opts.semantic_highlighting {
                let view_top = -(display_offset as i32);
                let mut gl = view_top;
                while gl < view_top + rows as i32 {
                    let lines = self.logical_rows(gl);
                    let start = *lines.start();
                    gl = *lines.end() + 1;
                    let stitched;
                    let map = if start == gl - 1 {
                        &row_maps[(start - view_top) as usize]
                    } else {
                        stitched = self.logical_text_map(lines);
                        &stitched
                    };
                    let spans = if !self.is_inside_osc133_output(start, history_size)
                        && let Some((cmd_byte, true)) =
                            detect_prompt_command_byte_offset(&map.text)
                    {
                        detect_input_spans_for_prompt_row(map, cmd_byte)
                    } else {
                        detect_output_spans_for_row(map)
                    };
                    for sp in spans {
                        let style = palette.semantic_style(sp.role);
                        for (r, sc, ec) in self.viewport_spans(start, sp.start_col, sp.end_col) {
                            let row = &self.term.grid()[Line(view_top + r as i32)];
                            for c in sc..ec {
                                let cell = &row[Column(c)];
                                if cell.fg == Color::Named(NamedColor::Foreground)
                                    && !(sp.role == SemanticRole::OutputUrl
                                        && cell.hyperlink().is_some())
                                {
                                    semantic_cell_overrides[r][c] = Some(style);
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut last_osc_style = None;
        let mut row_snapshots = vec![TerminalRowSnapshot::default(); rows];
        {
            let content = self.term.renderable_content();
            let term_colors = content.colors;

            for indexed in content.display_iter {
                let Some(vp) = point_to_viewport(content.display_offset, indexed.point) else {
                    continue;
                };
                let r = vp.line;
                let c = vp.column.0;
                if r >= rows || c >= cols {
                    continue;
                }

                let cell = indexed.cell;
                let flags = cell.flags;

                if flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }

                let wide = flags.contains(Flags::WIDE_CHAR);
                let col_width = if wide { 2 } else { 1 };
                let (mut fg, bg_opt) =
                    palette.resolve_cell_colors(cell.fg, cell.bg, flags, term_colors);

                let mut bold = flags.contains(Flags::BOLD);
                let mut underline = flags.intersects(Flags::ALL_UNDERLINES);
                let italic = flags.contains(Flags::ITALIC);
                let strikethrough = flags.contains(Flags::STRIKEOUT);

                // Apply semantic/syntax coloring ONLY if cell uses unstyled default fg/bg and no INVERSE/HIDDEN/DIM
                if let Some(row_ov) = semantic_cell_overrides.get(r)
                    && let Some(Some((sem_fg, sem_bold, sem_ul))) = row_ov.get(c)
                    && (cell.fg == Color::Named(NamedColor::Foreground)
                        || (white_rows[r] && cell.fg == Color::Named(NamedColor::White)))
                    && bg_opt.is_none()
                    && !flags.intersects(Flags::INVERSE | Flags::HIDDEN | Flags::DIM)
                {
                    fg = *sem_fg;
                    if *sem_bold {
                        bold = true;
                    }
                    if *sem_ul {
                        underline = true;
                    }
                }

                if semantic_opts.semantic_highlighting
                    && cell.fg == Color::Named(NamedColor::Foreground)
                    && bg_opt.is_none()
                    && !flags.intersects(Flags::INVERSE | Flags::HIDDEN | Flags::DIM)
                    && let Some(link) = cell.hyperlink()
                {
                    if last_osc_style
                        .as_ref()
                        .is_none_or(|(last, _)| last != &link)
                    {
                        let valid = validate_openable_http_url(link.uri()).is_some();
                        last_osc_style = Some((link, valid));
                    }
                    if last_osc_style.as_ref().is_some_and(|(_, valid)| *valid) {
                        fg = palette.semantic_style(SemanticRole::OutputUrl).0;
                    }
                }
                let row_snap = &mut row_snapshots[r];

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
                let has_zerowidth = cell.zerowidth().is_some_and(|zw| !zw.is_empty());
                if ch == ' ' && !has_zerowidth && !underline && !strikethrough {
                    continue;
                }

                // Coalesce ONLY narrow 1-cell runs onto narrow 1-cell runs so wide CJK/Emoji
                // glyphs NEVER shift subsequent narrow characters in GPUI `shape_line`!
                if let Some(last_run) = row_snap.text_runs.last_mut()
                    && !last_run.wide
                    && !wide
                    && !has_zerowidth
                    && last_run.start_col + last_run.col_len == c
                    && last_run.fg == fg
                    && last_run.bold == bold
                    && last_run.italic == italic
                    && last_run.underline == underline
                    && last_run.strikethrough == strikethrough
                {
                    last_run.text.push(ch);
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
                        wide,
                    });
                }
            }
        }

        let sel_range = self
            .term
            .selection
            .as_ref()
            .and_then(|s| s.to_range(&self.term));

        let mut selection_spans = Vec::new();
        if let Some(sel) = sel_range {
            for r in 0..rows {
                let grid_line = Line(r as i32 - display_offset as i32);
                if grid_line < sel.start.line || grid_line > sel.end.line {
                    continue;
                }
                let start_col = if sel.is_block || grid_line == sel.start.line {
                    sel.start.column.0
                } else {
                    0
                };
                let end_col = if sel.is_block || grid_line == sel.end.line {
                    (sel.end.column.0 + 1).min(cols)
                } else {
                    cols
                };
                if start_col < end_col {
                    selection_spans.push((r, start_col, end_col, palette.selection_bg));
                }
            }
        }

        // Visible search highlight spans (distinguishing active jumped match vs passive matches)
        let mut search_spans = Vec::new();
        if let Some(search) = &self.row_search {
            for (r, map) in row_maps.iter().enumerate() {
                let grid_line = Line(r as i32 - display_offset as i32);
                for hit in search.find_iter(&map.text) {
                    if let Some((sc, ec)) = map.byte_range_to_cols(hit.start(), hit.end()) {
                        let is_active = sel_range.is_some_and(|sel| {
                            sel.start.line == grid_line && sel.start.column.0 == sc
                        });
                        let bg = if is_active {
                            palette.search_active_bg
                        } else {
                            palette.search_match_bg
                        };
                        search_spans.push((r, sc, ec.min(cols), bg));
                    }
                }
            }
        }

        // Visible Command Blocks (OSC 133 + Prompt fallback)
        let command_blocks = if !is_tui && semantic_opts.command_blocks {
            self.compute_visible_command_blocks(&row_maps, display_offset, history_size)
        } else {
            Vec::new()
        };

        // Cached Overview Markers + Full-History Search Match Count
        let (overview_markers, total_search_matches) = if !is_tui
            && (semantic_opts.overview_markers || !self.search_query.is_empty())
        {
            self.compute_or_get_overview(history_size, semantic_opts.overview_markers)
        } else {
            (Vec::new(), 0)
        };

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
            block_divider: palette.block_divider,
            lines: row_snapshots,
            selection_spans,
            search_spans,
            command_blocks,
            overview_markers,
            cursor,
            display_offset,
            history_size,
            mode,
            search_match_count: total_search_matches,
        }
    }

    fn valid_osc_prompt(&self, record: &Osc133Record) -> bool {
        let grid = self.term.grid();
        if record.abs_start_line >= grid.history_size() + self.rows {
            return false;
        }
        let gl = record.abs_start_line as i32 - grid.history_size() as i32;
        if gl > grid.topmost_line().0 && self.row_has_wrapline(Line(gl - 1)) {
            return false;
        }
        let line = self.stitch_logical_line_at(gl, grid.bottommost_line().0, gl, &[]);
        matches!(
            detect_prompt_command_byte_offset(&line.text),
            Some((_, true))
        )
    }

    fn osc_record_has_syntax_target(&self, record: &Osc133Record) -> bool {
        let grid = self.term.grid();
        let gl = record.abs_start_line as i32 - grid.history_size() as i32;
        let line = self.stitch_logical_line_at(gl, grid.bottommost_line().0, gl, &[]);
        detect_prompt_command_byte_offset(&line.text)
            .filter(|&(_, has_cmd)| has_cmd)
            .and_then(|(off, _)| detect_command_syntax_target(&line.text[off..]))
            .is_some()
    }

    fn is_inside_osc133_output(&self, gl: i32, history_size: usize) -> bool {
        if self.osc133_records.is_empty() {
            return false;
        }
        let abs = (gl + history_size as i32).max(0) as usize;
        let cursor_abs = self.current_abs_cursor_line();
        self.osc133_records.iter().any(|rec| {
            if abs <= rec.abs_start_line || !self.valid_osc_prompt(rec) {
                return false;
            }
            if let Some(end_abs) = rec.abs_end_line {
                return abs <= end_abs;
            }
            abs <= cursor_abs && self.osc_record_has_syntax_target(rec)
        })
    }

    fn compute_visible_command_blocks(
        &self,
        row_maps: &[RowTextMap],
        display_offset: usize,
        history_size: usize,
    ) -> Vec<VisibleCommandBlock> {
        let rows = row_maps.len();
        if rows == 0 {
            return Vec::new();
        }

        // First prefer exact OSC 133 records if present
        if !self.osc133_records.is_empty() {
            let view_abs_top = history_size.saturating_sub(display_offset);
            let view_abs_bottom = view_abs_top + rows.saturating_sub(1);
            let cursor_abs =
                (history_size as i32 + self.term.grid().cursor.point.line.0).max(0) as usize;
            let mut out = Vec::new();
            let mut starts_above_viewport = false;
            for rec in self
                .osc133_records
                .iter()
                .filter(|r| self.valid_osc_prompt(r))
            {
                let end_abs = rec.abs_end_line.unwrap_or(cursor_abs);
                if end_abs < view_abs_top || rec.abs_start_line > view_abs_bottom {
                    continue;
                }
                let Some(start_row) = rec.abs_start_line.checked_sub(view_abs_top) else {
                    starts_above_viewport = true;
                    continue;
                };
                let end_row = (end_abs.saturating_sub(view_abs_top)).min(rows - 1);
                if start_row <= end_row {
                    out.push(VisibleCommandBlock {
                        start_row,
                        end_row,
                        show_top_divider: start_row > 0,
                        status: rec.status,
                    });
                }
            }
            if !out.is_empty() || starts_above_viewport {
                return out;
            }
        }

        // Universal prompt-heuristic command blocks (works on any unconfigured remote SSH server!)
        let view_top = -(display_offset as i32);
        let min_gl = -(history_size as i32);
        let max_gl = (self.rows as i32) - 1;
        let mut prompts: Vec<(usize, bool)> = Vec::new();
        for (r, map) in row_maps.iter().enumerate() {
            let gl = view_top + r as i32;
            if gl > min_gl && self.row_has_wrapline(Line(gl - 1)) {
                continue;
            }
            let stitched;
            let text = if gl < max_gl && self.row_has_wrapline(Line(gl)) {
                stitched = self.stitch_logical_line_at(gl, max_gl, view_top, row_maps);
                stitched.text.as_str()
            } else {
                map.text.as_str()
            };
            if let Some((_, has_cmd)) = detect_prompt_command_byte_offset(text) {
                prompts.push((r, has_cmd));
            }
        }

        let mut blocks = Vec::new();
        let cursor_vp_row = (display_offset == 0)
            .then(|| self.term.grid().cursor.point.line.0.max(0) as usize);

        for (idx, &(p_row, has_cmd)) in prompts.iter().enumerate() {
            if !has_cmd {
                continue;
            }
            let next_prompt_row = prompts.get(idx + 1).map(|(r, _)| *r);
            let end_row = match next_prompt_row {
                Some(nr) => nr.saturating_sub(1).max(p_row),
                None => cursor_vp_row.unwrap_or(rows - 1).min(rows - 1).max(p_row),
            };
            let has_err = ((p_row + 1)..=end_row)
                .any(|r| line_has_error_signature(&row_maps[r].text));
            let status = if has_err {
                CommandBlockStatus::Failure(1)
            } else if next_prompt_row.is_none() && display_offset == 0 {
                CommandBlockStatus::Running
            } else {
                CommandBlockStatus::Success
            };
            blocks.push(VisibleCommandBlock {
                start_row: p_row,
                end_row,
                show_top_divider: p_row > 0,
                status,
            });
        }
        blocks
    }

    fn compute_or_get_overview(
        &mut self,
        history_size: usize,
        include_command_markers: bool,
    ) -> (Vec<OverviewMarker>, usize) {
        if let Some((cached_gen, ref q, ref markers, count)) = self.cached_overview
            && cached_gen == self.dirty_generation
            && q == &self.search_query
        {
            let filtered = if include_command_markers {
                markers.clone()
            } else {
                markers
                    .iter()
                    .copied()
                    .filter(|m| m.kind == OverviewMarkerKind::SearchMatch)
                    .collect()
            };
            return (filtered, count);
        }

        let total_lines = (history_size + self.rows).max(1);
        let mut markers = Vec::new();
        let mut match_count = 0usize;

        // Add exact OSC 133 command markers if available
        let has_osc133 = self.osc133_records.iter().any(|r| self.valid_osc_prompt(r));
        if has_osc133 {
            for rec in self
                .osc133_records
                .iter()
                .filter(|r| self.valid_osc_prompt(r))
            {
                let ratio = (rec.abs_start_line as f32 / total_lines as f32).clamp(0.0, 1.0);
                let kind = match rec.status {
                    CommandBlockStatus::Running => OverviewMarkerKind::CommandRunning,
                    CommandBlockStatus::Success => OverviewMarkerKind::CommandSuccess,
                    CommandBlockStatus::Failure(_) => OverviewMarkerKind::CommandFailure,
                };
                markers.push(OverviewMarker {
                    ratio_from_top: ratio,
                    kind,
                });
            }
        }

        // Scan buffer lines for search matches and fallback prompt/error overview ticks
        if !has_osc133 || self.row_search.is_some() {
            let start_gl = -(history_size as i32);
            let end_gl = self.rows as i32;
            for gl in start_gl..end_gl {
                let line_str = self.extract_plain_grid_line(Line(gl));
                if line_str.trim().is_empty() {
                    continue;
                }
                let abs_idx = (gl + history_size as i32).max(0) as usize;
                let ratio = (abs_idx as f32 / total_lines as f32).clamp(0.0, 1.0);

                if let Some(search) = &self.row_search {
                    let c = search.find_iter(&line_str).count();
                    if c > 0 {
                        match_count += c;
                        markers.push(OverviewMarker {
                            ratio_from_top: ratio,
                            kind: OverviewMarkerKind::SearchMatch,
                        });
                    }
                }

                if !has_osc133 && (gl == start_gl || !self.row_has_wrapline(Line(gl - 1))) {
                    let stitched;
                    let check_str = if gl + 1 < end_gl && self.row_has_wrapline(Line(gl)) {
                        stitched = self.stitch_logical_line_at(gl, end_gl - 1, 0, &[]);
                        stitched.text.as_str()
                    } else {
                        line_str.as_str()
                    };
                    if line_has_error_signature(check_str) {
                        markers.push(OverviewMarker {
                            ratio_from_top: ratio,
                            kind: OverviewMarkerKind::CommandFailure,
                        });
                    } else if let Some((_, true)) = detect_prompt_command_byte_offset(check_str) {
                        markers.push(OverviewMarker {
                            ratio_from_top: ratio,
                            kind: OverviewMarkerKind::CommandSuccess,
                        });
                    }
                }
            }
        }

        self.cached_overview = Some((
            self.dirty_generation,
            self.search_query.clone(),
            markers.clone(),
            match_count,
        ));

        if !include_command_markers {
            markers.retain(|m| m.kind == OverviewMarkerKind::SearchMatch);
        }
        (markers, match_count)
    }

    fn row_has_wrapline(&self, grid_line: Line) -> bool {
        if self.cols == 0 {
            return false;
        }
        self.term.grid()[grid_line][Column(self.cols - 1)]
            .flags
            .contains(Flags::WRAPLINE)
    }

    fn stitch_logical_line_at(
        &self,
        start_gl: i32,
        max_gl: i32,
        view_top: i32,
        row_maps: &[RowTextMap],
    ) -> StitchedLogicalLine {
        let mut segments = Vec::with_capacity(2);
        let mut raw_parts: Vec<(i32, String)> = Vec::with_capacity(2);
        let mut gl = start_gl;

        loop {
            let vp_idx = gl - view_top;
            let row_str = if vp_idx >= 0 && (vp_idx as usize) < row_maps.len() {
                row_maps[vp_idx as usize].text.clone()
            } else {
                self.extract_plain_grid_line(Line(gl))
            };
            let wraps = gl < max_gl && self.row_has_wrapline(Line(gl));
            raw_parts.push((gl, row_str));
            if !wraps {
                break;
            }
            gl += 1;
        }

        let end_gl = gl;
        let last_part_idx = raw_parts.len() - 1;
        let mut text = String::new();

        for (idx, (part_gl, part_str)) in raw_parts.into_iter().enumerate() {
            // Preserve trailing spaces before soft-wrap points; trim right grid padding only on final segment
            let slice = if idx == last_part_idx {
                part_str.trim_end_matches(' ')
            } else {
                part_str.as_str()
            };
            let b_start = text.len();
            text.push_str(slice);
            let b_end = text.len();
            segments.push((part_gl, b_start, b_end));
        }

        StitchedLogicalLine {
            start_gl,
            end_gl,
            segments,
            text,
        }
    }

    fn find_enclosing_prompt_start(
        &mut self,
        view_top: i32,
        min_history_gl: i32,
        max_grid_gl: i32,
        row_maps: &[RowTextMap],
    ) -> i32 {
        let top_abs = (view_top - min_history_gl) as usize;
        if self.prompt_scan.checked.is_empty() || top_abs < self.prompt_scan.checked.start {
            let from = self
                .osc133_records
                .iter()
                .rev()
                .find(|r| r.abs_start_line <= top_abs && self.valid_osc_prompt(r))
                .map_or(0, |r| r.abs_start_line);
            if self.prompt_scan.checked.is_empty() {
                self.prompt_scan.checked = from..from;
            } else {
                let mut earlier = self
                    .scan_prompt_anchors(
                        from..self.prompt_scan.checked.start,
                        min_history_gl,
                        max_grid_gl,
                        view_top,
                        row_maps,
                    )
                    .0;
                earlier.append(&mut self.prompt_scan.anchors);
                self.prompt_scan.anchors = earlier;
                self.prompt_scan.checked.start = from;
            }
        }
        if self.prompt_scan.checked.end <= top_abs {
            let (anchors, end) = self.scan_prompt_anchors(
                self.prompt_scan.checked.end..top_abs + 1,
                min_history_gl,
                max_grid_gl,
                view_top,
                row_maps,
            );
            self.prompt_scan.anchors.extend(anchors);
            self.prompt_scan.checked.end = end;
        }
        let end = self.prompt_scan.anchors.partition_point(|&a| a <= top_abs);
        let history_size = (-min_history_gl).max(0) as usize;
        self.prompt_scan.anchors[..end]
            .iter()
            .rev()
            .find(|&&a| !self.is_inside_osc133_output(min_history_gl + a as i32, history_size))
            .map_or(view_top, |a| min_history_gl + *a as i32)
    }

    fn scan_prompt_anchors(
        &mut self,
        range: Range<usize>,
        min_gl: i32,
        max_gl: i32,
        view_top: i32,
        maps: &[RowTextMap],
    ) -> (Vec<usize>, usize) {
        let mut anchors = Vec::new();
        let mut abs = range.start;
        while abs < range.end {
            #[cfg(test)]
            {
                self.prompt_scan.scanned_lines += 1;
            }
            let line = self.stitch_logical_line_at(min_gl + abs as i32, max_gl, view_top, maps);
            if detect_prompt_command_byte_offset(&line.text).is_some() {
                anchors.push(abs);
            }
            abs = (line.end_gl - min_gl + 1) as usize;
        }
        (anchors, abs)
    }

    fn populate_code_syntax_overrides(
        &mut self,
        row_maps: &[RowTextMap],
        display_offset: usize,
        history_size: usize,
        palette: &TerminalPalette,
        overrides: &mut [Vec<Option<(Hsla, bool, bool)>>],
        white_rows: &mut [bool],
    ) {
        if self.rows == 0 || self.cols == 0 || row_maps.is_empty() {
            return;
        }

        let min_history_gl = -(history_size as i32);
        let max_screen_gl = (self.rows as i32) - 1;
        let cursor_gl = self.term.grid().cursor.point.line.0;
        let view_top = -(display_offset as i32);
        let view_bottom = view_top + (self.rows as i32) - 1;

        // The cursor can move back over existing output. Ignore only the blank padding below it.
        let content_end = (cursor_gl + 1..=max_screen_gl).rev()
            .find(|&gl| self.term.grid()[Line(gl)].line_length().0 > 0)
            .unwrap_or(cursor_gl);
        let scan_end_gl = content_end.min(view_bottom);

        let scan_start_gl =
            self.find_enclosing_prompt_start(view_top, min_history_gl, max_screen_gl, row_maps);
        let pack = SyntaxRegistry::global().current();

        if let Some(cache) = self.block_syntax_cache.as_ref()
            && cache.dirty_generation == self.dirty_generation
            && cache.syntax_revision == pack.revision
            && cache.cols == self.cols
            && cache.prompt_gl == scan_start_gl
            && let Some(cover) = cache.cover.as_ref()
            && scan_end_gl <= cover.covered_scan_end_gl
        {
            if cover.conpty_white
                && cover.out_end_gl >= view_top
                && cover.out_start_gl <= view_bottom
            {
                white_rows[(cover.out_start_gl.max(view_top) - view_top) as usize
                    ..=(cover.out_end_gl.min(view_bottom) - view_top) as usize]
                    .fill(true);
            }
            let first_vis = cache
                .completed_spans
                .partition_point(|entry| entry.end_gl < view_top);
            for entry in &cache.completed_spans[first_vis..] {
                if entry
                    .segments
                    .first()
                    .is_some_and(|&(s_gl, _, _)| s_gl > view_bottom)
                {
                    break;
                }
                project_segments_spans_to_viewport(
                    &entry.segments,
                    &entry.spans,
                    view_top,
                    view_bottom,
                    self.cols,
                    row_maps,
                    palette,
                    overrides,
                );
            }
            if let Some((t_start, t_end, ref segs, ref spans)) = cover.tail_spans
                && t_end >= view_top
                && t_start <= view_bottom
            {
                project_segments_spans_to_viewport(
                    segs,
                    spans,
                    view_top,
                    view_bottom,
                    self.cols,
                    row_maps,
                    palette,
                    overrides,
                );
            }
            return;
        }

        let mut logical_lines: Vec<StitchedLogicalLine> = Vec::new();
        let mut gl = scan_start_gl;
        while gl <= scan_end_gl {
            let line = self.stitch_logical_line_at(gl, max_screen_gl, view_top, row_maps);
            gl = line.end_gl + 1;
            if line.start_gl == min_history_gl || !self.row_has_wrapline(Line(line.start_gl - 1)) {
                logical_lines.push(line);
            }
        }

        let cmd_block_count = logical_lines
            .iter()
            .filter(|l| {
                !self.is_inside_osc133_output(l.start_gl, history_size)
                    && matches!(detect_prompt_command_byte_offset(&l.text), Some((_, true)))
            })
            .count();

        let mut idx = 0usize;
        while idx < logical_lines.len() {
            if self.is_inside_osc133_output(logical_lines[idx].start_gl, history_size) {
                idx += 1;
                continue;
            }
            let Some((cmd_byte, has_cmd)) =
                detect_prompt_command_byte_offset(&logical_lines[idx].text)
            else {
                idx += 1;
                continue;
            };

            let prompt_idx = idx;
            idx += 1;
            let out_start = idx;
            while idx < logical_lines.len()
                && (self.is_inside_osc133_output(logical_lines[idx].start_gl, history_size)
                    || detect_prompt_command_byte_offset(&logical_lines[idx].text).is_none())
            {
                idx += 1;
            }
            let out_end = idx;

            if !has_cmd || out_start >= out_end {
                continue;
            }

            let out_slice = &logical_lines[out_start..out_end];
            // Skip blocks that are completely above the visible viewport
            if out_slice.last().is_some_and(|l| l.end_gl < view_top) {
                continue;
            }

            let prompt_cmd = logical_lines[prompt_idx].text[cmd_byte..].trim();
            let Some(target) = detect_command_syntax_target(prompt_cmd) else {
                continue;
            };

            let start = out_slice[0].start_gl;
            let end = out_slice.last().unwrap().end_gl;
            let conpty_white = self.is_windows_local_pty
                && target.allow_conpty_white
                && self.monochrome_output(start, end, scan_end_gl, content_end);
            if conpty_white {
                white_rows[(start.max(view_top) - view_top) as usize
                    ..=(end.min(view_bottom) - view_top) as usize]
                    .fill(true);
            }
            let last_is_incomplete = out_end == logical_lines.len()
                && out_slice.last().is_some_and(|last| {
                    last.end_gl == content_end && (last.start_gl..=last.end_gl).contains(&cursor_gl)
                });
            let sole_cover_end = (cmd_block_count == 1
                && logical_lines[prompt_idx].start_gl == scan_start_gl)
                .then_some((scan_end_gl, start, end, conpty_white));
            self.highlight_single_output_block(
                logical_lines[prompt_idx].start_gl,
                prompt_cmd,
                &target,
                out_slice,
                last_is_incomplete,
                out_end + 1 >= logical_lines.len(),
                sole_cover_end,
                view_top,
                view_bottom,
                row_maps,
                &pack,
                palette,
                overrides,
            );
        }
    }

    fn monochrome_output(&self, start: i32, end: i32, scan_end: i32, content_end: i32) -> bool {
        let plain_row = |gl| {
            self.term.grid()[Line(gl)][..].iter().all(|c| {
                matches!(
                    c.fg,
                    Color::Named(NamedColor::Foreground | NamedColor::White)
                ) && c.bg == Color::Named(NamedColor::Background)
                    && (c.flags
                        - (Flags::WRAPLINE
                            | Flags::WIDE_CHAR
                            | Flags::WIDE_CHAR_SPACER
                            | Flags::LEADING_WIDE_CHAR_SPACER))
                        .is_empty()
            })
        };
        if !(start..=end).all(plain_row) {
            return false;
        }
        // If the viewport cuts off this block, inspect its remaining output before allowing white.
        if end >= scan_end {
            let mut gl = end + 1;
            while gl <= content_end {
                let next = self.stitch_logical_line_at(gl, content_end, gl, &[]);
                if detect_prompt_command_byte_offset(&next.text).is_some() {
                    break;
                }
                if !(gl..=next.end_gl).all(plain_row) {
                    return false;
                }
                gl = next.end_gl + 1;
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn highlight_single_output_block(
        &mut self,
        prompt_gl: i32,
        prompt_cmd: &str,
        target: &CommandSyntaxTarget,
        out_lines: &[StitchedLogicalLine],
        last_is_incomplete: bool,
        keep_checkpoint: bool,
        sole_cover: Option<(i32, i32, i32, bool)>,
        view_top: i32,
        view_bottom: i32,
        row_maps: &[RowTextMap],
        pack: &SyntaxPack,
        palette: &TerminalPalette,
        overrides: &mut [Vec<Option<(Hsla, bool, bool)>>],
    ) {
        let first_line = out_lines.first().map(|line| line.text.as_str());
        let Some(syntax) = pack.find_syntax(target.hint.as_deref(), first_line) else {
            return;
        };

        // Determine how many logical lines in `out_lines` are finalized vs an active streaming tail
        let completed_count = if last_is_incomplete {
            out_lines.len().saturating_sub(1)
        } else {
            out_lines.len()
        }
        .min(MAX_SYNTAX_BLOCK_LINES);

        // Check if our single-block streaming checkpoint cache can be reused
        // Earlier visible blocks should not evict the streaming block's checkpoint.
        let mut scratch = None;
        let slot = if keep_checkpoint { &mut self.block_syntax_cache } else { &mut scratch };
        let can_reuse = slot.as_ref().is_some_and(|c| {
            let same_grid = c.dirty_generation == self.dirty_generation && c.prompt_gl == prompt_gl;
            c.syntax_revision == pack.revision
                && c.cols == self.cols
                && c.prompt_cmd == prompt_cmd
                && c.syntax_name == syntax.name
                && (same_grid
                    || (c.completed_spans.len() <= completed_count
                        && c.completed_spans
                            .iter()
                            .zip(out_lines)
                            .enumerate()
                            .all(|(i, (entry, line))| entry.hash == hash_line_sample(i, &line.text))))
        });

        if !can_reuse {
            *slot = None;
        }
        let cache = slot.get_or_insert_with(|| BlockSyntaxCache {
            dirty_generation: self.dirty_generation,
            syntax_revision: pack.revision,
            cols: self.cols,
            prompt_gl,
            prompt_cmd: prompt_cmd.to_string(),
            syntax_name: syntax.name.clone(),
            completed_spans: Vec::with_capacity(completed_count.min(256)),
            checkpoint: (ParseState::new(syntax), ScopeStack::new()),
            cover: None,
        });
        cache.dirty_generation = self.dirty_generation;
        cache.prompt_gl = prompt_gl;
        cache.cover = None;

        let mut scope_cache = HashMap::new();

        let mut line_buf = String::new();
        for line_idx in cache.completed_spans.len()..completed_count {
            let line = &out_lines[line_idx];
            if line.text.len() > MAX_SYNTAX_LINE_BYTES {
                break;
            }
            line_buf.clear();
            line_buf.push_str(&line.text);
            line_buf.push('\n');
            let (parser, scopes) = &mut cache.checkpoint;
            let spans = pack.parse_line_spans(parser, scopes, &mut scope_cache, &line_buf);
            cache.completed_spans.push(CachedLogicalLineSpans {
                hash: hash_line_sample(line_idx, &line.text),
                end_gl: line.end_gl,
                segments: line.segments.clone(),
                spans,
            });
        }
        for (entry, line) in cache.completed_spans.iter_mut().zip(out_lines) {
            entry.end_gl = line.end_gl;
            entry.segments.clone_from(&line.segments);
        }

        // Speculatively parse the in-progress streaming tail line without mutating checkpoint
        let mut cached_tail = None;
        if last_is_incomplete
            && cache.completed_spans.len() == completed_count
            && completed_count < MAX_SYNTAX_BLOCK_LINES
            && let Some(tail_line) = out_lines.get(completed_count)
            && tail_line.text.len() <= MAX_SYNTAX_LINE_BYTES
        {
            let (mut tmp_parser, mut tmp_scopes) = cache.checkpoint.clone();
            let spans = pack.parse_line_spans(
                &mut tmp_parser,
                &mut tmp_scopes,
                &mut scope_cache,
                &tail_line.text,
            );
            project_stitched_spans_to_viewport(
                tail_line, &spans, view_top, view_bottom, self.cols, row_maps, palette, overrides,
            );
            cached_tail = Some((
                tail_line.start_gl,
                tail_line.end_gl,
                tail_line.segments.clone(),
                spans,
            ));
        }

        if let Some((covered_scan_end_gl, out_start_gl, out_end_gl, conpty_white)) = sole_cover {
            cache.cover = Some(CachedBlockCover {
                covered_scan_end_gl,
                out_start_gl,
                out_end_gl,
                conpty_white,
                tail_spans: cached_tail,
            });
        }

        // Project visible logical lines' byte spans back onto viewport physical columns
        for (stitched, entry) in out_lines
            .iter()
            .take(completed_count)
            .zip(&cache.completed_spans)
        {
            project_stitched_spans_to_viewport(
                stitched,
                &entry.spans,
                view_top,
                view_bottom,
                self.cols,
                row_maps,
                palette,
                overrides,
            );
        }
    }

    fn row_text_cells(&self, grid_line: Line) -> impl Iterator<Item = (usize, usize, char)> + '_ {
        self.term.grid()[grid_line][..]
            .iter()
            .enumerate()
            .filter(|(_, cell)| {
                !cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            })
            .flat_map(|(col, cell)| {
                let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                    2
                } else {
                    1
                };
                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                std::iter::once(ch)
                    .chain(cell.zerowidth().into_iter().flatten().copied())
                    .map(move |ch| (col, width, ch))
            })
    }

    fn build_row_text_map(&self, grid_line: Line) -> RowTextMap {
        RowTextMap::from_cells(self.row_text_cells(grid_line))
    }

    fn extract_plain_grid_line(&self, grid_line: Line) -> String {
        let mut text = String::with_capacity(self.cols);
        text.extend(self.row_text_cells(grid_line).map(|(_, _, ch)| ch));
        text
    }

    fn current_abs_cursor_line(&self) -> usize {
        let history = self.term.grid().history_size() as i32;
        let cur = self.term.grid().cursor.point.line.0;
        (history + cur).max(0) as usize
    }

    fn scan_osc_sequences(&mut self, bytes: &[u8]) -> Option<String> {
        let chunk = String::from_utf8_lossy(bytes);
        self.osc_tail.push_str(&chunk);
        if self.osc_tail.len() > 8192 {
            let drain_to = self.osc_tail.len() - 4096;
            let boundary = self.osc_tail.ceil_char_boundary(drain_to);
            self.osc_tail.drain(..boundary);
        }

        let mut latest_dir = None;
        while let Some(start) = self.osc_tail.find("\x1b]") {
            let after = &self.osc_tail[start + 2..];
            let end_rel = after
                .find('\x07')
                .map(|i| (i, 1))
                .into_iter()
                .chain(after.find("\x1b\\").map(|i| (i, 2)))
                .min();
            let Some((end_idx, term_len)) = end_rel else {
                if start > 0 {
                    self.osc_tail.drain(..start);
                }
                break;
            };

            let payload = after[..end_idx].to_string();
            let consume = start + 2 + end_idx + term_len;
            self.osc_tail.drain(..consume);

            if let Some(uri) = payload.strip_prefix("7;") {
                if let Some(parsed) = parse_osc7_file_uri(uri) {
                    latest_dir = Some(parsed);
                }
            } else if let Some(dir) = payload.strip_prefix("1337;CurrentDir=") {
                if !dir.trim().is_empty() {
                    latest_dir = Some(dir.trim().to_string());
                }
            } else if let Some(osc133) = payload.strip_prefix("133;") {
                self.handle_osc133_marker(osc133);
            }
        }
        if !self.osc_tail.contains("\x1b]") {
            let keep = usize::from(self.osc_tail.ends_with('\x1b'));
            self.osc_tail.drain(..self.osc_tail.len() - keep);
        }
        latest_dir
    }

    fn handle_osc133_marker(&mut self, marker: &str) {
        let abs_line = self.current_abs_cursor_line();
        let tag = marker.split(';').next().unwrap_or("");
        match tag {
            "A" | "C" => {
                if tag == "C"
                    && let Some(last) = self.osc133_records.back_mut()
                    && last.abs_end_line.is_none()
                {
                    last.status = CommandBlockStatus::Running;
                    return;
                }
                if let Some(prev) = self.osc133_records.back_mut()
                    && prev.abs_end_line.is_none()
                {
                    prev.abs_end_line = Some(abs_line.saturating_sub(1).max(prev.abs_start_line));
                    if matches!(prev.status, CommandBlockStatus::Running) {
                        prev.status = CommandBlockStatus::Success;
                    }
                }
                if self.osc133_records.len() >= MAX_OSC133_RECORDS {
                    self.osc133_records.pop_front();
                }
                self.osc133_records.push_back(Osc133Record {
                    abs_start_line: abs_line,
                    abs_end_line: None,
                    status: CommandBlockStatus::Running,
                });
            }
            "D" => {
                let code = marker
                    .split(';')
                    .nth(1)
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                if let Some(last) = self.osc133_records.back_mut() {
                    last.abs_end_line = Some(abs_line.saturating_sub(1).max(last.abs_start_line));
                    last.status = if code == 0 {
                        CommandBlockStatus::Success
                    } else {
                        CommandBlockStatus::Failure(code)
                    };
                }
            }
            _ => {}
        }
    }
}

fn hash_line_sample(index: usize, text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    index.hash(&mut hasher);
    text.hash(&mut hasher);
    hasher.finish()
}

#[allow(clippy::too_many_arguments)]
fn project_stitched_spans_to_viewport(
    stitched: &StitchedLogicalLine,
    spans: &[(Range<usize>, CodeTokenRole)],
    view_top: i32,
    view_bottom: i32,
    cols: usize,
    row_maps: &[RowTextMap],
    palette: &TerminalPalette,
    overrides: &mut [Vec<Option<(Hsla, bool, bool)>>],
) {
    if spans.is_empty() || stitched.end_gl < view_top || stitched.start_gl > view_bottom {
        return;
    }
    project_segments_spans_to_viewport(
        &stitched.segments,
        spans,
        view_top,
        view_bottom,
        cols,
        row_maps,
        palette,
        overrides,
    );
}

#[allow(clippy::too_many_arguments)]
fn project_segments_spans_to_viewport(
    segments: &[(i32, usize, usize)],
    spans: &[(Range<usize>, CodeTokenRole)],
    view_top: i32,
    view_bottom: i32,
    cols: usize,
    row_maps: &[RowTextMap],
    palette: &TerminalPalette,
    overrides: &mut [Vec<Option<(Hsla, bool, bool)>>],
) {
    if spans.is_empty() {
        return;
    }

    for &(seg_gl, seg_b_start, seg_b_end) in segments {
        if seg_gl < view_top || seg_gl > view_bottom || seg_b_start >= seg_b_end {
            continue;
        }
        let vp_row = (seg_gl - view_top) as usize;
        let Some(map) = row_maps.get(vp_row) else {
            continue;
        };
        let Some(row_ov) = overrides.get_mut(vp_row) else {
            continue;
        };

        let first = spans.partition_point(|(r, _)| r.end <= seg_b_start);
        for (range, role) in &spans[first..] {
            if range.start >= seg_b_end {
                break;
            }
            let local_start = range.start.max(seg_b_start) - seg_b_start;
            let local_end = range.end.min(seg_b_end) - seg_b_start;
            if let Some((sc, ec)) = map.byte_range_to_cols(local_start, local_end) {
                let style = palette.code_token_style(*role);
                let sc = sc.min(cols);
                let ec = ec.min(cols);
                for slot in &mut row_ov[sc..ec] {
                    *slot = Some(style);
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_cwd_uses_the_live_cursor_after_osc_and_title() {
        let mut engine = AlacrittyTerminalEngine::new(4, 60, 100);
        let mut cwd = |bytes: &[u8]| {
            engine
                .feed_bytes(bytes)
                .into_iter()
                .find_map(|effect| match effect {
                    EngineSideEffect::CurrentDirChanged(path) => Some(path),
                    _ => None,
                })
        };
        assert_eq!(cwd(b"root@box:/etc# ").as_deref(), Some("/etc"));
        assert_eq!(cwd(b"ls"), None);
        assert_eq!(cwd(b"\r\nroot@box:/old# \r\noutput"), None);
        assert_eq!(
            cwd(b"\r\nroot@box:/new# \x1b]2;user@box:/title\x07").as_deref(),
            Some("/title")
        );
        assert_eq!(
            cwd(b"\x1b]7;file://box/osc\x07\x1b]2;user@box:/title\x07").as_deref(),
            Some("/osc")
        );
        assert_eq!(
            cwd(b"\x1b]1337;CurrentDir=/explicit\x07").as_deref(),
            Some("/explicit")
        );
        assert_eq!(cwd(b"\x1b[?1049h\x1b[Hroot@box:/tui# "), None);
        assert_eq!(
            cwd(b"\x1b[?1049l\r\nroot@box:/live# ").as_deref(),
            Some("/live")
        );
        assert_eq!(cwd(b" "), None);

        let mut narrow = AlacrittyTerminalEngine::new(5, 12, 100);
        let effects = narrow.feed_bytes(b"root@box:/long/directory# ");
        assert!(effects.iter().any(
            |e| matches!(e, EngineSideEffect::CurrentDirChanged(dir) if dir == "/long/directory")
        ));
        let effects = narrow.feed_bytes(b"\r\nprinted textroot@box:/fake# ");
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, EngineSideEffect::CurrentDirChanged(_)))
        );
    }

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

        let snap = engine.snapshot(
            &TerminalPalette::dark(),
            true,
            SemanticOptions {
                semantic_highlighting: false,
                ..Default::default()
            },
        );
        assert_eq!(snap.lines[0].text_runs.len(), 2);
        assert_eq!(snap.lines[0].text_runs[0].start_col, 0);
        assert_eq!(snap.lines[0].text_runs[0].text, "READY");
        assert!(snap.lines[0].text_runs[0].bold);
        assert_eq!(snap.lines[0].text_runs[1].start_col, 6);
        assert_eq!(snap.lines[0].text_runs[1].text, "world");
    }

    #[test]
    fn mixed_cjk_and_ascii_runs_never_overlap_columns() {
        let mut engine = AlacrittyTerminalEngine::new(24, 80, 1000);
        let _ = engine.feed_bytes("中文: ok\r\n".as_bytes());
        let snap = engine.snapshot(
            &TerminalPalette::dark(),
            true,
            SemanticOptions {
                semantic_highlighting: false,
                ..Default::default()
            },
        );
        let runs = &snap.lines[0].text_runs;
        // '中' (0..2, wide), '文' (2..4, wide), ':' (4..5, narrow), "ok" (6..8, narrow)
        assert_eq!(runs.len(), 4);
        assert_eq!((runs[0].start_col, runs[0].col_len, runs[0].wide), (0, 2, true));
        assert_eq!(runs[0].text, "中");
        assert_eq!((runs[1].start_col, runs[1].col_len, runs[1].wide), (2, 2, true));
        assert_eq!(runs[1].text, "文");
        assert_eq!((runs[2].start_col, runs[2].col_len, runs[2].wide), (4, 1, false));
        assert_eq!(runs[2].text, ":");
        assert_eq!((runs[3].start_col, runs[3].col_len, runs[3].wide), (6, 2, false));
        assert_eq!(runs[3].text, "ok");
    }

    #[test]
    fn unicode_search_highlights_grid_columns_and_counts_scrollback() {
        let mut engine = AlacrittyTerminalEngine::new(2, 40, 100);
        engine.feed_bytes("中K.\r\nİ前中K.中k.中k!e\u{301}\r\n".as_bytes());
        let palette = TerminalPalette::dark();
        let opts = SemanticOptions {
            semantic_highlighting: false,
            code_highlighting: false,
            command_blocks: false,
            overview_markers: false,
        };

        engine.set_search_query("中K.");
        let snap = engine.snapshot(&palette, true, opts);
        assert_eq!(snap.search_match_count, 3);
        assert_eq!(
            snap.search_spans,
            [
                (0, 3, 7, palette.search_match_bg),
                (0, 7, 11, palette.search_match_bg),
            ]
        );

        engine.set_search_query("E\u{301}");
        let snap = engine.snapshot(&palette, true, opts);
        assert_eq!(snap.search_match_count, 1);
        assert_eq!(snap.search_spans, [(0, 15, 16, palette.search_match_bg)]);
    }

    #[test]
    fn case_insensitive_search_and_osc133_blocks_work() {
        let mut engine = AlacrittyTerminalEngine::new(24, 80, 1000);
        let _ = engine.feed_bytes(
            b"\x1b]133;A\x07user@dev:~$ cargo check\r\n\x1b]133;C\x07ERROR: failed to build\r\n\x1b]133;D;101\x07",
        );
        engine.set_search_query("error");
        assert!(engine.jump_search(true));

        let snap = engine.snapshot(&TerminalPalette::dark(), true, SemanticOptions::default());
        assert_eq!(snap.search_match_count, 1);
        assert_eq!(snap.command_blocks.len(), 1);
        assert_eq!(
            snap.command_blocks[0].status,
            CommandBlockStatus::Failure(101)
        );

        let disabled = SemanticOptions {
            command_blocks: false,
            ..Default::default()
        };
        assert!(
            engine
                .snapshot(&TerminalPalette::dark(), true, disabled)
                .command_blocks
                .is_empty()
        );
    }

    #[test]
    fn osc133_marks_only_the_actual_start_row() {
        let mut engine = AlacrittyTerminalEngine::new(3, 40, 100);
        let options = SemanticOptions {
            command_blocks: true,
            ..Default::default()
        };
        engine.feed_bytes(b"\x1b]133;A\x07");
        engine.feed_bytes(b"$ cat script\r\n# comment\r\n$ echoed prompt\r\noutput\r\n");
        let palette = TerminalPalette::dark();
        assert!(
            engine.snapshot(&palette, true, options).command_blocks.is_empty()
        );
        engine.scroll(Scroll::Top);
        let snap = engine.snapshot(&palette, true, options);
        assert_eq!(snap.command_blocks.len(), 1);
        assert_eq!(snap.command_blocks[0].start_row, 0);
    }

    #[test]
    fn highlights_cat_shell_script_with_cjk_urls_and_ansi_priority() {
        let mut engine = AlacrittyTerminalEngine::new(12, 80, 200);
        let palette = TerminalPalette::dark();
        let script = concat!(
            "root@clean-host:~# cat tcpx.sh\r\n",
            "#!/bin/bash\r\n",
            "# 常规交互式启动 https://example.com\r\n",
            "if [ -f /etc/hosts ]; then\r\n",
            "  echo \"\x1b[31mANSI_RED\x1b[0m plain_str\"\r\n",
            "fi\r\n",
            "root@clean-host:~# "
        );
        engine.feed_bytes(script.as_bytes());

        let snap = engine.snapshot(&palette, true, SemanticOptions::default());
        let comment_fg = palette.code_token_style(CodeTokenRole::Comment).0;
        let keyword_fg = palette.code_token_style(CodeTokenRole::Keyword).0;
        let string_fg = palette.code_token_style(CodeTokenRole::String).0;
        let url_fg = palette.semantic_style(crate::terminal::SemanticRole::OutputUrl).0;
        let ansi_red_fg = rgb_to_hsla(palette.ansi[1], 1.0);

        // Row 2: `# 常规交互式启动 https://example.com` -> comment colored + CJK wide aligned + URL color on top
        let row2 = &snap.lines[2].text_runs;
        assert!(row2.iter().any(|r| r.text == "常" && r.wide && r.fg == comment_fg));
        assert!(
            row2.iter()
                .any(|r| r.text.contains("https://example.com") && r.fg == url_fg && !r.underline)
        );
        assert_eq!(
            engine.link_at(2, 22).map(|m| m.url).as_deref(),
            Some("https://example.com")
        );

        // Row 3: `if` / `then` keywords colored
        let row3 = &snap.lines[3].text_runs;
        assert!(row3.iter().any(|r| r.text.trim() == "if" && r.fg == keyword_fg));
        assert!(row3.iter().any(|r| r.text.contains("then") && r.fg == keyword_fg));

        // Row 4: explicit `\x1b[31mANSI_RED` keeps native ANSI red while unstyled `plain_str"` gets string color
        let row4 = &snap.lines[4].text_runs;
        assert!(row4.iter().any(|r| r.text.contains("ANSI_RED") && r.fg == ansi_red_fg));
        assert!(row4.iter().any(|r| r.text.contains("plain_str") && r.fg == string_fg));

        // Disabling `code_highlighting` leaves URL semantic highlight active while restoring plain foreground
        let code_off = engine.snapshot(
            &palette,
            true,
            SemanticOptions {
                code_highlighting: false,
                ..Default::default()
            },
        );
        let default_fg = rgb_to_hsla(palette.foreground, 1.0);
        assert!(
            code_off.lines[2]
                .text_runs
                .iter()
                .any(|r| r.text == "常" && r.fg == default_fg)
        );
        assert!(
            code_off.lines[2]
                .text_runs
                .iter()
                .any(|r| r.text.contains("https://example.com") && r.fg == url_fg)
        );
    }

    #[test]
    fn wrapped_links_hit_and_paint_the_full_visible_range() {
        let mut engine = AlacrittyTerminalEngine::new(3, 20, 100);
        let url = "https://git:3000/very-long-path/wiki/Example_(one)?a=1&b=2#top";
        engine.feed_bytes(format!("中 [{url}]").as_bytes());
        let cols = engine.cols;
        let palette = TerminalPalette::dark();
        let fg = palette.semantic_style(SemanticRole::OutputUrl).0;
        for offset in [Scroll::Top, Scroll::Bottom] {
            engine.scroll(offset);
            let snap = engine.snapshot(&palette, true, SemanticOptions::default());
            let top = snap.history_size - snap.display_offset;
            let spans: Vec<_> = (0..snap.rows)
                .filter_map(|r| {
                    let begin = (r + top) * cols;
                    let end = (4 + url.len()).saturating_sub(begin).min(cols);
                    let start = 4usize.saturating_sub(begin);
                    (start < end).then_some((r, start, end))
                })
                .collect();
            for &(r, start, end) in &spans {
                for col in start..end {
                    let link = engine.link_at(r, col).unwrap();
                    assert_eq!(link.url, url);
                    assert_eq!(link.viewport_spans, spans);
                    assert!(snap.lines[r].text_runs.iter().any(|run| {
                        (run.start_col..run.start_col + run.col_len).contains(&col)
                            && run.fg == fg
                            && !run.underline
                    }));
                }
            }
        }
    }

    #[test]
    fn osc8_respects_identity_wide_cells_native_style_and_target_scheme() {
        let mut engine = AlacrittyTerminalEngine::new(4, 24, 100);
        engine.feed_bytes(
            concat!(
                "\x1b]8;id=1;https://a.com\x07一",
                "\x1b]8;id=2;https://a.com\x1b\\二\x1b]8;;\x07 ",
                "\x1b[4;31m\x1b]8;;https://git:3000\x07git\x1b]8;;\x07\x1b[0m",
            )
            .as_bytes(),
        );
        let palette = TerminalPalette::dark();
        let snap = engine.snapshot(&palette, true, SemanticOptions::default());
        for (col, left) in [(1, 0), (3, 2)] {
            let m = engine.link_at(0, col).unwrap();
            assert_eq!(m.viewport_spans, [(0, left, left + 2)]);
            assert_eq!(m.osc8_host_hint.as_deref(), Some("a.com"));
            let run = snap.lines[0]
                .text_runs
                .iter()
                .find(|r| r.start_col == left)
                .unwrap();
            assert_eq!(run.fg, palette.semantic_style(SemanticRole::OutputUrl).0);
            assert!(!run.underline);
        }
        assert!(engine.link_at(0, 6).unwrap().osc8_host_hint.is_none());
        assert!(
            snap.lines[0].text_runs.iter().any(|r| r.text == "git"
                && r.underline
                && r.fg == rgb_to_hsla(palette.ansi[1], 1.))
        );
        for invalid in [
            "file:///etc/passwd",
            r"\\server\share",
            "javascript:alert(1)",
            "custom://open",
            "http://",
        ] {
            engine.feed_bytes(format!("\r\x1b]8;;{invalid}\x07https://git\x1b]8;;\x07").as_bytes());
            assert!(engine.link_at(0, 1).is_none(), "{invalid}");
        }
    }

    #[test]
    fn long_cat_keeps_its_prompt_and_full_log_repaints_skip_history() {
        let opts = SemanticOptions {
            overview_markers: false,
            ..Default::default()
        };
        let palette = TerminalPalette::dark();
        let mut engine = AlacrittyTerminalEngine::new(6, 32, 5000);
        engine.feed_bytes(b"$ cat large.rs\r\n");
        engine.feed_bytes("fn test() {}\r\n".repeat(1500).as_bytes());
        let snap = engine.snapshot(&palette, true, opts);
        assert!(
            snap.lines[0]
                .text_runs
                .iter()
                .any(|r| r.text == "fn" && r.fg != snap.foreground)
        );
        for n in [5_000, 30_000] {
            let mut log = AlacrittyTerminalEngine::new(4, 20, n);
            log.feed_bytes("line\r\n".repeat(n + 4).as_bytes());
            log.snapshot(&palette, false, opts);
            let scanned = log.prompt_scan.scanned_lines;
            assert!(scanned >= n);
            for scroll in [Scroll::Delta(2), Scroll::Bottom] {
                log.scroll(scroll);
                log.snapshot(&palette, false, opts);
                assert_eq!(log.prompt_scan.scanned_lines, scanned);
            }
            log.feed_bytes(b"next\r\n");
            log.snapshot(&palette, false, opts);
            assert!(log.prompt_scan.scanned_lines - scanned <= 2);
            log.resize(5, 24);
            assert!(log.prompt_scan.checked.is_empty());
        }
    }

    #[test]
    fn conpty_white_applies_only_to_monochrome_local_reads() {
        let palette = TerminalPalette::dark();
        let white = rgb_to_hsla(palette.ansi[7], 1.);
        for (local, cmd, suffix, allowed) in [
            (false, "type a.rs", "", false),
            (true, "type a.rs", "", true),
            (true, "bat a.rs", "", false),
            (true, "type a.rs", "\x1b[32mgreen", false),
            (true, "type a.rs", "\x1b[44mbackground", false),
            (true, "type a.rs", "\x1b[1mbold", false),
        ] {
            let mut engine = AlacrittyTerminalEngine::new(3, 40, 100);
            engine.is_windows_local_pty = local;
            engine.feed_bytes(
                format!("C:\\src>{cmd}\r\n\x1b[37mfn main() {{}}\r\n\r\n{suffix}").as_bytes(),
            );
            engine.scroll(Scroll::Top);
            let snap = engine.snapshot(&palette, false, SemanticOptions::default());
            let run = &snap.lines[1].text_runs[0];
            assert_eq!(run.fg != white, allowed, "{cmd} {local} {suffix:?}");
        }
    }

    #[test]
    fn mixed_osc_terminators_preserve_cwd_and_only_real_command_anchors() {
        let mut e = AlacrittyTerminalEngine::new(4, 40, 100);
        let effects = e.feed_bytes(b"\x1b]7;file://host/one\x1b\\\x1b]133;A\x07$ cat a.rs\r\n\x1b]133;C\x07fn main() {}\r\n\x1b]133;D;7\x1b\\");
        assert!(
            effects
                .iter()
                .any(|v| matches!(v, EngineSideEffect::CurrentDirChanged(d) if d == "/one"))
        );
        let opts = SemanticOptions::default();
        let palette = TerminalPalette::dark();
        assert_eq!(
            e.snapshot(&palette, false, opts).command_blocks[0].status,
            CommandBlockStatus::Failure(7)
        );
        e.feed_bytes(b"\x1b]133;A\x07plain text");
        assert!(!e.valid_osc_prompt(e.osc133_records.back().unwrap()));
        e.osc133_records.back_mut().unwrap().abs_start_line = usize::MAX;
        assert!(!e.valid_osc_prompt(e.osc133_records.back().unwrap()));
        assert_eq!(e.snapshot(&palette, false, opts).command_blocks.len(), 1);
    }

    #[test]
    fn selecting_wide_words_and_wrapped_lines_preserves_text() {
        let mut e = AlacrittyTerminalEngine::new(4, 12, 100);
        e.feed_bytes("中文 path/file.txt\r\n".as_bytes());
        e.start_selection(0, 1, 2);
        assert_eq!(e.selected_text().as_deref(), Some("中文"));
        e.start_selection(1, 2, 3);
        assert_eq!(e.selected_text().as_deref(), Some("中文 path/file.txt\n"));
        e.start_selection(0, 2, 1);
        assert_eq!(e.selected_text(), None);
    }

    #[test]
    fn history_and_rows_below_a_repositioned_cursor_keep_their_colors() {
        let mut engine = AlacrittyTerminalEngine::new(21, 40, 100);
        let palette = TerminalPalette::dark();
        let opts = SemanticOptions { semantic_highlighting: false, ..Default::default() };
        engine.feed_bytes("\r\n".repeat(120).as_bytes());
        engine.feed_bytes(b"$ cat a.py\r\n");
        engine.feed_bytes("# row\r\n".repeat(28).as_bytes());
        engine.feed_bytes(b"# end\x1b[3;1H");
        engine.scroll(Scroll::Delta(25));
        let history = engine.snapshot(&palette, true, opts);
        let fg = palette.code_token_style(CodeTokenRole::Comment).0;
        assert!(history.lines[20].text_runs.iter().any(|r| r.text == "row" && r.fg == fg));
        let cache = engine.block_syntax_cache.as_ref().unwrap();
        assert_eq!(cache.completed_spans.len() as i32, -5 - cache.prompt_gl);

        engine.scroll(Scroll::Bottom);
        for cursor in [2, 0] {
            engine.feed_bytes(format!("\x1b[{};1H", cursor + 1).as_bytes());
            let snap = engine.snapshot(&palette, true, opts);
            assert!(snap.lines[20].text_runs.iter().any(|r| r.text == "end" && r.fg == fg));
        }
    }

    #[test]
    fn repeated_commands_and_middle_row_rewrites_do_not_share_stale_spans() {
        let mut engine = AlacrittyTerminalEngine::new(12, 40, 100);
        let palette = TerminalPalette::dark();
        let opts = SemanticOptions { semantic_highlighting: false, ..Default::default() };
        engine.feed_bytes(b"$ cat a.py\r\n1\r\n# first\r\n3\r\n$ cat a.py\r\n1\r\n222\r\n3\r\n");
        let snap = engine.snapshot(&palette, true, opts);
        assert_eq!(snap.lines[6].text_runs[0].fg, palette.code_token_style(CodeTokenRole::Number).0);
        engine.snapshot(&palette, true, SemanticOptions { code_highlighting: false, ..opts });
        engine.feed_bytes(b"\x1b[7;1H# edit\x1b[K\x1b[9;1H");
        let comment = palette.code_token_style(CodeTokenRole::Comment).0;
        for cols in [40, 8] {
            engine.resize(12, cols);
            let snap = engine.snapshot(&palette, true, opts);
            assert!(snap.lines.iter().flat_map(|row| &row.text_runs)
                .any(|run| run.text == "edit" && run.fg == comment));
        }
    }

    #[test]
    fn shortening_an_over_budget_line_recovers_the_following_rows() {
        let mut engine = AlacrittyTerminalEngine::new(6, MAX_SYNTAX_LINE_BYTES + 4, 100);
        let palette = TerminalPalette::dark();
        engine.feed_bytes(b"$ cat a.py\r\n1\r\n");
        engine.feed_bytes("x".repeat(MAX_SYNTAX_LINE_BYTES + 1).as_bytes());
        engine.feed_bytes(b"\r\n222\r\n");
        let number = palette.code_token_style(CodeTokenRole::Number).0;
        let snap = engine.snapshot(&palette, true, SemanticOptions::default());
        assert_ne!(snap.lines[3].text_runs[0].fg, number);
        engine.feed_bytes(b"\x1b[3;1H\x1b[K2\x1b[5;1H");
        let snap = engine.snapshot(&palette, true, SemanticOptions::default());
        assert_eq!(snap.lines[3].text_runs[0].fg, number);
    }

    #[test]
    fn soft_wrap_preserves_spaces_and_maps_combining_characters_to_the_base_cell() {
        let mut engine = AlacrittyTerminalEngine::new(4, 4, 100);
        engine.feed_bytes("中a e\u{301}界".as_bytes());
        let maps: Vec<_> = (0..2).map(|r| engine.build_row_text_map(Line(r))).collect();
        let line = engine.stitch_logical_line_at(0, 3, 0, &maps);
        assert_eq!(line.text, "中a e\u{301}界");
        assert_eq!(line.segments, [(0, 0, 5), (1, 5, 11)]);
        let mut colors = vec![vec![None; 4]; 2];
        project_stitched_spans_to_viewport(
            &line, &[(3..11, CodeTokenRole::String)], 0, 1, 4, &maps,
            &TerminalPalette::dark(), &mut colors,
        );
        assert_eq!(colors[0].iter().map(Option::is_some).collect::<Vec<_>>(), [false, false, true, true]);
        assert_eq!(colors[1].iter().map(Option::is_some).collect::<Vec<_>>(), [true, true, true, false]);
        engine.feed_bytes(" \u{301}".as_bytes());
        let snap = engine.snapshot(&TerminalPalette::dark(), true, SemanticOptions::default());
        assert!(snap.lines[1].text_runs.iter().any(|r| r.start_col == 3 && r.text == " \u{301}"));
    }

    #[test]
    fn soft_wrapped_cjk_lines_and_streaming_after_scrollback_full() {
        // Minimum scrollback 100, narrow 24-column terminal to force WRAPLINE soft wrapping
        let mut engine = AlacrittyTerminalEngine::new(10, 24, 100);
        let palette = TerminalPalette::dark();
        let opts = SemanticOptions::default();

        // Fill scrollback past the 100-line capacity first
        for i in 0..115 {
            engine.feed_bytes(format!("filler line {i}\r\n").as_bytes());
        }

        // Stream a Python file in small chunks across soft-wrapped CJK comments
        engine.feed_bytes(b"user@vm:~$ cat app.py\r\n");
        engine.feed_bytes(b"def greet():\r\n");
        let _ = engine.snapshot(&palette, true, opts); // checkpoint at line 1

        engine.feed_bytes("# 中文软折行长注释测试内容延伸到第二行".as_bytes());
        let mid_snap = engine.snapshot(&palette, true, opts); // speculative tail parse before newline
        let comment_fg = palette.code_token_style(CodeTokenRole::Comment).0;
        assert!(
            mid_snap
                .lines
                .iter()
                .flat_map(|l| &l.text_runs)
                .any(|r| r.text == "延" && r.fg == comment_fg)
        );

        engine.feed_bytes(b"\r\n    return \"ok\"\r\nuser@vm:~$ ");
        let final_snap = engine.snapshot(&palette, true, opts);
        let keyword_fg = palette.code_token_style(CodeTokenRole::Keyword).0;
        let string_fg = palette.code_token_style(CodeTokenRole::String).0;

        // Verify `def`, wrapped second-row CJK comment glyph, `return`, and `"ok"` all colored accurately
        let all_runs: Vec<&TerminalStyledRun> = final_snap
            .lines
            .iter()
            .flat_map(|l| &l.text_runs)
            .collect();
        assert!(all_runs.iter().any(|r| r.text == "def" && r.fg == keyword_fg));
        assert!(all_runs.iter().any(|r| r.text == "行" && r.fg == comment_fg));
        assert!(all_runs.iter().any(|r| r.text.contains("return") && r.fg == keyword_fg));
        assert!(all_runs.iter().any(|r| r.text.contains("\"ok\"") && r.fg == string_fg));
    }

    #[test]
    fn long_shell_script_with_regex_brackets_keeps_highlighting_to_the_end() {
        let mut engine = AlacrittyTerminalEngine::new(24, 100, 5_000);
        let palette = TerminalPalette::dark();
        let opts = SemanticOptions::default();

        engine.feed_bytes(b"root@debian:~# cat tcpx.sh\r\n");
        engine.feed_bytes(b"#!/usr/bin/env bash\r\n");
        for i in 0..600 {
            engine.feed_bytes(format!("echo \"header {i}\"\r\n").as_bytes());
        }
        let installcloud = concat!(
            "# 安装官方 Cloud 内核\r\n",
            "installcloud() {\r\n",
            "        pre_install_check || return 1\r\n",
            "        if [[ \"${OS_ID}\" == \"ubuntu\" ]]; then\r\n",
            "                read -e -p \"是否同时安装配套 Headers? [y/N]: \" u_ans\r\n",
            "                [[ \"$u_ans\" =~ ^[yY]$ ]] && u_hdr=\"linux-headers-virtual\"\r\n",
            "                apt-get update >/dev/null 2>&1\r\n",
            "                if apt-get install -y linux-image-virtual $u_hdr; then\r\n",
            "                        return 0\r\n",
            "                fi\r\n",
            "        fi\r\n",
            "}\r\n",
        );
        engine.feed_bytes(installcloud.as_bytes());
        for i in 0..600 {
            engine.feed_bytes(
                format!("if [[ \"$OS_ARCH\" == \"x86_64\" ]]; then local v_{i}=\"ok\"; fi\r\n")
                    .as_bytes(),
            );
        }
        engine.feed_bytes(b"root@debian:~# ");

        let snap = engine.snapshot(&palette, true, opts);
        let keyword_fg = palette.code_token_style(CodeTokenRole::Keyword).0;
        let string_fg = palette.code_token_style(CodeTokenRole::String).0;

        // Bottom half of the long `.sh` file must still have keyword & string highlighting,
        // and no false command-block divider inside the script output.
        let tail_runs: Vec<&TerminalStyledRun> = snap.lines[..23]
            .iter()
            .flat_map(|l| &l.text_runs)
            .collect();
        assert!(tail_runs.iter().any(|r| r.text.contains("if") && r.fg == keyword_fg));
        assert!(tail_runs.iter().any(|r| r.text.contains("then") && r.fg == keyword_fg));
        assert!(tail_runs.iter().any(|r| r.text.contains("\"x86_64\"") && r.fg == string_fg));
        assert!(snap.command_blocks.is_empty());

        // Scroll directly onto `installcloud()` (~600 lines up) and confirm there is no fake
        // command-block divider on `[[ "$u_ans" =~ ^[yY]$ ]]`, while lines below it stay colored.
        engine.scroll(Scroll::Delta(605));
        let mid_snap = engine.snapshot(&palette, true, opts);
        assert!(mid_snap.command_blocks.is_empty());
        let mid_runs: Vec<&TerminalStyledRun> = mid_snap
            .lines
            .iter()
            .flat_map(|l| &l.text_runs)
            .collect();
        assert!(mid_runs.iter().any(|r| r.text.contains("installcloud")));
        assert!(
            mid_runs
                .iter()
                .any(|r| r.text.contains("\"linux-headers-virtual\"") && r.fg == string_fg)
        );
        assert!(mid_runs.iter().any(|r| r.text.contains("return") && r.fg == keyword_fg));
    }

    #[test]
    fn over_8000_lines_soft_wrap_continuations_and_osc133_echoed_prompts_stay_intact() {
        let palette = TerminalPalette::dark();
        let opts = SemanticOptions::default();
        let keyword_fg = palette.code_token_style(CodeTokenRole::Keyword).0;

        // 1. > 8,000 lines (`MAX_SYNTAX_BLOCK_LINES = 32_000`) with 10,000 scrollback
        let mut huge = AlacrittyTerminalEngine::new(10, 80, 10_000);
        huge.feed_bytes(b"root@debian:~# cat huge.sh\r\n");
        huge.feed_bytes("if true; then return 0; fi\r\n".repeat(8_200).as_bytes());
        let snap = huge.snapshot(&palette, true, opts);
        assert!(
            snap.lines[0]
                .text_runs
                .iter()
                .any(|r| r.text.contains("if") && r.fg == keyword_fg)
        );
        // Scroll up via cached cover fast-path and verify colors remain intact
        huge.scroll(Scroll::Delta(4_000));
        let scrolled = huge.snapshot(&palette, true, opts);
        assert!(
            scrolled.lines[0]
                .text_runs
                .iter()
                .any(|r| r.text.contains("if") && r.fg == keyword_fg)
        );

        // 2. Narrow 16-col soft-wrap continuation starting with `$ ls"` must not create fake dividers
        let mut narrow = AlacrittyTerminalEngine::new(6, 16, 100);
        narrow.feed_bytes(b"$ cat a.sh\r\necho \"1234567890$ ls\"\r\nif true; then\r\nfi\r\n");
        let n_snap = narrow.snapshot(&palette, true, opts);
        assert_eq!(n_snap.command_blocks.len(), 1);
        assert_eq!(n_snap.command_blocks[0].start_row, 0);

        // 3. OSC 133 confirmed output containing `$ make` keeps `cat` highlighting & full-history SGR resets
        let mut osc = AlacrittyTerminalEngine::new(8, 60, 100);
        for i in 0..110 {
            osc.feed_bytes(format!("pad {i}\r\n").as_bytes());
        }
        osc.feed_bytes(
            b"\x1b]133;A\x07$ cat build.sh\r\n\x1b]133;C\x07#!/bin/sh\r\n$ make\r\nif true; then exit 1; fi\r\n\x1b]133;D;7\x07\x1b]133;A\x07$ ",
        );
        osc.feed_bytes("\x1b[0m".repeat(60).as_bytes());
        let o_snap = osc.snapshot(&palette, true, opts);
        assert_eq!(o_snap.command_blocks.len(), 1);
        assert_eq!(
            o_snap.command_blocks[0].status,
            CommandBlockStatus::Failure(7)
        );
        let input_cmd_fg = palette.semantic_style(SemanticRole::InputCommand).0;
        assert!(
            o_snap.lines[5]
                .text_runs
                .iter()
                .all(|r| r.fg != input_cmd_fg)
        );
        assert!(
            o_snap.lines[6]
                .text_runs
                .iter()
                .any(|r| r.text.contains("if") && r.fg == keyword_fg)
        );
    }
}
