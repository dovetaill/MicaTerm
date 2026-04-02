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
pub enum NativePresentPath {
    RenderingNotifier,
    EventLoop,
}

impl NativePresentPath {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RenderingNotifier => "rendering-notifier",
            Self::EventLoop => "event-loop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppRuntimeProfile {
    pub build_flavor: AppBuildFlavor,
    pub renderer_mode: RendererMode,
    pub terminal_render_mode: TerminalRenderMode,
    pub native_present_path: NativePresentPath,
}

impl AppRuntimeProfile {
    pub fn development() -> Self {
        Self {
            build_flavor: AppBuildFlavor::Development,
            renderer_mode: RendererMode::Software,
            terminal_render_mode: TerminalRenderMode::Native,
            native_present_path: NativePresentPath::EventLoop,
        }
    }

    pub fn mainline() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsMainline,
            renderer_mode: RendererMode::SkiaSoftware,
            terminal_render_mode: TerminalRenderMode::Native,
            native_present_path: NativePresentPath::RenderingNotifier,
        }
    }

    /// Preferred Windows shipping profile for packaged mainline builds.
    /// Preferred native-only shipping profile for packaged mainline builds.
    pub fn mainline_native() -> Self {
        Self::mainline()
    }

    /// Transitional non-shipping software profile while native Linux terminal surfaces are still landing.
    /// fallback-only compatibility profile for software packages that still need the bitmap presenter path.
    pub fn software_compat() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsSoftwareCompat,
            renderer_mode: RendererMode::Software,
            terminal_render_mode: TerminalRenderMode::Bitmap,
            native_present_path: NativePresentPath::EventLoop,
        }
    }

    pub fn packaged() -> Self {
        let build_flavor = option_env!("MICA_TERM_BUILD_FLAVOR");
        let renderer = option_env!("MICA_TERM_PACKAGE_RENDERER");
        let terminal_renderer = option_env!("MICA_TERM_PACKAGE_TERMINAL_RENDERER");
        let native_present_path = option_env!("MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH");

        match (build_flavor, renderer, terminal_renderer) {
            (Some("windows-mainline"), Some("skia-software"), _) => {
                let mut profile = Self::mainline_native();
                if let Some(path) = native_present_path.and_then(Self::parse_native_present_path) {
                    profile.native_present_path = path;
                }
                profile
            }
            (Some("windows-software-compat"), Some("software"), Some("bitmap")) => {
                let mut profile = Self::software_compat();
                if let Some(path) = native_present_path.and_then(Self::parse_native_present_path) {
                    profile.native_present_path = path;
                }
                profile
            }
            _ => Self::development(),
        }
    }

    fn parse_native_present_path(raw: &'static str) -> Option<NativePresentPath> {
        match raw {
            "rendering-notifier" => Some(NativePresentPath::RenderingNotifier),
            "event-loop" => Some(NativePresentPath::EventLoop),
            _ => None,
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

    pub fn native_present_path(self) -> NativePresentPath {
        self.native_present_path
    }

    pub fn prefers_native_terminal_renderer(self) -> bool {
        matches!(
            self.build_flavor,
            AppBuildFlavor::Development
                | AppBuildFlavor::WindowsMainline
                | AppBuildFlavor::WindowsSoftwareCompat
        ) && matches!(self.terminal_render_mode, TerminalRenderMode::Native)
    }

    pub fn terminal_render_mode_label(self) -> &'static str {
        self.terminal_render_mode().as_str()
    }

    pub fn native_present_path_label(self) -> &'static str {
        self.native_present_path().as_str()
    }
}
