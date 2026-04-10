//! Source-level contract coverage for the native terminal render host seam.

use std::fs;
use std::path::Path;

fn block_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source.find(start).expect("start marker");
    let rest = &source[start_index..];
    let end_index = rest.find(end).expect("end marker");
    &rest[..end_index]
}

#[test]
fn terminal_session_host_source_exposes_native_render_contract() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        host_source.contains("in property <string> session-render-mode: \"bitmap\";"),
        "terminal session host should keep a bitmap/native render-mode selector so the software wrapper can fall back to the Slint image path"
    );
    assert!(
        host_source.contains("in property <image> session-surface-image;"),
        "terminal session host should keep the bitmap surface image contract for the software wrapper fallback"
    );
    assert!(
        host_source.contains("in property <float> session-device-scale-factor: 1.0;"),
        "terminal session host should accept the live device scale factor so bitmap-composited terminal content can snap its layout box onto physical pixels on fractional-DPI Windows displays"
    );
    assert!(
        host_source.contains("in property <int> session-native-frame-token: 0;"),
        "terminal session host should expose a native frame token for renderer hook invalidation"
    );
    assert!(
        host_source.contains("image-fit: fill;"),
        "terminal session host should stretch the scene-owned bitmap exactly onto the terminal grid instead of preserving a contain layout that can introduce secondary scaling"
    );
    assert!(
        host_source.contains("image-rendering: pixelated;"),
        "terminal session host should request nearest-neighbor bitmap scaling for the scene-owned terminal image so software rendering does not blur glyph edges"
    );
    assert!(
        host_source.contains("function terminal-visible-grid-width() -> length"),
        "terminal session host should define a visible grid width helper so the displayed terminal box can clamp to the live content viewport during sidebar and window resizes"
    );
    assert!(
        host_source.contains("function terminal-visible-grid-height() -> length"),
        "terminal session host should define a visible grid height helper so the displayed terminal box can clamp to the live content viewport during window height changes"
    );
    assert!(
        host_source.contains("function snap-length-to-device-pixel(value: length) -> length"),
        "terminal session host should expose a device-pixel snapping helper so the scene-image terminal box can land on integer physical pixels under fractional DPI"
    );
    assert!(
        host_source.contains("clip: true;"),
        "terminal session host should clip the terminal surface frame so stale grid geometry cannot momentarily paint over sibling UI during scene-image resize races"
    );
    assert!(
        host_source.contains("width: root.snapped-terminal-visible-grid-width();"),
        "terminal session host should size the blank terminal surface against a device-pixel-snapped visible grid width instead of the stale session grid width"
    );
    assert!(
        host_source.contains("height: root.snapped-terminal-visible-grid-height();"),
        "terminal session host should size the blank terminal surface against a device-pixel-snapped visible grid height instead of the stale session grid height"
    );
    assert!(
        host_source.contains("blank-surface := Rectangle {") && host_source.contains("clip: true;"),
        "terminal session host should make the blank surface itself a clipped viewport so oversized scene-image frames are cropped instead of being rescaled during layout races"
    );
    assert!(
        host_source.contains("x: root.terminal-surface-origin-x();")
            && host_source.contains("y: root.terminal-surface-origin-y();"),
        "terminal session host should snap the scene-image terminal origin onto device pixels instead of anchoring the bitmap at unsnapped logical padding offsets"
    );
    assert!(
        host_source.contains(
            "return root.snap-length-to-device-pixel(root.terminal-surface-origin-x() + (col * root.terminal-cell-width));"
        ) && host_source.contains(
            "return root.snap-length-to-device-pixel(root.terminal-surface-origin-y() + (row * root.terminal-cell-height));"
        ),
        "terminal session host should snap per-cell geometry onto device pixels so cursor and selection overlays stay aligned with bitmap glyphs on high-DPI renders"
    );
    assert!(
        host_source.contains("function scrollbar-track-y() -> length {\n        return root.terminal-surface-origin-y();\n    }")
            && host_source.contains("x: root.scrollbar-track-x();")
            && host_source.contains("y: root.scrollbar-track-y();"),
        "terminal session host should anchor terminal overlays to the snapped surface origin so scrollbars and viewport affordances do not drift against the scene-image viewport"
    );
    assert!(
        host_source.contains("root.terminal-surface-origin-x() + self.mouse-x,")
            && host_source.contains("root.terminal-surface-origin-y() + self.mouse-y,"),
        "terminal session host should open the context menu from the snapped surface origin so right-click affordances stay aligned with high-DPI scene-image coordinates"
    );
    assert!(
        host_source.contains("width: root.snapped-terminal-visible-grid-width();")
            && host_source.contains("height: root.snapped-terminal-visible-grid-height();"),
        "terminal session host should snap the visible terminal viewport box onto device pixels so Slint does not have to map the bitmap through a half-pixel destination rect"
    );
    assert!(
        host_source.contains("width: root.terminal-grid-width();"),
        "scene-image bitmap content should keep the frame-owned grid width so sidebar resizes crop stale frames instead of horizontally squeezing glyphs"
    );
    assert!(
        host_source.contains("height: root.terminal-grid-height();"),
        "scene-image bitmap content should keep the frame-owned grid height so vertical viewport changes crop stale frames instead of vertically squeezing rows"
    );
    assert!(
        host_source.contains("width: parent.width;")
            && host_source.contains("height: parent.height;"),
        "input capture inside the clipped viewport should track the visible terminal box, not the stale full-grid extent"
    );
}

