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
    Skia,
    SkiaSoftware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRenderMode {
    Bitmap,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentPath {
    EventLoop,
    RenderingNotifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsApiRequirement {
    Direct3D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCompositionMode {
    SceneImage,
    PostRenderNativeSurface,
}

impl TerminalRenderMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bitmap => "bitmap",
            Self::Native => "native",
        }
    }
}

impl NativePresentPath {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EventLoop => "event-loop",
            Self::RenderingNotifier => "rendering-notifier",
        }
    }
}

impl GraphicsApiRequirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct3D => "direct3d",
        }
    }
}

impl RendererMode {
    pub fn renderer_name(self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::Skia => "skia",
            Self::SkiaSoftware => "skia-software",
        }
    }

    pub fn selector_label(self) -> &'static str {
        match self {
            Self::Software => "winit-software",
            Self::Skia => "winit-skia",
            Self::SkiaSoftware => "winit-skia-software",
        }
    }
}

const WINDOWS_MAINLINE_RENDERER_FALLBACK_CHAIN: &[RendererMode] = &[
    RendererMode::Skia,
    RendererMode::SkiaSoftware,
    RendererMode::Software,
];
const SOFTWARE_RENDERER_FALLBACK_CHAIN: &[RendererMode] = &[RendererMode::Software];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppRuntimeProfile {
    pub build_flavor: AppBuildFlavor,
    pub renderer_mode: RendererMode,
    pub terminal_render_mode: TerminalRenderMode,
    native_present_path: NativePresentPath,
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
            renderer_mode: RendererMode::Skia,
            terminal_render_mode: TerminalRenderMode::Native,
            native_present_path: NativePresentPath::RenderingNotifier,
        }
    }

    /// Preferred native-only shipping profile for packaged mainline builds.
    pub fn mainline_native() -> Self {
        Self::mainline()
    }

    fn mainline_software_fallback() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsMainline,
            renderer_mode: RendererMode::SkiaSoftware,
            terminal_render_mode: TerminalRenderMode::Native,
            native_present_path: NativePresentPath::RenderingNotifier,
        }
    }

    /// native-first Windows software profile for GNU/software packages while bitmap remains an internal fallback.
    pub fn software_compat() -> Self {
        Self {
            build_flavor: AppBuildFlavor::WindowsSoftwareCompat,
            renderer_mode: RendererMode::Software,
            terminal_render_mode: TerminalRenderMode::Native,
            native_present_path: NativePresentPath::RenderingNotifier,
        }
    }

    pub fn packaged() -> Self {
        let mut profile = match (
            option_env!("MICA_TERM_BUILD_FLAVOR"),
            option_env!("MICA_TERM_PACKAGE_RENDERER"),
            option_env!("MICA_TERM_PACKAGE_TERMINAL_RENDERER"),
        ) {
            (Some("windows-mainline"), Some("skia"), _) => Self::mainline_native(),
            (Some("windows-mainline"), Some("skia-software"), _) => {
                Self::mainline_software_fallback()
            }
            (Some("windows-software-compat"), Some("software"), _) => Self::software_compat(),
            _ => Self::development(),
        };
        if let Some(terminal_mode) = match option_env!("MICA_TERM_PACKAGE_TERMINAL_RENDERER") {
            Some("bitmap") => Some(TerminalRenderMode::Bitmap),
            Some("native") => Some(TerminalRenderMode::Native),
            _ => None,
        } {
            profile = profile.with_terminal_render_mode(terminal_mode);
        }

        let packaged_present_path = match option_env!("MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH") {
            Some("rendering-notifier") => Some(NativePresentPath::RenderingNotifier),
            Some("event-loop") => Some(NativePresentPath::EventLoop),
            _ => None,
        };

        if let Some(native_present_path) = packaged_present_path {
            profile = profile.with_native_present_path(native_present_path);
        }

        profile
    }

    pub fn forced_backend(self) -> Option<&'static str> {
        Some("winit")
    }

    pub fn forced_renderer(self) -> Option<&'static str> {
        Some(self.renderer_mode.renderer_name())
    }

    pub fn selector_label(self) -> &'static str {
        self.renderer_mode.selector_label()
    }

    pub fn preferred_graphics_api(self) -> Option<GraphicsApiRequirement> {
        match self.build_flavor {
            AppBuildFlavor::WindowsMainline => Some(GraphicsApiRequirement::Direct3D),
            AppBuildFlavor::Development | AppBuildFlavor::WindowsSoftwareCompat => None,
        }
    }

    pub fn prefers_direct3d(self) -> bool {
        self.preferred_graphics_api() == Some(GraphicsApiRequirement::Direct3D)
            && matches!(self.renderer_mode, RendererMode::Skia)
    }

    pub fn preferred_graphics_api_label(self) -> Option<&'static str> {
        self.preferred_graphics_api()
            .map(GraphicsApiRequirement::as_str)
    }

    pub fn renderer_fallback_chain(self) -> &'static [RendererMode] {
        match self.build_flavor {
            AppBuildFlavor::WindowsMainline => WINDOWS_MAINLINE_RENDERER_FALLBACK_CHAIN,
            AppBuildFlavor::Development | AppBuildFlavor::WindowsSoftwareCompat => {
                SOFTWARE_RENDERER_FALLBACK_CHAIN
            }
        }
    }

    pub fn terminal_render_mode(self) -> TerminalRenderMode {
        self.terminal_render_mode
    }

    pub fn native_present_path(self) -> NativePresentPath {
        self.native_present_path
    }

    pub fn terminal_composition_mode(self) -> TerminalCompositionMode {
        match self.build_flavor {
            AppBuildFlavor::WindowsSoftwareCompat => TerminalCompositionMode::SceneImage,
            AppBuildFlavor::WindowsMainline if self.prefers_direct3d() => {
                TerminalCompositionMode::PostRenderNativeSurface
            }
            AppBuildFlavor::Development | AppBuildFlavor::WindowsMainline => {
                TerminalCompositionMode::PostRenderNativeSurface
            }
        }
    }

    pub fn prefers_native_terminal_renderer(self) -> bool {
        matches!(self.terminal_render_mode, TerminalRenderMode::Native)
    }

    pub fn terminal_render_mode_label(self) -> &'static str {
        self.terminal_render_mode().as_str()
    }

    pub fn native_present_path_label(self) -> &'static str {
        self.native_present_path().as_str()
    }

    fn with_native_present_path(mut self, native_present_path: NativePresentPath) -> Self {
        self.native_present_path = native_present_path;
        self
    }

    fn with_terminal_render_mode(mut self, terminal_render_mode: TerminalRenderMode) -> Self {
        self.terminal_render_mode = terminal_render_mode;
        self
    }
}
