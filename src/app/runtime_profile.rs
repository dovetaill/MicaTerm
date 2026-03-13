#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppBuildFlavor {
    Formal,
    FemtoVgWgpuExperimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererMode {
    Software,
    FemtoVgWgpu,
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

    pub fn femtovg_wgpu_experimental() -> Self {
        Self {
            build_flavor: AppBuildFlavor::FemtoVgWgpuExperimental,
            renderer_mode: RendererMode::FemtoVgWgpu,
        }
    }

    pub fn is_experimental(self) -> bool {
        matches!(self.build_flavor, AppBuildFlavor::FemtoVgWgpuExperimental)
    }

    pub fn requires_backend_lock(self) -> bool {
        self.forced_backend().is_some()
    }

    pub fn forced_backend(self) -> Option<&'static str> {
        match self.renderer_mode {
            RendererMode::Software => None,
            RendererMode::FemtoVgWgpu => Some("winit"),
        }
    }

    pub fn forced_renderer(self) -> Option<&'static str> {
        match self.renderer_mode {
            RendererMode::Software => None,
            RendererMode::FemtoVgWgpu => Some("femtovg-wgpu"),
        }
    }

    pub fn requires_wgpu_28(self) -> bool {
        matches!(self.renderer_mode, RendererMode::FemtoVgWgpu)
    }
}
