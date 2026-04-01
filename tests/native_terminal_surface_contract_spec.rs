//! Source-level contract coverage for the native terminal render host seam.

use std::fs;
use std::path::Path;

#[test]
fn terminal_session_host_source_exposes_native_render_contract() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        !host_source.contains("session-render-mode"),
        "terminal session host should remove the bitmap render mode selector once the UI becomes native-only"
    );
    assert!(
        !host_source.contains("session-surface-image"),
        "terminal session host should remove the bitmap image property once the UI becomes native-only"
    );
    assert!(
        host_source.contains("in property <int> session-native-frame-token: 0;"),
        "terminal session host should expose a native frame token for renderer hook invalidation"
    );
    assert!(
        !host_source.contains("image-fit:"),
        "terminal session host should no longer contain bitmap image-fit rules once the surface image path is removed"
    );
}

#[test]
fn workspace_and_window_sources_thread_native_render_contract() {
    let workspace_source =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let app_window_source = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        !workspace_source.contains("workspace-session-render-mode"),
        "workspace pane should stop carrying a bitmap/native render-mode switch once only the native surface contract remains"
    );
    assert!(
        !workspace_source.contains("workspace-session-surface-image"),
        "workspace pane should remove the bitmap surface image binding once only native frames remain"
    );
    assert!(
        workspace_source.contains("in property <int> workspace-session-native-frame-token: 0;"),
        "workspace pane should accept the workspace session native frame token contract"
    );
    assert!(
        !workspace_source.contains("session-render-mode:"),
        "workspace pane should stop forwarding a render mode selector to the terminal session host"
    );
    assert!(
        workspace_source
            .contains("session-native-frame-token: root.workspace-session-native-frame-token;"),
        "workspace pane should forward the native frame token to the terminal session host"
    );
    assert!(
        !app_window_source.contains("workspace-session-render-mode"),
        "app window should stop storing a workspace terminal render mode once bitmap/native switching is removed"
    );
    assert!(
        !app_window_source.contains("workspace-session-surface-image"),
        "app window should remove the bitmap surface image property once the host becomes native-only"
    );
    assert!(
        app_window_source
            .contains("in-out property <int> workspace-session-native-frame-token: 0;"),
        "app window should store the workspace terminal native frame token"
    );
}

#[test]
fn runtime_profile_source_exposes_terminal_render_mode_contract() {
    let runtime_profile_source =
        fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(
        runtime_profile_source.contains("pub enum TerminalRenderMode"),
        "runtime profile should define a terminal render mode enum"
    );
    assert!(
        runtime_profile_source.contains("pub terminal_render_mode: TerminalRenderMode"),
        "runtime profile should carry the selected terminal render mode"
    );
    assert!(
        !runtime_profile_source.contains("TerminalRenderMode::Bitmap"),
        "runtime profile should remove the bitmap terminal render mode from the native-only contract"
    );
    assert!(
        runtime_profile_source.contains("TerminalRenderMode::Native"),
        "runtime profile should keep a native terminal render mode contract for logging and packaging metadata"
    );
    assert!(
        runtime_profile_source.contains("Preferred native-only shipping profile"),
        "runtime profile docs should mark the Windows mainline profile as the preferred native-only shipping path"
    );
    assert!(
        runtime_profile_source
            .contains("Transitional non-shipping software profile while native Linux terminal surfaces are still landing."),
        "runtime profile docs should describe the software profile as transitional instead of a bitmap fallback shipping path"
    );
    assert!(
        !runtime_profile_source.contains("fallback-only compatibility profile"),
        "runtime profile docs should stop documenting the software package as a fallback-only compatibility shipping path"
    );
    assert!(
        runtime_profile_source.contains("pub fn mainline_native() -> Self"),
        "runtime profile should keep an explicit native-first Windows mainline constructor"
    );
    assert!(
        runtime_profile_source.contains("pub fn prefers_native_terminal_renderer(self) -> bool"),
        "runtime profile should expose an explicit native-first preference helper for Windows presenter installation"
    );
}

