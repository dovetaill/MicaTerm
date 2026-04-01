//! Shared platform backend selection for native terminal surface hosting.

pub mod backend;
pub mod wayland;
pub mod windows;
pub mod x11;

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
        return Box::new(WindowsNativeSurfaceBackend::default());
    }

    #[cfg(target_os = "linux")]
    {
        if host_prefers_wayland_backend() {
            return Box::new(WaylandNativeSurfaceBackend::default());
        }

        if host_prefers_x11_backend() {
            return Box::new(X11NativeSurfaceBackend::default());
        }
    }

    Box::new(DetachedPlatformSurfaceBackend::default())
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

#[derive(Default)]
struct DetachedPlatformSurfaceBackend {
    rect: NativeTerminalSurfaceRect,
    frame: Option<RetainedNativeTerminalSurfaceFrame>,
}

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

    fn present(&mut self) {}

    fn detach(&mut self) {
        self.frame = None;
        self.rect = NativeTerminalSurfaceRect::default();
    }
}