#[test]
fn bitmap_host_selection_source_exposes_local_overlay_contract() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        host_source.contains("function selection-overlay-active() -> bool")
            && host_source
                .contains("root.session-render-mode == \"bitmap\" && root.selection-active"),
        "terminal session host should expose a bitmap-only local selection overlay gate so drag selection can repaint immediately without waiting for a Rust-side scene-image redraw"
    );
    assert!(
        host_source.contains("selection-overlay-single-row := Rectangle {")
            && host_source.contains("selection-overlay-first-row := Rectangle {")
            && host_source.contains("selection-overlay-middle-rows := Rectangle {")
            && host_source.contains("selection-overlay-last-row := Rectangle {"),
        "terminal session host should render local single-row, first-row, middle-row, and last-row selection rectangles for the bitmap path"
    );
    assert!(
        host_source.contains("background: ThemeTokens.terminal-selection-surface;"),
        "terminal session host should source bitmap host selection overlays from the shared terminal selection token so Catppuccin palette updates stay consistent across local overlays and presenter-rendered frames"
    );
    assert!(
        bootstrap_source.contains(
            "fn workspace_session_uses_host_selection_overlay(window: &AppWindow) -> bool"
        ),
        "bootstrap should centralize the bitmap host-selection decision so the presenter and selection callback stay in sync"
    );
    assert!(
        bootstrap_source.contains("if workspace_session_uses_host_selection_overlay(&window) {"),
        "selection-changed callback should skip Rust-side scene-image syncs when the Slint host owns the live bitmap selection overlay"
    );
    assert!(
        bootstrap_source
            .contains("let selection = if workspace_session_uses_host_selection_overlay(window) {"),
        "workspace terminal sync should stop baking selection overlays into bitmap presenter frames when the host draws selection locally"
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
        "workspace pane should keep carrying a bitmap/native render-mode switch so the software wrapper can select the Slint image fallback"
    );
    assert!(
        workspace_source.contains("in property <image> workspace-session-surface-image;"),
        "workspace pane should keep the bitmap surface image binding for the software wrapper fallback"
    );
    assert!(
        workspace_source
            .contains("in property <float> workspace-session-device-scale-factor: 1.0;"),
        "workspace pane should carry the workspace terminal device scale factor so the session host can snap scene-image layout onto physical pixels"
    );
    assert!(
        workspace_source.contains("in property <int> workspace-session-native-frame-token: 0;"),
        "workspace pane should accept the workspace session native frame token contract"
    );
    assert!(
        workspace_source.contains("session-render-mode: root.workspace-session-render-mode;"),
        "workspace pane should forward the render mode selector to the terminal session host"
    );
    assert!(
        workspace_source.contains("session-surface-image: root.workspace-session-surface-image;"),
        "workspace pane should forward the bitmap surface image to the terminal session host"
    );
    assert!(
        workspace_source
            .contains("session-device-scale-factor: root.workspace-session-device-scale-factor;"),
        "workspace pane should forward the live device scale factor to the terminal session host so the bitmap layout box can be snapped at the final scene edge"
    );
    assert!(
        workspace_source
            .contains("session-native-frame-token: root.workspace-session-native-frame-token;"),
        "workspace pane should forward the native frame token to the terminal session host"
    );
    assert!(
        app_window_source
            .contains("in-out property <string> workspace-session-render-mode: \"bitmap\";"),
        "app window should store the workspace terminal render mode so the software wrapper can select the bitmap fallback"
    );
    assert!(
        app_window_source.contains("in-out property <image> workspace-session-surface-image;"),
        "app window should keep the bitmap surface image property for the software wrapper fallback"
    );
    assert!(
        app_window_source
            .contains("in-out property <float> workspace-session-device-scale-factor: 1.0;"),
        "app window should store the workspace terminal device scale factor so bootstrap can project fractional-DPI information into the Slint scene host"
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
        "runtime profile should keep the bitmap terminal render mode available as an internal fallback"
    );
    assert!(
        runtime_profile_source.contains("TerminalRenderMode::Native"),
        "runtime profile should keep a native terminal render mode contract for logging and packaging metadata"
    );
    assert!(
        runtime_profile_source.contains("scene-image remains the default terminal subsystem"),
        "runtime profile docs should note that packaged Windows mainline keeps the native renderer while scene-image remains the default terminal subsystem"
    );
    assert!(
        runtime_profile_source.contains("native-first Windows software profile"),
        "runtime profile docs should describe the software profile as a native-first shipping path"
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
fn terminal_renderer_host_source_exposes_render_mode_label_for_diagnostics() {
    let host_source =
        fs::read_to_string("src/app/terminal_renderer/host.rs").expect("read terminal host source");

    assert!(
        host_source.contains("pub fn render_mode_label(&self) -> &'static str"),
        "terminal renderer host should expose a render-mode label helper so bootstrap diagnostics can log active presenter modes without reaching into raw enum formatting at every call site"
    );
}

#[test]
fn native_surface_source_exposes_present_bridge_contract() {
    assert!(
        Path::new("src/app/terminal_renderer/present_driver.rs").exists(),
        "terminal renderer should add a dedicated present-driver module instead of depending on a single rendering notifier hook"
    );

    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let present_driver_source = fs::read_to_string("src/app/terminal_renderer/present_driver.rs")
        .expect("read present driver");
    let renderer_mod_source =
        fs::read_to_string("src/app/terminal_renderer/mod.rs").expect("read terminal renderer mod");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        native_surface_source.contains("RetainedNativeTerminalSurfaceFrame"),
        "native surface bridge should define a retained native frame payload contract"
    );
    assert!(
        native_surface_source
            .contains("retained_frame: Option<RetainedNativeTerminalSurfaceFrame>"),
        "native surface bridge should retain full frame state instead of storing only a frame token"
    );
    assert!(
        native_surface_source
            .contains("pub fn update_frame_state(&self, frame: NativeTerminalFrame)"),
        "native surface bridge should expose a retained frame-state update entrypoint"
    );
    assert!(
        native_surface_source
            .contains("fn draw_retained_frame(state: &mut NativeTerminalSurfaceState)"),
        "native surface bridge should expose an explicit draw hook for retained native frames"
    );
    assert!(
        present_driver_source.contains("pub trait NativeSurfacePresentDriver"),
        "present driver module should define a scheduling seam for native surface present work"
    );
    assert!(
        present_driver_source.contains("pub struct RenderingNotifierPresentDriver"),
        "present driver module should keep the rendering notifier path as one possible present driver"
    );
    assert!(
        present_driver_source.contains("pub struct EventLoopPresentDriver"),
        "present driver module should add an event-loop present driver for runtimes where the notifier hook is unavailable"
    );
    assert!(
        present_driver_source.contains("pub fn install_rendering_notifier"),
        "present driver module should centralize rendering notifier registration instead of hard-coding it inside the native surface"
    );
    assert!(
        present_driver_source.contains("RenderingState::AfterRendering"),
        "rendering notifier present driver should reach the retained-frame draw hook after Slint paints the host surface so native terminal pixels are not overdrawn"
    );
    assert!(
        !present_driver_source.contains("RenderingState::BeforeRendering => on_after_rendering()"),
        "present driver registration should stop presenting retained frames before Slint paints because the terminal region background would overdraw native text"
    );
    assert!(
        renderer_mod_source.contains("RetainedNativeTerminalSurfaceFrame"),
        "terminal renderer module should re-export the retained native surface frame contract"
    );
    assert!(
        renderer_mod_source.contains("pub mod present_driver;"),
        "terminal renderer module should expose the present-driver module"
    );
    assert!(
        renderer_mod_source.contains("NativeSurfacePresentDriver"),
        "terminal renderer module should re-export the present-driver contract for bootstrap/runtime wiring"
    );
    assert!(
        native_surface_source.contains("present_driver: Rc<dyn NativeSurfacePresentDriver>"),
        "native surface bridge should own a shareable present-driver abstraction so scheduling can clone the driver out of surface state before low-latency callbacks run"
    );
    assert!(
        native_surface_source.contains("dirty: bool"),
        "native surface bridge should track dirty state independently of notifier registration success"
    );
    assert!(
        native_surface_source.contains("pending_present: PendingPresentGate"),
        "native surface bridge should keep an explicit present gate so repeated rect/frame churn before the next draw pass coalesces into one host redraw request instead of spamming request_redraw"
    );
    assert!(
        native_surface_source.contains("pending_host_redraw: PendingPresentGate"),
        "native surface bridge should track host redraw scheduling separately from retained-frame present scheduling so immediate native replays do not automatically spam host redraw requests"
    );
    assert!(
        native_surface_source.contains("EventLoopPresentDriver::new(window)"),
        "native surface bridge should install an event-loop present fallback before attempting notifier-specific registration"
    );
    assert!(
        native_surface_source.contains("RenderingNotifierPresentDriver::new(window)"),
        "native surface bridge should be able to switch to a rendering-notifier driver when the hook is available"
    );
    assert!(
        native_surface_source.contains("state.dirty = true;"),
        "native surface bridge should mark itself dirty when retained frame or geometry state changes"
    );
    assert!(
        native_surface_source
            .contains("present_driver.schedule_present(callback, request_host_redraw);"),
        "native surface bridge should schedule present work through a cloned driver handle instead of reaching directly into RefCell state while low-latency present callbacks may run"
    );
    assert!(
        native_surface_source.contains("if !state.pending_present.mark_scheduled()"),
        "native surface bridge should skip redundant schedule_present calls while one redraw is already pending so geometry+frame updates in the same turn do not multiply host redraw requests"
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
        bootstrap_source.contains("surface.present(frame);"),
        "bootstrap should route retained native frame updates through the present-driver aware surface entrypoint"
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
fn software_winit_sources_expose_after_draw_present_contract() {
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let present_driver_source = fs::read_to_string("src/app/terminal_renderer/present_driver.rs")
        .expect("read present driver");
    let winit_backend_source =
        fs::read_to_string("vendor/i-slint-backend-winit/lib.rs").expect("read winit backend");
    let winit_adapter_source =
        fs::read_to_string("vendor/i-slint-backend-winit/winitwindowadapter.rs")
            .expect("read winit window adapter");

    assert!(
        present_driver_source.contains("pub fn install_winit_after_draw_hook"),
        "present driver module should expose a dedicated winit after-draw hook installer for software runtimes that cannot register a Slint rendering notifier"
    );
    assert!(
        winit_backend_source.contains("pub trait WinitWindowAfterDrawHook"),
        "vendored winit backend should expose a window-level after-draw hook trait so software renderer users can replay native overlays after each host redraw"
    );
    assert!(
        winit_adapter_source
            .contains("after_draw_hook: Cell<Option<Box<dyn FnMut(&corelib::api::Window)>>>"),
        "winit window adapter should store a per-window after-draw hook that survives normal redraw scheduling"
    );
    assert!(
        winit_adapter_source.contains("if let Some(mut callback) = self.after_draw_hook.take()"),
        "winit window adapter should run the after-draw hook immediately after renderer.render() finishes so same-HWND native surfaces can repaint on top of the host surface"
    );
    assert!(
        native_surface_source.contains("host_redraw_sync_pending: bool"),
        "native surface bridge should track when the shell redraw still owes the child HWND a synchronization replay without implying same-HWND overpaint ownership"
    );
    assert!(
        native_surface_source.contains("state.host_redraw_sync_pending = true;"),
        "native surface bridge should mark a host-redraw sync hint before replaying retained native content from after-draw hooks"
    );
    assert!(
        native_surface_source
            .contains("if !state.dirty && !state.damage_tracker.has_damage() && !state.host_redraw_sync_pending"),
        "native surface draw gate should allow a retained frame replay after host redraw even when no new terminal frame arrived"
    );
}

#[test]
fn present_drivers_invoke_immediate_native_repaint_before_requesting_host_redraw() {
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let present_driver_source = fs::read_to_string("src/app/terminal_renderer/present_driver.rs")
        .expect("read present driver");

    assert!(
        present_driver_source.contains(
            "fn schedule_present(&self, callback: NativeSurfacePresentCallback, request_host_redraw: bool)"
        ),
        "present drivers should consume the supplied retained-frame callback plus a host-redraw decision so immediate native updates can stay low-latency without always scheduling another shell redraw"
    );
    assert!(
        present_driver_source.contains("callback();")
            && present_driver_source.contains("if request_host_redraw {")
            && present_driver_source.contains("window.window().request_redraw();"),
        "present drivers should replay the retained native frame immediately and request a host redraw only when the shell still needs an overlay repaint"
    );
    assert!(
        native_surface_source.contains("Rc::clone(&state.present_driver)")
            && native_surface_source
                .contains("let request_host_redraw = state.pending_host_redraw.mark_scheduled();")
            && native_surface_source
                .contains("present_driver.schedule_present(callback, request_host_redraw);"),
        "native surface scheduling should clone the driver out of RefCell state, keep host redraws on a separate gate, and forward that decision into the low-latency present callback"
    );
    assert!(
        native_surface_source.contains("state.pending_host_redraw.clear();"),
        "native surface should clear the host redraw gate only after the shell redraw replay hook runs so repeated immediate presents before that point collapse into one host redraw request"
    );
}

#[test]
fn rendering_notifier_path_treats_host_redraw_as_child_surface_sync_hint() {
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let present_driver_source = fs::read_to_string("src/app/terminal_renderer/present_driver.rs")
        .expect("read present driver");

    assert!(
        native_surface_source.contains("host_redraw_sync_pending: bool"),
        "native surface scheduling should track host redraw as a child-surface sync hint instead of assuming the shell repaint directly owns native terminal visibility"
    );
    assert!(
        native_surface_source.contains("state.host_redraw_sync_pending = true;"),
        "after-draw replay should mark a child-surface sync hint before replaying retained-native content"
    );
    assert!(
        !native_surface_source.contains("host_surface_invalidated"),
        "native surface scheduling should stop encoding same-HWND overpaint assumptions once the child HWND owns visible terminal output"
    );
    assert!(
        present_driver_source.contains("host redraw stays a synchronization hint")
            && present_driver_source.contains("child HWND owns visible terminal output"),
        "present driver docs should describe host redraw as a synchronization hint now that the child HWND, not the shell redraw, owns visible retained-native pixels"
    );
}

#[test]
fn bootstrap_source_exposes_scroll_projection_debounce_contract() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let workspace_terminal_source = fs::read_to_string("src/app/bootstrap/workspace_terminal.rs")
        .expect("read workspace terminal bootstrap module");

    assert!(
        bootstrap_source.contains("WORKSPACE_SCROLL_VIEWPORT_PROJECTION_DEBOUNCE_MS")
            && bootstrap_source.contains("WORKSPACE_SCROLL_THUMB_DRAG_PROJECTION_DEBOUNCE_MS"),
        "bootstrap should declare separate debounce budgets for wheel/jump viewport refreshes and thumb-drag coalescing so terminal scrollback can feel immediate without turning drag updates into synchronous full repaints"
    );
    assert!(
        bootstrap_source.contains("DeferredWorkspaceProjectionRefreshGate"),
        "bootstrap should keep a dedicated scroll projection refresh gate so repeated wheel and scrollbar drag callbacks can collapse into one scheduled projection refresh"
    );
    assert!(
        bootstrap_source
            .contains("workspace_terminal::schedule_workspace_scroll_projection_refresh(")
            && workspace_terminal_source
                .contains("pub(super) fn schedule_workspace_scroll_projection_refresh("),
        "bootstrap should centralize workspace terminal scroll projection refresh scheduling in the workspace terminal module instead of inlining immediate refresh calls in every scroll callback"
    );
    assert!(
        workspace_terminal_source
            .contains("Duration::from_millis(WORKSPACE_SCROLL_VIEWPORT_PROJECTION_DEBOUNCE_MS)")
            && workspace_terminal_source.contains(
                "Duration::from_millis(WORKSPACE_SCROLL_THUMB_DRAG_PROJECTION_DEBOUNCE_MS)"
            )
            && workspace_terminal_source.contains("TimerMode::SingleShot"),
        "workspace terminal bootstrap should drive wheel/jump and thumb-drag projection refreshes through dedicated single-shot timers so high-frequency viewport updates can use different latency budgets"
    );
}

#[test]
fn windows_software_sources_expose_scene_owned_terminal_composition_contract() {
    let runtime_profile_source =
        fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");

    assert!(
        runtime_profile_source.contains("pub enum TerminalCompositionMode"),
        "runtime profile should define an explicit terminal composition mode so Windows software packages can move visible terminal pixels back into the Slint scene"
    );
    assert!(
        runtime_profile_source.contains("SceneImage"),
        "runtime profile should keep a scene-owned image composition mode for software builds where overlays must stay above terminal content"
    );
    assert!(
        runtime_profile_source.contains("PostRenderNativeSurface"),
        "runtime profile should preserve an explicit post-render native surface mode for the remaining whole-window overlay path"
    );
    assert!(
        bootstrap_source.contains("profile.terminal_composition_mode()"),
        "bootstrap should branch on an explicit terminal composition mode instead of routing every native-quality frame through NativeTerminalSurface"
    );
    let install_presenter_block = block_between(
        &bootstrap_source,
        "fn ensure_workspace_terminal_presenter(",
        "\nfn window_scale_factor(",
    );
    assert!(
        !install_presenter_block.contains("window.set_workspace_session_render_mode("),
        "presenter installation should not flip the host render mode before the first real terminal frame is ready because that can expose an empty payload during startup or renderer reconfiguration"
    );
    assert!(
        !install_presenter_block
            .contains("window.set_workspace_session_surface_image(Image::default());"),
        "presenter installation should not proactively blank the scene image before sync_workspace_session_state publishes the first frame"
    );
    assert!(
        bootstrap_source.contains("TerminalCompositionMode::SceneImage"),
        "bootstrap should recognize the scene-image composition mode when selecting the visible Windows software terminal path"
    );
    assert!(
        bootstrap_source.contains("TerminalRenderMode::Bitmap"),
        "bootstrap should keep using the Slint image composition path when the software runtime selects scene-image terminal composition"
    );
    assert!(
        presenter_source.contains("pub struct WindowsSceneImagePresenter"),
        "terminal presenter should define a dedicated Windows scene-image presenter instead of forcing software builds through the whole-window native surface path"
    );
    assert!(
        presenter_source.contains("SceneImageTerminalRenderer"),
        "terminal presenter should use a dedicated scene-image renderer for native-quality glyph output inside the Slint scene"
    );
    assert!(
        bootstrap_source.contains("window.set_workspace_session_surface_image(frame.image);"),
        "bootstrap should keep projecting terminal frames back into the Slint scene image path once software builds stop using whole-window native post-pass composition"
    );
    assert!(
        bootstrap_source.contains("window.set_workspace_session_native_frame_token(0);"),
        "bitmap or scene-image composition paths should clear the native frame token so hit-testing and overlay logic do not treat the software scene as a retained native surface"
    );
    assert!(
        bootstrap_source.contains("window.set_workspace_session_cell_width(")
            && bootstrap_source.contains("frame.cell_width_px as f32 / scale_factor"),
        "scene-image composition should project the renderer cell width back into Slint logical units from the same frame that produced the visible pixels"
    );
    assert!(
        bootstrap_source.contains("window.set_workspace_session_cell_height(")
            && bootstrap_source.contains("frame.cell_height_px as f32 / scale_factor"),
        "scene-image composition should project the renderer cell height back into Slint logical units from the same frame that produced the visible pixels"
    );
    assert!(
        !bootstrap_source.contains(
            "window.set_workspace_session_rows(i32::try_from(surface.rows).unwrap_or(i32::MAX));"
        ),
        "bootstrap should stop projecting workspace terminal rows from the live surface before presentation because that splits the visible bitmap/native payload from the geometry the host uses to size and hit-test it"
    );
    assert!(
        !bootstrap_source.contains(
            "window.set_workspace_session_cols(i32::try_from(surface.cols).unwrap_or(i32::MAX));"
        ),
        "bootstrap should stop projecting workspace terminal cols from the live surface before presentation because that lets stale geometry race ahead of the frame that will actually be displayed"
    );
    let workspace_surface_sync_block = block_between(
        &bootstrap_source,
        "if let Some(surface) = state.active_workspace_terminal_surface() {",
        "if native_frame_presented {",
    );
    let workspace_surface_projection_block = block_between(
        &bootstrap_source,
        "if let Some(surface) = state.active_workspace_terminal_surface() {",
        "\n    } else {\n        let preset = terminal_theme_preset;",
    );
    assert!(
        workspace_surface_projection_block.contains("let mut next_render_mode = None;"),
        "workspace surface sync should stage the next render mode locally so frame payloads and host overlay state can be projected before the host flips modes"
    );
    assert!(
        workspace_surface_projection_block.contains("let mut next_surface_seqno = None;"),
        "workspace surface sync should stage the next surface seqno locally so host blink/reset state can be updated in lockstep with the frame payload"
    );
    let bitmap_block = block_between(
        workspace_surface_sync_block,
        "Ok(PresentedTerminalFrame::Bitmap(frame)) => {",
        "Ok(PresentedTerminalFrame::Native(frame)) => {",
    );
    let workspace_surface_seqno = workspace_surface_projection_block
        .find("window.set_workspace_session_surface_seqno(")
        .expect("workspace surface seqno");
    let workspace_render_mode = workspace_surface_projection_block
        .find("window.set_workspace_session_render_mode(")
        .expect("workspace render mode");
    let bitmap_rows = bitmap_block
        .find("window.set_workspace_session_rows(")
        .expect("bitmap rows");
    let bitmap_cols = bitmap_block
        .find("window.set_workspace_session_cols(")
        .expect("bitmap cols");
    let bitmap_cell_width = bitmap_block
        .find("window.set_workspace_session_cell_width(")
        .expect("bitmap cell width");
    let bitmap_cell_height = bitmap_block
        .find("window.set_workspace_session_cell_height(")
        .expect("bitmap cell height");
    let bitmap_image = bitmap_block
        .find("window.set_workspace_session_surface_image(frame.image);")
        .expect("bitmap image");
    let bitmap_native_clear = bitmap_block
        .find("clear_workspace_retained_native_terminal_surface(window);")
        .expect("bitmap native clear");
    assert!(
        bitmap_rows < bitmap_image
            && bitmap_cols < bitmap_image
            && bitmap_cell_width < bitmap_image
            && bitmap_cell_height < bitmap_image,
        "bitmap path should publish rows/cols and cell metrics from the same bitmap frame before swapping in the new scene-owned image so the host never stretches fresh pixels inside stale grid geometry"
    );
    assert!(
        bitmap_native_clear < bitmap_image,
        "bitmap path should tear down any retained native surface before publishing the new scene-owned image so renderer switches do not leave a stale native child surface lingering over the host"
    );
    assert!(
        bitmap_image < workspace_render_mode,
        "bitmap path should publish the new scene-owned image before switching render_mode so the terminal host does not briefly enter bitmap mode with stale or empty payload"
    );
    assert!(
        bitmap_image < workspace_surface_seqno && workspace_surface_seqno < workspace_render_mode,
        "bitmap path should publish the new frame image before updating surface seqno, and both should settle before render_mode flips so cursor blink reset tracks the same payload"
    );

    let native_block = block_between(
        workspace_surface_sync_block,
        "Ok(PresentedTerminalFrame::Native(frame)) => {",
        "Err(err) => {",
    );
    let native_rows = native_block
        .find("window.set_workspace_session_rows(")
        .expect("native rows");
    let native_cols = native_block
        .find("window.set_workspace_session_cols(")
        .expect("native cols");
    let native_present = native_block
        .find("present_workspace_native_terminal_frame(window, frame);")
        .expect("native present");
    let native_resync = native_block
        .find("sync_workspace_native_terminal_surface_geometry(window);")
        .expect("native geometry resync");
    assert!(
        native_rows < native_present && native_cols < native_present,
        "native path should project rows/cols from the retained frame payload before presenting it so overlay geometry and host hit-testing stay tied to the exact frame staged for display"
    );
    assert!(
        native_resync < native_present,
        "native path should resync the host-reported terminal rect after applying frame cell metrics so the retained surface presents into the same geometry the Slint host just committed"
    );
    assert!(
        !native_block.contains("window.set_workspace_session_surface_image(Image::default());"),
        "native path should stop blanking the scene-image payload before the retained surface is ready so bitmap-to-native switches do not flash an empty frame in the host"
    );
    assert!(
        native_present < workspace_render_mode,
        "native path should stage the retained native frame before switching render_mode so the UI does not briefly expose native mode without a ready surface payload"
    );
    assert!(
        native_present < workspace_surface_seqno && workspace_surface_seqno < workspace_render_mode,
        "native path should publish the new retained frame before updating surface seqno, and both should settle before render_mode flips so host blink/reset state matches the presented payload"
    );

    let error_start = workspace_surface_sync_block
        .find("Err(err) => {")
        .expect("error block start");
    let error_block = &workspace_surface_sync_block[error_start..];
    let error_clear = error_block
        .find("clear_workspace_native_terminal_frame(window);")
        .expect("error clear");
    let default_fg = workspace_surface_projection_block
        .find("window.set_workspace_session_default_fg(")
        .expect("default fg");
    let viewport_at_bottom = workspace_surface_projection_block
        .find("window.set_workspace_session_viewport_at_bottom(")
        .expect("viewport bottom");
    assert!(
        error_clear < workspace_render_mode,
        "error fallback should clear the native surface/image payload before switching render_mode back to bitmap so the host does not momentarily show a stale native frame"
    );
    assert!(
        default_fg < workspace_render_mode && viewport_at_bottom < workspace_render_mode,
        "workspace surface sync should project terminal colors and viewport state before flipping render_mode so the host does not briefly combine a new payload mode with stale overlay metadata"
    );
    let no_surface_block = block_between(
        &bootstrap_source,
        "\n    } else {\n        let preset = terminal_theme_preset;",
        "\n    }\n}\n\nfn sync_workspace_tabs(",
    );
    let no_surface_clear = no_surface_block
        .find("clear_workspace_native_terminal_frame(window);")
        .expect("no surface clear");
    let no_surface_render_mode = no_surface_block
        .find(
            "window.set_workspace_session_render_mode(TerminalRenderMode::Bitmap.as_str().into());",
        )
        .expect("no surface render mode reset");
    let no_surface_viewport_bottom = no_surface_block
        .find("window.set_workspace_session_viewport_at_bottom(true);")
        .expect("no surface viewport bottom reset");
    let no_surface_surface_seqno = no_surface_block
        .find("window.set_workspace_session_surface_seqno(0);")
        .expect("no surface surface seqno reset");
    assert!(
        no_surface_clear < no_surface_render_mode
            && no_surface_viewport_bottom < no_surface_render_mode,
        "when no terminal surface is active the host should clear retained payloads and reset viewport metadata before forcing render_mode back to bitmap"
    );
    assert!(
        no_surface_viewport_bottom < no_surface_surface_seqno
            && no_surface_surface_seqno < no_surface_render_mode,
        "when no terminal surface is active the host should reset surface seqno after clearing payload metadata and before forcing render_mode back to bitmap"
    );
}

#[test]
fn windows_mainline_direct3d_source_routes_terminal_subsystem_explicitly_with_packaged_scene_image_default()
 {
    let runtime_profile_source =
        fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let composition_block = block_between(
        &runtime_profile_source,
        "pub fn terminal_composition_mode(self) -> TerminalCompositionMode {",
        "\n    pub fn prefers_native_terminal_renderer(self) -> bool {",
    );

    assert!(
        runtime_profile_source.contains("pub enum TerminalSubsystemMode"),
        "runtime profile should expose a terminal subsystem mode once Windows mainline switches to the retained native surface by default"
    );
    assert!(
        runtime_profile_source.contains("RetainedNativeSurface"),
        "runtime profile should make the retained native surface an explicit subsystem mode instead of hiding the default behind render-mode heuristics"
    );
    assert!(
        runtime_profile_source.contains("std::env::var(\"MICA_TERM_TERMINAL_SUBSYSTEM\")"),
        "runtime profile should keep a runtime rollback switch so bring-up can force the scene-image subsystem without rebuilding"
    );
    assert!(
        runtime_profile_source.contains("option_env!(\"MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM\")"),
        "runtime profile should keep a packaged terminal subsystem override so build wrappers can pin packaged Windows mainline to the scene-image presenter until retained native surface verification is complete"
    );
    assert!(
        runtime_profile_source
            .contains("pub fn terminal_subsystem_mode(self) -> TerminalSubsystemMode"),
        "runtime profile should expose the selected terminal subsystem mode so bootstrap can log and route the packaged scene-image default versus retained-native-surface bring-up path explicitly"
    );
    assert!(
        composition_block
            .contains("TerminalSubsystemMode::SceneImage => TerminalCompositionMode::SceneImage"),
        "scene-image composition should remain the explicit packaged presenter path while the retained native surface stays behind a bring-up switch"
    );
    assert!(
        composition_block.contains("TerminalSubsystemMode::RetainedNativeSurface => {")
            && composition_block.contains("TerminalCompositionMode::PostRenderNativeSurface"),
        "runtime profile should keep the retained native surface composition route available behind the explicit subsystem mode"
    );
    assert!(
        !composition_block.contains(
            "AppBuildFlavor::WindowsMainline if self.prefers_direct3d() => {\n                TerminalCompositionMode::SceneImage"
        ),
        "Windows mainline should route composition entirely through terminal_subsystem_mode instead of hard-coding scene-image by build flavor"
    );
    assert!(
        bootstrap_source.contains("profile.terminal_subsystem_mode()"),
        "bootstrap should consult the terminal subsystem mode explicitly so packaged builds keep the scene-image presenter by default while retained-native-surface stays opt-in"
    );
    assert!(
        bootstrap_source.contains("requested_render_mode = profile.terminal_render_mode_label()"),
        "workspace presenter initialization should log the requested render mode so packaged runtime diagnostics can distinguish profile intent from the active presenter that actually got installed"
    );
    assert!(
        bootstrap_source.contains("active_render_mode = active_render_mode.as_str()"),
        "workspace presenter initialization should log the active presenter mode so packaged runtime diagnostics can show whether bootstrap installed bitmap or native presentation"
    );
}

#[test]
fn terminal_host_cursor_overlay_is_bitmap_only() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        host_source.contains(
            "if root.session-render-mode == \"bitmap\" && root.session-cursor-visible && root.cursor-blink-visible : cursor-overlay := Rectangle {"
        ),
        "terminal session host should only render the Slint cursor rectangle while the bitmap fallback path owns terminal presentation"
    );
    assert!(
        host_source.contains(
            "if root.mode == \"terminal\" && root.session-render-mode == \"bitmap\" && root.session-cursor-visible && root.session-cursor-blinking {"
        ),
        "terminal host should only start cursor blink timing while bitmap mode owns the cursor overlay"
    );
}

