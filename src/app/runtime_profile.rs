#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppBuildFlavor {
    Formal,
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
    pub fn formal() -> Self {
        Self {
            build_flavor: AppBuildFlavor::Formal,
            renderer_mode: RendererMode::Software,
        }
    }

    pub fn is_experimental(self) -> bool {
        false
    }

    pub fn requires_backend_lock(self) -> bool {
        false
    }

    pub fn forced_backend(self) -> Option<&'static str> {
        None
    }
}