#[test]
fn native_surface_source_exposes_present_bridge_contract() {
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let renderer_mod_source = fs::read_to_string("src/app/terminal_renderer/mod.rs")
        .expect("read terminal renderer mod");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        native_surface_source.contains("RetainedNativeTerminalSurfaceFrame"),
        "native surface bridge should define a retained native frame payload contract"
    );
    assert!(
        native_surface_source.contains("retained_frame: Option<RetainedNativeTerminalSurfaceFrame>"),
        "native surface bridge should retain full frame state instead of storing only a frame token"
    );
    assert!(
        native_surface_source.contains("pub fn update_frame_state(&self, frame: NativeTerminalFrame)"),
        "native surface bridge should expose a retained frame-state update entrypoint"
    );
    assert!(
        native_surface_source.contains("fn draw_retained_frame(state: &mut NativeTerminalSurfaceState)"),
        "native surface bridge should expose an explicit draw hook for retained native frames"
    );
    assert!(
        native_surface_source.contains("RenderingState::BeforeRendering"),
        "rendering notifier should reach the retained-frame draw hook during Slint rendering"
    );
    assert!(
        renderer_mod_source.contains("RetainedNativeTerminalSurfaceFrame"),
        "terminal renderer module should re-export the retained native surface frame contract"
    );
    assert!(
        bootstrap_source.contains("let rect = workspace_native_terminal_rect(window);"),
        "bootstrap should materialize geometry before updating the native surface bridge"
    );
    assert!(
        bootstrap_source.contains("surface.update_terminal_rect(rect);"),
        "bootstrap should keep updating the native surface geometry when presenting native frames"
    );
    assert!(
        bootstrap_source.contains("surface.update_frame_state(frame);"),
        "bootstrap should update the native surface bridge with retained frame state, not just a token"
    );
    assert!(
        bootstrap_source.contains("presentable_frame.selection_overlay"),
        "native surface host should keep selection overlay payloads attached to the retained native frame state"
    );
    assert!(
        bootstrap_source.contains("presentable_frame.ime_preview_overlay"),
        "native surface host should keep IME preview overlay payloads attached to the retained native frame state"
    );
}

#[test]
fn platform_surface_backend_source_exposes_shared_native_surface_abstraction() {
    assert!(
        Path::new("src/app/terminal_renderer/platform/mod.rs").exists(),
        "terminal renderer should add a shared platform module for native surface backends"
    );
    assert!(
        Path::new("src/app/terminal_renderer/platform/backend.rs").exists(),
        "terminal renderer should add a shared backend contract for native surface backends"
    );

    let platform_mod_source = fs::read_to_string("src/app/terminal_renderer/platform/mod.rs")
        .expect("read platform mod");
    let backend_source = fs::read_to_string("src/app/terminal_renderer/platform/backend.rs")
        .expect("read platform backend contract");
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let renderer_mod_source = fs::read_to_string("src/app/terminal_renderer/mod.rs")
        .expect("read terminal renderer mod");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        backend_source.contains("pub trait PlatformNativeSurfaceBackend"),
        "shared platform backend contract should define a native surface backend trait"
    );
    assert!(
        backend_source.contains("fn attach(&mut self, window: &AppWindow) -> Result<()>"),
        "shared platform backend contract should expose an attach hook"
    );
    assert!(
        backend_source.contains("fn update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect)"),
        "shared platform backend contract should expose a surface-rect update hook"
    );
    assert!(
        backend_source.contains(
            "fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>)"
        ),
        "shared platform backend contract should expose a retained-frame update hook"
    );
    assert!(
        backend_source.contains("fn present(&mut self)"),
        "shared platform backend contract should expose a present hook"
    );
    assert!(
        backend_source.contains("fn detach(&mut self)"),
        "shared platform backend contract should expose a detach hook"
    );
    assert!(
        platform_mod_source.contains(
            "pub fn create_platform_native_surface_backend() -> Box<dyn PlatformNativeSurfaceBackend>"
        ),
        "platform module should expose a backend factory for native surface attachment"
    );
    assert!(
        native_surface_source.contains("backend: Box<dyn PlatformNativeSurfaceBackend>"),
        "native surface bridge should hold a backend object instead of only a retained frame token"
    );
    assert!(
        native_surface_source.contains("create_platform_native_surface_backend()"),
        "native surface bridge should create its backend through the shared platform factory"
    );
    assert!(
        native_surface_source.contains("state.backend.update_surface_rect(rect);"),
        "native surface bridge should forward geometry changes through the shared backend abstraction"
    );
    assert!(
        native_surface_source.contains("state.backend.update_frame("),
        "native surface bridge should forward retained frame updates through the shared backend abstraction"
    );
    assert!(
        native_surface_source.contains("state.backend.present();"),
        "native surface bridge should ask the shared backend to present during retained-frame draw"
    );
    assert!(
        native_surface_source.contains("state.backend.detach();"),
        "native surface bridge should forward teardown through the shared backend abstraction"
    );
    assert!(
        renderer_mod_source.contains("pub mod platform;"),
        "terminal renderer module should expose the shared platform backend module"
    );
    assert!(
        renderer_mod_source.contains("PlatformNativeSurfaceBackend"),
        "terminal renderer module should re-export the shared platform backend contract"
    );
    assert!(
        bootstrap_source.contains("NativeTerminalSurface::attach(window)"),
        "bootstrap should instantiate the native surface bridge through the shared backend-aware attach entrypoint"
    );
}

