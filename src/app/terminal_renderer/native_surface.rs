//! Retained native present scheduling for the explicit
//! `MICA_TERM_TERMINAL_SUBSYSTEM=retained-native-surface` bring-up path.

use std::cell::RefCell;
use std::rc::Rc;

use crate::AppWindow;
use crate::app::runtime_profile::NativePresentPath;
use crate::app::terminal_presenter::NativeTerminalFrame;

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
    present_driver: Rc<dyn NativeSurfacePresentDriver>,
    retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    rect: NativeTerminalSurfaceRect,
    last_drawn_frame_token: u64,
    latest_diagnostics: NativeTerminalSurfaceDiagnostics,
    damage_tracker: NativeFrameDamageTracker,
    surface_alive: bool,
    host_redraw_sync_pending: bool,
    dirty: bool,
    pending_present: PendingPresentGate,
    pending_host_redraw: PendingPresentGate,
    scheduled_present_count: u64,
    host_redraw_request_count: u64,
    host_redraw_replay_count: u64,
}

#[derive(Default)]
struct PendingPresentGate {
    scheduled: bool,
}

impl PendingPresentGate {
    fn mark_scheduled(&mut self) -> bool {
        if self.scheduled {
            false
        } else {
            self.scheduled = true;
            true
        }
    }

    fn clear(&mut self) {
        self.scheduled = false;
    }
}

