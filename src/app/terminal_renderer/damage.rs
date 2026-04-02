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
                        || previous.frame.presentable_frame.underline_overlay
                            != next.frame.presentable_frame.underline_overlay
                        || previous.frame.presentable_frame.ime_preview_overlay
                            != next.frame.presentable_frame.ime_preview_overlay;
                if overlays_changed {
                    self.pending = Some(NativeSurfaceDamage {
                        kind: NativeSurfaceDamageKind::OverlayOnly,
                        rect: overlay_damage_rect(previous, next).unwrap_or(next.rect),
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

fn overlay_damage_rect(
    previous: &RetainedNativeTerminalSurfaceFrame,
    next: &RetainedNativeTerminalSurfaceFrame,
) -> Option<NativeTerminalSurfaceRect> {
    let mut damage = None;
    extend_overlay_damage_rect(&mut damage, previous);
    extend_overlay_damage_rect(&mut damage, next);
    damage
}

fn extend_overlay_damage_rect(
    damage: &mut Option<NativeTerminalSurfaceRect>,
    frame: &RetainedNativeTerminalSurfaceFrame,
) {
    let presentable = &frame.frame.presentable_frame;
    union_rect(damage, cursor_overlay_rect(frame.rect, presentable.cursor_overlay));

    if presentable.selection_overlay.active {
        for rect in &presentable.selection_overlay.rects {
            union_rect(
                damage,
                cell_span_rect(
                    frame.rect,
                    rect.row,
                    rect.start_col,
                    rect.end_col,
                    frame.frame.cell_width_px,
                    frame.frame.cell_height_px,
                ),
            );
        }
    }

    if presentable.underline_overlay.visible {
        for run in &presentable.underline_overlay.runs {
            union_rect(
                damage,
                cell_span_rect(
                    frame.rect,
                    run.row,
                    run.start_col,
                    run.end_col,
                    frame.frame.cell_width_px,
                    frame.frame.cell_height_px,
                ),
            );
        }
    }

    if presentable.ime_preview_overlay.active {
        union_rect(
            damage,
            cell_span_rect(
                frame.rect,
                presentable.ime_preview_overlay.row,
                presentable.ime_preview_overlay.start_col,
                presentable.ime_preview_overlay.end_col,
                frame.frame.cell_width_px,
                frame.frame.cell_height_px,
            ),
        );
    }
}

fn cursor_overlay_rect(
    surface_rect: NativeTerminalSurfaceRect,
    cursor: crate::app::terminal_presenter::NativeCursorOverlay,
) -> Option<NativeTerminalSurfaceRect> {
    if !cursor.visible {
        return None;
    }

    cell_span_rect(
        surface_rect,
        cursor.row,
        cursor.col,
        cursor.col,
        cursor.cell_width_px,
        cursor.cell_height_px,
    )
}

fn cell_span_rect(
    surface_rect: NativeTerminalSurfaceRect,
    row: u32,
    start_col: u32,
    end_col: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> Option<NativeTerminalSurfaceRect> {
    if cell_width_px == 0 || cell_height_px == 0 || surface_rect.width <= 0 || surface_rect.height <= 0
    {
        return None;
    }

    let left = surface_rect
        .x
        .saturating_add((start_col.saturating_mul(cell_width_px)) as i32);
    let top = surface_rect
        .y
        .saturating_add((row.saturating_mul(cell_height_px)) as i32);
    let right = surface_rect.x.saturating_add(
        (end_col.saturating_add(1).saturating_mul(cell_width_px)) as i32,
    );
    let bottom = surface_rect
        .y
        .saturating_add((row.saturating_add(1).saturating_mul(cell_height_px)) as i32);

    intersect_rect(
        surface_rect,
        NativeTerminalSurfaceRect {
            x: left,
            y: top,
            width: right.saturating_sub(left),
            height: bottom.saturating_sub(top),
        },
    )
}

fn union_rect(damage: &mut Option<NativeTerminalSurfaceRect>, next: Option<NativeTerminalSurfaceRect>) {
    let Some(next) = next else {
        return;
    };

    *damage = Some(match damage.take() {
        Some(current) => {
            let left = current.x.min(next.x);
            let top = current.y.min(next.y);
            let right = current
                .x
                .saturating_add(current.width)
                .max(next.x.saturating_add(next.width));
            let bottom = current
                .y
                .saturating_add(current.height)
                .max(next.y.saturating_add(next.height));
            NativeTerminalSurfaceRect {
                x: left,
                y: top,
                width: right.saturating_sub(left),
                height: bottom.saturating_sub(top),
            }
        }
        None => next,
    });
}

fn intersect_rect(
    surface_rect: NativeTerminalSurfaceRect,
    next: NativeTerminalSurfaceRect,
) -> Option<NativeTerminalSurfaceRect> {
    if surface_rect.width <= 0 || surface_rect.height <= 0 || next.width <= 0 || next.height <= 0 {
        return None;
    }

    let left = surface_rect.x.max(next.x);
    let top = surface_rect.y.max(next.y);
    let right = surface_rect
        .x
        .saturating_add(surface_rect.width)
        .min(next.x.saturating_add(next.width));
    let bottom = surface_rect
        .y
        .saturating_add(surface_rect.height)
        .min(next.y.saturating_add(next.height));

    (right > left && bottom > top).then_some(NativeTerminalSurfaceRect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    })
}
