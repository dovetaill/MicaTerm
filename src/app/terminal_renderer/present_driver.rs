//! Present-driver seams for native terminal surface scheduling.

use std::rc::Rc;
use i_slint_backend_winit::WinitWindowAfterDrawHook;
use slint::{ComponentHandle, RenderingState, SetRenderingNotifierError};

use crate::AppWindow;
use crate::app::runtime_profile::NativePresentPath;

pub type NativeSurfacePresentCallback = Rc<dyn Fn()>;

pub trait NativeSurfacePresentDriver {
    fn schedule_present(&self, callback: NativeSurfacePresentCallback);
}

#[derive(Clone)]
pub struct RenderingNotifierPresentDriver {
    window: slint::Weak<AppWindow>,
}

impl RenderingNotifierPresentDriver {
    pub fn new(window: &AppWindow) -> Self {
        Self {
            window: window.as_weak(),
        }
    }
}

impl NativeSurfacePresentDriver for RenderingNotifierPresentDriver {
    fn schedule_present(&self, _callback: NativeSurfacePresentCallback) {
        if let Some(window) = self.window.upgrade() {
            window.window().request_redraw();
        }
    }
}

#[derive(Clone)]
pub struct EventLoopPresentDriver {
    window: slint::Weak<AppWindow>,
}

impl EventLoopPresentDriver {
    pub fn new(window: &AppWindow) -> Self {
        Self {
            window: window.as_weak(),
        }
    }
}

impl NativeSurfacePresentDriver for EventLoopPresentDriver {
    fn schedule_present(&self, _callback: NativeSurfacePresentCallback) {
        if let Some(window) = self.window.upgrade() {
            window.window().request_redraw();
        }
    }
}

pub fn create_present_driver(
    window: &AppWindow,
    native_present_path: NativePresentPath,
) -> Box<dyn NativeSurfacePresentDriver> {
    match native_present_path {
        NativePresentPath::EventLoop => Box::new(EventLoopPresentDriver::new(window)),
        NativePresentPath::RenderingNotifier => Box::new(RenderingNotifierPresentDriver::new(window)),
    }
}

pub fn install_rendering_notifier(
    window: &AppWindow,
    mut on_after_rendering: impl FnMut() + 'static,
    mut on_rendering_teardown: impl FnMut() + 'static,
) -> Result<(), SetRenderingNotifierError> {
    window
        .window()
        .set_rendering_notifier(move |rendering_state, _graphics_api| match rendering_state {
            RenderingState::AfterRendering => on_after_rendering(),
            RenderingState::RenderingTeardown => on_rendering_teardown(),
            _ => {}
        })
}

pub fn install_winit_after_draw_hook(
    window: &AppWindow,
    mut on_after_draw: impl FnMut() + 'static,
) {
    window
        .window()
        .on_winit_after_draw(move |_slint_window| on_after_draw());
}
