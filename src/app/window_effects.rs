use std::rc::Rc;

use crate::AppWindow;
use crate::app::windowing::{MaterialKind, WindowAppearance};
use crate::theme::ThemeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowTheme {
    Dark,
    Light,
}

impl NativeWindowTheme {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropPreference {
    None,
    Mica,
    MicaAlt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeWindowAppearanceRequest {
    pub theme: NativeWindowTheme,
    pub backdrop: BackdropPreference,
    pub request_redraw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropApplyStatus {
    Applied,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowAppearanceSyncReport {
    pub theme_applied: bool,
    pub backdrop_status: BackdropApplyStatus,
    pub redraw_requested: bool,
}

impl WindowAppearanceSyncReport {
    pub fn skipped() -> Self {
        Self {
            theme_applied: false,
            backdrop_status: BackdropApplyStatus::Skipped,
            redraw_requested: false,
        }
    }
}

pub fn build_native_window_appearance_request(
    mode: ThemeMode,
    appearance: WindowAppearance,
) -> NativeWindowAppearanceRequest {
    let theme = match mode {
        ThemeMode::Dark => NativeWindowTheme::Dark,
        ThemeMode::Light => NativeWindowTheme::Light,
    };

    let backdrop = match appearance.material {
        MaterialKind::MicaAlt => BackdropPreference::MicaAlt,
    };

    NativeWindowAppearanceRequest {
        theme,
        backdrop,
        request_redraw: true,
    }
}

pub trait PlatformWindowEffects {
    fn apply_to_app_window(
        &self,
        window: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport;
}

#[derive(Default)]
pub struct NoopWindowEffects;

impl PlatformWindowEffects for NoopWindowEffects {
    fn apply_to_app_window(
        &self,
        _window: &AppWindow,
        _request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        WindowAppearanceSyncReport::skipped()
    }
}

pub fn default_platform_window_effects() -> Rc<dyn PlatformWindowEffects> {
    #[cfg(target_os = "windows")]
    {
        Rc::new(WindowsWindowEffects)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Rc::new(NoopWindowEffects)
    }
}

#[cfg(target_os = "windows")]
#[derive(Default)]
pub struct WindowsWindowEffects;

#[cfg(target_os = "windows")]
impl PlatformWindowEffects for WindowsWindowEffects {
    fn apply_to_app_window(
        &self,
        app: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        use slint::ComponentHandle;
        use slint::winit_030::{WinitWindowAccessor, winit};

        let mut theme_applied = false;
        let mut backdrop_status = BackdropApplyStatus::Skipped;
        let mut redraw_requested = false;

        let _ = app
            .window()
            .with_winit_window(|window: &winit::window::Window| {
                let theme = match request.theme {
                    NativeWindowTheme::Dark => winit::window::Theme::Dark,
                    NativeWindowTheme::Light => winit::window::Theme::Light,
                };

                window.set_theme(Some(theme));
                theme_applied = true;

                backdrop_status = match request.backdrop {
                    BackdropPreference::None => BackdropApplyStatus::Skipped,
                    BackdropPreference::MicaAlt => {
                        match window_vibrancy::apply_tabbed(window, Some(request.theme.is_dark())) {
                            Ok(()) => BackdropApplyStatus::Applied,
                            Err(_) => BackdropApplyStatus::Failed,
                        }
                    }
                    BackdropPreference::Mica => {
                        match window_vibrancy::apply_mica(window, Some(request.theme.is_dark())) {
                            Ok(()) => BackdropApplyStatus::Applied,
                            Err(_) => BackdropApplyStatus::Failed,
                        }
                    }
                };

                if request.request_redraw {
                    window.request_redraw();
                    redraw_requested = true;
                }
            });

        WindowAppearanceSyncReport {
            theme_applied,
            backdrop_status,
            redraw_requested,
        }
    }
}
