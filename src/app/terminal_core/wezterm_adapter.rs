//! Wezterm-backed terminal core adapter.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use termwiz::input::{KeyCode, KeyCodeEncodeModes, KeyboardEncoding, Modifiers as KeyModifiers};
use uuid::Uuid;
use wezterm_surface::{CursorShape, CursorVisibility};
use wezterm_term::color::{ColorAttribute, ColorPalette, SrgbaTuple};
use wezterm_term::{Intensity, Line, Terminal, TerminalConfiguration, TerminalSize, Underline};

use crate::app::ssh::runtime::{
    TerminalCellState, TerminalCursorShape, TerminalCursorState, TerminalKeyEvent, TerminalKeyKind,
    TerminalMouseButton, TerminalMouseEventKind, TerminalMouseInput, TerminalRowState,
    TerminalSurfaceState,
};
use crate::app::terminal_core::{
    SelectionState, TerminalCoreAdapter, TerminalFrameSnapshot, ViewportState,
};
use crate::app::terminal_theme::{palette_for_theme, preset_for_theme};
use crate::theme::{ThemeMode, ThemeVariant};

const DEFAULT_TERMINAL_ROWS: usize = 24;
const DEFAULT_TERMINAL_COLS: usize = 80;
const DEFAULT_TERMINAL_SCROLLBACK_LINES: usize = 1_500;
const FILTERED_EXACT_BANNER: &str =
    "Activate the web console with: systemctl enable --now cockpit.socket";

pub struct WeztermTerminalCoreAdapter {
    terminal: Terminal,
    config: Arc<SessionTerminalConfig>,
    writer: SharedWriteBuffer,
    fallback_mouse_button: Option<TerminalMouseButton>,
    pending_remote_line_buffer: PendingRemoteLineBuffer,
    pending_paste_highlight_filter: Option<PendingPasteHighlightFilter>,
    keyboard_modes: TerminalKeyboardModes,
    mouse_modes: TerminalMouseModes,
    viewport_offset_lines: usize,
}

impl WeztermTerminalCoreAdapter {
    pub fn new(rows: usize, cols: usize, scrollback_lines: usize) -> Self {
        let writer = SharedWriteBuffer::default();
        let config = Arc::new(SessionTerminalConfig::new(
            ThemeMode::Dark,
            ThemeVariant::PremiumDefault,
            scrollback_lines.max(1),
        ));
        let terminal = Terminal::new(
            TerminalSize {
                rows,
                cols,
                pixel_width: cols * 8,
                pixel_height: rows * 16,
                dpi: 96,
            },
            config.clone(),
            "MicaTerm",
            env!("CARGO_PKG_VERSION"),
            Box::new(writer.clone()),
        );

        Self {
            terminal,
            config,
            writer,
            fallback_mouse_button: None,
            pending_remote_line_buffer: PendingRemoteLineBuffer::default(),
            pending_paste_highlight_filter: None,
            keyboard_modes: TerminalKeyboardModes::default(),
            mouse_modes: TerminalMouseModes::default(),
            viewport_offset_lines: 0,
        }
    }

    pub fn sequence_number(&self) -> usize {
        self.terminal.current_seqno()
    }

    pub fn apply_remote_bytes(&mut self, bytes: &[u8]) {
        let filtered = self.pending_remote_line_buffer.push_and_filter(bytes);
        let filtered = if let Some(filter) = self.pending_paste_highlight_filter.as_mut() {
            let filtered = filter.filter(filtered.as_slice());
            if filter.is_finished() {
                self.pending_paste_highlight_filter = None;
            }
            filtered
        } else {
            filtered
        };
        self.keyboard_modes.observe(filtered.as_slice());
        self.mouse_modes.observe(filtered.as_slice());
        if !filtered.is_empty() {
            let was_at_bottom = self.viewport_offset_lines == 0;
            let previous_total_rows = self.terminal.screen().scrollback_rows();
            self.terminal.advance_bytes(filtered.as_slice());
            if self.terminal.is_alt_screen_active() {
                self.viewport_offset_lines = 0;
            } else if !was_at_bottom {
                let next_total_rows = self.terminal.screen().scrollback_rows();
                let appended_rows = next_total_rows.saturating_sub(previous_total_rows);
                self.viewport_offset_lines =
                    self.viewport_offset_lines.saturating_add(appended_rows);
            }
        }
        self.clamp_viewport_offset();
    }

