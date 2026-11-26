//! Custom GPU-accelerated Terminal View rendered natively via GPUI `canvas` & `TextSystem`.
//!
//! Features:
//! - Strict 2:1 CJK / Emoji / Nerd-Font cell snapping with bundled `Sarasa Term SC Nerd` fallback
//! - Full OS IME support (`EntityInputHandler`) with inline preedit overlay & cursor popup anchor
//! - OSC 133 Command Block left gutter, divider lines and configurable block navigation
//! - Output rule highlighting (`Ctrl+Click` URL open) & live input syntax highlighting
//! - Scrollbar overview markers (errors, commands, search hits)

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use alacritty_terminal::term::TermMode;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Bounds, ClipboardItem, Context, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    FontStyle, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
    ScrollDelta, ScrollEvent, SharedString, StrikethroughStyle, Styled as _, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, canvas, div, fill, point, prelude::FluentBuilder as _,
    px, size,
};

use gpui_kit::{CursorStyle, HoverListenerMode, Modifiers, StatefulInteractiveElement as _, Task};

use crate::domain::{
    RightClickAction, ShortcutAction, ShortcutConfig, ShortcutScope, tooltip,
};
use crate::terminal::{
    MouseReportKind, RenderCursorShape, SemanticOptions, TerminalPalette, TerminalSessionEntity,
    session::SessionPhase,
};
use crate::ui::fonts::{TerminalFont, installed_font_names};
use crate::ui::theme::macos_squircle_badge;

pub enum TerminalPaneEvent {
    Activate,
    CloseRequested,
}

#[derive(Clone)]
pub struct PanePreferences {
    pub font_family: String,
    pub default_font_size: f32,
    pub dark_mode: bool,
    pub theme_id: String,
    pub scrollback_lines: usize,
    pub semantic: SemanticOptions,
    pub right_click_action: RightClickAction,
    pub copy_on_select: bool,
    pub shortcuts: ShortcutConfig,
}

#[derive(Default, Clone, Copy)]
enum LeftGesture {
    #[default]
    Idle,
    Scrollbar,
    LinkOpen,
    Tui(Option<TermMode>),
    Selecting {
        origin: (usize, usize, bool),
        copy: bool,
    },
}

impl LeftGesture {
    fn should_copy(self) -> bool {
        matches!(self, Self::Selecting { copy: true, .. })
    }
}

#[derive(Default, Clone, Copy)]
enum RightGesture {
    #[default]
    Menu,
    Smart,
    Tui(Option<TermMode>),
}

fn link_modifier(mods: Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        mods.platform
    } else {
        mods.control
    }
}

pub struct TerminalPaneView {
    pub session: Entity<TerminalSessionEntity>,
    pub is_split_member: bool,
    pub is_active_pane: bool,
    focus_handle: FocusHandle,
    search_input: Option<Entity<InputState>>,
    terminal_font: TerminalFont,
    default_font_size: f32,
    palette: TerminalPalette,
    scrollback_lines: usize,
    semantic_opts: SemanticOptions,
    right_click_action: RightClickAction,
    copy_on_select: bool,
    shortcuts: ShortcutConfig,
    marked_text: Option<String>,
    left_gesture: LeftGesture,
    right_gesture: RightGesture,
    middle_press: Option<TermMode>,
    hover_pointer: Option<Point<Pixels>>,
    hover_modifiers: Modifiers,
    search_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    drag_pointer: Option<Point<Pixels>>,
    auto_scroll_task: Option<Task<()>>,
    scroll_line_remainder: f32,
    last_grid_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    last_cell_size: Rc<Cell<(Pixels, Pixels)>>,
    last_cursor_cell: (usize, usize),
}

impl EventEmitter<TerminalPaneEvent> for TerminalPaneView {}

impl Focusable for TerminalPaneView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TerminalPaneView {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        self.marked_text.clone()
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let len = self
            .marked_text
            .as_ref()
            .map(|s| s.encode_utf16().count())
            .unwrap_or(0);
        Some(UTF16Selection {
            range: len..len,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_text
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| 0..s.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.marked_text.take().is_some() {
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = None;
        if !text.is_empty() {
            self.session.update(cx, |s, cx| s.send_ime_text(text, cx));
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = (!new_text.is_empty()).then(|| new_text.to_string());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let base = self.last_grid_bounds.get().unwrap_or(element_bounds);
        let (cw, ch) = self.last_cell_size.get();
        let (row, col) = self.last_cursor_cell;
        Some(Bounds {
            origin: point(
                base.origin.x + cw * (col as f32),
                base.origin.y + ch * (row as f32),
            ),
            size: size(cw * 2.0, ch),
        })
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = item.text() {
            self.marked_text = None;
            self.request_paste(text, window, cx);
        }
    }
}

impl TerminalPaneView {
    pub fn new(
        session: Entity<TerminalSessionEntity>,
        prefs: PanePreferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        cx.observe(&session, |_, _, cx| cx.notify()).detach();

        cx.on_blur(&focus_handle, window, |this, window, cx| {
            this.cancel_gestures(window.mouse_position(), cx);
        })
        .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.cancel_gestures(window.mouse_position(), cx);
                this.hover_pointer = None;
                this.hover_modifiers = Modifiers::default();
                cx.notify();
            }
        })
        .detach();
        let palette = TerminalPalette::for_theme_id(&prefs.theme_id, prefs.dark_mode);
        Self {
            session,
            is_split_member: false,
            is_active_pane: true,
            focus_handle,
            search_input: None,
            terminal_font: TerminalFont::resolve(&prefs.font_family, palette.dark, || {
                installed_font_names(cx)
            }),
            default_font_size: prefs.default_font_size,
            palette,
            scrollback_lines: prefs.scrollback_lines,
            semantic_opts: prefs.semantic,
            right_click_action: prefs.right_click_action,
            shortcuts: prefs.shortcuts,
            marked_text: None,
            left_gesture: LeftGesture::Idle,
            right_gesture: RightGesture::Menu,
            middle_press: None,
            hover_pointer: None,
            hover_modifiers: Modifiers::default(),
            search_bounds: Rc::new(Cell::new(None)),
            drag_pointer: None,
            auto_scroll_task: None,
            scroll_line_remainder: 0.,
            copy_on_select: prefs.copy_on_select,
            last_grid_bounds: Rc::new(Cell::new(None)),
            last_cell_size: Rc::new(Cell::new((px(8.4), px(19.0)))),
            last_cursor_cell: (0, 0),
        }
    }

