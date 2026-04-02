//! Shared platform backend selection for native terminal surface hosting.

pub mod backend;
pub mod wayland;
pub mod windows;
pub mod x11;

#[cfg(not(target_os = "windows"))]
use crate::AppWindow;

pub use backend::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
};
pub use wayland::WaylandNativeSurfaceBackend;
pub use windows::WindowsNativeSurfaceBackend;
pub use x11::X11NativeSurfaceBackend;

pub fn create_platform_native_surface_backend() -> Box<dyn PlatformNativeSurfaceBackend> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsNativeSurfaceBackend::default())
    }

    #[cfg(target_os = "linux")]
    {
        if host_prefers_wayland_backend() {
            Box::new(WaylandNativeSurfaceBackend::default())
        } else if host_prefers_x11_backend() {
            Box::new(X11NativeSurfaceBackend::default())
        } else {
            Box::new(DetachedPlatformSurfaceBackend::default())
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Box::new(DetachedPlatformSurfaceBackend::default())
    }
}

#[cfg(target_os = "linux")]
fn host_prefers_wayland_backend() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|value| value.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn host_prefers_x11_backend() -> bool {
    std::env::var_os("DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|value| value.eq_ignore_ascii_case("x11"))
            .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
#[derive(Default)]
struct DetachedPlatformSurfaceBackend {
    rect: NativeTerminalSurfaceRect,
    frame: Option<RetainedNativeTerminalSurfaceFrame>,
}

#[cfg(not(target_os = "windows"))]
impl PlatformNativeSurfaceBackend for DetachedPlatformSurfaceBackend {
    fn attach(&mut self, _window: &AppWindow) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect) {
        self.rect = rect;
    }

    fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>) {
        self.frame = frame;
    }

    fn present(&mut self, _damage: crate::app::terminal_renderer::NativeSurfaceDamage) {}

    fn diagnostics_snapshot(&self) -> crate::app::terminal_renderer::NativeTerminalSurfaceDiagnostics {
        crate::app::terminal_renderer::NativeTerminalSurfaceDiagnostics::default()
    }

    fn detach(&mut self) {
        self.frame = None;
        self.rect = NativeTerminalSurfaceRect::default();
    }
}