#[test]
fn windows_platform_backend_source_exposes_backend_selection_contract() {
    assert!(
        Path::new("src/app/terminal_renderer/platform/windows.rs").exists(),
        "terminal renderer should add a Windows platform backend source file"
    );

    let platform_mod_source = fs::read_to_string("src/app/terminal_renderer/platform/mod.rs")
        .expect("read platform mod");
    let renderer_mod_source = fs::read_to_string("src/app/terminal_renderer/mod.rs")
        .expect("read terminal renderer mod");

    assert!(
        platform_mod_source.contains("pub mod windows;"),
        "platform module should expose the Windows native surface backend module"
    );
    assert!(
        platform_mod_source.contains("WindowsNativeSurfaceBackend::default()"),
        "platform backend factory should instantiate the Windows backend on Windows hosts"
    );
    assert!(
        renderer_mod_source.contains("WindowsNativeSurfaceBackend"),
        "terminal renderer module should re-export the Windows native surface backend type"
    );
}

#[test]
fn wayland_platform_backend_source_exposes_backend_selection_contract() {
    assert!(
        Path::new("src/app/terminal_renderer/platform/wayland.rs").exists(),
        "terminal renderer should add a Wayland platform backend source file"
    );

    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read cargo toml");
    let platform_mod_source = fs::read_to_string("src/app/terminal_renderer/platform/mod.rs")
        .expect("read platform mod");
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");

    assert!(
        cargo_toml.contains("backend-winit-wayland"),
        "Cargo.toml should enable Slint's Wayland winit backend feature for Linux native surface hosting"
    );
    assert!(
        platform_mod_source.contains("pub mod wayland;"),
        "platform module should expose the Wayland native surface backend module"
    );
    assert!(
        platform_mod_source.contains("WaylandNativeSurfaceBackend::default()"),
        "platform backend factory should instantiate the Wayland backend on Wayland Linux hosts"
    );
    assert!(
        platform_mod_source.contains("WAYLAND_DISPLAY")
            || platform_mod_source.contains("XDG_SESSION_TYPE"),
        "platform backend factory should detect Wayland host sessions through standard Linux environment hints"
    );
    assert!(
        native_surface_source.contains("create_platform_native_surface_backend()"),
        "native surface bridge should keep using the shared platform backend factory when Wayland support lands"
    );
}

#[test]
fn x11_platform_backend_source_exposes_backend_selection_contract() {
    assert!(
        Path::new("src/app/terminal_renderer/platform/x11.rs").exists(),
        "terminal renderer should add an X11 platform backend source file"
    );

    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read cargo toml");
    let platform_mod_source = fs::read_to_string("src/app/terminal_renderer/platform/mod.rs")
        .expect("read platform mod");
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");

    assert!(
        cargo_toml.contains("backend-winit-x11"),
        "Cargo.toml should enable Slint's X11 winit backend feature for Linux native surface hosting"
    );
    assert!(
        platform_mod_source.contains("pub mod x11;"),
        "platform module should expose the X11 native surface backend module"
    );
    assert!(
        platform_mod_source.contains("X11NativeSurfaceBackend::default()"),
        "platform backend factory should instantiate the X11 backend on X11 Linux hosts"
    );
    assert!(
        platform_mod_source.contains("DISPLAY")
            || platform_mod_source.contains("XDG_SESSION_TYPE"),
        "platform backend factory should detect X11 host sessions through standard Linux environment hints"
    );
    assert!(
        native_surface_source.contains("create_platform_native_surface_backend()"),
        "native surface bridge should keep using the shared platform backend factory when X11 support lands"
    );
}