#[test]
fn bootstrap_source_clears_host_cursor_when_native_frame_is_active() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source
            .contains("fn clear_workspace_session_cursor_overlay(window: &AppWindow) {"),
        "bootstrap should define a dedicated helper that clears the Slint cursor overlay state when native rendering owns the cursor"
    );
    assert!(
        bootstrap_source.contains(
            "if native_frame_presented {\n            clear_workspace_session_cursor_overlay(window);\n        } else {"
        ),
        "workspace terminal sync should explicitly clear the Slint cursor overlay whenever a retained native frame is presented"
    );
    assert!(
        !bootstrap_source.contains("if let Some(cursor) = native_cursor {"),
        "workspace terminal sync should stop projecting native cursor payloads back into Slint as if the host still owned the final cursor rectangle"
    );
}

#[test]
fn native_cursor_blink_source_is_driven_from_bootstrap_timer() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("struct WorkspaceNativeCursorBlinkState"),
        "bootstrap should keep native cursor blink phase in Rust so the retained child-HWND path can blink independently of the bitmap-only Slint cursor timer"
    );
    assert!(
        bootstrap_source.contains("native_cursor_blink_timer.start(")
            && bootstrap_source
                .contains("Duration::from_millis(WORKSPACE_TERMINAL_CURSOR_BLINK_INTERVAL_MS)"),
        "bootstrap should drive native cursor blinking from a repeated Rust timer because the Slint cursor timer is intentionally bitmap-only"
    );
    assert!(
        bootstrap_source.contains("workspace_native_cursor_overlay_visible_for_surface(surface)")
            && bootstrap_source
                .contains("frame.presentable_frame.cursor_overlay.visible = cursor_overlay_visible;"),
        "native presentation should override the retained cursor overlay visibility from the bootstrap-managed blink phase before presenting the frame"
    );
}