    pub fn sync_preferences(&mut self, prefs: &PanePreferences, cx: &mut Context<Self>) {
        self.palette = TerminalPalette::for_theme_id(&prefs.theme_id, prefs.dark_mode);
        self.terminal_font = TerminalFont::resolve(&prefs.font_family, self.palette.dark, || {
            installed_font_names(cx)
        });
        self.default_font_size = prefs.default_font_size;
        self.scrollback_lines = prefs.scrollback_lines;
        self.semantic_opts = prefs.semantic;
        self.copy_on_select = prefs.copy_on_select;
        self.right_click_action = prefs.right_click_action;
        self.shortcuts.clone_from(&prefs.shortcuts);
        cx.notify();
    }

    pub fn has_marked_text(&self) -> bool {
        self.marked_text.is_some()
    }

    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.ensure_search_input(window, cx);
        self.session.update(cx, |s, cx| {
            s.search_open = true;
            cx.notify();
        });
        input.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.session.update(cx, |s, cx| {
            s.search_open = false;
            s.set_search_query(String::new(), cx);
        });
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn ensure_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(existing) = &self.search_input {
            return existing.clone();
        }
        let session = self.session.clone();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索终端内容..."));
        cx.subscribe_in(&input, window, move |this, state, event, window, cx| match event {
            InputEvent::Change => {
                let q = state.read(cx).value().to_string();
                session.update(cx, |s, cx| {
                    s.set_search_query(q, cx);
                });
            }
            InputEvent::PressEnter { shift, .. } => {
                let forward = !*shift;
                this.session.update(cx, |s, cx| s.jump_search(forward, cx));
                let _ = window;
            }
            _ => {}
        })
        .detach();
        self.search_input = Some(input.clone());
        input
    }

    fn copy_selection(&self, cx: &mut Context<Self>) {
        if let Some(text) = self.session.read(cx).selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn request_paste(&self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.session.read(cx);
        if text.is_empty() || !matches!(session.phase, SessionPhase::Connected) {
            return;
        }
        if needs_paste_confirmation(session.mode(), &text) {
            let target = self.session.downgrade();
            super::modals::open_multiline_paste_confirm_dialog(
                text,
                window,
                cx,
                move |snapshot, cx| {
                    let _ = target.update(cx, |s, cx| s.send_paste(snapshot, cx));
                },
            );
        } else {
            self.session.update(cx, |s, cx| s.send_paste(&text, cx));
        }
    }

    fn paste_from_clipboard(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard()
            && let Some(text) = item.text()
        {
            self.request_paste(text, window, cx);
        }
    }

    fn point_to_grid_cell(
        &self,
        position: Point<Pixels>,
        rows: usize,
        cols: usize,
    ) -> (usize, usize, bool) {
        let Some(bounds) = self.last_grid_bounds.get() else {
            return (0, 0, false);
        };
        let (cw, ch) = self.last_cell_size.get();
        let cw_f = f32::from(cw).max(4.0);
        let ch_f = f32::from(ch).max(8.0);
        let rel_x = (f32::from(position.x) - f32::from(bounds.origin.x)).max(0.0);
        let rel_y = (f32::from(position.y) - f32::from(bounds.origin.y)).max(0.0);
        let raw_col = rel_x / cw_f;
        let col = (raw_col.floor() as usize).min(cols.saturating_sub(1));
        let right_half = (raw_col - raw_col.floor()) > 0.5;
        let row = ((rel_y / ch_f).floor() as usize).min(rows.saturating_sub(1));
        (row, col, right_half)
    }

    fn press_tui(&self, ev: &MouseDownEvent, row: usize, col: usize, cx: &App) -> Option<TermMode> {
        let session = self.session.read(cx);
        session
            .try_report_mouse(MouseReportKind::Press(ev.button), col, row, ev.modifiers)
            .then(|| session.mode())
    }

    pub fn sync_modifiers(&mut self, mods: Modifiers, cx: &mut Context<Self>) {
        if self.hover_pointer.is_some() && self.hover_modifiers != mods {
            self.hover_modifiers = mods;
            cx.notify();
        }
    }

    fn grid_hit(&self, pos: Point<Pixels>, cx: &App) -> Option<(usize, usize)> {
        let s = self.session.read(cx);
        let bounds = self.last_grid_bounds.get()?;
        let (rows, cols) = s.dimensions();
        let (w, h) = self.last_cell_size.get();
        let x = pos.x - bounds.origin.x;
        let y = pos.y - bounds.origin.y;
        if (s.search_open && self.search_bounds.get().is_some_and(|b| b.contains(&pos)))
            || x < px(0.)
            || x >= bounds.size.width.min(w * cols as f32)
            || y < px(0.)
            || y >= bounds.size.height.min(h * rows as f32)
        {
            return None;
        }
        let (r, c, _) = self.point_to_grid_cell(pos, rows, cols);
        Some((r, c))
    }

    fn release_mouse(
        &self,
        button: MouseButton,
        mode: TermMode,
        pos: Point<Pixels>,
        mods: Modifiers,
        cx: &App,
    ) {
        let session = self.session.read(cx);
        let (rows, cols) = session.dimensions();
        let (mut row, mut col, _) = self.point_to_grid_cell(pos, rows, cols);
        if !mode.contains(TermMode::SGR_MOUSE) {
            row = row.min(222);
            col = col.min(222);
        }
        if let Some(bytes) = crate::terminal::encode_mouse_report(
            MouseReportKind::Release(button),
            col,
            row,
            mods,
            mode,
        ) {
            session.write_raw(bytes);
        }
    }

    fn finish_gesture(&mut self, ev: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.end_gesture(
            ev.button,
            ev.position,
            ev.modifiers,
            self.copy_on_select,
            cx,
        );
    }

    fn end_gesture(
        &mut self,
        button: MouseButton,
        pos: Point<Pixels>,
        mods: Modifiers,
        copy_on_select: bool,
        cx: &mut Context<Self>,
    ) {
        let mode = match button {
            MouseButton::Left => {
                self.auto_scroll_task = None;
                self.drag_pointer = None;
                let gesture = std::mem::take(&mut self.left_gesture);
                if copy_on_select && gesture.should_copy() {
                    self.copy_selection(cx);
                }
                if let LeftGesture::Tui(mode) = gesture {
                    mode
                } else {
                    None
                }
            }
            MouseButton::Middle => self.middle_press.take(),
            MouseButton::Right => match std::mem::take(&mut self.right_gesture) {
                RightGesture::Tui(mode) => mode,
                _ => None,
            },
            _ => None,
        };
        if let Some(mode) = mode {
            self.release_mouse(button, mode, pos, mods, cx);
        }
    }

    fn cancel_gestures(&mut self, pos: Point<Pixels>, cx: &mut Context<Self>) {
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            self.end_gesture(button, pos, self.hover_modifiers, false, cx);
        }
    }

    fn edge_scroll_delta(&self, pos: Point<Pixels>, rows: usize) -> i32 {
        let Some(bounds) = self.last_grid_bounds.get() else {
            return 0;
        };
        let height = self.last_cell_size.get().1;
        let y = f32::from(pos.y - bounds.origin.y) / f32::from(height);
        if y < 0. {
            ((-y).ceil() as i32).clamp(1, 5)
        } else if y >= rows as f32 {
            -((y - rows as f32).ceil() as i32).clamp(1, 5)
        } else {
            0
        }
    }

    fn drag_selection(&mut self, pos: Point<Pixels>, cx: &mut Context<Self>) {
        let (rows, cols) = self.session.read(cx).dimensions();
        let cell = self.point_to_grid_cell(pos, rows, cols);
        let LeftGesture::Selecting { origin, copy } = &mut self.left_gesture else {
            return;
        };
        if cell != *origin || *copy {
            *copy = true;
            self.session
                .update(cx, |s, cx| s.update_selection(cell.0, cell.1, cell.2, cx));
        }
        self.drag_pointer = Some(pos);
        if self.edge_scroll_delta(pos, rows) == 0 {
            self.auto_scroll_task = None;
        } else if self.auto_scroll_task.is_none() {
            self.auto_scroll_task = Some(cx.spawn(async move |weak, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(32))
                        .await;
                    let more = weak.update(cx, |this, cx| {
                        let Some(pos) = this.drag_pointer else {
                            return false;
                        };
                        let (rows, cols) = this.session.read(cx).dimensions();
                        let delta = this.edge_scroll_delta(pos, rows);
                        let (r, c, side) = this.point_to_grid_cell(pos, rows, cols);
                        if delta != 0
                            && this.session.update(cx, |s, cx| {
                                if !s.scroll_selection(delta) {
                                    return false;
                                }
                                s.update_selection(r, c, side, cx);
                                true
                            })
                        {
                            if let LeftGesture::Selecting { copy, .. } = &mut this.left_gesture {
                                *copy = true;
                            }
                            true
                        } else {
                            this.auto_scroll_task = None;
                            false
                        }
                    });
                    if !matches!(more, Ok(true)) {
                        break;
                    }
                }
            }));
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // When an OS IME composition is actively editing preedit text, let the platform IME consume keys
        if self.has_marked_text() {
            return;
        }

        let ks = &event.keystroke;
        let mods = ks.modifiers;
        let key = ks.key.as_str();

        if key == "escape" && self.session.read(cx).search_open {
            self.close_search(window, cx);
            cx.stop_propagation();
            return;
        }

        if let Some(action) = self.shortcuts.match_action(ks, ShortcutScope::Terminal) {
            match action {
                ShortcutAction::Copy => self.copy_selection(cx),
                ShortcutAction::Paste => self.paste_from_clipboard(window, cx),
                ShortcutAction::SelectAll => self.session.update(cx, |s, cx| s.select_all(cx)),
                ShortcutAction::FindInTerminal => self.open_search(window, cx),
                ShortcutAction::ClearTerminal => {
                    self.session.update(cx, |s, cx| s.clear_screen(cx))
                }
                ShortcutAction::JumpPrevBlock | ShortcutAction::JumpNextBlock => {
                    self.session.update(cx, |s, cx| {
                        s.jump_command_block(action == ShortcutAction::JumpPrevBlock, cx)
                    });
                }
                ShortcutAction::ZoomIn => self.session.update(cx, |s, cx| s.zoom_font(1.0, cx)),
                ShortcutAction::ZoomOut => self.session.update(cx, |s, cx| s.zoom_font(-1.0, cx)),
                ShortcutAction::ZoomReset => {
                    let size = self.default_font_size;
                    self.session.update(cx, |s, cx| s.reset_font(size, cx));
                }
                _ => unreachable!("终端收到工作区快捷键 {action:?}"),
            }
            cx.stop_propagation();
            return;
        }

        // Shift+PageUp / Shift+PageDown local scrollback navigation using actual viewport rows
        if mods.shift && !mods.control && !mods.alt {
            if key == "pageup" {
                let half = (self.session.read(cx).viewport_rows() / 2).max(5) as i32;
                self.session.update(cx, |s, cx| s.scroll_lines(half, cx));
                cx.stop_propagation();
                return;
            } else if key == "pagedown" {
                let half = (self.session.read(cx).viewport_rows() / 2).max(5) as i32;
                self.session.update(cx, |s, cx| s.scroll_lines(-half, cx));
                cx.stop_propagation();
                return;
            }
        }

        if self.session.update(cx, |s, cx| s.handle_keystroke(ks, cx)) {
            cx.stop_propagation();
        }
    }

    #[inline(never)]
    fn render_phase_overlay(&self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let session_ref = self.session.read(cx);
        match &session_ref.phase {
            SessionPhase::Connected => None,
            SessionPhase::Connecting(step) => {
                let step_text = step.clone();
                let subtitle = session_ref.endpoint_subtitle();
                Some(
                    div()
                        .absolute()
                        .occlude()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().background.opacity(0.76))
                        .child(
                            h_flex()
                                .w(px(380.0))
                                .p_4()
                                .gap_3p5()
                                .items_center()
                                .rounded(px(12.0))
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().popover)
                                .child(macos_squircle_badge(
                                    LucideIcon::Server,
                                    cx.theme().primary,
                                    38.0,
                                    9.5,
                                ))
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .flex_1()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .truncate()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_sm()
                                                .text_color(cx.theme().foreground)
                                                .child(subtitle),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(step_text),
                                        ),
                                ),
                        )
                        .into_any_element(),
                )
            }
            SessionPhase::AwaitingHostKey(prompt) => {
                let host_label = format!("{}:{}", prompt.host, prompt.port);
                let fp = prompt.fingerprint.clone();
                let session_trust = self.session.clone();
                let session_reject = self.session.clone();
                Some(
                    div()
                        .absolute()
                        .occlude()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().background.opacity(0.82))
                        .child(
                            v_flex()
                                .w(px(440.0))
                                .p_5()
                                .gap_3p5()
                                .rounded(px(12.0))
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().popover)
                                .child(
                                    h_flex()
                                        .gap_3()
                                        .items_center()
                                        .child(macos_squircle_badge(
                                            LucideIcon::ShieldCheck,
                                            cx.theme().warning,
                                            36.0,
                                            9.0,
                                        ))
                                        .child(
                                            v_flex()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_sm()
                                                        .text_color(cx.theme().foreground)
                                                        .child("验证首次连接的 SSH 主机指纹"),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(host_label),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .p_2p5()
                                        .rounded(px(8.0))
                                        .bg(cx.theme().secondary)
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_xs()
                                        .text_color(cx.theme().foreground)
                                        .child(fp),
                                )
                                .child(
                                    h_flex()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("reject-host-key")
                                                .secondary()
                                                .label("取消断开")
                                                .small()
                                                .on_click(move |_, _, cx| {
                                                    session_reject.update(cx, |s, cx| {
                                                        s.respond_host_key(false, cx);
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("trust-host-key")
                                                .primary()
                                                .small()
                                                .label("信任并连接")
                                                .on_click(move |_, _, cx| {
                                                    session_trust.update(cx, |s, cx| {
                                                        s.respond_host_key(true, cx);
                                                    });
                                                }),
                                        ),
                                ),
                        )
                        .into_any_element(),
                )
            }
            SessionPhase::Disconnected(reason) => {
                let text = reason
                    .clone()
                    .unwrap_or_else(|| "会话已断开".to_string());
                let session_reconnect = self.session.clone();
                let scrollback = self.scrollback_lines;
                Some(
                    div()
                        .absolute()
                        .bottom_4()
                        .left_4()
                        .right_4()
                        .flex()
                        .justify_end()
                        .child(
                            h_flex()
                                .occlude()
                                .min_w_0()
                                .max_w(px(460.0))
                                .px_3p5()
                                .py_2()
                                .gap_3()
                                .items_center()
                                .rounded(px(10.0))
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().popover)
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(text),
                                )
                                .child(
                                    Button::new("reconnect-session")
                                        .flex_shrink_0()
                                        .primary()
                                        .xsmall()
                                        .icon(LucideIcon::RotateCcw)
                                        .label("重新连接")
                                        .on_click(move |_, _, cx| {
                                            session_reconnect.update(cx, |s, cx| {
                                                s.reconnect(scrollback, cx);
                                            });
                                        }),
                                ),
                        )
                        .into_any_element(),
                )
            }
            SessionPhase::Error(err) => {
                let err_msg = err.clone();
                let session_retry = self.session.clone();
                let scrollback = self.scrollback_lines;
                Some(
                    div()
                        .absolute()
                        .occlude()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().background.opacity(0.82))
                        .child(
                            v_flex()
                                .w(px(440.0))
                                .p_5()
                                .gap_3p5()
                                .rounded(px(12.0))
                                .border_1()
                                .border_color(cx.theme().danger.opacity(0.45))
                                .bg(cx.theme().popover)
                                .child(
                                    h_flex()
                                        .gap_3()
                                        .items_center()
                                        .child(macos_squircle_badge(
                                            LucideIcon::TriangleAlert,
                                            cx.theme().danger,
                                            36.0,
                                            9.0,
                                        ))
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_sm()
                                                .text_color(cx.theme().foreground)
                                                .child("连接失败"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(err_msg),
                                )
                                .child(
                                    h_flex().justify_end().child(
                                        Button::new("retry-error-session")
                                            .primary()
                                            .small()
                                            .icon(LucideIcon::RotateCcw)
                                            .label("重试连接")
                                            .on_click(move |_, _, cx| {
                                                session_retry.update(cx, |s, cx| {
                                                    s.reconnect(scrollback, cx);
                                                });
                                            }),
                                    ),
                                ),
                        )
                        .into_any_element(),
                )
            }
        }
    }
}

