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

pub fn resolve_startup_bounds(
    saved: Option<PersistedWindowBounds>,
    desired_size: (u32, u32),
    monitors: &[MonitorWorkArea],
) -> Option<PersistedWindowBounds> {
    let fallback_monitor = monitors.first().copied()?;

    if let Some(saved) = saved
        && let Some(monitor) = first_intersecting_monitor(saved, monitors)
    {
        return Some(clamp_bounds_to_monitor(saved, monitor));
    }

    Some(center_bounds_in_monitor(desired_size, fallback_monitor))
}

pub fn persisted_window_bounds_for_placement(
    placement: WindowPlacementKind,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<PersistedWindowBounds> {
    if placement != WindowPlacementKind::Restored || width == 0 || height == 0 {
        return None;
    }

    Some(PersistedWindowBounds {
        x,
        y,
        width,
        height,
    })
}

fn first_intersecting_monitor(
    bounds: PersistedWindowBounds,
    monitors: &[MonitorWorkArea],
) -> Option<MonitorWorkArea> {
    monitors
        .iter()
        .copied()
        .find(|monitor| bounds_intersects_monitor(bounds, *monitor))
}

fn bounds_intersects_monitor(bounds: PersistedWindowBounds, monitor: MonitorWorkArea) -> bool {
    let bounds_right = bounds.x + bounds.width as i32;
    let bounds_bottom = bounds.y + bounds.height as i32;
    let monitor_right = monitor.x + monitor.width as i32;
    let monitor_bottom = monitor.y + monitor.height as i32;

    bounds.x < monitor_right
        && bounds_right > monitor.x
        && bounds.y < monitor_bottom
        && bounds_bottom > monitor.y
}

fn clamp_bounds_to_monitor(
    bounds: PersistedWindowBounds,
    monitor: MonitorWorkArea,
) -> PersistedWindowBounds {
    let width = bounds.width.min(monitor.width);
    let height = bounds.height.min(monitor.height);
    let max_x = monitor.x + monitor.width.saturating_sub(width) as i32;
    let max_y = monitor.y + monitor.height.saturating_sub(height) as i32;

    PersistedWindowBounds {
        x: bounds.x.clamp(monitor.x, max_x),
        y: bounds.y.clamp(monitor.y, max_y),
        width,
        height,
    }
}

fn center_bounds_in_monitor(
    desired_size: (u32, u32),
    monitor: MonitorWorkArea,
) -> PersistedWindowBounds {
    let width = desired_size.0.min(monitor.width);
    let height = desired_size.1.min(monitor.height);
    let x = monitor.x + (monitor.width.saturating_sub(width) / 2) as i32;
    let y = monitor.y + (monitor.height.saturating_sub(height) / 2) as i32;

    PersistedWindowBounds {
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
        let saved = PersistedWindowBounds {
            x: 4000,
            y: 2800,
            width: 1600,
            height: 960,
        };

        let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
            .expect("resolved bounds");

        assert_eq!(resolved.x, 160);
        assert_eq!(resolved.y, 40);
    }

    #[test]
    fn resolve_startup_bounds_keeps_visible_saved_bounds() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];
        let saved = PersistedWindowBounds {
            x: 120,
            y: 80,
            width: 1500,
            height: 900,
        };

        let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
            .expect("resolved bounds");

        assert_eq!(resolved, saved);
    }

    #[test]
    fn resolve_startup_bounds_clamps_saved_bounds_into_work_area() {
        let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];
        let saved = PersistedWindowBounds {
            x: 600,
            y: 400,
            width: 1800,
            height: 980,
        };

        let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
            .expect("resolved bounds");

        assert_eq!(resolved.x, 120);
        assert_eq!(resolved.y, 60);
        assert_eq!(resolved.width, 1800);
        assert_eq!(resolved.height, 980);
    }

    #[test]
    fn persisted_window_bounds_for_placement_skips_non_restored_states() {
        assert_eq!(
            persisted_window_bounds_for_placement(
                WindowPlacementKind::Maximized,
                10,
                20,
                1200,
                800,
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
            ),
            None
        );
    }

    #[test]
    fn persisted_window_bounds_for_placement_keeps_restored_bounds() {
        assert_eq!(
            persisted_window_bounds_for_placement(
                WindowPlacementKind::Restored,
                160,
                120,
                1680,
                980,
            ),
            Some(PersistedWindowBounds {
                x: 160,
                y: 120,
                width: 1680,
                height: 980,
            })
        );
    }
}
