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
pub enum TerminalRenderMode {
    Bitmap,
    Native,
}

impl TerminalRenderMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bitmap => "bitmap",
            Self::Native => "native",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppRuntimeProfile {
    pub build_flavor: AppBuildFlavor,
    pub renderer_mode: RendererMode,
    pub terminal_render_mode: TerminalRenderMode,
}

impl AppRuntimeProfile {
    pub fn development() -> Self {
        Self {
            build_flavor: AppBuildFlavor::Development,
            renderer_mode: RendererMode::Software,
            terminal_render_mode: TerminalRenderMode::Bitmap,
        }
    }

    pub fn mainline() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsMainline,
            renderer_mode: RendererMode::SkiaSoftware,
            terminal_render_mode: TerminalRenderMode::Bitmap,
        }
    }

    /// Preferred Windows shipping profile for packaged mainline builds.
    pub fn mainline_native() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsMainline,
            renderer_mode: RendererMode::SkiaSoftware,
            terminal_render_mode: TerminalRenderMode::Native,
        }
    }

    /// Bitmap fallback-only compatibility profile for software packages.
    pub fn software_compat() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsSoftwareCompat,
            renderer_mode: RendererMode::Software,
            terminal_render_mode: TerminalRenderMode::Bitmap,
        }
    }

    pub fn packaged() -> Self {
        match (
            option_env!("MICA_TERM_BUILD_FLAVOR"),
            option_env!("MICA_TERM_PACKAGE_RENDERER"),
            option_env!("MICA_TERM_PACKAGE_TERMINAL_RENDERER"),
        ) {
            (Some("windows-mainline"), Some("skia-software"), Some("native")) => {
                Self::mainline_native()
            }
            (Some("windows-mainline"), Some("skia-software"), Some("bitmap") | None) => {
                Self::mainline()
            }
            (Some("windows-software-compat"), Some("software"), _) => Self::software_compat(),
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

    pub fn terminal_render_mode(self) -> TerminalRenderMode {
        self.terminal_render_mode
    }

    pub fn prefers_native_terminal_renderer(self) -> bool {
        matches!(self.build_flavor, AppBuildFlavor::WindowsMainline)
            || matches!(self.terminal_render_mode, TerminalRenderMode::Native)
    }

    pub fn terminal_render_mode_label(self) -> &'static str {
        self.terminal_render_mode().as_str()
    }
}
