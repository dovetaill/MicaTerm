use crate::app::ui_preferences::PersistedWindowBounds;
use crate::app::window_state::WindowPlacementKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorWorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl MonitorWorkArea {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedWindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn resolve_startup_bounds(
    saved: Option<PersistedWindowBounds>,
    desired_size: (u32, u32),
    monitors: &[MonitorWorkArea],
) -> Option<ResolvedWindowBounds> {
    let fallback_monitor = monitors.first().copied()?;

    if let Some(saved) = saved
        && let Some(monitor) = first_intersecting_monitor(saved, desired_size, monitors)
    {
        return Some(clamp_position_to_monitor(saved, desired_size, monitor));
    }

    Some(center_bounds_in_monitor(desired_size, fallback_monitor))
}

pub fn persisted_window_bounds_for_placement(
    placement: WindowPlacementKind,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    monitors: &[MonitorWorkArea],
) -> Option<PersistedWindowBounds> {
    if placement != WindowPlacementKind::Restored || width == 0 || height == 0 {
        return None;
    }

    if !bounds_fit_any_monitor(x, y, width, height, monitors) {
        return None;
    }

    Some(PersistedWindowBounds { x, y })
}

fn first_intersecting_monitor(
    saved: PersistedWindowBounds,
    desired_size: (u32, u32),
    monitors: &[MonitorWorkArea],
) -> Option<MonitorWorkArea> {
    monitors.iter().copied().find(|monitor| {
        bounds_intersects_monitor(
            saved.x,
            saved.y,
            desired_size.0,
            desired_size.1,
            *monitor,
        )
    })
}

fn bounds_intersects_monitor(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    monitor: MonitorWorkArea,
) -> bool {
    let bounds_right = x + width as i32;
    let bounds_bottom = y + height as i32;
    let monitor_right = monitor.x + monitor.width as i32;
    let monitor_bottom = monitor.y + monitor.height as i32;

    x < monitor_right
        && bounds_right > monitor.x
        && y < monitor_bottom
        && bounds_bottom > monitor.y
}

fn bounds_fit_any_monitor(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    monitors: &[MonitorWorkArea],
) -> bool {
    monitors
        .iter()
        .copied()
        .any(|monitor| bounds_fit_monitor(x, y, width, height, monitor))
}

fn bounds_fit_monitor(x: i32, y: i32, width: u32, height: u32, monitor: MonitorWorkArea) -> bool {
    let bounds_right = x + width as i32;
    let bounds_bottom = y + height as i32;
    let monitor_right = monitor.x + monitor.width as i32;
    let monitor_bottom = monitor.y + monitor.height as i32;

    x >= monitor.x
        && bounds_right <= monitor_right
        && y >= monitor.y
        && bounds_bottom <= monitor_bottom
}

fn clamp_position_to_monitor(
    saved: PersistedWindowBounds,
    desired_size: (u32, u32),
    monitor: MonitorWorkArea,
) -> ResolvedWindowBounds {
    let width = desired_size.0.min(monitor.width);
    let height = desired_size.1.min(monitor.height);
    let max_x = monitor.x + monitor.width.saturating_sub(width) as i32;
    let max_y = monitor.y + monitor.height.saturating_sub(height) as i32;

    ResolvedWindowBounds {
        x: saved.x.clamp(monitor.x, max_x),
        y: saved.y.clamp(monitor.y, max_y),
        width,
        height,
    }
}

fn center_bounds_in_monitor(
    desired_size: (u32, u32),
    monitor: MonitorWorkArea,
) -> ResolvedWindowBounds {
    let width = desired_size.0.min(monitor.width);
    let height = desired_size.1.min(monitor.height);
    let x = monitor.x + (monitor.width.saturating_sub(width) / 2) as i32;
    let y = monitor.y + (monitor.height.saturating_sub(height) / 2) as i32;

    ResolvedWindowBounds {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_startup_bounds_centers_first_launch() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];

        let resolved =
            resolve_startup_bounds(None, (1600, 960), &monitors).expect("resolved bounds");

        assert_eq!(resolved.width, 1600);
        assert_eq!(resolved.height, 960);
        assert_eq!(resolved.x, 160);
        assert_eq!(resolved.y, 40);
    }

    #[test]
    fn resolve_startup_bounds_rehomes_offscreen_saved_bounds() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];
        let saved = PersistedWindowBounds { x: 4000, y: 2800 };

        let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
            .expect("resolved bounds");

        assert_eq!(resolved.x, 160);
        assert_eq!(resolved.y, 40);
    }

    #[test]
    fn resolve_startup_bounds_keeps_visible_saved_position_and_uses_default_size() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];
        let saved = PersistedWindowBounds { x: 120, y: 80 };

        let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
            .expect("resolved bounds");

        assert_eq!(resolved.x, 120);
        assert_eq!(resolved.y, 80);
        assert_eq!(resolved.width, 1600);
        assert_eq!(resolved.height, 960);
    }

    #[test]
    fn resolve_startup_bounds_clamps_saved_position_into_work_area() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];
        let saved = PersistedWindowBounds { x: 600, y: 400 };

        let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
            .expect("resolved bounds");

        assert_eq!(resolved.x, 320);
        assert_eq!(resolved.y, 80);
        assert_eq!(resolved.width, 1600);
        assert_eq!(resolved.height, 960);
    }

    #[test]
    fn persisted_window_bounds_for_placement_skips_non_restored_states() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];

        assert_eq!(
            persisted_window_bounds_for_placement(
                WindowPlacementKind::Maximized,
                10,
                20,
                1200,
                800,
                &monitors,
            ),
            None
        );
        assert_eq!(
            persisted_window_bounds_for_placement(
                WindowPlacementKind::SnappedLeft,
                10,
                20,
                1200,
                800,
                &monitors,
            ),
            None
        );
    }

    #[test]
    fn persisted_window_bounds_for_placement_skips_partially_offscreen_restored_bounds() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];

        assert_eq!(
            persisted_window_bounds_for_placement(
                WindowPlacementKind::Restored,
                120,
                120,
                1800,
                980,
                &monitors,
            ),
            None
        );
    }

    #[test]
    fn persisted_window_bounds_for_placement_keeps_fully_visible_restored_position() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];

        assert_eq!(
            persisted_window_bounds_for_placement(
                WindowPlacementKind::Restored,
                160,
                40,
                1600,
                960,
                &monitors,
            ),
            Some(PersistedWindowBounds { x: 160, y: 40 })
        );
    }
}