#[test]
fn retained_native_frame_sources_expose_background_display_list_contract() {
    let segmentation_source = fs::read_to_string("src/app/terminal_layout/run_segmentation.rs")
        .expect("read run segmentation");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");

    assert!(
        segmentation_source.contains("pub bg_rgba: u32"),
        "text style keys should retain background color so the Windows backend can draw ANSI backgrounds without rebuilding style state"
    );
    assert!(
        renderer_source.contains("pub struct PreparedBackgroundRun"),
        "prepared native frames should define explicit background runs for the retained display list"
    );
    assert!(
        renderer_source.contains("pub background_runs: Vec<PreparedBackgroundRun>"),
        "prepared native frames should carry retained background runs for backend consumption"
    );
    assert!(
        presenter_source.contains("pub background_runs: Vec<PreparedBackgroundRun>"),
        "presentable native frames should thread background runs through the presenter contract"
    );
}

#[test]
fn windows_backend_source_exposes_d2d_lifecycle_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");
    let windows_frame_source =
        fs::read_to_string("src/app/windows_frame.rs").expect("read windows frame interop");

    assert!(
        windows_backend_source.contains("pub struct WindowsD2DFactoryState"),
        "windows backend should define an explicit D2D factory lifecycle state"
    );
    assert!(
        windows_backend_source.contains("pub struct WindowsHwndRenderTargetState"),
        "windows backend should define an explicit HWND render-target lifecycle state"
    );
    assert!(
        windows_backend_source.contains("ID2D1HwndRenderTarget"),
        "windows backend should carry a child-surface HWND render target contract instead of a host-window DC target"
    );
    assert!(
        windows_backend_source.contains("pub d2d_factory: Option<WindowsD2DFactoryState>"),
        "windows native surface state should retain D2D factory ownership instead of only HWND and frame token bookkeeping"
    );
    assert!(
        windows_backend_source
            .contains("pub hwnd_render_target: Option<WindowsHwndRenderTargetState>"),
        "windows native surface state should retain HWND render-target ownership instead of only HWND and frame token bookkeeping"
    );
    assert!(
        windows_backend_source.contains("pub render_target_generation: u64"),
        "windows backend should track render-target generation so recreate events can invalidate stale resources"
    );
    assert!(
        windows_backend_source.contains("pub render_target_dirty: bool"),
        "windows backend should track whether the HWND render target needs rebuild after attach or rect changes"
    );
    assert!(
        windows_backend_source.contains("fn mark_render_target_dirty(&mut self)"),
        "windows backend should expose a helper that marks the render target dirty when HWND, geometry, or retained resources change"
    );
    assert!(
        windows_backend_source.contains("fn ensure_d2d_factory(&mut self)"),
        "windows backend should expose a helper that ensures D2D factory state exists before render-target creation"
    );
    assert!(
        windows_backend_source.contains("fn ensure_hwnd_render_target(&mut self)"),
        "windows backend should expose a helper that ensures an HWND render target exists before present"
    );
    assert!(
        windows_backend_source.contains("CreateHwndRenderTarget"),
        "windows backend should create a child-owned HWND render target instead of binding Direct2D to the host window DC"
    );
    assert!(
        !windows_backend_source.contains("CreateDCRenderTarget"),
        "windows backend should stop creating a DC render target once retained-native output owns its own child HWND"
    );
    assert!(
        windows_backend_source.contains("fn clear_device_resources(&mut self)"),
        "windows backend should expose a helper that clears render-target owned resources during recreate and detach"
    );
    assert!(
        windows_backend_source.contains("self.state.mark_render_target_dirty();"),
        "windows backend should mark the render target dirty when backend state changes"
    );
    assert!(
        windows_backend_source.contains("self.state.ensure_hwnd_render_target();"),
        "windows backend present path should ensure the HWND render target exists before consuming retained frames"
    );
    assert!(
        windows_frame_source.contains("use slint::ComponentHandle;"),
        "windows frame interop should import ComponentHandle so host HWND resolution compiles on Windows targets"
    );
}

