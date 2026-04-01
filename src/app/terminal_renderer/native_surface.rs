//! Slint rendering-notifier integration for native terminal surface hosting.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, RenderingState};

use crate::AppWindow;
use crate::app::terminal_presenter::NativeTerminalFrame;

use super::platform::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
    create_platform_native_surface_backend,
};

#[derive(Clone)]
pub struct NativeTerminalSurface {
    window: slint::Weak<AppWindow>,
    state: Rc<RefCell<NativeTerminalSurfaceState>>,
}

struct NativeTerminalSurfaceState {
    backend: Box<dyn PlatformNativeSurfaceBackend>,
    retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    rect: NativeTerminalSurfaceRect,
    last_drawn_frame_token: u64,
}

impl NativeTerminalSurfaceState {
    fn new() -> Self {
        Self {
            backend: create_platform_native_surface_backend(),
            retained_frame: None,
            rect: NativeTerminalSurfaceRect::default(),
            last_drawn_frame_token: 0,
        }
    }
}

impl NativeTerminalSurface {
    pub fn attach(window: &AppWindow) -> Self {
        let mut native_surface_state = NativeTerminalSurfaceState::new();
        if let Err(err) = native_surface_state.backend.attach(window) {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                "platform native terminal surface backend failed to attach; keeping detached fallback"
            );
        }

        let surface = Self {
            window: window.as_weak(),
            state: Rc::new(RefCell::new(native_surface_state)),
        };

        let state = Rc::clone(&surface.state);
        match window
            .window()
            .set_rendering_notifier(move |rendering_state, _graphics_api| {
                let mut state = state.borrow_mut();
                match rendering_state {
                    RenderingState::AfterRendering => draw_retained_frame(&mut state),
                    RenderingState::RenderingTeardown => teardown_native_surface(&mut state),
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
                if let Some(retained_frame) = state.retained_frame.as_mut() {
                    retained_frame.rect = rect;
                }
                state.backend.update_surface_rect(rect);
                let retained_frame = state.retained_frame.clone();
                state.backend.update_frame(retained_frame);
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
            if state.retained_frame.as_ref() == Some(&next_frame) {
                false
            } else {
                state.retained_frame = Some(next_frame);
                let retained_frame = state.retained_frame.clone();
                state.backend.update_frame(retained_frame);
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
    if let Some(retained_frame) = state.retained_frame.as_ref() {
        state.last_drawn_frame_token = retained_frame.frame.frame_token;
    }
    state.backend.present();
}

fn clear_retained_frame(state: &mut NativeTerminalSurfaceState) {
    state.retained_frame = None;
    state.last_drawn_frame_token = 0;
    state.backend.update_frame(state.retained_frame.clone());
}

fn teardown_native_surface(state: &mut NativeTerminalSurfaceState) {
    clear_retained_frame(state);
    state.backend.detach();
}
