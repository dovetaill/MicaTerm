//! Runtime profile descriptors for the supported build flavor and renderer stack.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppBuildFlavor {
    Mainline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererMode {
    Software,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppRuntimeProfile {
    pub build_flavor: AppBuildFlavor,
    pub renderer_mode: RendererMode,
}

impl AppRuntimeProfile {
    pub fn mainline() -> Self {
        Self {
            build_flavor: AppBuildFlavor::Mainline,
            renderer_mode: RendererMode::Software,
        }
    }

    pub fn forced_backend(self) -> Option<&'static str> {
        Some("winit")
    }

    pub fn forced_renderer(self) -> Option<&'static str> {
        Some("software")
    }
}
