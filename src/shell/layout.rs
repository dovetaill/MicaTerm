//! Responsive layout policy for deciding when auxiliary shell panels may remain visible.

use crate::shell::metrics::ShellMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellLayoutInput {
    pub window_width: u32,
    pub request_assets_sidebar: bool,
    pub request_right_panel: bool,
    pub requested_assets_sidebar_width: u32,
    pub requested_right_panel_width: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellLayoutDecision {
    pub show_assets_sidebar: bool,
    pub show_right_panel: bool,
    pub main_workspace_width: u32,
}

fn clamped_assets_sidebar_width(width: u32) -> u32 {
    width.clamp(
        ShellMetrics::ASSETS_SIDEBAR_MIN_WIDTH,
        ShellMetrics::ASSETS_SIDEBAR_MAX_WIDTH,
    )
}

fn clamped_right_panel_width(width: u32) -> u32 {
    width.clamp(
        ShellMetrics::RIGHT_PANEL_MIN_WIDTH,
        ShellMetrics::RIGHT_PANEL_MAX_WIDTH,
    )
}

pub fn resolve_shell_layout(input: ShellLayoutInput) -> ShellLayoutDecision {
    let assets_sidebar_width = clamped_assets_sidebar_width(input.requested_assets_sidebar_width);
    let right_panel_width = clamped_right_panel_width(input.requested_right_panel_width);

    let mut show_assets_sidebar = input.request_assets_sidebar;
    let mut show_right_panel = input.request_right_panel;

    let fits_requested_layout = |show_assets_sidebar: bool, show_right_panel: bool| {
        let occupied = ShellMetrics::ACTIVITY_BAR_WIDTH
            + if show_assets_sidebar {
                assets_sidebar_width
            } else {
                0
            }
            + if show_right_panel {
                right_panel_width
            } else {
                0
            };
        input.window_width >= occupied + ShellMetrics::MAIN_WORKSPACE_MIN_WIDTH
    };

    if show_assets_sidebar && !fits_requested_layout(show_assets_sidebar, show_right_panel) {
        show_assets_sidebar = false;
    }

    if show_right_panel && !fits_requested_layout(show_assets_sidebar, show_right_panel) {
        show_right_panel = false;
    }

    let occupied = ShellMetrics::ACTIVITY_BAR_WIDTH
        + if show_assets_sidebar {
            assets_sidebar_width
        } else {
            0
        }
        + if show_right_panel {
            right_panel_width
        } else {
            0
        };

    ShellLayoutDecision {
        show_assets_sidebar,
        show_right_panel,
        main_workspace_width: input.window_width.saturating_sub(occupied),
    }
}
