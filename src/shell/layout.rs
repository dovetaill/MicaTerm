use crate::shell::metrics::ShellMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellLayoutInput {
    pub window_width: u32,
    pub request_assets_sidebar: bool,
    pub request_right_panel: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellLayoutDecision {
    pub show_assets_sidebar: bool,
    pub show_right_panel: bool,
    pub main_workspace_width: u32,
}

pub fn resolve_shell_layout(input: ShellLayoutInput) -> ShellLayoutDecision {
    let show_assets_sidebar =
        input.request_assets_sidebar && input.window_width >= ShellMetrics::FULL_LAYOUT_MIN_WIDTH;

    let right_panel_threshold = if show_assets_sidebar {
        ShellMetrics::FULL_LAYOUT_MIN_WIDTH
    } else {
        ShellMetrics::RIGHT_PANEL_ONLY_MIN_WIDTH
    };

    let show_right_panel = input.request_right_panel && input.window_width >= right_panel_threshold;

    let occupied = ShellMetrics::ACTIVITY_BAR_WIDTH
        + if show_assets_sidebar {
            ShellMetrics::ASSETS_SIDEBAR_WIDTH
        } else {
            0
        }
        + if show_right_panel {
            ShellMetrics::RIGHT_PANEL_WIDTH
        } else {
            0
        };

    ShellLayoutDecision {
        show_assets_sidebar,
        show_right_panel,
        main_workspace_width: input.window_width.saturating_sub(occupied),
    }
}
