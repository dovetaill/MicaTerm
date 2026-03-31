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
}