#[test]
fn windows_backend_source_exposes_background_and_monochrome_draw_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("pub struct WindowsD2DBrushState"),
        "windows backend should define a retained brush cache contract for Direct2D background and glyph fills"
    );
    assert!(
        windows_backend_source.contains("pub struct WindowsMonochromeGlyphBitmapState"),
        "windows backend should define a retained monochrome glyph bitmap cache contract"
    );
    assert!(
        windows_backend_source.contains("pub d2d_brushes: HashMap<u32, WindowsD2DBrushState>"),
        "windows native surface state should retain Direct2D brush cache state for repeated background and glyph colors"
    );
    assert!(
        windows_backend_source.contains(
            "pub monochrome_glyph_bitmaps: HashMap<u32, WindowsMonochromeGlyphBitmapState>"
        ),
        "windows native surface state should retain monochrome glyph bitmap cache state keyed by atlas slot"
    );
    assert!(
        windows_backend_source.contains("fn ensure_brush(&mut self, rgba: u32)"),
        "windows backend should expose a helper that ensures a brush exists before background or glyph drawing"
    );
    assert!(
        windows_backend_source.contains(
            "fn ensure_monochrome_glyph_bitmap(&mut self, draw: &PreparedMonochromeGlyphDraw)"
        ),
        "windows backend should expose a helper that creates retained monochrome glyph bitmap resources from upload payloads"
    );
    assert!(
        windows_backend_source.contains(
            "fn draw_background_runs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame)"
        ),
        "windows backend should expose an explicit background-run draw stage"
    );
    assert!(
        windows_backend_source.contains(
            "fn draw_monochrome_glyphs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame)"
        ),
        "windows backend should expose an explicit monochrome glyph draw stage"
    );
    assert!(
        windows_backend_source.contains("frame.frame.presentable_frame.background_runs"),
        "background draw stage should iterate retained background runs instead of inferring ANSI colors again"
    );
    assert!(
        windows_backend_source.contains("frame.frame.presentable_frame.monochrome_glyph_draws"),
        "monochrome glyph draw stage should iterate retained monochrome glyph draws instead of reshaping text"
    );
    assert!(
        windows_backend_source.contains("draw.upload.as_ref()"),
        "monochrome glyph bitmap creation should consume upload payloads on first use"
    );
    assert!(
        windows_backend_source.contains("self.state.draw_background_runs(frame);"),
        "windows backend present path should draw background runs before text"
    );
    assert!(
        windows_backend_source.contains("self.state.draw_monochrome_glyphs(frame);"),
        "windows backend present path should draw monochrome glyphs after background fills"
    );
    assert!(
        windows_backend_source.contains("BeginDraw();"),
        "windows backend should start a real Direct2D draw pass before consuming retained frame stages"
    );
    assert!(
        windows_backend_source.contains("EndDraw("),
        "windows backend should finish the Direct2D draw pass and surface any target-loss errors"
    );
    assert!(
        windows_backend_source.contains("PushAxisAlignedClip("),
        "windows backend should clip drawing to the retained terminal surface rect"
    );
    assert!(
        windows_backend_source.contains("presentOptions: D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS"),
        "child-HWND presents should retain previous contents between clipped redraws, otherwise overlay-only or damage-rect redraws can expose undefined background behind DirectWrite text"
    );
    assert!(
        windows_backend_source.contains("PopAxisAlignedClip();"),
        "windows backend should release the terminal clip after drawing the retained frame"
    );
    assert!(
        windows_backend_source.contains("FillRectangle("),
        "windows backend should use Direct2D rectangle fills for row backgrounds and ANSI background runs"
    );
    assert!(
        windows_backend_source.contains("FillOpacityMask("),
        "windows backend should draw monochrome glyph alpha masks through Direct2D instead of only counting cache entries"
    );
    assert!(
        windows_backend_source.contains("fn draw_directwrite_text("),
        "Task 4 should add a dedicated DirectWrite text draw stage so Windows mainline text no longer depends only on bitmap opacity masks"
    );
    assert!(
        windows_backend_source.contains("self.state.draw_directwrite_text(frame);"),
        "windows backend present path should run the DirectWrite text stage before compatibility bitmap fallback logic"
    );
    assert!(
        windows_backend_source.contains("DrawGlyphRun("),
        "windows backend should issue real DirectWrite glyph draw calls for the primary monochrome text path"
    );
    assert!(
        windows_backend_source.contains("D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE"),
        "windows backend should request ClearType-capable text antialiasing for the primary DirectWrite path"
    );
}

