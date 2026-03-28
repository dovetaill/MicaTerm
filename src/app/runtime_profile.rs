//! Runtime profile descriptors for the supported build flavor and renderer stack.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppBuildFlavor {
    Development,
    WindowsMainline,
    WindowsSoftwareCompat,
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
    pub fn development() -> Self {
        Self {
            build_flavor: AppBuildFlavor::Development,
            renderer_mode: RendererMode::Software,
        }
    }

    pub fn mainline() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsMainline,
            renderer_mode: RendererMode::SkiaSoftware,
        }
    }

    pub fn software_compat() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsSoftwareCompat,
            renderer_mode: RendererMode::Software,
        }
    }

    pub fn packaged() -> Self {
        match (
            option_env!("MICA_TERM_BUILD_FLAVOR"),
            option_env!("MICA_TERM_PACKAGE_RENDERER"),
        ) {
            (Some("windows-mainline"), Some("skia-software")) => Self::mainline(),
            (Some("windows-software-compat"), Some("software")) => Self::software_compat(),
            _ => Self::development(),
        }
    }

    pub fn forced_backend(self) -> Option<&'static str> {
        Some("winit")
    }

    pub fn forced_renderer(self) -> Option<&'static str> {
        Some(match self.renderer_mode {
            RendererMode::Software => "software",
            RendererMode::SkiaSoftware => "skia-software",
        })
    }

    pub fn selector_label(self) -> &'static str {
        match self.renderer_mode {
            RendererMode::Software => "winit-software",
            RendererMode::SkiaSoftware => "winit-skia-software",
        }
    }
}
