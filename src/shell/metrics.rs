pub struct ShellMetrics;

impl ShellMetrics {
    pub const TITLEBAR_HEIGHT: u32 = 48;
    pub const TITLEBAR_NAV_WIDTH: u32 = 44;
    pub const TITLEBAR_BRAND_WIDTH: u32 = 188;
    pub const TITLEBAR_UTILITY_WIDTH: u32 = 136;
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
    pub const ASSETS_SIDEBAR_WIDTH: u32 = 256;
    pub const ASSETS_SIDEBAR_HEADER_HEIGHT: u32 = 44;
    pub const ASSETS_SIDEBAR_SECTION_GAP: u32 = 12;
    pub const TAB_BAR_HEIGHT: u32 = 38;
    pub const RIGHT_PANEL_WIDTH: u32 = 392;
    pub const BASE_SPACING: u32 = 8;
}