#[test]
fn windows_backend_source_exposes_color_glyph_and_overlay_draw_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("pub struct WindowsColorGlyphBitmapState"),
        "windows backend should define a retained color glyph bitmap cache contract"
    );
    assert!(
        windows_backend_source
            .contains("pub color_glyph_bitmaps: HashMap<u32, WindowsColorGlyphBitmapState>"),
        "windows native surface state should retain color glyph bitmap cache state keyed by color cache slot"
    );
    assert!(
        windows_backend_source
            .contains("fn ensure_color_glyph_bitmap(&mut self, draw: &PreparedColorGlyphDraw)"),
        "windows backend should expose a helper that creates retained color glyph bitmap resources from upload payloads"
    );
    assert!(
        windows_backend_source.contains(
            "fn draw_color_glyphs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame)"
        ),
        "windows backend should expose an explicit color glyph draw stage"
    );
    assert!(
        windows_backend_source.contains(
            "fn draw_selection_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame)"
        ),
        "windows backend should expose an explicit selection overlay draw stage"
    );
    assert!(
        windows_backend_source.contains(
            "fn draw_underline_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame)"
        ),
        "windows backend should expose an explicit underline overlay draw stage"
    );
    assert!(
        windows_backend_source.contains(
            "fn draw_cursor_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame)"
        ),
        "windows backend should expose an explicit cursor overlay draw stage"
    );
    assert!(
        windows_backend_source.contains(
            "fn draw_ime_preview_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame)"
        ),
        "windows backend should expose an explicit IME preview overlay draw stage"
    );
    assert!(
        windows_backend_source.contains("self.state.draw_selection_overlay(frame);"),
        "windows backend present path should draw selection overlays after background fills"
    );
    assert!(
        windows_backend_source.contains("self.state.draw_color_glyphs(frame);"),
        "windows backend present path should draw color glyphs after monochrome glyphs are prepared"
    );
    assert!(
        windows_backend_source.contains("self.state.draw_underline_overlay(frame);"),
        "windows backend present path should draw underline overlays after text"
    );
    assert!(
        windows_backend_source.contains("self.state.draw_cursor_overlay(frame);"),
        "windows backend present path should draw the cursor overlay after text and underline passes"
    );
    assert!(
        windows_backend_source.contains("self.state.draw_ime_preview_overlay(frame);"),
        "windows backend present path should draw IME preview after the cursor overlay"
    );
    assert!(
        windows_backend_source.contains("DrawBitmap("),
        "windows backend should draw color glyph bitmaps through Direct2D instead of only tracking cache slots"
    );
    assert!(
        windows_backend_source.contains("D2DERR_RECREATE_TARGET"),
        "windows backend should handle Direct2D target-loss by invalidating render-target owned resources"
    );
    assert!(
        windows_backend_source.contains("CreateMonitorRenderingParams"),
        "windows backend should source monitor-aware DirectWrite rendering parameters instead of hard-coded text AA heuristics"
    );
    assert!(
        windows_backend_source.contains("SetTextRenderingParams("),
        "windows backend should bind DirectWrite rendering params onto the Direct2D text target before drawing glyph runs"
    );
    assert!(
        windows_backend_source.contains("D2D1_ALPHA_MODE_IGNORE"),
        "windows backend should keep the primary text draw target opaque so ClearType-friendly rendering does not go through a premultiplied-alpha text surface"
    );
}

