use crate::app::window_state::WindowPlacementKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowVisibilitySnapshot {
    pub total_area: u64,
    pub visible_area: u64,
}

impl WindowVisibilitySnapshot {
    pub const fn new(total_area: u64, visible_area: u64) -> Self {
        Self {
            total_area,
            visible_area,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRecoveryAction {
    None,
    RequestRedraw,
    NudgeWindowSize { width: u32, height: u32 },
    RestoreWindowSize { width: u32, height: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingWindowSizeRestore {
    nudged_width: u32,
    nudged_height: u32,
    restore_width: u32,
    restore_height: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WindowRecoveryController {
    pending_visible_area: Option<u64>,
    pending_restore_size: Option<PendingWindowSizeRestore>,
}

impl WindowRecoveryController {
    pub fn arm_visibility_recovery(&mut self, snapshot: WindowVisibilitySnapshot) {
        self.pending_restore_size = None;
        self.pending_visible_area =
            (snapshot.total_area > 0 && snapshot.visible_area < snapshot.total_area)
                .then_some(snapshot.visible_area);
    }

    pub fn on_placement_changed(
        &mut self,
        previous: WindowPlacementKind,
        next: WindowPlacementKind,
        snapshot: WindowVisibilitySnapshot,
        _width: u32,
        _height: u32,
    ) -> WindowRecoveryAction {
        self.pending_restore_size = None;

        if previous == next {
            return WindowRecoveryAction::None;
        }

        if is_recoverable_placement(previous) && next == WindowPlacementKind::Restored {
            self.arm_visibility_recovery(snapshot);
            return WindowRecoveryAction::RequestRedraw;
        }

        self.pending_visible_area = None;
        WindowRecoveryAction::None
    }

    pub fn on_visibility_changed(
        &mut self,
        snapshot: WindowVisibilitySnapshot,
        width: u32,
        height: u32,
    ) -> WindowRecoveryAction {
        if self.pending_restore_size.is_some() {
            return WindowRecoveryAction::None;
        }

        let Some(previous_visible_area) = self.pending_visible_area else {
            return WindowRecoveryAction::None;
        };

        if snapshot.visible_area <= previous_visible_area {
            return WindowRecoveryAction::None;
        }

        self.pending_visible_area =
            (snapshot.visible_area < snapshot.total_area).then_some(snapshot.visible_area);

        let Some((nudged_width, nudged_height)) = nudged_window_size(width, height) else {
            return WindowRecoveryAction::None;
        };

        self.pending_restore_size = Some(PendingWindowSizeRestore {
            nudged_width,
            nudged_height,
            restore_width: width,
            restore_height: height,
        });

        WindowRecoveryAction::NudgeWindowSize {
            width: nudged_width,
            height: nudged_height,
        }
    }

    pub fn on_resize_ack(&mut self, width: u32, height: u32) -> WindowRecoveryAction {
        let Some(pending_restore) = self.pending_restore_size else {
            return WindowRecoveryAction::None;
        };

        if width == pending_restore.nudged_width && height == pending_restore.nudged_height {
            self.pending_restore_size = None;
            return WindowRecoveryAction::RestoreWindowSize {
                width: pending_restore.restore_width,
                height: pending_restore.restore_height,
            };
        }

        if width != pending_restore.restore_width || height != pending_restore.restore_height {
            self.pending_restore_size = None;
        }

        WindowRecoveryAction::None
    }
}

fn is_recoverable_placement(placement: WindowPlacementKind) -> bool {
    matches!(
        placement,
        WindowPlacementKind::Maximized
            | WindowPlacementKind::SnappedLeft
            | WindowPlacementKind::SnappedRight
            | WindowPlacementKind::SnappedTop
            | WindowPlacementKind::SnappedBottom
    )
}

fn nudged_window_size(window_width: u32, window_height: u32) -> Option<(u32, u32)> {
    if let Some(width) = window_width.checked_add(1) {
        return Some((width, window_height));
    }

    window_height
        .checked_add(1)
        .map(|height| (window_width, height))
}
