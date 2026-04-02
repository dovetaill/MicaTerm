//! Source-level contract coverage for the native terminal render host seam.

use std::fs;

#[test]
fn terminal_session_host_source_exposes_native_render_contract() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        host_source.contains("in property <string> session-render-mode: \"bitmap\";"),
        "terminal session host should expose a render mode selector with a bitmap fallback default"
    );
    assert!(
        host_source.contains("in property <image> session-surface-image;"),
        "terminal session host should keep the bitmap image property as a fallback path"
    );
    assert!(
        host_source.contains("in property <int> session-native-frame-token: 0;"),
        "terminal session host should expose a native frame token for renderer hook invalidation"
    );
    assert!(
        host_source.contains("image-fit: contain;"),
        "terminal session host should keep the bitmap fallback on aspect-preserving image composition instead of stretching the surface image with fill"
    );
    assert!(
        !host_source.contains("image-fit: fill;"),
        "terminal session host should stop stretching the bitmap surface image because that reintroduces blur after hidpi atlas rendering"
    );
}

#[test]
fn workspace_and_window_sources_thread_native_render_contract() {
    let workspace_source =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let app_window_source = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        workspace_source
            .contains("in property <string> workspace-session-render-mode: \"bitmap\";"),
        "workspace pane should accept the workspace session render mode contract"
    );
    assert!(
        workspace_source.contains("in property <int> workspace-session-native-frame-token: 0;"),
        "workspace pane should accept the workspace session native frame token contract"
    );
    assert!(
        workspace_source.contains("session-render-mode: root.workspace-session-render-mode;"),
        "workspace pane should forward the render mode to the terminal session host"
    );
    assert!(
        workspace_source
            .contains("session-native-frame-token: root.workspace-session-native-frame-token;"),
        "workspace pane should forward the native frame token to the terminal session host"
    );
    assert!(
        app_window_source
            .contains("in-out property <string> workspace-session-render-mode: \"bitmap\";"),
        "app window should store the workspace terminal render mode"
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
        runtime_profile_source.contains("TerminalRenderMode::Bitmap"),
        "runtime profile should expose a bitmap terminal render mode"
    );
    assert!(
        runtime_profile_source.contains("TerminalRenderMode::Native"),
        "runtime profile should expose a native terminal render mode"
    );
    assert!(
        runtime_profile_source.contains("Preferred Windows shipping profile"),
        "runtime profile docs should mark the native-first Windows mainline profile as the preferred shipping path"
    );
    assert!(
        runtime_profile_source.contains("fallback-only compatibility profile"),
        "runtime profile docs should keep the bitmap software compatibility package documented as fallback-only"
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
        native_surface_source.contains("RenderingState::AfterRendering"),
        "rendering notifier should reach the retained-frame draw hook after Slint paints the host surface so native terminal pixels are not overdrawn"
    );
    assert!(
        !native_surface_source.contains("RenderingState::BeforeRendering => draw_retained_frame"),
        "native surface bridge should stop presenting retained frames before Slint paints because the terminal region background would overdraw native text"
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
