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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowAppearanceSyncReport {
    pub theme_applied: bool,
    pub backdrop_status: BackdropApplyStatus,
    pub backdrop_error: Option<String>,
    pub redraw_requested: bool,
}

impl WindowAppearanceSyncReport {
    pub fn skipped() -> Self {
        Self {
            theme_applied: false,
            backdrop_status: BackdropApplyStatus::Skipped,
            backdrop_error: None,
            redraw_requested: false,
        }
    }
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn classify_backdrop_result(
    result: Result<(), window_vibrancy::Error>,
) -> (BackdropApplyStatus, Option<String>) {
    match result {
        Ok(()) => (BackdropApplyStatus::Applied, None),
        Err(window_vibrancy::Error::UnsupportedPlatformVersion(_)) => {
            (BackdropApplyStatus::Skipped, None)
        }
        Err(err) => (BackdropApplyStatus::Failed, Some(err.to_string())),
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
        let mut backdrop_error = None;
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

                (backdrop_status, backdrop_error) = match request.backdrop {
                    BackdropPreference::None => (BackdropApplyStatus::Skipped, None),
                    BackdropPreference::MicaAlt => classify_backdrop_result(
                        window_vibrancy::apply_tabbed(window, Some(request.theme.is_dark())),
                    ),
                    BackdropPreference::Mica => classify_backdrop_result(
                        window_vibrancy::apply_mica(window, Some(request.theme.is_dark())),
                    ),
                };

                if request.request_redraw {
                    window.request_redraw();
                    redraw_requested = true;
                }
            });

        WindowAppearanceSyncReport {
            theme_applied,
            backdrop_status,
            backdrop_error,
            redraw_requested,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BackdropApplyStatus;

    #[test]
    fn unsupported_backdrop_version_is_treated_as_skipped() {
        let (status, error) = super::classify_backdrop_result(Err(
            window_vibrancy::Error::UnsupportedPlatformVersion(
                "\"apply_tabbed()\" is only available on Windows 11.",
            ),
        ));

        assert_eq!(status, BackdropApplyStatus::Skipped);
        assert_eq!(error, None);
    }

    #[test]
    fn unexpected_backdrop_error_is_reported_as_failed() {
        let (status, error) = super::classify_backdrop_result(Err(
            window_vibrancy::Error::NotMainThread("main thread required"),
        ));

        assert_eq!(status, BackdropApplyStatus::Failed);
        assert_eq!(error.as_deref(), Some("main thread required"));
    }
}