#[test]
fn native_surface_source_exposes_damage_tracker_and_shutdown_guard_contract() {
    assert!(
        Path::new("src/app/terminal_renderer/damage.rs").exists(),
        "terminal renderer should add a dedicated damage tracker module for resize, overlay-only, and shutdown invalidation"
    );

    let damage_source = fs::read_to_string("src/app/terminal_renderer/damage.rs")
        .expect("read native surface damage tracker");
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let renderer_mod_source =
        fs::read_to_string("src/app/terminal_renderer/mod.rs").expect("read terminal renderer mod");

    assert!(
        renderer_mod_source.contains("pub mod damage;"),
        "terminal renderer module should expose the damage tracker module"
    );
    assert!(
        renderer_mod_source.contains("NativeFrameDamageTracker"),
        "terminal renderer module should re-export the native frame damage tracker contract"
    );
    assert!(
        damage_source.contains("pub struct NativeFrameDamageTracker"),
        "damage module should define a dedicated tracker for retained native frame invalidation"
    );
    assert!(
        damage_source.contains("pub enum NativeSurfaceDamageKind"),
        "damage module should classify pending invalidation as full-surface or overlay-only damage"
    );
    assert!(
        damage_source.contains("fn mark_full_damage(&mut self, rect: NativeTerminalSurfaceRect)"),
        "damage tracker should expose a helper that invalidates the full retained surface after resize"
    );
    assert!(
        damage_source.contains("fn track_frame_damage("),
        "damage tracker should expose a helper that compares retained frames before scheduling present"
    );
    assert!(
        damage_source.contains("previous.frame.frame_token == next.frame.frame_token"),
        "damage tracker should treat stable prepared-frame tokens as a signal that overlay-only changes can repaint without a text rebuild"
    );
    assert!(
        damage_source.contains("previous.frame.presentable_frame.cursor_overlay")
            && damage_source.contains("previous.frame.presentable_frame.selection_overlay")
            && damage_source.contains("previous.frame.presentable_frame.ime_preview_overlay")
            && damage_source.contains("previous.frame.presentable_frame.underline_overlay"),
        "damage tracker should compare cursor, selection, underline, and IME overlay payloads when deciding whether overlay-only damage exists"
    );
    assert!(
        native_surface_source.contains("damage_tracker: NativeFrameDamageTracker"),
        "native surface bridge should retain a damage tracker alongside the retained frame and diagnostics state"
    );
    assert!(
        native_surface_source.contains("surface_alive: bool"),
        "native surface bridge should keep an explicit alive/detached lifecycle guard for shutdown sequencing"
    );
    assert!(
        native_surface_source.contains("state.damage_tracker.mark_full_damage(rect);"),
        "native surface bridge should invalidate the whole retained surface when the terminal rect changes"
    );
    assert!(
        native_surface_source
            .contains(".track_frame_damage(previous_frame.as_ref(), Some(&next_frame));"),
        "native surface bridge should feed retained-frame transitions through the damage tracker before present"
    );
    assert!(
        native_surface_source.contains("if !state.surface_alive {"),
        "native surface draw callbacks should bail out once the surface has detached"
    );
    assert!(
        native_surface_source.contains("state.surface_alive = false;"),
        "native surface teardown should flip the alive flag before backend detach completes"
    );
}

#[test]
fn native_surface_damage_contract_threads_damage_kind_into_backend_present() {
    let damage_source = fs::read_to_string("src/app/terminal_renderer/damage.rs")
        .expect("read native surface damage tracker");
    let backend_source = fs::read_to_string("src/app/terminal_renderer/platform/backend.rs")
        .expect("read platform backend contract");
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows native surface backend");

    assert!(
        damage_source.contains("fn take_damage(&mut self) -> Option<NativeSurfaceDamage>"),
        "damage tracker should expose a take_damage helper so native surface present can consume the pending invalidation kind instead of flattening everything into a boolean"
    );
    assert!(
        backend_source.contains("fn present(&mut self, damage: NativeSurfaceDamage)"),
        "platform backend contract should accept the concrete native-surface damage payload during present so overlay-only invalidation can avoid the same path as a full repaint"
    );
    assert!(
        native_surface_source
            .contains("let damage = state.damage_tracker.take_damage().unwrap_or_default();"),
        "native surface draw hook should consume the pending damage payload before dispatching present"
    );
    assert!(
        native_surface_source.contains("state.backend.present(damage);"),
        "native surface draw hook should forward the concrete damage payload into the backend present call"
    );
    assert!(
        windows_backend_source.contains("fn present(&mut self, damage: NativeSurfaceDamage)"),
        "windows native surface backend should accept the damage payload in its present entrypoint"
    );
    assert!(
        windows_backend_source.contains("match damage.kind")
            || windows_backend_source.contains("if matches!(damage.kind"),
        "windows native surface backend should branch on damage kind so overlay-only invalidation can stop sharing the exact same redraw path as a full repaint"
    );
}

#[test]
fn windows_backend_source_clips_present_to_damage_rect() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows native surface backend");

    assert!(
        windows_backend_source
            .contains("let present_rect = translate_rect_to_surface_local(")
            && windows_backend_source
                .contains("resolved_present_rect(self.state.rect, damage),"),
        "windows backend should resolve a present clip rect from the damage payload before starting a draw pass"
    );
    assert!(
        windows_backend_source.contains("if !self.state.begin_frame(present_rect)"),
        "windows backend should bind Direct2D against the damage-scoped present rect instead of always clipping the full terminal surface"
    );
    assert!(
        windows_backend_source.contains("fn resolved_present_rect("),
        "windows backend should expose a helper that clamps overlay-only damage to a valid present rect"
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

    let platform_mod_source =
        fs::read_to_string("src/app/terminal_renderer/platform/mod.rs").expect("read platform mod");
    let backend_source = fs::read_to_string("src/app/terminal_renderer/platform/backend.rs")
        .expect("read platform backend contract");
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let renderer_mod_source =
        fs::read_to_string("src/app/terminal_renderer/mod.rs").expect("read terminal renderer mod");
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
        backend_source
            .contains("fn update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect)"),
        "shared platform backend contract should expose a surface-rect update hook"
    );
    assert!(
        backend_source.contains(
            "fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>)"
        ),
        "shared platform backend contract should expose a retained-frame update hook"
    );
    assert!(
        backend_source.contains("fn present(&mut self, damage: NativeSurfaceDamage)"),
        "shared platform backend contract should expose a damage-aware present hook"
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
        native_surface_source.contains("state.backend.present(damage);"),
        "native surface bridge should ask the shared backend to present with the concrete damage payload during retained-frame draw"
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

    let platform_mod_source =
        fs::read_to_string("src/app/terminal_renderer/platform/mod.rs").expect("read platform mod");
    let renderer_mod_source =
        fs::read_to_string("src/app/terminal_renderer/mod.rs").expect("read terminal renderer mod");

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
fn windows_backend_source_exposes_lazy_host_hwnd_reacquire_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("host_window: Option<slint::Weak<AppWindow>>"),
        "windows backend should retain a weak AppWindow handle so it can retry host HWND resolution after bootstrap finishes and the native winit window actually exists"
    );
    assert!(
        windows_backend_source.contains("self.host_window = Some(window.as_weak());"),
        "windows backend attach should store the host window handle instead of resolving HWND only once during bootstrap"
    );
    assert!(
        windows_backend_source.contains("fn resolve_host_hwnd_if_needed(&mut self)"),
        "windows backend should expose a helper that retries host HWND resolution when present happens before the first native window lookup succeeds"
    );
    assert!(
        windows_backend_source.contains("self.resolve_host_hwnd_if_needed();"),
        "windows backend should retry host HWND resolution during runtime updates instead of permanently staying detached after an early attach"
    );
}

#[test]
fn windows_backend_source_exposes_child_hwnd_present_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");
    let child_host_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows_child_host.rs")
            .expect("read windows child-host helper");

    assert!(
        windows_backend_source.contains("host_hwnd: Option<isize>"),
        "windows backend should retain the top-level host HWND so the child presenter can stay parented to the real shell window"
    );
    assert!(
        windows_backend_source.contains("surface_hwnd: Option<isize>"),
        "windows backend should retain a dedicated child surface HWND so retained-native rendering stops drawing through the host window DC"
    );
    assert!(
        child_host_source.contains("CreateWindowExW("),
        "windows backend should create a dedicated child HWND so retained-native output owns its own presentation boundary"
    );
    assert!(
        child_host_source.contains("SetWindowPos("),
        "windows backend should move the child HWND whenever the pane rect changes"
    );
    assert!(
        child_host_source.contains("DestroyWindow("),
        "windows backend should destroy the child HWND during detach so retained-native host state does not leak after the pane disappears"
    );
    assert!(
        child_host_source.contains("RegisterClassW(") || child_host_source.contains("WNDCLASSW"),
        "windows child-host helper should register a dedicated window class instead of borrowing a stock control class so retained-native D2D output owns a predictable Win32 surface"
    );
    assert!(
        !child_host_source.contains("w!(\"STATIC\")"),
        "windows child-host helper should stop creating retained-native presenters as stock STATIC controls because those controls can carry class behavior that fights D2D child-surface ownership"
    );
    assert!(
        !windows_backend_source.contains("GetDC(HWND(host_hwnd as _))"),
        "windows backend should stop acquiring a host-window DC once retained-native output is isolated behind a child HWND"
    );
    assert!(
        !windows_backend_source.contains("render_target.BindDC("),
        "windows backend should stop binding a Direct2D DC target to the host window once a child HWND owns presentation"
    );
    assert!(
        windows_backend_source.contains("self.rect = NativeTerminalSurfaceRect {")
            && windows_backend_source.contains("x: 0,")
            && windows_backend_source.contains("y: 0,"),
        "windows backend should keep retained frame drawing in child-local coordinates even though the diagnostics later project those bounds back into host coordinates"
    );
}

