//! Slint rendering-notifier integration for native terminal surface hosting.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, RenderingState};

use crate::AppWindow;
use crate::app::terminal_presenter::NativeTerminalFrame;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedNativeTerminalSurfaceFrame {
    pub frame: NativeTerminalFrame,
    pub rect: NativeTerminalSurfaceRect,
}

#[derive(Debug, Default)]
struct NativeTerminalSurfaceState {
    retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    rect: NativeTerminalSurfaceRect,
    last_drawn_frame_token: u64,
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
                let mut state = state.borrow_mut();
                match rendering_state {
                    RenderingState::BeforeRendering => draw_retained_frame(&mut state),
                    RenderingState::RenderingTeardown => clear_retained_frame(&mut state),
                    _ => {}
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
                if let Some(mut retained_frame) = state.retained_frame {
                    retained_frame.rect = rect;
                    state.retained_frame = Some(retained_frame);
                }
                true
            }
        };

        if changed {
            self.request_redraw();
        }
    }

    pub fn update_frame_state(&self, frame: NativeTerminalFrame) {
        let changed = {
            let mut state = self.state.borrow_mut();
            let next_frame = RetainedNativeTerminalSurfaceFrame {
                frame,
                rect: state.rect,
            };
            if state.retained_frame == Some(next_frame) {
                false
            } else {
                state.retained_frame = Some(next_frame);
                true
            }
        };

        if changed {
            self.request_redraw();
        }
    }

    pub fn present(&self, frame: NativeTerminalFrame) {
        self.update_frame_state(frame);
    }

    pub fn clear_frame(&self) {
        let changed = {
            let mut state = self.state.borrow_mut();
            let had_frame = state.retained_frame.is_some() || state.last_drawn_frame_token != 0;
            if had_frame {
                clear_retained_frame(&mut state);
            }
            had_frame
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

fn draw_retained_frame(state: &mut NativeTerminalSurfaceState) {
    if let Some(retained_frame) = state.retained_frame {
        state.last_drawn_frame_token = retained_frame.frame.frame_token;
    }
}

fn clear_retained_frame(state: &mut NativeTerminalSurfaceState) {
    state.retained_frame = None;
    state.last_drawn_frame_token = 0;
}