impl NativeTerminalSurfaceState {
    fn new(window: &AppWindow) -> Self {
        Self {
            backend: create_platform_native_surface_backend(),
            present_driver: Rc::new(EventLoopPresentDriver::new(window)),
            retained_frame: None,
            rect: NativeTerminalSurfaceRect::default(),
            last_drawn_frame_token: 0,
            latest_diagnostics: NativeTerminalSurfaceDiagnostics::default(),
            damage_tracker: NativeFrameDamageTracker::default(),
            surface_alive: true,
            host_redraw_sync_pending: false,
            dirty: false,
            pending_present: PendingPresentGate::default(),
            pending_host_redraw: PendingPresentGate::default(),
            scheduled_present_count: 0,
            host_redraw_request_count: 0,
            host_redraw_replay_count: 0,
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

    pub fn configure_present_path(
        &self,
        window: &AppWindow,
        native_present_path: NativePresentPath,
    ) {
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
                    state
                        .damage_tracker
                        .track_frame_damage(previous_frame.as_ref(), Some(&next_frame));
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
                state.present_driver = Rc::new(RenderingNotifierPresentDriver::new(window));
                refresh_diagnostics(&mut state);
            }
            Err(err) => {
                tracing::warn!(
                    target: "app.terminal",
                    error = %err,
                    "native terminal rendering notifier is unavailable; falling back to host-window after-draw present scheduling"
                );
                let mut state = self.state.borrow_mut();
                state.present_driver = Rc::new(EventLoopPresentDriver::new(window));
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

        let present_driver = {
            let mut state = self.state.borrow_mut();
            if !state.pending_present.mark_scheduled() {
                return;
            }
            state.scheduled_present_count = state.scheduled_present_count.saturating_add(1);
            let request_host_redraw = state.pending_host_redraw.mark_scheduled();
            if request_host_redraw {
                state.host_redraw_request_count = state.host_redraw_request_count.saturating_add(1);
            }
            // Clone the driver out of RefCell state before it can choose an
            // immediate callback path; otherwise the callback re-enters while
            // `self.state` is still borrowed and panics.
            (Rc::clone(&state.present_driver), request_host_redraw)
        };

        let (present_driver, request_host_redraw) = present_driver;
        present_driver.schedule_present(callback, request_host_redraw);
    }
}

fn draw_retained_frame(state: &mut NativeTerminalSurfaceState) {
    state.pending_present.clear();
    if !state.surface_alive {
        return;
    }
    if !state.dirty && !state.damage_tracker.has_damage() && !state.host_redraw_sync_pending {
        return;
    }
    let damage = state.damage_tracker.take_damage().unwrap_or_default();
    let damage = effective_present_damage(
        state.rect,
        damage,
        state.dirty,
        state.host_redraw_sync_pending,
    );
    if let Some(retained_frame) = state.retained_frame.as_ref() {
        state.last_drawn_frame_token = retained_frame.frame.frame_token;
    } else {
        state.last_drawn_frame_token = 0;
    }
    state.backend.present(damage);
    state.host_redraw_sync_pending = false;
    state.dirty = false;
    refresh_diagnostics(state);
}

fn replay_after_host_redraw(state: &mut NativeTerminalSurfaceState) {
    state.pending_host_redraw.clear();
    state.host_redraw_replay_count = state.host_redraw_replay_count.saturating_add(1);
    state.host_redraw_sync_pending = true;
    draw_retained_frame(state);
}

fn effective_present_damage(
    rect: NativeTerminalSurfaceRect,
    damage: NativeSurfaceDamage,
    dirty: bool,
    host_redraw_sync_pending: bool,
) -> NativeSurfaceDamage {
    if host_redraw_sync_pending || (dirty && matches!(damage.kind, NativeSurfaceDamageKind::None)) {
        NativeSurfaceDamage {
            kind: NativeSurfaceDamageKind::Full,
            rect,
        }
    } else {
        damage
    }
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
    state.host_redraw_sync_pending = false;
    state.pending_present.clear();
    state.pending_host_redraw.clear();
    state.backend.detach();
    refresh_diagnostics(state);
}

fn refresh_diagnostics(state: &mut NativeTerminalSurfaceState) {
    let mut diagnostics = state.backend.diagnostics_snapshot();
    diagnostics.scheduled_present_count = state.scheduled_present_count;
    diagnostics.host_redraw_request_count = state.host_redraw_request_count;
    diagnostics.host_redraw_replay_count = state.host_redraw_replay_count;
    state.latest_diagnostics = diagnostics;
}

#[cfg(test)]
mod tests {
    use super::{PendingPresentGate, effective_present_damage};
    use crate::app::terminal_renderer::damage::{NativeSurfaceDamage, NativeSurfaceDamageKind};
    use crate::app::terminal_renderer::platform::NativeTerminalSurfaceRect;

    #[test]
    fn pending_present_gate_coalesces_redundant_schedule_requests() {
        let mut gate = PendingPresentGate::default();

        assert!(gate.mark_scheduled());
        assert!(
            !gate.mark_scheduled(),
            "repeated schedule requests before the next draw should collapse into a single host redraw"
        );
    }

    #[test]
    fn pending_present_gate_reopens_after_clear() {
        let mut gate = PendingPresentGate::default();

        assert!(gate.mark_scheduled());
        gate.clear();
        assert!(
            gate.mark_scheduled(),
            "after a draw pass consumes the pending redraw, the next frame update should be able to schedule another present"
        );
    }

    #[test]
    fn host_redraw_sync_promotes_overlay_damage_to_full_repaint() {
        let rect = NativeTerminalSurfaceRect {
            x: 374,
            y: 92,
            width: 560,
            height: 552,
        };
        let overlay_damage = NativeSurfaceDamage {
            kind: NativeSurfaceDamageKind::OverlayOnly,
            rect: NativeTerminalSurfaceRect {
                x: 414,
                y: 136,
                width: 80,
                height: 44,
            },
        };

        assert_eq!(
            effective_present_damage(rect, overlay_damage, false, true),
            NativeSurfaceDamage {
                kind: NativeSurfaceDamageKind::Full,
                rect,
            },
            "after a host redraw, child-HWND native presents should repaint the full terminal surface instead of preserving an overlay-only clip that can leave the rest of the child area visually stale or transparent"
        );
    }

    #[test]
    fn dirty_present_without_damage_promotes_to_full_repaint() {
        let rect = NativeTerminalSurfaceRect {
            x: 374,
            y: 92,
            width: 560,
            height: 552,
        };

        assert_eq!(
            effective_present_damage(rect, NativeSurfaceDamage::default(), true, false),
            NativeSurfaceDamage {
                kind: NativeSurfaceDamageKind::Full,
                rect,
            },
            "dirty retained frames without a concrete damage payload should still repaint the full child HWND surface"
        );
    }

    #[test]
    fn overlay_damage_stays_scoped_without_host_redraw_sync() {
        let rect = NativeTerminalSurfaceRect {
            x: 374,
            y: 92,
            width: 560,
            height: 552,
        };
        let overlay_damage = NativeSurfaceDamage {
            kind: NativeSurfaceDamageKind::OverlayOnly,
            rect: NativeTerminalSurfaceRect {
                x: 414,
                y: 136,
                width: 80,
                height: 44,
            },
        };

        assert_eq!(
            effective_present_damage(rect, overlay_damage, false, false),
            overlay_damage,
            "when no host redraw replay is pending, overlay-only updates should keep their narrow damage rect"
        );
    }
}