#[test]
fn windows_backend_source_exposes_child_surface_window_lifecycle_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("pub window_rect: NativeTerminalSurfaceRect"),
        "windows backend state should retain the Slint-reported terminal rect so the child HWND can stay aligned with the terminal pane"
    );
    assert!(
        windows_backend_source.contains("host_hwnd: Option<isize>"),
        "windows backend should retain the top-level host HWND so the child presenter can stay attached to the shell window"
    );
    assert!(
        windows_backend_source.contains("fn ensure_child_surface_window(&mut self)"),
        "windows backend should expose a helper that lazily creates the retained-native child host window"
    );
    assert!(
        windows_backend_source.contains("fn sync_child_surface_window_rect(&mut self)"),
        "windows backend should expose a helper that keeps the child host window aligned with the pane rect"
    );
    assert!(
        windows_backend_source.contains("fn destroy_child_surface_window(&mut self)"),
        "windows backend should expose a helper that tears down the child host window during detach or parent HWND changes"
    );
    assert!(
        windows_backend_source.contains("self.ensure_child_surface_window();"),
        "windows backend should ensure the child host exists during attach and present before using retained-native state"
    );
    assert!(
        windows_backend_source.contains("self.sync_child_surface_window_rect();"),
        "windows backend should resync child-host geometry whenever layout updates or host HWND resolution changes"
    );
    assert!(
        windows_backend_source.contains("retained_frame.rect = self.state.rect;"),
        "windows backend should keep threading child-local rects into retained frames while the child host owns visible presentation"
    );
    assert!(
        windows_backend_source.contains("self.destroy_child_surface_window();"),
        "windows backend detach path should tear down the retained-native child HWND when the backend detaches from the shell"
    );
}

#[test]
fn bootstrap_source_exposes_native_surface_scale_factor_bridge_contract() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("fn window_scale_factor(window: &AppWindow) -> f32"),
        "bootstrap should expose a helper that reads the live Slint window scale factor before bridging logical layout lengths into native surface pixels"
    );
    assert!(
        bootstrap_source.contains("let scale_factor = window_scale_factor(window);"),
        "native surface geometry and cell metrics should be converted with the live window scale factor instead of assuming logical and physical pixels are identical on Windows"
    );
    assert!(
        bootstrap_source
            .contains("window.get_layout_workspace_session_native_surface_x() * scale_factor"),
        "workspace native terminal x should be converted from Slint logical length into physical child-HWND coordinates"
    );
    assert!(
        bootstrap_source
            .contains("window.get_layout_workspace_session_native_surface_width() * scale_factor"),
        "workspace native terminal width should be converted from Slint logical length into physical child-HWND coordinates"
    );
    assert!(
        bootstrap_source.contains("default_cell_width_px as f32 / scale_factor"),
        "native terminal cell width should be projected back into Slint logical units so cursor overlays and resize math stay aligned on HiDPI Windows displays"
    );
    assert!(
        bootstrap_source.contains("frame.cell_width_px as f32 / scale_factor"),
        "prepared native frame cell width should be projected back into Slint logical units instead of being exposed as raw physical pixels"
    );
    assert!(
        bootstrap_source
            .contains("window.set_workspace_session_device_scale_factor(scale_factor);"),
        "bootstrap should thread the live window scale factor into the Slint workspace session contract so the scene-image host can snap terminal origin and viewport lengths onto physical pixels"
    );
}

#[test]
fn bootstrap_source_offsets_native_surface_y_by_titlebar_height() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let rect_block = block_between(
        &bootstrap_source,
        "fn workspace_native_terminal_rect(window: &AppWindow) -> NativeTerminalSurfaceRect {",
        "\n}\n\nfn sync_workspace_native_terminal_surface_geometry(window: &AppWindow) {",
    );

    let titlebar = rect_block
        .find("window.get_layout_titlebar_height()")
        .expect("titlebar height in native rect calculation");
    let surface_y = rect_block
        .find("window.get_layout_workspace_session_native_surface_y()")
        .expect("surface y in native rect calculation");

    assert!(
        titlebar < surface_y,
        "native child-HWND y should include the custom titlebar offset before projecting the workspace terminal y into client coordinates, otherwise the retained surface drifts upward into the tab strip"
    );
}

#[test]
fn wayland_platform_backend_source_exposes_backend_selection_contract() {
    assert!(
        Path::new("src/app/terminal_renderer/platform/wayland.rs").exists(),
        "terminal renderer should add a Wayland platform backend source file"
    );

    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read cargo toml");
    let platform_mod_source =
        fs::read_to_string("src/app/terminal_renderer/platform/mod.rs").expect("read platform mod");
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
    let platform_mod_source =
        fs::read_to_string("src/app/terminal_renderer/platform/mod.rs").expect("read platform mod");
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
        platform_mod_source.contains("DISPLAY") || platform_mod_source.contains("XDG_SESSION_TYPE"),
        "platform backend factory should detect X11 host sessions through standard Linux environment hints"
    );
    assert!(
        native_surface_source.contains("create_platform_native_surface_backend()"),
        "native surface bridge should keep using the shared platform backend factory when X11 support lands"
    );
}

#[test]
fn windows_backend_source_tears_down_child_surface_when_not_visible() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("if !should_show {")
            && windows_backend_source.contains("self.destroy_child_surface_window();"),
        "windows backend should tear down the retained-native child HWND whenever the pane is hidden or modal-blocked so Slint overlays cannot race a stale child window that only received a best-effort hide request"
    );
}

#[test]
fn windows_child_host_source_keeps_parent_terminal_input_route_alive() {
    let child_host_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows_child_host.rs")
            .expect("read windows child host helper");

    assert!(
        child_host_source.contains("WM_NCHITTEST") && child_host_source.contains("HTTRANSPARENT"),
        "retained-native child HWND host should return HTTRANSPARENT for hit-testing so the parent Slint terminal surface keeps receiving mouse input instead of the child presenter swallowing clicks and selection gestures"
    );
    assert!(
        child_host_source.contains("WM_MOUSEACTIVATE")
            && child_host_source.contains("MA_NOACTIVATE"),
        "retained-native child HWND host should refuse activation on mouse input so keyboard focus stays on the parent terminal input path until child-HWND input ownership is implemented deliberately"
    );
}

#[test]
fn windows_child_host_source_paints_opaque_fallback_background() {
    let child_host_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows_child_host.rs")
            .expect("read windows child host helper");
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows backend");

    assert!(
        child_host_source.contains("pub fn set_background_rgba(&self, rgba: u32)")
            && child_host_source.contains("SetWindowLongPtrW")
            && child_host_source.contains("InvalidateRect"),
        "retained-native child HWND host should expose a background-color setter that invalidates the child window so native fallback paint never leaves a freshly created or resized surface visually transparent"
    );
    assert!(
        child_host_source.contains("fn paint_retained_native_child_host_background(")
            && child_host_source.contains("WM_ERASEBKGND => {")
            && child_host_source.contains("WM_PAINT => {"),
        "retained-native child HWND host should actively paint an opaque fallback background during WM_ERASEBKGND and WM_PAINT instead of validating an unpainted child surface that can show through the translucent shell window"
    );
    assert!(
        windows_backend_source.contains("child_surface_host.set_background_rgba("),
        "windows backend should feed the current terminal default background into the retained child HWND so fallback GDI paints match the active terminal surface"
    );
}

#[test]
fn windows_host_frame_source_clips_child_surfaces_for_native_terminal() {
    let windows_frame_source =
        fs::read_to_string("src/app/windows_frame.rs").expect("read windows frame source");

    assert!(
        windows_frame_source.contains("WS_CLIPCHILDREN")
            && windows_frame_source.contains("SetWindowLongPtrW")
            && windows_frame_source.contains("GWL_STYLE"),
        "windows host frame adapter should force WS_CLIPCHILDREN onto the top-level host HWND so parent Mica/Skia repaints cannot redraw through the retained native child terminal surface"
    );
}
