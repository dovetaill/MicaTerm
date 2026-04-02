//! Damage tracking helpers for retained native terminal surface presents.

use super::platform::{NativeTerminalSurfaceRect, RetainedNativeTerminalSurfaceFrame};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeSurfaceDamageKind {
    #[default]
    None,
    OverlayOnly,
    Full,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeSurfaceDamage {
    pub kind: NativeSurfaceDamageKind,
    pub rect: NativeTerminalSurfaceRect,
}

#[derive(Clone, Debug, Default)]
pub struct NativeFrameDamageTracker {
    pending: Option<NativeSurfaceDamage>,
}

impl NativeFrameDamageTracker {
    pub fn mark_full_damage(&mut self, rect: NativeTerminalSurfaceRect) {
        self.pending = Some(NativeSurfaceDamage {
            kind: NativeSurfaceDamageKind::Full,
            rect,
        });
    }

    pub fn track_frame_damage(
        &mut self,
        previous: Option<&RetainedNativeTerminalSurfaceFrame>,
        next: Option<&RetainedNativeTerminalSurfaceFrame>,
    ) {
        match (previous, next) {
            (Some(previous), Some(next)) if previous == next => {}
            (Some(previous), Some(next))
                if previous.rect == next.rect
                    && previous.frame.frame_token == next.frame.frame_token =>
            {
                let overlays_changed =
                    previous.frame.presentable_frame.cursor_overlay
                        != next.frame.presentable_frame.cursor_overlay
                        || previous.frame.presentable_frame.selection_overlay
                            != next.frame.presentable_frame.selection_overlay
                        || previous.frame.presentable_frame.ime_preview_overlay
                            != next.frame.presentable_frame.ime_preview_overlay;
                if overlays_changed {
                    self.pending = Some(NativeSurfaceDamage {
                        kind: NativeSurfaceDamageKind::OverlayOnly,
                        rect: next.rect,
                    });
                } else {
                    self.mark_full_damage(next.rect);
                }
            }
            (Some(_), Some(next)) => self.mark_full_damage(next.rect),
            (None, Some(next)) => self.mark_full_damage(next.rect),
            (Some(previous), None) => self.mark_full_damage(previous.rect),
            (None, None) => self.clear(),
        }
    }

    pub fn has_damage(&self) -> bool {
        self.pending.is_some()
    }

    pub fn take_damage(&mut self) -> Option<NativeSurfaceDamage> {
        self.pending.take()
    }

    pub fn clear(&mut self) {
        self.pending = None;
    }
}
