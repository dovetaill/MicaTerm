//! Native terminal surface orchestration for retained present scheduling.

use std::cell::RefCell;
use std::rc::Rc;

use crate::AppWindow;
use crate::app::terminal_presenter::NativeTerminalFrame;
use crate::app::runtime_profile::NativePresentPath;

use super::damage::{NativeFrameDamageTracker, NativeSurfaceDamage, NativeSurfaceDamageKind};
use super::diagnostics::NativeTerminalSurfaceDiagnostics;
use super::platform::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
    create_platform_native_surface_backend,
};
use super::present_driver::{
    EventLoopPresentDriver, NativeSurfacePresentCallback, NativeSurfacePresentDriver,
    RenderingNotifierPresentDriver, create_present_driver, install_rendering_notifier,
    install_winit_after_draw_hook,
};

#[derive(Clone)]
pub struct NativeTerminalSurface {
    state: Rc<RefCell<NativeTerminalSurfaceState>>,
}

struct NativeTerminalSurfaceState {
    backend: Box<dyn PlatformNativeSurfaceBackend>,
    present_driver: Box<dyn NativeSurfacePresentDriver>,
    retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    rect: NativeTerminalSurfaceRect,
    last_drawn_frame_token: u64,
    latest_diagnostics: NativeTerminalSurfaceDiagnostics,
    damage_tracker: NativeFrameDamageTracker,
    surface_alive: bool,
    host_surface_invalidated: bool,
    dirty: bool,
}

impl NativeTerminalSurfaceState {
    fn new(window: &AppWindow) -> Self {
        Self {
            backend: create_platform_native_surface_backend(),
            present_driver: Box::new(EventLoopPresentDriver::new(window)),
            retained_frame: None,
            rect: NativeTerminalSurfaceRect::default(),
            last_drawn_frame_token: 0,
            latest_diagnostics: NativeTerminalSurfaceDiagnostics::default(),
            damage_tracker: NativeFrameDamageTracker::default(),
            surface_alive: true,
            host_surface_invalidated: false,
            dirty: false,
        }
    }
}

impl NativeTerminalSurface {
    pub fn attach(window: &AppWindow) -> Self {
        let mut native_surface_state = NativeTerminalSurfaceState::new(window);
        if let Err(err) = native_surface_state.backend.attach(window) {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                "platform native terminal surface backend failed to attach; keeping detached fallback"
            );
        }

        let surface = Self {
            state: Rc::new(RefCell::new(native_surface_state)),
        };

        refresh_diagnostics(&mut surface.state.borrow_mut());

