#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowPlacementKind {
    Restored,
    Maximized,
    SnappedLeft,
    SnappedRight,
    SnappedTop,
    SnappedBottom,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowChromeMode {
    Rounded,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl WindowPlacementKind {
    pub fn chrome_mode(self) -> WindowChromeMode {
        match self {
            Self::Restored | Self::Unknown => WindowChromeMode::Rounded,
            Self::Maximized
            | Self::SnappedLeft
            | Self::SnappedRight
            | Self::SnappedTop
            | Self::SnappedBottom => WindowChromeMode::Flat,
        }
    }

    pub fn is_maximized(self) -> bool {
        matches!(self, Self::Maximized)
    }
}

pub fn classify_window_placement(
    window_rect: Rect,
    work_area: Rect,
    maximized: bool,
) -> WindowPlacementKind {
    if maximized {
        return WindowPlacementKind::Maximized;
    }

    let half_width = work_area.width / 2;
    let half_height = work_area.height / 2;

    if window_rect.x == work_area.x
        && window_rect.y == work_area.y
        && window_rect.width == half_width
        && window_rect.height == work_area.height
    {
        return WindowPlacementKind::SnappedLeft;
    }

    if window_rect.x == work_area.x + half_width as i32
        && window_rect.y == work_area.y
        && window_rect.width == half_width
        && window_rect.height == work_area.height
    {
        return WindowPlacementKind::SnappedRight;
    }

    if window_rect.x == work_area.x
        && window_rect.y == work_area.y
        && window_rect.width == work_area.width
        && window_rect.height == half_height
    {
        return WindowPlacementKind::SnappedTop;
    }

    if window_rect.x == work_area.x
        && window_rect.y == work_area.y + half_height as i32
        && window_rect.width == work_area.width
        && window_rect.height == half_height
    {
        return WindowPlacementKind::SnappedBottom;
    }

    WindowPlacementKind::Restored
}