impl Render for TerminalPaneView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window);
        let palette = self.palette.clone();
        let sem_opts = self.semantic_opts;
        let (font_size_f32, search_open, snapshot) = self.session.update(cx, |s, _| {
            (
                s.font_size,
                s.search_open,
                s.render_snapshot(&palette, is_focused, sem_opts),
            )
        });
        if let Some(ref cur) = snapshot.cursor {
            self.last_cursor_cell = (cur.row, cur.col);
        }

        let search_bar = if search_open {
            let input = self.ensure_search_input(window, cx);
            let match_count = snapshot.search_match_count;
            let session_prev = self.session.clone();
            let session_next = self.session.clone();
            let view_close = cx.entity().clone();
            let bounds = self.search_bounds.clone();
            Some(
                h_flex()
                    .child(
                        canvas(move |b, _, _| bounds.set(Some(b)), |_, _, _, _| {})
                            .absolute()
                            .inset_0(),
                    )
                    .absolute()
                    .occlude()
                    .top_2()
                    .right_4()
                    .w(px(320.0))
                    .px_2()
                    .py_1p5()
                    .gap_1p5()
                    .items_center()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            this.close_search(window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(div().flex_1().child(Input::new(&input).xsmall().cleanable(true)))
                    .child(
                        div()
                            .text_xs()
                            .px_1()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{match_count}")),
                    )
                    .child(
                        Button::new("term-search-prev")
                            .tab_stop(false)
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::ArrowUp)
                            .tooltip("上一个")
                            .on_click(move |_, _, cx| {
                                session_prev.update(cx, |s, cx| s.jump_search(false, cx));
                            }),
                    )
                    .child(
                        Button::new("term-search-next")
                            .tab_stop(false)
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::ChevronDown)
                            .tooltip("下一个")
                            .on_click(move |_, _, cx| {
                                session_next.update(cx, |s, cx| s.jump_search(true, cx));
                            }),
                    )
                    .child(
                        Button::new("term-search-close")
                            .tab_stop(false)
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(LucideIcon::X)
                            .on_click(move |_, window, cx| {
                                view_close.update(cx, |this, cx| this.close_search(window, cx));
                            }),
                    ),
            )
        } else {
            None
        };

        // Subtle close button on split panes so individual splits can be closed in 1 click
        let split_close_badge = (self.is_split_member && !search_open).then(|| {
            let pane = cx.entity();
            div().absolute().occlude().top_1p5().right_3().child(
                Button::new(SharedString::from(format!("close-pane-{}", cx.entity_id())))
                    .tab_stop(false)
                    .ghost()
                    .xsmall()
                    .compact()
                    .icon(LucideIcon::X)
                    .tooltip(tooltip(
                        "关闭当前分屏",
                        self.shortcuts.get(ShortcutAction::ClosePaneOrTab).as_ref(),
                    ))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        pane.update(cx, |_, cx| cx.emit(TerminalPaneEvent::CloseRequested));
                    }),
            )
        });

        let phase_overlay = self.render_phase_overlay(cx);
        let grid_bounds_cell = self.last_grid_bounds.clone();
        let cell_size_cell = self.last_cell_size.clone();
        let view_for_ime = cx.entity().clone();
        let focus_for_ime = self.focus_handle.clone();
        let marked_for_paint = self.marked_text.clone();
        let palette_for_paint = palette.clone();
        let session_for_canvas = self.session.clone();
        let terminal_font = self.terminal_font.clone();
        let snap_rows = snapshot.rows;
        let snap_cols = snapshot.cols;
        let bg_hsla = snapshot.background;

        let session_ctx_copy = self.session.clone();
        let view_paste = cx.entity();
        let session_ctx_all = self.session.clone();
        let session_ctx_prev_blk = self.session.clone();
        let session_ctx_next_blk = self.session.clone();
        let session_ctx_clear = self.session.clone();
        let session_ctx_reconnect = self.session.clone();
        let view_ctx = cx.entity();
        let is_split_ctx = self.is_split_member;
        let scrollback_ctx = self.scrollback_lines;
        let has_sel = self.session.read(cx).selected_text().is_some();

        let hover_cell = self
            .hover_pointer
            .filter(|_| window.is_window_hovered() && window.is_window_active())
            .and_then(|p| self.grid_hit(p, cx));
        let active_hover_link = hover_cell.and_then(|(r, c)| self.session.read(cx).link_at(r, c));
        let activated = link_modifier(self.hover_modifiers);
        let host_hint = active_hover_link
            .as_ref()
            .and_then(|link| link.osc8_host_hint.clone());
        let cursor = if activated && active_hover_link.is_some() {
            CursorStyle::PointingHand
        } else if hover_cell.is_none()
            || (snapshot.mode.intersects(TermMode::MOUSE_MODE) && !self.hover_modifiers.shift)
        {
            CursorStyle::Arrow
        } else {
            CursorStyle::IBeam
        };
        let terminal_canvas = div()
            .id(SharedString::from(format!(
                "term-canvas-{}",
                cx.entity_id()
            )))
            .track_focus(&self.focus_handle)
            .key_context("TerminalPane")
            .on_key_down(cx.listener(Self::handle_key_down))
            .size_full()
            .cursor(cursor)
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(cx.listener(|this, inside: &bool, window, cx| {
                this.hover_pointer = inside.then(|| window.mouse_position());
                this.hover_modifiers = window.modifiers();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    this.focus_handle.focus(window, cx);
                    if let Some(bounds) = this.last_grid_bounds.get()
                        && ev.position.x >= bounds.right()
                        && ev.position.x < bounds.right() + px(12.)
                    {
                        this.left_gesture = LeftGesture::Scrollbar;
                        let ratio = f32::from(ev.position.y - bounds.origin.y)
                            / f32::from(bounds.size.height);
                        this.session
                            .update(cx, |s, cx| s.scroll_to_ratio(ratio, cx));
                        return;
                    }
                    let Some((row, col)) = this.grid_hit(ev.position, cx) else {
                        return;
                    };
                    let session = this.session.read(cx);
                    if link_modifier(ev.modifiers)
                        && let Some(link) = session.link_at(row, col)
                    {
                        this.left_gesture = LeftGesture::LinkOpen;
                        cx.open_url(&link.url);
                    } else if !ev.modifiers.shift && session.mode().intersects(TermMode::MOUSE_MODE)
                    {
                        this.left_gesture = LeftGesture::Tui(this.press_tui(ev, row, col, cx));
                    } else {
                        let origin = this.point_to_grid_cell(ev.position, snap_rows, snap_cols);
                        this.left_gesture = LeftGesture::Selecting {
                            origin,
                            copy: ev.click_count >= 2,
                        };
                        this.session
                            .update(cx, |s, cx| s.begin_selection(row, col, ev.click_count, cx));
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    this.focus_handle.focus(window, cx);
                    this.right_gesture = RightGesture::Menu;
                    if ev.modifiers.shift {
                        return;
                    }
                    let Some((row, col)) = this.grid_hit(ev.position, cx) else {
                        return;
                    };
                    let session = this.session.read(cx);
                    if session.mode().intersects(TermMode::MOUSE_MODE) {
                        this.right_gesture = RightGesture::Tui(this.press_tui(ev, row, col, cx));
                    } else if this.right_click_action == RightClickAction::SmartCopyPaste {
                        this.right_gesture = RightGesture::Smart;
                        if let Some(text) = session.selected_text() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            this.session.update(cx, |s, cx| s.clear_selection(cx));
                        } else {
                            this.paste_from_clipboard(window, cx);
                        }
                    } else {
                        return;
                    }
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    this.focus_handle.focus(window, cx);
                    let Some((row, col)) = this.grid_hit(ev.position, cx) else {
                        return;
                    };
                    let session = this.session.read(cx);
                    if !ev.modifiers.shift && session.mode().intersects(TermMode::MOUSE_MODE) {
                        this.middle_press = this.press_tui(ev, row, col, cx);
                    } else {
                        this.paste_from_clipboard(window, cx);
                    }
                }),
            )
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_gesture))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_gesture))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::finish_gesture))
            .on_mouse_up_out(MouseButton::Right, cx.listener(Self::finish_gesture))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::finish_gesture))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(Self::finish_gesture))
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                let cell = this.grid_hit(ev.position, cx);
                let previous = this.hover_pointer.and_then(|p| this.grid_hit(p, cx));
                if cell != previous || ev.modifiers != this.hover_modifiers { cx.notify(); }
                this.hover_pointer = Some(ev.position);
                this.hover_modifiers = ev.modifiers;
                if ev.pressed_button == Some(MouseButton::Left)
                    && matches!(this.left_gesture, LeftGesture::Scrollbar)
                {
                    if let Some(bounds) = this.last_grid_bounds.get() {
                        let ratio = f32::from(ev.position.y - bounds.origin.y)
                            / f32::from(bounds.size.height);
                        this.session
                            .update(cx, |s, cx| s.scroll_to_ratio(ratio, cx));
                    }
                    return;
                }
                let Some((row, col)) = cell else {
                    return;
                };
                let kind = match ev.pressed_button {
                    None => MouseReportKind::Move,
                    Some(MouseButton::Left)
                        if matches!(this.left_gesture, LeftGesture::Tui(Some(_))) =>
                    {
                        MouseReportKind::Drag(MouseButton::Left)
                    }
                    Some(MouseButton::Right)
                        if matches!(this.right_gesture, RightGesture::Tui(Some(_))) =>
                    {
                        MouseReportKind::Drag(MouseButton::Right)
                    }
                    Some(MouseButton::Middle) if this.middle_press.is_some() => {
                        MouseReportKind::Drag(MouseButton::Middle)
                    }
                    _ => return,
                };
                this.session
                    .read(cx)
                    .try_report_mouse(kind, col, row, ev.modifiers);
            }))
            .on_scroll(cx.listener(move |this, ev: &ScrollEvent, _, cx| {
                let raw_lines = match ev.delta {
                    ScrollDelta::Lines(pt) => pt.y * 3.0,
                    ScrollDelta::Pixels(pt) => {
                        let ch = f32::from(this.last_cell_size.get().1).max(14.0);
                        f32::from(pt.y) / ch
                    }
                };
                if raw_lines == 0.0 {
                    return;
                }
                if ev.modifiers.control {
                    let zoom_step = if raw_lines > 0.0 { 1.0 } else { -1.0 };
                    this.session.update(cx, |s, cx| s.zoom_font(zoom_step, cx));
                    cx.stop_propagation();
                    return;
                }

                let (row, col, _) = this.point_to_grid_cell(ev.position, snap_rows, snap_cols);
                let report_kind = if raw_lines > 0.0 {
                    MouseReportKind::WheelUp
                } else {
                    MouseReportKind::WheelDown
                };
                if this
                    .session
                    .read(cx)
                    .try_report_mouse(report_kind, col, row, ev.modifiers)
                {
                    cx.stop_propagation();
                    return;
                }

                let lines = match ev.delta {
                    ScrollDelta::Pixels(_) => {
                        take_pixel_lines(&mut this.scroll_line_remainder, raw_lines)
                    }
                    ScrollDelta::Lines(_) => {
                        this.scroll_line_remainder = 0.;
                        (raw_lines.abs().ceil() * raw_lines.signum()) as i32
                    }
                };
                if lines != 0 {
                    this.session.update(cx, |s, cx| s.scroll_lines(lines, cx));
                }
                cx.stop_propagation();
            }))
            .context_menu(move |menu, _, _| {
                let s_copy = session_ctx_copy.clone();
                let pane_paste = view_paste.clone();
                let s_all = session_ctx_all.clone();
                let s_pblk = session_ctx_prev_blk.clone();
                let s_nblk = session_ctx_next_blk.clone();
                let s_clear = session_ctx_clear.clone();
                let s_rec = session_ctx_reconnect.clone();
                let v_find = view_ctx.clone();

                let menu = menu
                    .item(
                        PopupMenuItem::new("复制")
                            .icon(LucideIcon::Copy)
                            .disabled(!has_sel)
                            .on_click(move |_, _, cx| {
                                if let Some(text) = s_copy.read(cx).selected_text() {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                                }
                            }),
                    )
                    .item(PopupMenuItem::new("粘贴").on_click(move |_, window, cx| {
                        pane_paste.update(cx, |p, cx| p.paste_from_clipboard(window, cx));
                    }))
                    .item(PopupMenuItem::new("全选").on_click(move |_, _, cx| {
                        s_all.update(cx, |s, cx| s.select_all(cx));
                    }))
                    .separator()
                    .item(
                        PopupMenuItem::new("终端内搜索")
                            .icon(LucideIcon::Search)
                            .on_click(move |_, window, cx| {
                                v_find.update(cx, |this, cx| this.open_search(window, cx));
                            }),
                    )
                    .item(
                        PopupMenuItem::new("跳至上一条命令块")
                            .icon(LucideIcon::ArrowUp)
                            .on_click(move |_, _, cx| {
                                s_pblk.update(cx, |s, cx| {
                                    s.jump_command_block(true, cx);
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new("跳至下一条命令块")
                            .icon(LucideIcon::ChevronDown)
                            .on_click(move |_, _, cx| {
                                s_nblk.update(cx, |s, cx| {
                                    s.jump_command_block(false, cx);
                                });
                            }),
                    )
                    .item(PopupMenuItem::new("清屏").on_click(move |_, _, cx| {
                        s_clear.update(cx, |s, cx| s.clear_screen(cx));
                    }))
                    .separator()
                    .item(
                        PopupMenuItem::new("重新连接会话")
                            .icon(LucideIcon::RotateCcw)
                            .on_click(move |_, _, cx| {
                                s_rec.update(cx, |s, cx| s.reconnect(scrollback_ctx, cx));
                            }),
                    );

                if is_split_ctx {
                    let pane = view_ctx.clone();
                    menu.separator().item(
                        PopupMenuItem::new("关闭当前分屏")
                            .icon(LucideIcon::X)
                            .on_click(move |_, _, cx| {
                                pane.update(cx, |_, cx| cx.emit(TerminalPaneEvent::CloseRequested));
                            }),
                    )
                } else {
                    menu
                }
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let pad_left = px(12.0);
                        let pad_right = px(12.0);
                        let pad_y = px(6.0);
                        let inner_bounds = Bounds {
                            origin: point(bounds.origin.x + pad_left, bounds.origin.y + pad_y),
                            size: size(
                                (bounds.size.width - pad_left - pad_right).max(px(32.0)),
                                (bounds.size.height - pad_y * 2.0).max(px(24.0)),
                            ),
                        };

                        let metrics = terminal_font.measure(font_size_f32, window.text_system());
                        let cell_w = metrics.cell_w;
                        let cell_h = metrics.cell_h;
                        let fit_cols = (f32::from(inner_bounds.size.width) / f32::from(cell_w))
                            .floor() as usize;
                        let fit_rows = (f32::from(inner_bounds.size.height) / f32::from(cell_h))
                            .floor() as usize;
                        let fit_cols = fit_cols.max(8);
                        let fit_rows = fit_rows.max(3);

                        grid_bounds_cell.set(Some(inner_bounds));
                        cell_size_cell.set((cell_w, cell_h));

                        if fit_rows != snapshot.rows || fit_cols != snapshot.cols {
                            let s_resize = session_for_canvas.clone();
                            window.defer(cx, move |_, cx| {
                                s_resize.update(cx, |s, cx| s.resize_grid(fit_rows, fit_cols, cx));
                            });
                        }

                        (inner_bounds, metrics, terminal_font, snapshot)
                    },
                    move |outer_bounds,
                          (inner_bounds, metrics, terminal_font, snapshot),
                          window,
                          cx| {
                        let cell_w = metrics.cell_w;
                        let cell_h = metrics.cell_h;
                        let font_size = metrics.font_size;
                        let base_font = &terminal_font.font;
                        let pane = view_for_ime.clone();
                        window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                            if phase.capture()
                                && ev.pressed_button == Some(MouseButton::Left)
                                && matches!(
                                    pane.read(cx).left_gesture,
                                    LeftGesture::Selecting { .. }
                                )
                            {
                                pane.update(cx, |p, cx| p.drag_selection(ev.position, cx));
                            }
                        });
                        // Register OS InputHandler for Chinese/CJK IME support & candidate popup positioning
                        window.handle_input(
                            &focus_for_ime,
                            ElementInputHandler::new(inner_bounds, view_for_ime.clone()),
                            cx,
                        );

                        // 1. Terminal surface background
                        window.paint_quad(fill(outer_bounds, snapshot.background));

                        let origin_x = inner_bounds.origin.x;
                        let origin_y = inner_bounds.origin.y;
                        let grid_pixel_w = cell_w * (snapshot.cols as f32);

                        // 2. OSC 133 / Prompt Command Block dividers & non-success prompt status marker
                        let gutter_x = outer_bounds.origin.x + px(4.0);
                        let marker_h = (cell_h * 0.46).clamp(px(7.0), px(10.0));
                        for blk in &snapshot.command_blocks {
                            let y_top = origin_y + cell_h * (blk.start_row as f32);
                            if blk.show_top_divider {
                                window.paint_quad(fill(
                                    Bounds {
                                        origin: point(origin_x, y_top - px(1.0)),
                                        size: size(grid_pixel_w, px(1.0)),
                                    },
                                    snapshot.block_divider,
                                ));
                            }
                            if !matches!(blk.status, crate::terminal::CommandBlockStatus::Success) {
                                window.paint_quad(fill(
                                    Bounds {
                                        origin: point(gutter_x, y_top + (cell_h - marker_h) * 0.5),
                                        size: size(px(2.5), marker_h),
                                    },
                                    palette_for_paint.command_block_color(blk.status),
                                ));
                            }
                        }

                        // 3. Per-row non-default cell background spans
                        for (r, row_snap) in snapshot.lines.iter().enumerate() {
                            let y = origin_y + cell_h * (r as f32);
                            for span in &row_snap.bg_spans {
                                let x = origin_x + cell_w * (span.start_col as f32);
                                let w = cell_w * ((span.end_col - span.start_col) as f32);
                                window.paint_quad(fill(
                                    Bounds {
                                        origin: point(x, y),
                                        size: size(w, cell_h),
                                    },
                                    span.color,
                                ));
                            }
                        }

                        // 4. Search highlight quads
                        for &(r, sc, ec, color) in &snapshot.search_spans {
                            let x = origin_x + cell_w * (sc as f32);
                            let y = origin_y + cell_h * (r as f32);
                            let w = cell_w * ((ec - sc) as f32);
                            window.paint_quad(fill(
                                Bounds {
                                    origin: point(x, y),
                                    size: size(w, cell_h),
                                },
                                color,
                            ));
                        }

                        // 5. Selection highlight quads
                        for &(r, sc, ec, color) in &snapshot.selection_spans {
                            let x = origin_x + cell_w * (sc as f32);
                            let y = origin_y + cell_h * (r as f32);
                            let w = cell_w * ((ec - sc) as f32);
                            window.paint_quad(fill(
                                Bounds {
                                    origin: point(x, y),
                                    size: size(w, cell_h),
                                },
                                color,
                            ));
                        }

                        // 6. Shaped text runs with exact 1x / 2x monospace cell-width snapping
                        for (r, row_snap) in snapshot.lines.iter().enumerate() {
                            let y = origin_y + cell_h * (r as f32);
                            for run in &row_snap.text_runs {
                                let mut font = base_font.clone();
                                if run.bold {
                                    font.weight = terminal_font.tuning.bold_weight;
                                }
                                if run.italic {
                                    font.style = FontStyle::Italic;
                                }
                                let text_run = TextRun {
                                    len: run.text.len(),
                                    font,
                                    color: run.fg,
                                    background_color: None,
                                    underline: run.underline.then_some(UnderlineStyle {
                                        thickness: px(1.0),
                                        color: Some(run.fg),
                                        wavy: false,
                                    }),
                                    strikethrough: run.strikethrough.then_some(
                                        StrikethroughStyle {
                                            thickness: px(1.0),
                                            color: Some(run.fg),
                                        },
                                    ),
                                };
                                let force_cell =
                                    Some(metrics.advance(run.wide));
                                let shaped = window.text_system().shape_line(
                                    SharedString::from(run.text.clone()),
                                    font_size,
                                    &[text_run],
                                    force_cell,
                                );
                                let x = origin_x + cell_w * (run.start_col as f32);
                                let _ = shaped.paint(
                                    metrics.text_origin(x, y, run.wide),
                                    cell_h,
                                    TextAlign::Left,
                                    None,
                                    window,
                                    cx,
                                );
                            }
                        }

                        // 7. Cursor & Inline IME Preedit Composition Overlay
                        if let Some(cur) = &snapshot.cursor {
                            let cx_pos = origin_x + cell_w * (cur.col as f32);
                            let cy_pos = origin_y + cell_h * (cur.row as f32);
                            let cw = metrics.advance(cur.wide);

                            if let Some(ref preedit) = marked_for_paint {
                                let pre_cols = preedit
                                    .chars()
                                    .map(|c| if c.is_ascii() { 1 } else { 2 })
                                    .sum::<usize>()
                                    .max(1);
                                let pre_w = cell_w * (pre_cols as f32);
                                window.paint_quad(fill(
                                    Bounds {
                                        origin: point(cx_pos, cy_pos),
                                        size: size(pre_w, cell_h),
                                    },
                                    snapshot.background,
                                ));
                                let tr = TextRun {
                                    len: preedit.len(),
                                    font: base_font.clone(),
                                    color: cur.color,
                                    background_color: None,
                                    underline: Some(UnderlineStyle {
                                        thickness: px(2.0),
                                        color: Some(cur.color),
                                        wavy: false,
                                    }),
                                    strikethrough: None,
                                };
                                let shaped = window.text_system().shape_line(
                                    SharedString::from(preedit.clone()),
                                    font_size,
                                    &[tr],
                                    None,
                                );
                                let _ = shaped.paint(
                                    metrics.text_origin(cx_pos, cy_pos, false),
                                    cell_h,
                                    TextAlign::Left,
                                    None,
                                    window,
                                    cx,
                                );
                            } else {
                                match cur.shape {
                                    RenderCursorShape::Block => {
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin: point(cx_pos, cy_pos),
                                                size: size(cw, cell_h),
                                            },
                                            cur.color,
                                        ));
                                        if cur.ch != ' ' {
                                            let s = cur.ch.to_string();
                                            let tr = TextRun {
                                                len: s.len(),
                                                font: base_font.clone(),
                                                color: cur.text_color,
                                                background_color: None,
                                                underline: None,
                                                strikethrough: None,
                                            };
                                            let shaped = window.text_system().shape_line(
                                                SharedString::from(s),
                                                font_size,
                                                &[tr],
                                                Some(cw),
                                            );
                                            let _ = shaped.paint(
                                                metrics.text_origin(cx_pos, cy_pos, cur.wide),
                                                cell_h,
                                                TextAlign::Left,
                                                None,
                                                window,
                                                cx,
                                            );
                                        }
                                    }
                                    RenderCursorShape::HollowBlock => {
                                        let t = px(1.0);
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin: point(cx_pos, cy_pos),
                                                size: size(cw, t),
                                            },
                                            cur.color,
                                        ));
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin: point(cx_pos, cy_pos + cell_h - t),
                                                size: size(cw, t),
                                            },
                                            cur.color,
                                        ));
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin: point(cx_pos, cy_pos),
                                                size: size(t, cell_h),
                                            },
                                            cur.color,
                                        ));
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin: point(cx_pos + cw - t, cy_pos),
                                                size: size(t, cell_h),
                                            },
                                            cur.color,
                                        ));
                                    }
                                    RenderCursorShape::Beam => {
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin: point(cx_pos, cy_pos),
                                                size: size(px(2.0), cell_h),
                                            },
                                            cur.color,
                                        ));
                                    }
                                    RenderCursorShape::Underline => {
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin: point(cx_pos, cy_pos + cell_h - px(2.0)),
                                                size: size(cw, px(2.0)),
                                            },
                                            cur.color,
                                        ));
                                    }
                                    RenderCursorShape::Hidden => {}
                                }
                            }
                        }

                        if let Some(link) = active_hover_link {
                            let fg = palette_for_paint
                                .semantic_style(crate::terminal::SemanticRole::OutputUrl)
                                .0;
                            let thickness = px(if activated { 1.5 } else { 1.0 });
                            for (row, sc, ec) in link.viewport_spans {
                                window.paint_quad(fill(
                                    Bounds {
                                        origin: point(
                                            origin_x + cell_w * sc as f32,
                                            origin_y + cell_h * (row + 1) as f32 - thickness,
                                        ),
                                        size: size(cell_w * (ec - sc) as f32, thickness),
                                    },
                                    fg.opacity(if activated { 0.95 } else { 0.45 }),
                                ));
                            }
                        }
                        // 8. Right Overview Ruler Markers + Scrollback Thumb
                        let track_h = f32::from(outer_bounds.size.height) - 8.0;
                        let right_x =
                            f32::from(outer_bounds.origin.x + outer_bounds.size.width);
                        if track_h > 24.0 {
                            for marker in &snapshot.overview_markers {
                                let my = f32::from(outer_bounds.origin.y)
                                    + 4.0
                                    + marker.ratio_from_top * (track_h - 3.0);
                                window.paint_quad(fill(
                                    Bounds {
                                        origin: point(px(right_x - 9.0), px(my)),
                                        size: size(px(6.0), px(2.5)),
                                    },
                                    palette_for_paint.overview_marker_color(marker.kind),
                                ));
                            }

                            if snapshot.history_size > 0 {
                                let total =
                                    (snapshot.history_size + snapshot.rows).max(1) as f32;
                                let thumb_h = ((snapshot.rows as f32 / total) * track_h)
                                    .clamp(20.0, track_h);
                                let ratio_from_top = 1.0
                                    - (snapshot.display_offset as f32
                                        / snapshot.history_size as f32)
                                        .clamp(0.0, 1.0);
                                let thumb_y = f32::from(outer_bounds.origin.y)
                                    + 4.0
                                    + ratio_from_top * (track_h - thumb_h);
                                window.paint_quad(fill(
                                    Bounds {
                                        origin: point(px(right_x - 6.0), px(thumb_y)),
                                        size: size(px(4.0), px(thumb_h)),
                                    },
                                    snapshot.foreground.opacity(0.28),
                                ));
                            }
                        }
                    },
                )
                .size_full(),
            );

        div()
            .id(SharedString::from(format!("term-pane-{}", cx.entity_id())))
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(bg_hsla)
            .when(self.is_split_member && !self.is_active_pane, |el| {
                el.capture_any_mouse_down(
                    cx.listener(|_, _, _, cx| cx.emit(TerminalPaneEvent::Activate)),
                )
            })
            .child(terminal_canvas)
            .when(self.is_split_member && self.is_active_pane, |el| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(1.5))
                        .bg(cx.theme().primary),
                )
            })
            .when_some(host_hint, |el, host| {
                el.child(
                    div()
                        .absolute()
                        .bottom_2()
                        .left_3()
                        .px_2()
                        .py_0p5()
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("打开 {host}")),
                )
            })
            .children(split_close_badge)
            .children(search_bar)
            .children(phase_overlay)
    }
}