    pub fn screen_text(&self) -> String {
        self.visible_lines().join("\n")
    }

    pub fn visible_rows(&self) -> Vec<TerminalRowState> {
        let size = self.terminal.get_size();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let mut rows = Vec::with_capacity(size.rows.max(1));
        let lines = self
            .terminal
            .screen()
            .lines_in_phys_range(visible_start..visible_end);
        for (visible_row, line) in lines.iter().enumerate() {
            rows.push(project_terminal_row(
                line,
                visible_row as u32,
                size.cols.max(1),
            ));
        }

        while rows.len() < size.rows.max(1) {
            rows.push(TerminalRowState {
                index: rows.len() as u32,
                text: String::new(),
                wrapped: false,
            });
        }

        rows
    }

    pub fn visible_lines(&self) -> Vec<String> {
        visible_lines_from_rows(&self.visible_rows())
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.terminal.resize(TerminalSize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: cols.max(1) * 8,
            pixel_height: rows.max(1) * 16,
            dpi: 96,
        });
        self.clamp_viewport_offset();
    }

    pub fn surface_state(&self, session_id: Uuid) -> TerminalSurfaceState {
        let size = self.terminal.get_size();
        let palette = self.terminal.palette();
        let preset = preset_for_theme(self.config.theme_mode(), self.config.theme_variant());
        let visible_rows = self.visible_rows();
        let visible_lines = visible_lines_from_rows(&visible_rows);
        let cells = self.visible_cells(&palette);
        let cursor = self.cursor_state(&palette);
        let viewport_bg_top_rgba = 0xff00_0000 | preset.viewport_bg_top;
        let viewport_bg_bottom_rgba = 0xff00_0000 | preset.viewport_bg_bottom;
        TerminalSurfaceState {
            session_id,
            seqno: self.sequence_number(),
            rows: size.rows as u32,
            cols: size.cols as u32,
            default_fg_rgba: color_to_rgba_u32(palette.foreground),
            default_bg_rgba: color_to_rgba_u32(palette.background),
            row_bg_even_rgba: viewport_bg_top_rgba,
            row_bg_odd_rgba: viewport_bg_bottom_rgba,
            viewport_offset_lines: self.viewport_offset_lines as u32,
            viewport_max_offset_lines: self.max_viewport_offset_lines() as u32,
            viewport_at_bottom: self.viewport_offset_lines == 0,
            visible_lines,
            visible_rows,
            cells,
            cursor,
            alternate_screen_active: self.terminal.is_alt_screen_active(),
            mouse_grabbed: self.terminal.is_mouse_grabbed(),
            bracketed_paste_enabled: self.terminal.bracketed_paste_enabled(),
        }
    }

    pub fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>> {
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

    pub fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>> {
        self.snap_viewport_to_bottom();
        let sanitized = strip_bracketed_paste_markers(text);
        if self.terminal.bracketed_paste_enabled() {
            self.pending_paste_highlight_filter = PendingPasteHighlightFilter::arm(&sanitized);
            return Ok(format!("\x1b[200~{sanitized}\x1b[201~").into_bytes());
        }

        self.pending_paste_highlight_filter = None;
        Ok(sanitized.into_bytes())
    }

    pub fn scroll_viewport_lines(&mut self, delta: i32) {
        if self.terminal.is_alt_screen_active() || delta == 0 {
            self.viewport_offset_lines = 0;
            return;
        }
        if delta > 0 {
            self.viewport_offset_lines = self.viewport_offset_lines.saturating_add(delta as usize);
        } else if delta < 0 {
            self.viewport_offset_lines = self
                .viewport_offset_lines
                .saturating_sub(delta.unsigned_abs() as usize);
        }
        self.clamp_viewport_offset();
    }

    pub fn scroll_viewport_to_top(&mut self) {
        if self.terminal.is_alt_screen_active() {
            self.viewport_offset_lines = 0;
            return;
        }
        self.viewport_offset_lines = self.max_viewport_offset_lines();
    }

    pub fn scroll_viewport_to_bottom(&mut self) {
        self.viewport_offset_lines = 0;
    }

    pub fn set_theme(&mut self, mode: ThemeMode, variant: ThemeVariant) {
        if self.config.set_theme(mode, variant) {
            self.terminal.increment_seqno();
        }
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.set_theme(mode, ThemeVariant::PremiumDefault);
    }

    pub fn send_key_down(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>> {
        self.snap_viewport_to_bottom();
        let encoded = key.encode(
            modifiers,
            KeyCodeEncodeModes {
                encoding: KeyboardEncoding::Xterm,
                newline_mode: false,
                application_cursor_keys: self.keyboard_modes.application_cursor_keys,
                modify_other_keys: self.keyboard_modes.modify_other_keys,
            },
            true,
        )?;
        let bytes = encoded.into_bytes();

        let mut writer = self.writer.clone();
        writer.write_all(&bytes)?;
        writer.flush()?;

        Ok(self.writer.take())
    }

    pub fn send_mouse_input(&mut self, event: TerminalMouseInput) -> Result<Vec<u8>> {
        let fallback_button = self.resolve_fallback_mouse_button(event);
        self.terminal.mouse_event(wezterm_term::MouseEvent {
            kind: match event.kind {
                TerminalMouseEventKind::Down => wezterm_term::MouseEventKind::Press,
                TerminalMouseEventKind::Up => wezterm_term::MouseEventKind::Release,
                TerminalMouseEventKind::Move => wezterm_term::MouseEventKind::Move,
                TerminalMouseEventKind::Scroll => wezterm_term::MouseEventKind::Press,
            },
            x: event.col as usize,
            y: event.row as i64,
            x_pixel_offset: 0,
            y_pixel_offset: 0,
            button: match event.button {
                TerminalMouseButton::Left => wezterm_term::MouseButton::Left,
                TerminalMouseButton::Middle => wezterm_term::MouseButton::Middle,
                TerminalMouseButton::Right => wezterm_term::MouseButton::Right,
                TerminalMouseButton::WheelUp => wezterm_term::MouseButton::WheelUp(1),
                TerminalMouseButton::WheelDown => wezterm_term::MouseButton::WheelDown(1),
                TerminalMouseButton::None => wezterm_term::MouseButton::None,
            },
            modifiers: mouse_modifiers(event),
        })?;

        let bytes = self.writer.take();
        if !self.terminal.is_mouse_grabbed() {
            return Ok(bytes);
        }

        match event.kind {
            TerminalMouseEventKind::Down | TerminalMouseEventKind::Scroll if !bytes.is_empty() => {
                return Ok(bytes);
            }
            TerminalMouseEventKind::Move
                if matches!(fallback_button, TerminalMouseButton::None)
                    && !self.mouse_modes.any_event_mouse =>
            {
                return Ok(bytes);
            }
            TerminalMouseEventKind::Up if matches!(fallback_button, TerminalMouseButton::None) => {
                return Ok(bytes);
            }
            _ => {}
        }

        Ok(encode_sgr_mouse_fallback(event, fallback_button))
    }

    fn visible_cells(&self, palette: &ColorPalette) -> Vec<TerminalCellState> {
        let size = self.terminal.get_size();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let mut cells = Vec::new();
        let lines = self
            .terminal
            .screen()
            .lines_in_phys_range(visible_start..visible_end);
        for (visible_row, line) in lines.iter().enumerate() {
            let row = visible_row as u32;
            for cell in line.visible_cells() {
                if cell.cell_index() >= size.cols {
                    continue;
                }

                let attrs = cell.attrs();
                let (fg_rgba, bg_rgba) = resolve_cell_colors(palette, attrs);
                cells.push(TerminalCellState {
                    row,
                    col: cell.cell_index() as u32,
                    width: cell.width() as u32,
                    text: cell.str().to_string(),
                    bold: matches!(attrs.intensity(), Intensity::Bold),
                    underline: attrs.underline() != Underline::None,
                    fg_rgba,
                    bg_rgba,
                });
            }
        }

        cells
    }

    fn cursor_state(&self, palette: &ColorPalette) -> TerminalCursorState {
        let cursor = self.terminal.cursor_pos();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let cursor_phys = self.terminal.screen().phys_row(cursor.y);
        let cursor_visible = matches!(cursor.visibility, CursorVisibility::Visible)
            && cursor_phys >= visible_start
            && cursor_phys < visible_end;
        TerminalCursorState {
            row: cursor_phys.saturating_sub(visible_start) as u32,
            col: cursor.x as u32,
            visible: cursor_visible,
            blinking: cursor_shape_blinks(cursor.shape),
            shape: project_cursor_shape(cursor.shape),
            fg_rgba: pack_color(palette.cursor_fg),
            bg_rgba: pack_color(palette.cursor_bg),
        }
    }

    fn resolve_fallback_mouse_button(&mut self, event: TerminalMouseInput) -> TerminalMouseButton {
        match event.kind {
            TerminalMouseEventKind::Down => {
                if event.button != TerminalMouseButton::None {
                    self.fallback_mouse_button = Some(event.button);
                    event.button
                } else {
                    self.fallback_mouse_button
                        .unwrap_or(TerminalMouseButton::None)
                }
            }
            TerminalMouseEventKind::Move => {
                if event.button != TerminalMouseButton::None {
                    self.fallback_mouse_button = Some(event.button);
                    event.button
                } else {
                    self.fallback_mouse_button
                        .unwrap_or(TerminalMouseButton::None)
                }
            }
            TerminalMouseEventKind::Up => {
                let effective = if event.button != TerminalMouseButton::None {
                    event.button
                } else {
                    self.fallback_mouse_button
                        .unwrap_or(TerminalMouseButton::None)
                };
                self.fallback_mouse_button = None;
                effective
            }
            TerminalMouseEventKind::Scroll => event.button,
        }
    }

    fn visible_phys_row_bounds(&self) -> (usize, usize) {
        let total_rows = self.terminal.screen().scrollback_rows();
        if total_rows == 0 {
            return (0, 0);
        }

        let size = self.terminal.get_size();
        let visible_rows = size.rows.max(1).min(total_rows);
        let visible_start = self
            .terminal
            .screen()
            .scrollback_or_visible_row(-(self.viewport_offset_lines as i32))
            .min(total_rows);
        let visible_end = visible_start.saturating_add(visible_rows).min(total_rows);
        let visible_start = visible_end.saturating_sub(visible_rows);
        (visible_start, visible_end)
    }

    fn max_viewport_offset_lines(&self) -> usize {
        if self.terminal.is_alt_screen_active() {
            return 0;
        }
        let size = self.terminal.get_size();
        self.terminal
            .screen()
            .scrollback_rows()
            .saturating_sub(size.rows.max(1))
    }

    fn clamp_viewport_offset(&mut self) {
        if self.terminal.is_alt_screen_active() {
            self.viewport_offset_lines = 0;
            return;
        }
        self.viewport_offset_lines = self
            .viewport_offset_lines
            .min(self.max_viewport_offset_lines());
    }

    #[allow(dead_code)]
    fn snap_viewport_to_bottom(&mut self) {
        if self.viewport_offset_lines > 0 {
            self.scroll_viewport_to_bottom();
        }
    }
}

#[derive(Debug, Default)]
struct PendingRemoteLineBuffer {
    bytes: Vec<u8>,
    passthrough_until_newline: bool,
}

impl PendingRemoteLineBuffer {
    fn push_and_filter(&mut self, incoming: &[u8]) -> Vec<u8> {
        let mut forwarded = Vec::with_capacity(incoming.len());

        for &byte in incoming {
            if self.passthrough_until_newline {
                forwarded.push(byte);
                if byte == b'\n' {
                    self.passthrough_until_newline = false;
                }
                continue;
            }

            self.bytes.push(byte);
            if byte == b'\n' {
                if !matches_filtered_exact_banner(&self.bytes) {
                    forwarded.extend_from_slice(&self.bytes);
                }
                self.bytes.clear();
                continue;
            }

            if !matches_filtered_banner_prefix(&self.bytes) {
                forwarded.extend_from_slice(&self.bytes);
                self.bytes.clear();
                self.passthrough_until_newline = true;
            }
        }

        forwarded
    }
}

#[derive(Debug)]
struct PendingPasteHighlightFilter {
    expected_echo: Vec<u8>,
    observed_output: Vec<u8>,
    pending_bytes: Vec<u8>,
    highlight_active: bool,
    finished: bool,
}

impl PendingPasteHighlightFilter {
    const MAX_OBSERVED_BYTES: usize = 4096;

    fn arm(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }

        Some(Self {
            expected_echo: text.as_bytes().to_vec(),
            observed_output: Vec::new(),
            pending_bytes: Vec::new(),
            highlight_active: false,
            finished: false,
        })
    }

    fn filter(&mut self, incoming: &[u8]) -> Vec<u8> {
        if self.finished {
            return incoming.to_vec();
        }

        if !self.pending_bytes.is_empty() {
            self.pending_bytes.extend_from_slice(incoming);
        } else {
            self.pending_bytes = incoming.to_vec();
        }

        let mut output = Vec::with_capacity(self.pending_bytes.len());
        let mut index = 0;

        while index < self.pending_bytes.len() {
            match classify_sgr_sequence(&self.pending_bytes[index..]) {
                SgrSequenceKind::ReverseOn(len) => {
                    self.highlight_active = true;
                    index += len;
                }
                SgrSequenceKind::ReverseOff(len) if self.highlight_active => {
                    self.highlight_active = false;
                    index += len;
                }
                SgrSequenceKind::ReverseOff(len) => {
                    output.extend_from_slice(&self.pending_bytes[index..index + len]);
                    index += len;
                }
                SgrSequenceKind::Other(len) => {
                    output.extend_from_slice(&self.pending_bytes[index..index + len]);
                    index += len;
                }
                SgrSequenceKind::Partial => break,
                SgrSequenceKind::None => {
                    output.push(self.pending_bytes[index]);
                    index += 1;
                }
            }
        }

        self.pending_bytes.drain(..index);
        self.record_output(output.as_slice());
        output
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn record_output(&mut self, output: &[u8]) {
        if output.is_empty() {
            return;
        }

        self.observed_output.extend_from_slice(output);
        if self.observed_output.len() > Self::MAX_OBSERVED_BYTES {
            let drain_len = self.observed_output.len() - Self::MAX_OBSERVED_BYTES;
            self.observed_output.drain(..drain_len);
        }

        if contains_subslice(&self.observed_output, &self.expected_echo)
            || self.observed_output.len() >= self.expected_echo.len().saturating_add(1024)
        {
            self.finished = true;
            self.pending_bytes.clear();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SgrSequenceKind {
    None,
    Partial,
    Other(usize),
    ReverseOn(usize),
    ReverseOff(usize),
}

fn classify_sgr_sequence(bytes: &[u8]) -> SgrSequenceKind {
    if !bytes.starts_with(b"\x1b[") {
        return SgrSequenceKind::None;
    }

    let Some(final_index) = bytes
        .iter()
        .enumerate()
        .skip(2)
        .find_map(|(index, byte)| ((*byte >= 0x40) && (*byte <= 0x7e)).then_some(index))
    else {
        return SgrSequenceKind::Partial;
    };
    if bytes[final_index] != b'm' {
        return SgrSequenceKind::Other(final_index + 1);
    }
    let sequence_len = final_index + 1;
    let params = &bytes[2..final_index];

    if params.is_empty() {
        return SgrSequenceKind::ReverseOff(sequence_len);
    }

    let values = params
        .split(|byte| *byte == b';')
        .map(|value| {
            std::str::from_utf8(value)
                .ok()
                .and_then(|value| value.parse::<i16>().ok())
        })
        .collect::<Option<Vec<_>>>();
    let Some(values) = values else {
        return SgrSequenceKind::Other(sequence_len);
    };

    if values.contains(&7) {
        return SgrSequenceKind::ReverseOn(sequence_len);
    }
    if values.iter().any(|value| matches!(*value, 0 | 27)) {
        return SgrSequenceKind::ReverseOff(sequence_len);
    }

    SgrSequenceKind::Other(sequence_len)
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn strip_bracketed_paste_markers(text: &str) -> String {
    text.replace("\x1b[200~", "").replace("\x1b[201~", "")
}

#[derive(Debug, Default)]
struct TerminalKeyboardModes {
    application_cursor_keys: bool,
    modify_other_keys: Option<i64>,
    trailing_bytes: Vec<u8>,
}

impl TerminalKeyboardModes {
    fn observe(&mut self, incoming: &[u8]) {
        let mut observed = Vec::with_capacity(self.trailing_bytes.len() + incoming.len());
        observed.extend_from_slice(&self.trailing_bytes);
        observed.extend_from_slice(incoming);

        for index in 0..observed.len() {
            let remaining = &observed[index..];
            if remaining.starts_with(b"\x1b[?1h") {
                self.application_cursor_keys = true;
                continue;
            }
            if remaining.starts_with(b"\x1b[?1l") {
                self.application_cursor_keys = false;
                continue;
            }
        }

        const MAX_TRAILING_BYTES: usize = 8;
        if observed.len() <= MAX_TRAILING_BYTES {
            self.trailing_bytes = observed;
        } else {
            self.trailing_bytes = observed[observed.len() - MAX_TRAILING_BYTES..].to_vec();
        }
    }
}

#[derive(Debug, Default)]
struct TerminalMouseModes {
    any_event_mouse: bool,
    trailing_bytes: Vec<u8>,
}

impl TerminalMouseModes {
    fn observe(&mut self, incoming: &[u8]) {
        let mut observed = Vec::with_capacity(self.trailing_bytes.len() + incoming.len());
        observed.extend_from_slice(&self.trailing_bytes);
        observed.extend_from_slice(incoming);

        for index in 0..observed.len() {
            let remaining = &observed[index..];
            if remaining.starts_with(b"\x1b[?1003h") {
                self.any_event_mouse = true;
                continue;
            }
            if remaining.starts_with(b"\x1b[?1003l") {
                self.any_event_mouse = false;
                continue;
            }
        }

        const MAX_TRAILING_BYTES: usize = 10;
        if observed.len() <= MAX_TRAILING_BYTES {
            self.trailing_bytes = observed;
        } else {
            self.trailing_bytes = observed[observed.len() - MAX_TRAILING_BYTES..].to_vec();
        }
    }
}

pub fn encode_named_key_input(
    key_name: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
) -> Result<Option<Vec<u8>>> {
    let Some(key) = named_key_code(key_name) else {
        return Ok(None);
    };

    let mut session = WeztermTerminalCoreAdapter::new(
        DEFAULT_TERMINAL_ROWS,
        DEFAULT_TERMINAL_COLS,
        DEFAULT_TERMINAL_SCROLLBACK_LINES,
    );
    let bytes = session.send_key_down(key, key_modifiers(alt, ctrl, shift))?;
    Ok(Some(bytes))
}

#[derive(Debug)]
struct SessionTerminalConfig {
    scrollback_lines: usize,
    state: Mutex<SessionTerminalConfigState>,
}

#[derive(Debug, Clone, Copy)]
struct SessionTerminalConfigState {
    theme_mode: ThemeMode,
    theme_variant: ThemeVariant,
    generation: usize,
}

impl SessionTerminalConfig {
    fn new(theme_mode: ThemeMode, theme_variant: ThemeVariant, scrollback_lines: usize) -> Self {
        Self {
            scrollback_lines: scrollback_lines.max(1),
            state: Mutex::new(SessionTerminalConfigState {
                theme_mode,
                theme_variant,
                generation: 0,
            }),
        }
    }

    fn set_theme(&self, theme_mode: ThemeMode, theme_variant: ThemeVariant) -> bool {
        let mut state = self.state.lock().expect("lock session terminal config");
        if state.theme_mode == theme_mode && state.theme_variant == theme_variant {
            return false;
        }
        state.theme_mode = theme_mode;
        state.theme_variant = theme_variant;
        state.generation = state.generation.saturating_add(1);
        true
    }

    fn theme_mode(&self) -> ThemeMode {
        self.state
            .lock()
            .expect("lock session terminal config")
            .theme_mode
    }

    fn theme_variant(&self) -> ThemeVariant {
        self.state
            .lock()
            .expect("lock session terminal config")
            .theme_variant
    }
}

impl TerminalConfiguration for SessionTerminalConfig {
    fn generation(&self) -> usize {
        self.state
            .lock()
            .expect("lock session terminal config")
            .generation
    }

    fn scrollback_size(&self) -> usize {
        self.scrollback_lines.max(1)
    }

    fn color_palette(&self) -> ColorPalette {
        palette_for_theme(self.theme_mode(), self.theme_variant())
    }
}

fn project_terminal_row(line: &Line, index: u32, cols: usize) -> TerminalRowState {
    TerminalRowState {
        index,
        text: line.columns_as_str(0..cols).trim_end().to_string(),
        wrapped: line.last_cell_was_wrapped(),
    }
}

pub(super) fn visible_lines_from_rows(rows: &[TerminalRowState]) -> Vec<String> {
    let mut lines = rows.iter().map(|row| row.text.clone()).collect::<Vec<_>>();
    while lines.first().is_some_and(String::is_empty) {
        let _ = lines.remove(0);
    }
    while lines.last().is_some_and(String::is_empty) {
        let _ = lines.pop();
    }
    lines
}

fn matches_filtered_exact_banner(bytes: &[u8]) -> bool {
    normalized_remote_line(bytes) == FILTERED_EXACT_BANNER.as_bytes()
}

fn matches_filtered_banner_prefix(bytes: &[u8]) -> bool {
    let normalized = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    FILTERED_EXACT_BANNER.as_bytes().starts_with(normalized)
}

fn normalized_remote_line(bytes: &[u8]) -> &[u8] {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    bytes.strip_suffix(b"\r").unwrap_or(bytes)
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

fn resolve_cell_colors(palette: &ColorPalette, attrs: &wezterm_term::CellAttributes) -> (u32, u32) {
    let mut fg = resolve_palette_color(palette, attrs.foreground(), false);
    let mut bg = resolve_palette_color(palette, attrs.background(), true);
    if attrs.reverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    if attrs.invisible() {
        fg = bg;
    }
    (fg, bg)
}

fn resolve_palette_color(palette: &ColorPalette, color: ColorAttribute, background: bool) -> u32 {
    let rgba = if background {
        palette.resolve_bg(color)
    } else {
        palette.resolve_fg(color)
    };
    pack_color(rgba)
}

fn color_to_rgba_u32(color: SrgbaTuple) -> u32 {
    pack_color(color)
}

fn pack_color(color: SrgbaTuple) -> u32 {
    let channel = |value: f32| -> u32 { (value.clamp(0.0, 1.0) * 255.0).round() as u32 };
    let r = channel(color.0);
    let g = channel(color.1);
    let b = channel(color.2);
    let a = channel(color.3);
    (a << 24) | (r << 16) | (g << 8) | b
}

fn project_cursor_shape(shape: CursorShape) -> TerminalCursorShape {
    match shape {
        CursorShape::BlinkingUnderline | CursorShape::SteadyUnderline => {
            TerminalCursorShape::Underline
        }
        CursorShape::BlinkingBar | CursorShape::SteadyBar => TerminalCursorShape::Bar,
        CursorShape::Default | CursorShape::BlinkingBlock | CursorShape::SteadyBlock => {
            TerminalCursorShape::Block
        }
    }
}

fn cursor_shape_blinks(shape: CursorShape) -> bool {
    matches!(
        shape,
        CursorShape::Default
            | CursorShape::BlinkingBlock
            | CursorShape::BlinkingUnderline
            | CursorShape::BlinkingBar
    )
}

fn mouse_modifiers(event: TerminalMouseInput) -> wezterm_term::KeyModifiers {
    let mut modifiers = wezterm_term::KeyModifiers::NONE;
    if event.shift {
        modifiers |= wezterm_term::KeyModifiers::SHIFT;
    }
    if event.ctrl {
        modifiers |= wezterm_term::KeyModifiers::CTRL;
    }
    if event.alt {
        modifiers |= wezterm_term::KeyModifiers::ALT;
    }
    modifiers
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

#[derive(Clone, Debug, Default)]
struct SharedWriteBuffer {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl SharedWriteBuffer {
    fn take(&self) -> Vec<u8> {
        let mut buffer = self.inner.lock().expect("lock terminal write buffer");
        std::mem::take(&mut *buffer)
    }
}

impl Write for SharedWriteBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut buffer = self.inner.lock().expect("lock terminal write buffer");
        buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TerminalCoreAdapter for WeztermTerminalCoreAdapter {
    fn sequence_number(&self) -> usize {
        WeztermTerminalCoreAdapter::sequence_number(self)
    }

    fn apply_remote_bytes(&mut self, bytes: &[u8]) {
        WeztermTerminalCoreAdapter::apply_remote_bytes(self, bytes);
    }

    fn screen_text(&self) -> String {
        WeztermTerminalCoreAdapter::screen_text(self)
    }

    fn visible_rows(&self) -> Vec<TerminalRowState> {
        WeztermTerminalCoreAdapter::visible_rows(self)
    }

    fn visible_lines(&self) -> Vec<String> {
        WeztermTerminalCoreAdapter::visible_lines(self)
    }

    fn frame_snapshot(&self) -> TerminalFrameSnapshot {
        let surface = self.surface_state(Uuid::nil());
        TerminalFrameSnapshot {
            seqno: surface.seqno,
            rows: surface.rows,
            cols: surface.cols,
            default_fg_rgba: surface.default_fg_rgba,
            default_bg_rgba: surface.default_bg_rgba,
            row_bg_even_rgba: surface.row_bg_even_rgba,
            row_bg_odd_rgba: surface.row_bg_odd_rgba,
            viewport: ViewportState {
                offset_lines: surface.viewport_offset_lines,
                max_offset_lines: surface.viewport_max_offset_lines,
                at_bottom: surface.viewport_at_bottom,
            },
            visible_rows: surface.visible_rows,
            visible_lines: surface.visible_lines,
            cells: surface.cells,
            cursor: surface.cursor,
            selection: SelectionState::default(),
            alternate_screen_active: surface.alternate_screen_active,
            mouse_grabbed: surface.mouse_grabbed,
            bracketed_paste_enabled: surface.bracketed_paste_enabled,
        }
    }

    fn resize(&mut self, rows: usize, cols: usize) {
        WeztermTerminalCoreAdapter::resize(self, rows, cols);
    }

    fn set_theme(&mut self, mode: ThemeMode, variant: ThemeVariant) {
        WeztermTerminalCoreAdapter::set_theme(self, mode, variant);
    }

    fn scroll_viewport_lines(&mut self, delta: i32) {
        WeztermTerminalCoreAdapter::scroll_viewport_lines(self, delta);
    }

    fn send_key_down(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>> {
        WeztermTerminalCoreAdapter::send_key_down(self, key, modifiers)
    }

    fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>> {
        WeztermTerminalCoreAdapter::send_key_event(self, event)
    }

    fn send_mouse_input(&mut self, event: TerminalMouseInput) -> Result<Vec<u8>> {
        WeztermTerminalCoreAdapter::send_mouse_input(self, event)
    }

    fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>> {
        WeztermTerminalCoreAdapter::encode_paste(self, text)
    }
}
