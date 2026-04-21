//! Shared geometry constants that keep Rust layout rules and Slint sizing aligned.

pub struct ShellMetrics;

impl ShellMetrics {
    pub const WINDOW_DEFAULT_WIDTH: u32 = 1440;
    pub const WINDOW_DEFAULT_HEIGHT: u32 = 900;
    pub const WINDOW_MIN_HEIGHT: u32 = 640;
    pub const MAIN_WORKSPACE_MIN_WIDTH: u32 = 640;
    pub const TITLEBAR_HEIGHT: u32 = 48;
    pub const TITLEBAR_NAV_WIDTH: u32 = 44;
    pub const TITLEBAR_BRAND_WIDTH: u32 = 188;
    pub const TITLEBAR_UTILITY_WIDTH: u32 = 220;
    pub const TITLEBAR_WINDOW_CONTROL_WIDTH: u32 = 138;
    pub const TITLEBAR_MIN_DRAG_WIDTH: u32 = 96;
    pub const TITLEBAR_TOOL_BUTTON_SIZE: u32 = 36;
    pub const TITLEBAR_TOOL_ICON_SIZE: u32 = 20;
    pub const TITLEBAR_TOOLTIP_DELAY_MS: u32 = 280;
    pub const TITLEBAR_TOOLTIP_CLOSE_DEBOUNCE_MS: u32 = 80;
    pub const TITLEBAR_TOOLTIP_OFFSET_Y: u32 = 8;
    pub const TITLEBAR_TOOLTIP_MIN_WIDTH: u32 = 96;
    pub const ACTIVITY_BAR_WIDTH: u32 = 48;
    pub const ACTIVITY_BAR_BUTTON_SIZE: u32 = 36;
    pub const ACTIVITY_BAR_ICON_SIZE: u32 = 20;
    pub const ACTIVITY_BAR_DIVIDER_WIDTH: u32 = 1;
    pub const ACTIVITY_BAR_DIVIDER_HEIGHT: u32 = 20;
    pub const ASSETS_SIDEBAR_DEFAULT_WIDTH: u32 = 320;
    pub const ASSETS_SIDEBAR_MIN_WIDTH: u32 = 240;
    pub const ASSETS_SIDEBAR_MAX_WIDTH: u32 = 420;
    pub const ASSETS_SIDEBAR_COLLAPSE_THRESHOLD: u32 = 180;
    pub const ASSETS_SIDEBAR_WIDTH: u32 = Self::ASSETS_SIDEBAR_DEFAULT_WIDTH;
    pub const ASSETS_SIDEBAR_HEADER_HEIGHT: u32 = 44;
    pub const ASSETS_TOOLBAR_HEIGHT: u32 = 44;
    pub const ASSETS_TOOLBAR_BUTTON_SIZE: u32 = 28;
    pub const ASSETS_SEARCH_ROW_HEIGHT: u32 = 40;
    pub const ASSETS_SIDEBAR_SECTION_GAP: u32 = 12;
    pub const TAB_BAR_HEIGHT: u32 = 38;
    pub const RIGHT_PANEL_DEFAULT_WIDTH: u32 = 392;
    pub const RIGHT_PANEL_MIN_WIDTH: u32 = 320;
    pub const RIGHT_PANEL_MAX_WIDTH: u32 = 520;
    pub const RIGHT_PANEL_COLLAPSE_THRESHOLD: u32 = 220;
    pub const RIGHT_PANEL_WIDTH: u32 = Self::RIGHT_PANEL_DEFAULT_WIDTH;
    pub const BASE_SPACING: u32 = 8;
    pub const FULL_LAYOUT_MIN_WIDTH: u32 = Self::ACTIVITY_BAR_WIDTH
        + Self::ASSETS_SIDEBAR_DEFAULT_WIDTH
        + Self::MAIN_WORKSPACE_MIN_WIDTH
        + Self::RIGHT_PANEL_DEFAULT_WIDTH;
    pub const RIGHT_PANEL_ONLY_MIN_WIDTH: u32 =
        Self::ACTIVITY_BAR_WIDTH + Self::MAIN_WORKSPACE_MIN_WIDTH + Self::RIGHT_PANEL_DEFAULT_WIDTH;
    pub const WINDOW_MIN_WIDTH: u32 = Self::ACTIVITY_BAR_WIDTH + Self::MAIN_WORKSPACE_MIN_WIDTH;
}