fn needs_paste_confirmation(mode: TermMode, text: &str) -> bool {
    !mode.contains(TermMode::BRACKETED_PASTE) && text.contains(['\r', '\n'])
}

fn take_pixel_lines(remainder: &mut f32, delta: f32) -> i32 {
    if *remainder * delta < 0. {
        *remainder = 0.;
    }
    *remainder += delta;
    let whole = remainder.trunc();
    *remainder -= whole;
    whole as i32
}

#[cfg(test)]
mod tests {

    #[test]
    fn paste_guard_includes_trailing_returns_and_preserves_bracketed_paste() {
        use super::{TermMode, needs_paste_confirmation};
        for text in ["cmd\r", "cmd\n", "a\r\nb"] {
            assert!(needs_paste_confirmation(TermMode::empty(), text));
            assert!(!needs_paste_confirmation(TermMode::BRACKETED_PASTE, text));
        }
        assert!(!needs_paste_confirmation(TermMode::empty(), "cmd"));
    }

    #[test]
    fn copies_only_completed_local_selections_and_accumulates_pixel_scroll() {
        use super::{LeftGesture, take_pixel_lines};
        let origin = (0, 0, false);
        assert!(LeftGesture::Selecting { origin, copy: true }.should_copy());
        for gesture in [
            LeftGesture::Idle,
            LeftGesture::LinkOpen,
            LeftGesture::Scrollbar,
            LeftGesture::Tui(None),
            LeftGesture::Selecting {
                origin,
                copy: false,
            },
        ] {
            assert!(!gesture.should_copy());
        }
        let mut remainder = 0.;
        for (delta, lines) in [(0.25, 0), (0.5, 0), (0.5, 1), (-0.75, 0), (-0.25, -1)] {
            assert_eq!(take_pixel_lines(&mut remainder, delta), lines);
        }
    }
}