        surface
    }

    pub fn configure_present_path(&self, window: &AppWindow, native_present_path: NativePresentPath) {
        self.install_present_driver(window, native_present_path);
    }

    pub fn update_terminal_rect(&self, rect: NativeTerminalSurfaceRect) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if !state.surface_alive || state.rect == rect {
                false
            } else {
                state.rect = rect;
                if let Some(retained_frame) = state.retained_frame.as_mut() {
                    retained_frame.rect = rect;
                }
                state.damage_tracker.mark_full_damage(rect);
                state.backend.update_surface_rect(rect);
                let retained_frame = state.retained_frame.clone();
                state.backend.update_frame(retained_frame);
                state.dirty = true;
                refresh_diagnostics(&mut state);
                true
            }
        };

        if changed {
            self.schedule_present();
        }
    }

    pub fn update_frame_state(&self, frame: NativeTerminalFrame) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if !state.surface_alive {
                false
            } else {
                let previous_frame = state.retained_frame.clone();
                let next_frame = RetainedNativeTerminalSurfaceFrame {
                    frame,
                    rect: state.rect,
                };
                if previous_frame.as_ref() == Some(&next_frame) {
                    false
                } else {
                    state.damage_tracker.track_frame_damage(previous_frame.as_ref(), Some(&next_frame));
                    state.retained_frame = Some(next_frame);
                    let retained_frame = state.retained_frame.clone();
                    state.backend.update_frame(retained_frame);
                    state.dirty = true;
                    refresh_diagnostics(&mut state);
                    true
                }
            }
        };

        if changed {
            self.schedule_present();
        }
    }

    pub fn present(&self, frame: NativeTerminalFrame) {
        self.update_frame_state(frame);
    }

    pub fn diagnostics_snapshot(&self) -> NativeTerminalSurfaceDiagnostics {
        self.state.borrow().latest_diagnostics.clone()
    }

    pub fn clear_frame(&self) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if !state.surface_alive {
                false
            } else {
                let had_frame = state.retained_frame.is_some() || state.last_drawn_frame_token != 0;
                if had_frame {
                    let rect = state.rect;
                    state.damage_tracker.mark_full_damage(rect);
                    clear_retained_frame(&mut state);
                    state.dirty = true;
                    refresh_diagnostics(&mut state);
                }
                had_frame
            }
        };

        if changed {
            self.schedule_present();
        }
    }

    fn install_present_driver(&self, window: &AppWindow, native_present_path: NativePresentPath) {
        {
            let mut state = self.state.borrow_mut();
            state.present_driver = create_present_driver(window, native_present_path);
            refresh_diagnostics(&mut state);
        }

        let install_after_draw_hook = |state: &Rc<RefCell<NativeTerminalSurfaceState>>| {
            let draw_state = Rc::downgrade(state);
            install_winit_after_draw_hook(window, move || {
                if let Some(state) = draw_state.upgrade() {
                    replay_after_host_redraw(&mut state.borrow_mut());
                }
            });
        };

        if native_present_path != NativePresentPath::RenderingNotifier {
            install_after_draw_hook(&self.state);
            return;
        }

        let draw_state = Rc::downgrade(&self.state);
        let teardown_state = Rc::downgrade(&self.state);
        match install_rendering_notifier(
            window,
            move || {
                if let Some(state) = draw_state.upgrade() {
                    replay_after_host_redraw(&mut state.borrow_mut());
                }
            },
            move || {
                if let Some(state) = teardown_state.upgrade() {
                    teardown_native_surface(&mut state.borrow_mut());
                }
            },
        ) {
            Ok(()) => {
                let mut state = self.state.borrow_mut();
                state.present_driver = Box::new(RenderingNotifierPresentDriver::new(window));
                refresh_diagnostics(&mut state);
            }
            Err(err) => {
                tracing::warn!(
                    target: "app.terminal",
                    error = %err,
                    "native terminal rendering notifier is unavailable; falling back to host-window after-draw present scheduling"
                );
                let mut state = self.state.borrow_mut();
                state.present_driver = Box::new(EventLoopPresentDriver::new(window));
                refresh_diagnostics(&mut state);
                drop(state);
                install_after_draw_hook(&self.state);
            }
        }
    }

    fn schedule_present(&self) {
        let callback_state = Rc::downgrade(&self.state);
        let callback: NativeSurfacePresentCallback = Rc::new(move || {
            if let Some(state) = callback_state.upgrade() {
                draw_retained_frame(&mut state.borrow_mut());
            }
        });
        let state = self.state.borrow();
        state.present_driver.schedule_present(callback);
    }
}

fn draw_retained_frame(state: &mut NativeTerminalSurfaceState) {
    if !state.surface_alive {
        return;
    }
    if !state.dirty && !state.damage_tracker.has_damage() && !state.host_surface_invalidated {
        return;
    }
    let damage = state.damage_tracker.take_damage().unwrap_or_default();
    let damage = if matches!(damage.kind, NativeSurfaceDamageKind::None)
        && (state.dirty || state.host_surface_invalidated)
    {
        NativeSurfaceDamage {
            kind: NativeSurfaceDamageKind::Full,
            rect: state.rect,
        }
    } else {
        damage
    };
    if let Some(retained_frame) = state.retained_frame.as_ref() {
        state.last_drawn_frame_token = retained_frame.frame.frame_token;
    } else {
        state.last_drawn_frame_token = 0;
    }
    state.backend.present(damage);
    state.host_surface_invalidated = false;
    state.dirty = false;
    refresh_diagnostics(state);
}

fn replay_after_host_redraw(state: &mut NativeTerminalSurfaceState) {
    state.host_surface_invalidated = true;
    draw_retained_frame(state);
}

fn clear_retained_frame(state: &mut NativeTerminalSurfaceState) {
    state.retained_frame = None;
    state.last_drawn_frame_token = 0;
    state.backend.update_frame(state.retained_frame.clone());
    refresh_diagnostics(state);
}

fn teardown_native_surface(state: &mut NativeTerminalSurfaceState) {
    state.surface_alive = false;
    state.damage_tracker.clear();
    clear_retained_frame(state);
    state.dirty = false;
    state.host_surface_invalidated = false;
    state.backend.detach();
    refresh_diagnostics(state);
}

fn refresh_diagnostics(state: &mut NativeTerminalSurfaceState) {
    state.latest_diagnostics = state.backend.diagnostics_snapshot();
}
