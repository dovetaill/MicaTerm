//! Custom GPU-accelerated Terminal View rendered natively via GPUI `canvas` & `TextSystem`.

use std::rc::Rc;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable, Font,
    FontFeatures, FontStyle, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    Render, ScrollDelta, ScrollEvent, SharedString, StrikethroughStyle, Styled as _, TextAlign,
    TextRun, UnderlineStyle, Window, canvas, div, fill, point, prelude::FluentBuilder as _, px,
    size,
};

use crate::domain::default_terminal_font_family;
use crate::terminal::{
    MouseReportKind, RenderCursorShape, TerminalPalette, TerminalSessionEntity,
    session::SessionPhase,
};

pub struct TerminalPaneView {
    pub pane_id: String,
    pub session: Entity<TerminalSessionEntity>,
    focus_handle: FocusHandle,
    search_input: Option<Entity<InputState>>,
    font_family: SharedString,
    dark_mode: bool,
    scrollback_lines: usize,
    selecting: bool,
    dragging_scrollbar: bool,
    last_grid_bounds: Option<Bounds<Pixels>>,
    last_cell_size: (Pixels, Pixels),
    on_activate: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
}

impl Focusable for TerminalPaneView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TerminalPaneView {
    pub fn new(
        session: Entity<TerminalSessionEntity>,
        font_family: String,
        dark_mode: bool,
        scrollback_lines: usize,
        on_activate: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let pane_id = uuid::Uuid::new_v4().to_string();
        let focus_handle = cx.focus_handle();
        cx.observe(&session, |_, _, cx| cx.notify()).detach();

        Self {
            pane_id,
            session,
            focus_handle,
            search_input: None,
            font_family: font_family.into(),
            dark_mode,
            scrollback_lines,
            selecting: false,
            dragging_scrollbar: false,
            last_grid_bounds: None,
            last_cell_size: (px(8.4), px(19.0)),
            on_activate,
        }
    }

