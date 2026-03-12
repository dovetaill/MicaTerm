use anyhow::{Result, anyhow};
use slint::{ComponentHandle, PhysicalSize, Window};

use crate::shell::metrics::ShellMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialKind {
    MicaAlt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowAppearance {
    pub no_frame: bool,
    pub material: MaterialKind,
}

pub fn window_appearance() -> WindowAppearance {
    WindowAppearance {
        no_frame: true,
        material: MaterialKind::MicaAlt,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowCommandSpec {
    pub uses_winit_drag: bool,
    pub uses_winit_drag_resize: bool,
    pub self_drawn_controls: bool,
    pub supports_double_click_maximize: bool,
    pub supports_always_on_top: bool,
    pub supports_true_window_state_tracking: bool,
    pub supports_native_frame_adapter: bool,
    pub resize_border_width: u32,
    pub min_window_width: u32,
    pub min_window_height: u32,
}

pub fn window_command_spec() -> WindowCommandSpec {
    WindowCommandSpec {
        uses_winit_drag: true,
        uses_winit_drag_resize: true,
        self_drawn_controls: true,
        supports_double_click_maximize: true,
        supports_always_on_top: true,
        supports_true_window_state_tracking: true,
        supports_native_frame_adapter: true,
        resize_border_width: 6,
        min_window_width: ShellMetrics::WINDOW_MIN_WIDTH,
        min_window_height: ShellMetrics::WINDOW_MIN_HEIGHT,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowResizeDirection {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

pub fn parse_resize_direction(value: &str) -> Option<WindowResizeDirection> {
    match value {
        "north" => Some(WindowResizeDirection::North),
        "south" => Some(WindowResizeDirection::South),
        "east" => Some(WindowResizeDirection::East),
        "west" => Some(WindowResizeDirection::West),
        "north-east" => Some(WindowResizeDirection::NorthEast),
        "north-west" => Some(WindowResizeDirection::NorthWest),
        "south-east" => Some(WindowResizeDirection::SouthEast),
        "south-west" => Some(WindowResizeDirection::SouthWest),
        _ => None,
    }
}

pub fn next_maximize_state(is_maximized: bool) -> bool {
    !is_maximized
}

pub fn apply_restored_window_size<C: ComponentHandle>(component: &C, size: (u32, u32)) {
    component
        .window()
        .set_size(PhysicalSize::new(size.0, size.1));
}

pub struct WindowController<C: ComponentHandle> {
    component: slint::Weak<C>,
}

impl<C: ComponentHandle> WindowController<C> {
    pub fn new(component: &C) -> Self {
        Self {
            component: component.as_weak(),
        }
    }

    fn with_window<T>(&self, callback: impl FnOnce(&Window) -> T) -> Option<T> {
        self.component
            .upgrade()
            .map(|component| callback(component.window()))
    }

    pub fn minimize(&self) {
        let _ = self.with_window(|window| {
            window.set_minimized(true);
        });
    }

    pub fn toggle_maximize(&self, current: bool) -> bool {
        let next = next_maximize_state(current);
        let _ = self.with_window(|window| {
            window.set_maximized(next);
        });
        next
    }

    pub fn close(&self) -> Result<()> {
        self.with_window(|window| window.hide().map_err(|err| anyhow!(err.to_string())))
            .unwrap_or_else(|| Err(anyhow!("window is unavailable")))
    }

    pub fn drag(&self) -> Result<()> {
        use slint::winit_030::{WinitWindowAccessor, winit};

        self.with_window(|window| {
            window
                .with_winit_window(|window: &winit::window::Window| {
                    window.drag_window().map_err(|err| anyhow!(err.to_string()))
                })
                .unwrap_or_else(|| Err(anyhow!("winit window is unavailable")))
        })
        .unwrap_or_else(|| Err(anyhow!("window is unavailable")))
    }

    pub fn drag_resize(&self, direction: WindowResizeDirection) -> Result<()> {
        use slint::winit_030::{WinitWindowAccessor, winit};

        let mapped = match direction {
            WindowResizeDirection::North => winit::window::ResizeDirection::North,
            WindowResizeDirection::South => winit::window::ResizeDirection::South,
            WindowResizeDirection::East => winit::window::ResizeDirection::East,
            WindowResizeDirection::West => winit::window::ResizeDirection::West,
            WindowResizeDirection::NorthEast => winit::window::ResizeDirection::NorthEast,
            WindowResizeDirection::NorthWest => winit::window::ResizeDirection::NorthWest,
            WindowResizeDirection::SouthEast => winit::window::ResizeDirection::SouthEast,
            WindowResizeDirection::SouthWest => winit::window::ResizeDirection::SouthWest,
        };

        self.with_window(|window| {
            window
                .with_winit_window(|window: &winit::window::Window| {
                    window
                        .drag_resize_window(mapped)
                        .map_err(|err| anyhow!(err.to_string()))
                })
                .unwrap_or_else(|| Err(anyhow!("winit window is unavailable")))
        })
        .unwrap_or_else(|| Err(anyhow!("window is unavailable")))
    }
}
