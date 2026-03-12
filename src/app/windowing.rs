use anyhow::{Result, anyhow};
use slint::{ComponentHandle, PhysicalSize, Window};

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
    pub self_drawn_controls: bool,
    pub supports_double_click_maximize: bool,
    pub supports_always_on_top: bool,
    pub supports_true_window_state_tracking: bool,
    pub supports_native_frame_adapter: bool,
    pub resize_border_width: u32,
}

pub fn window_command_spec() -> WindowCommandSpec {
    WindowCommandSpec {
        uses_winit_drag: true,
        self_drawn_controls: true,
        supports_double_click_maximize: true,
        supports_always_on_top: true,
        supports_true_window_state_tracking: true,
        supports_native_frame_adapter: true,
        resize_border_width: 6,
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
}