    pub fn sync_preferences(
        &mut self,
        font_family: &str,
        dark_mode: bool,
        scrollback_lines: usize,
        cx: &mut Context<Self>,
    ) {
        self.font_family = SharedString::from(font_family.to_string());
        self.dark_mode = dark_mode;
        self.scrollback_lines = scrollback_lines;
        cx.notify();
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

    fn paste_from_clipboard(&self, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard()
            && let Some(text) = item.text()
        {
            self.session.update(cx, |s, cx| s.send_paste(&text, cx));
        }
    }

    fn point_to_grid_cell(&self, position: Point<Pixels>, rows: usize, cols: usize) -> (usize, usize, bool) {
        let Some(bounds) = self.last_grid_bounds else {
            return (0, 0, false);
        };
        let (cw, ch) = self.last_cell_size;
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

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;
        let mods = ks.modifiers;
        let key = ks.key.as_str();

        // Escape closes active search bar first
        if key == "escape" && self.session.read(cx).search_open {
            self.close_search(window, cx);
            cx.stop_propagation();
            return;
        }

        let primary_shortcut = if cfg!(target_os = "macos") {
            mods.platform
        } else {
            mods.control && mods.shift
        };

        if primary_shortcut {
            match key {
                "c" | "C" => {
                    self.copy_selection(cx);
                    cx.stop_propagation();
                    return;
                }
                "v" | "V" => {
                    self.paste_from_clipboard(cx);
                    cx.stop_propagation();
                    return;
                }
                "f" | "F" => {
                    self.open_search(window, cx);
                    cx.stop_propagation();
                    return;
                }
                "a" | "A" => {
                    self.session.update(cx, |s, cx| s.select_all(cx));
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }

        // Ctrl + +/-/0 font zoom
        if (mods.control || mods.platform) && !mods.alt {
            match key {
                "=" | "+" => {
                    self.session.update(cx, |s, cx| s.zoom_font(1.0, cx));
                    cx.stop_propagation();
                    return;
                }
                "-" | "_" => {
                    self.session.update(cx, |s, cx| s.zoom_font(-1.0, cx));
                    cx.stop_propagation();
                    return;
                }
                "0" => {
                    self.session.update(cx, |s, cx| s.reset_font(14.0, cx));
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }

        // Shift+PageUp / Shift+PageDown local scrollback navigation
        if mods.shift && !mods.control && !mods.alt {
            if key == "pageup" {
                let half = (self.session.read(cx).render_snapshot_rows_hint() / 2).max(5) as i32;
                self.session.update(cx, |s, cx| s.scroll_lines(half, cx));
                cx.stop_propagation();
                return;
            } else if key == "pagedown" {
                let half = (self.session.read(cx).render_snapshot_rows_hint() / 2).max(5) as i32;
                self.session.update(cx, |s, cx| s.scroll_lines(-half, cx));
                cx.stop_propagation();
                return;
            }
        }

        if self.session.update(cx, |s, cx| s.handle_keystroke(ks, cx)) {
            cx.stop_propagation();
        }
    }

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
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().background.opacity(0.82))
                        .child(
                            v_flex()
                                .w(px(380.0))
                                .p_5()
                                .gap_2()
                                .rounded(cx.theme().radius_lg)
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().popover)
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(LucideIcon::Server)
                                                .small()
                                                .text_color(cx.theme().primary),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_sm()
                                                .text_color(cx.theme().foreground)
                                                .child(subtitle),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(step_text),
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
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().background.opacity(0.88))
                        .child(
                            v_flex()
                                .w(px(440.0))
                                .p_5()
                                .gap_3()
                                .rounded(cx.theme().radius_lg)
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().popover)
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(LucideIcon::TriangleAlert)
                                                .text_color(cx.theme().warning),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(cx.theme().foreground)
                                                .child("首次连接未知 SSH 主机"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "请核对远程主机 {host_label} 的公钥指纹，确认无误后加入本地信任列表："
                                        )),
                                )
                                .child(
                                    div()
                                        .p_2p5()
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().secondary)
                                        .font_family(self.font_family.clone())
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
                                                .label("拒绝断开")
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
                        .right_4()
                        .child(
                            h_flex()
                                .px_3()
                                .py_2()
                                .gap_3()
                                .items_center()
                                .rounded(cx.theme().radius_lg)
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().popover)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(text),
                                )
                                .child(
                                    Button::new("reconnect-session")
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
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().background.opacity(0.86))
                        .child(
                            v_flex()
                                .w(px(440.0))
                                .p_5()
                                .gap_3()
                                .rounded(cx.theme().radius_lg)
                                .border_1()
                                .border_color(cx.theme().danger.opacity(0.5))
                                .bg(cx.theme().popover)
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(LucideIcon::TriangleAlert)
                                                .text_color(cx.theme().danger),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
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
        let palette = TerminalPalette::for_mode(self.dark_mode);
        let (font_size_f32, search_open, snapshot) = self.session.update(cx, |s, _| {
            (s.font_size, s.search_open, s.render_snapshot(&palette, is_focused))
        });

        let search_bar = if search_open {
            let input = self.ensure_search_input(window, cx);
            let match_count = snapshot.search_match_count;
            let session_prev = self.session.clone();
            let session_next = self.session.clone();
            let view_close = cx.entity().clone();
            Some(
                h_flex()
                    .absolute()
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

        let phase_overlay = self.render_phase_overlay(cx);
        let view_for_canvas = cx.entity().clone();
        let session_for_canvas = self.session.clone();
        let font_family = self.font_family.clone();
        let snap_rows = snapshot.rows;
        let snap_cols = snapshot.cols;
        let bg_hsla = snapshot.background;

        let session_ctx_copy = self.session.clone();
        let session_ctx_paste = self.session.clone();
        let session_ctx_all = self.session.clone();
        let session_ctx_clear = self.session.clone();
        let session_ctx_reconnect = self.session.clone();
        let view_ctx_find = cx.entity().clone();
        let scrollback_ctx = self.scrollback_lines;
        let has_sel = self.session.read(cx).selected_text().is_some();

        div()
            .id(SharedString::from(format!("term-pane-{}", self.pane_id)))
            .track_focus(&self.focus_handle)
            .key_context("TerminalPane")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(bg_hsla)
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    this.focus_handle.focus(window, cx);
                    if let Some(cb) = &this.on_activate {
                        let pid = this.pane_id.clone();
                        cb(&pid, window, cx);
                    }
                    // Check if clicking right-edge scrollbar track (last 10px)
                    if let Some(bounds) = this.last_grid_bounds {
                        let right_edge = f32::from(bounds.origin.x + bounds.size.width);
                        if f32::from(ev.position.x) >= right_edge - 10.0 {
                            let rel_y = (f32::from(ev.position.y) - f32::from(bounds.origin.y))
                                / f32::from(bounds.size.height).max(1.0);
                            this.dragging_scrollbar = true;
                            this.session
                                .update(cx, |s, cx| s.scroll_to_ratio(rel_y, cx));
                            return;
                        }
                    }

                    let (row, col, _) = this.point_to_grid_cell(ev.position, snap_rows, snap_cols);
                    if this.session.read(cx).try_report_mouse(
                        MouseReportKind::Press(MouseButton::Left),
                        col,
                        row,
                        ev.modifiers,
                    ) {
                        return;
                    }
                    this.selecting = true;
                    this.session.update(cx, |s, cx| {
                        s.begin_selection(row, col, ev.click_count.max(1), cx);
                    });
                }),
            )
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                if this.dragging_scrollbar {
                    if ev.pressed_button == Some(MouseButton::Left) {
                        if let Some(bounds) = this.last_grid_bounds {
                            let rel_y = (f32::from(ev.position.y) - f32::from(bounds.origin.y))
                                / f32::from(bounds.size.height).max(1.0);
                            this.session
                                .update(cx, |s, cx| s.scroll_to_ratio(rel_y, cx));
                        }
                    } else {
                        this.dragging_scrollbar = false;
                    }
                    return;
                }

