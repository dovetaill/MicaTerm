#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppBuildFlavor {
    Formal,
    SkiaExperimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererMode {
    Software,
    SkiaSoftware,
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

    pub fn skia_experimental() -> Self {
        Self {
            build_flavor: AppBuildFlavor::SkiaExperimental,
            renderer_mode: RendererMode::SkiaSoftware,
        }
    }

    pub fn is_experimental(self) -> bool {
        matches!(self.build_flavor, AppBuildFlavor::SkiaExperimental)
    }

    pub fn requires_backend_lock(self) -> bool {
        self.is_experimental()
    }

    pub fn forced_backend(self) -> Option<&'static str> {
        match self.renderer_mode {
            RendererMode::Software => None,
            RendererMode::SkiaSoftware => Some("winit-skia-software"),
        }
    }
}
