//! Slint rendering-notifier integration for native terminal surface hosting.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, RenderingState};

use crate::AppWindow;
use crate::app::terminal_presenter::NativeTerminalFrame;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NativeTerminalSurfaceRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone)]
pub struct NativeTerminalSurface {
    window: slint::Weak<AppWindow>,
    state: Rc<RefCell<NativeTerminalSurfaceState>>,
}

#[derive(Debug, Default)]
struct NativeTerminalSurfaceState {
    frame_token: u64,
    rect: NativeTerminalSurfaceRect,
}

impl NativeTerminalSurface {
    pub fn attach_or_detach(window: &AppWindow) -> Self {
        let surface = Self {
            window: window.as_weak(),
            state: Rc::new(RefCell::new(NativeTerminalSurfaceState::default())),
        };

        let state = Rc::clone(&surface.state);
        match window
            .window()
            .set_rendering_notifier(move |rendering_state, _graphics_api| {
                if matches!(rendering_state, RenderingState::RenderingTeardown) {
                    state.borrow_mut().frame_token = 0;
                }
            }) {
            Ok(()) => {}
            Err(err) => {
                tracing::warn!(
                    target: "app.terminal",
                    error = %err,
                    "native terminal rendering notifier is unavailable; keeping detached fallback"
                );
            }
        }

        surface
    }

    pub fn update_terminal_rect(&self, rect: NativeTerminalSurfaceRect) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.rect == rect {
                false
            } else {
                state.rect = rect;
                true
            }
        };

        if changed {
            self.request_redraw();
        }
    }

    pub fn present(&self, frame: NativeTerminalFrame) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.frame_token == frame.frame_token {
                false
            } else {
                state.frame_token = frame.frame_token;
                true
            }
        };

        if changed {
            self.request_redraw();
        }
    }

    pub fn clear_frame(&self) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.frame_token == 0 {
                false
            } else {
                state.frame_token = 0;
                true
            }
        };

        if changed {
            self.request_redraw();
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = self.window.upgrade() {
            window.window().request_redraw();
        }
    }
}