                let (row, col, right_half) =
                    this.point_to_grid_cell(ev.position, snap_rows, snap_cols);
                if let Some(btn) = ev.pressed_button {
                    if this.session.read(cx).try_report_mouse(
                        MouseReportKind::Drag(btn),
                        col,
                        row,
                        ev.modifiers,
                    ) {
                        return;
                    }
                    if btn == MouseButton::Left && this.selecting {
                        this.session.update(cx, |s, cx| {
                            s.update_selection(row, col, right_half, cx);
                        });
                    }
                } else {
                    let _ = this.session.read(cx).try_report_mouse(
                        MouseReportKind::Move,
                        col,
                        row,
                        ev.modifiers,
                    );
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseUpEvent, _, cx| {
                    this.dragging_scrollbar = false;
                    this.selecting = false;
                    let (row, col, _) = this.point_to_grid_cell(ev.position, snap_rows, snap_cols);
                    let _ = this.session.read(cx).try_report_mouse(
                        MouseReportKind::Release(MouseButton::Left),
                        col,
                        row,
                        ev.modifiers,
                    );
                }),
            )
            .on_scroll(cx.listener(move |this, ev: &ScrollEvent, _, cx| {
                let raw_lines = match ev.delta {
                    ScrollDelta::Lines(pt) => pt.y * 3.0,
                    ScrollDelta::Pixels(pt) => {
                        let ch = f32::from(this.last_cell_size.1).max(14.0);
                        f32::from(pt.y) / ch
                    }
                };
                if raw_lines.abs() < 0.01 {
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

                let lines = if raw_lines > 0.0 {
                    (raw_lines.ceil() as i32).max(1)
                } else {
                    (raw_lines.floor() as i32).min(-1)
                };
                this.session.update(cx, |s, cx| s.scroll_lines(lines, cx));
                cx.stop_propagation();
            }))
            .context_menu(move |menu, _, _| {
                let s_copy = session_ctx_copy.clone();
                let s_paste = session_ctx_paste.clone();
                let s_all = session_ctx_all.clone();
                let s_clear = session_ctx_clear.clone();
                let s_rec = session_ctx_reconnect.clone();
                let v_find = view_ctx_find.clone();

                menu.item(
                    PopupMenuItem::new("复制")
                        .icon(LucideIcon::Copy)
                        .when(!has_sel, |it| it)
                        .on_click(move |_, _, cx| {
                            if let Some(text) = s_copy.read(cx).selected_text() {
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                            }
                        }),
                )
                .item(
                    PopupMenuItem::new("粘贴")
                        .on_click(move |_, _, cx| {
                            if let Some(item) = cx.read_from_clipboard()
                                && let Some(text) = item.text()
                            {
                                s_paste.update(cx, |s, cx| s.send_paste(&text, cx));
                            }
                        }),
                )
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
                )
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let pad_x = px(8.0);
                        let pad_y = px(6.0);
                        let inner_bounds = Bounds {
                            origin: point(bounds.origin.x + pad_x, bounds.origin.y + pad_y),
                            size: size(
                                (bounds.size.width - pad_x * 2.0 - px(6.0)).max(px(32.0)),
                                (bounds.size.height - pad_y * 2.0).max(px(24.0)),
                            ),
                        };

                        let font_size = px(font_size_f32);
                        let base_font = resolve_mono_font(&font_family);
                        let probe_run = TextRun {
                            len: 1,
                            font: base_font.clone(),
                            color: snapshot.foreground,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let probe = window.text_system().shape_line(
                            "M".into(),
                            font_size,
                            &[probe_run],
                            None,
                        );
                        let cell_w = if f32::from(probe.width()) > 2.0 {
                            probe.width()
                        } else {
                            px((font_size_f32 * 0.6).round().max(5.0))
                        };
                        let cell_h = px((font_size_f32 * 1.38).round().max(12.0));

                        let fit_cols =
                            (f32::from(inner_bounds.size.width) / f32::from(cell_w)).floor() as usize;
                        let fit_rows =
                            (f32::from(inner_bounds.size.height) / f32::from(cell_h)).floor() as usize;
                        let fit_cols = fit_cols.max(8);
                        let fit_rows = fit_rows.max(3);

                        view_for_canvas.update(cx, |this, _| {
                            this.last_grid_bounds = Some(inner_bounds);
                            this.last_cell_size = (cell_w, cell_h);
                        });

                        if fit_rows != snapshot.rows || fit_cols != snapshot.cols {
                            let s_resize = session_for_canvas.clone();
                            window.defer(cx, move |_, cx| {
                                s_resize.update(cx, |s, cx| s.resize_grid(fit_rows, fit_cols, cx));
                            });
                        }

                        (inner_bounds, cell_w, cell_h, font_size, base_font, snapshot)
                    },
                    move |outer_bounds,
                          (inner_bounds, cell_w, cell_h, font_size, base_font, snapshot),
                          window,
                          cx| {
                        // 1. Terminal surface background
                        window.paint_quad(fill(outer_bounds, snapshot.background));

                        let origin_x = inner_bounds.origin.x;
                        let origin_y = inner_bounds.origin.y;

                        // 2. Per-row non-default cell background spans
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

                        // 3. Search highlight quads
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

                        // 4. Selection highlight quads
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

                        // 5. Shaped text runs with monospace cell-width snapping
                        for (r, row_snap) in snapshot.lines.iter().enumerate() {
                            let y = origin_y + cell_h * (r as f32);
                            for run in &row_snap.text_runs {
                                let mut font = base_font.clone();
                                if run.bold {
                                    font.weight = FontWeight::BOLD;
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
                                    strikethrough: run.strikethrough.then_some(StrikethroughStyle {
                                        thickness: px(1.0),
                                        color: Some(run.fg),
                                    }),
                                };
                                let force_cell = if run.col_len == 2 && run.text.chars().count() == 1 {
                                    Some(cell_w * 2.0)
                                } else {
                                    Some(cell_w)
                                };
                                let shaped = window.text_system().shape_line(
                                    SharedString::from(run.text.clone()),
                                    font_size,
                                    &[text_run],
                                    force_cell,
                                );
                                let x = origin_x + cell_w * (run.start_col as f32);
                                let _ = shaped.paint(
                                    point(x, y),
                                    cell_h,
                                    TextAlign::Left,
                                    None,
                                    window,
                                    cx,
                                );
                            }
                        }

                        // 6. Cursor
                        if let Some(cur) = &snapshot.cursor {
                            let cx_pos = origin_x + cell_w * (cur.col as f32);
                            let cy_pos = origin_y + cell_h * (cur.row as f32);
                            let cw = if cur.wide { cell_w * 2.0 } else { cell_w };
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
                                            point(cx_pos, cy_pos),
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

                        // 7. Scrollback indicator thumb
                        if snapshot.history_size > 0 {
                            let total = (snapshot.history_size + snapshot.rows).max(1) as f32;
                            let track_h = f32::from(outer_bounds.size.height) - 8.0;
                            if track_h > 24.0 {
                                let thumb_h =
                                    ((snapshot.rows as f32 / total) * track_h).clamp(20.0, track_h);
                                let ratio_from_top = 1.0
                                    - (snapshot.display_offset as f32
                                        / snapshot.history_size as f32)
                                        .clamp(0.0, 1.0);
                                let thumb_y = f32::from(outer_bounds.origin.y)
                                    + 4.0
                                    + ratio_from_top * (track_h - thumb_h);
                                let thumb_x = f32::from(
                                    outer_bounds.origin.x + outer_bounds.size.width,
                                ) - 6.0;
                                window.paint_quad(fill(
                                    Bounds {
                                        origin: point(px(thumb_x), px(thumb_y)),
                                        size: size(px(4.0), px(thumb_h)),
                                    },
                                    snapshot.foreground.opacity(0.26),
                                ));
                            }
                        }
                    },
                )
                .size_full(),
            )
            .children(search_bar)
            .children(phase_overlay)
    }
}

impl TerminalSessionEntity {
    pub(crate) fn render_snapshot_rows_hint(&self) -> usize {
        24
    }
}

fn resolve_mono_font(family: &str) -> Font {
    let name = if family.trim().is_empty() {
        default_terminal_font_family()
    } else {
        family.trim().to_string()
    };
    Font {
        family: SharedString::from(name),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    }
}
