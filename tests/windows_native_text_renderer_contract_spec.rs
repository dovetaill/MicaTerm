use std::fs;

#[test]
fn windows_native_text_renderer_source_exposes_directwrite_primary_text_path() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows native backend");

    assert!(
        windows_backend_source.contains("pub struct WindowsDirectWriteTextRendererState"),
        "Task 4 should define explicit retained DirectWrite renderer state instead of hiding text draw mode behind the monochrome bitmap cache"
    );
    assert!(
        windows_backend_source.contains("fn ensure_directwrite_text_renderer(&mut self)"),
        "Task 4 should expose a helper that prepares the DirectWrite text renderer before the present pass"
    );
    assert!(
        windows_backend_source.contains("fn draw_directwrite_text("),
        "Task 4 should expose a dedicated DirectWrite text stage for primary monochrome terminal text"
    );
    assert!(
        windows_backend_source.contains("fn resolve_directwrite_font_face("),
        "Task 4 should resolve a real DirectWrite font face for each retained monochrome glyph draw"
    );
    assert!(
        windows_backend_source.contains("DrawGlyphRun("),
        "Task 4 should issue DirectWrite glyph-run drawing instead of relying only on FillOpacityMask monochrome blits"
    );
    assert!(
        windows_backend_source.contains("ID2D1HwndRenderTarget")
            || windows_backend_source.contains("CreateHwndRenderTarget"),
        "child-host retained-native text rendering should target a child-owned HWND render target instead of a host-window DC binding path"
    );
    assert!(
        !windows_backend_source.contains("GetDC(HWND(host_hwnd as _))")
            && !windows_backend_source.contains("BindDC(hdc, &bind_rect)"),
        "child-host retained-native text rendering should stop binding Direct2D to the host window DC"
    );
}

#[test]
fn windows_native_text_renderer_source_uses_monitor_aware_clear_type_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows native backend");

    for expected in [
        "CreateMonitorRenderingParams",
        "SetTextRenderingParams(",
        "SetTextAntialiasMode(",
        "D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE",
        "D2D1_ALPHA_MODE_IGNORE",
    ] {
        assert!(
            windows_backend_source.contains(expected),
            "windows native text path should reference `{expected}` so ClearType-friendly rendering uses monitor-aware params on an opaque target"
        );
    }

    for expected in [
        "CreateCustomRenderingParams",
        "GetGamma()",
        "GetEnhancedContrast()",
        "GetClearTypeLevel()",
        "GetPixelGeometry()",
        "GetRenderingMode()",
    ] {
        assert!(
            windows_backend_source.contains(expected),
            "windows native text path should reference `{expected}` so monitor params can be inspected and, when needed, promoted into explicit DirectWrite rendering params instead of leaving the chain under-specified"
        );
    }
}

#[test]
fn windows_native_text_renderer_source_tracks_true_fallback_path_when_directwrite_bails() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows native backend");

    for expected in [
        "fn mark_directwrite_text_fallback(&mut self, reason: &'static str)",
        "fallback_reason = reason",
        "self.mark_directwrite_text_fallback(\"glyph-id-overflow\")",
        "self.mark_directwrite_text_fallback(\"font-face-unresolved\")",
        "self.mark_directwrite_text_fallback(\"missing-text-brush\")",
    ] {
        assert!(
            windows_backend_source.contains(expected),
            "windows native text renderer should reference `{expected}` so diagnostics stop claiming the directwrite path stayed active after the draw stage bails out to bitmap fallback"
        );
    }
}

#[test]
fn windows_native_text_renderer_source_can_build_directwrite_faces_for_bundled_fonts() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows native backend");

    for expected in [
        "CreateInMemoryFontFileLoader",
        "CreateInMemoryFontFileReference",
        "RegisterFontFileLoader",
        "DWRITE_FONT_FACE_TYPE_TRUETYPE",
    ] {
        assert!(
            windows_backend_source.contains(expected),
            "windows native text renderer should reference `{expected}` so bundled terminal fonts like JetBrains Mono and Sarasa Term SC can stay on the DirectWrite path even when they are not installed in the system font collection"
        );
    }
}

#[test]
fn windows_native_text_renderer_source_avoids_duplicate_directwrite_rendering_mode_alias_arms() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows native backend");

    for forbidden in [
        "DWRITE_RENDERING_MODE_GDI_CLASSIC =>",
        "DWRITE_RENDERING_MODE_GDI_NATURAL =>",
        "DWRITE_RENDERING_MODE_NATURAL =>",
        "DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC =>",
    ] {
        assert!(
            !windows_backend_source.contains(forbidden),
            "windows native text renderer should avoid `{forbidden}` because the windows crate aliases these values to the ClearType constants and the extra match arms trigger unreachable-pattern warnings in Windows builds"
        );
    }
}

#[test]
fn native_surface_diagnostics_source_exposes_text_renderer_path() {
    let diagnostics_source = fs::read_to_string("src/app/terminal_renderer/diagnostics.rs")
        .expect("read native surface diagnostics");
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows backend");
    let windows_frame_source =
        fs::read_to_string("src/app/windows_frame.rs").expect("read windows frame helper");

    assert!(
        diagnostics_source.contains("pub text_renderer_path: Option<&'static str>"),
        "native surface diagnostics should expose the active Windows text renderer path for runtime inspection"
    );
    assert!(
        windows_backend_source.contains("text_renderer_path: Some("),
        "windows backend diagnostics snapshot should publish which primary text renderer path is active"
    );
    assert!(
        native_surface_source
            .contains("let mut diagnostics = state.backend.diagnostics_snapshot();")
            && native_surface_source
                .contains("diagnostics.scheduled_present_count = state.scheduled_present_count;")
            && native_surface_source.contains(
                "diagnostics.host_redraw_request_count = state.host_redraw_request_count;"
            )
            && native_surface_source
                .contains("diagnostics.host_redraw_replay_count = state.host_redraw_replay_count;")
            && native_surface_source.contains("state.latest_diagnostics = diagnostics;"),
        "native surface should keep refreshing the diagnostics snapshot after backend state changes and surface present/redraw counter state"
    );
    assert!(
        windows_frame_source.contains("pub fn native_surface_text_renderer_path("),
        "windows frame helpers should expose a stable accessor for the native surface text renderer diagnostics field"
    );
}

#[test]
fn native_surface_rollout_sources_document_default_and_rollback_paths() {
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let scene_image_source =
        fs::read_to_string("src/app/terminal_scene_image.rs").expect("read scene image");
    let readme_source = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        native_surface_source.contains("MICA_TERM_TERMINAL_SUBSYSTEM=retained-native-surface"),
        "native surface source should document the explicit retained-native-surface bring-up switch now that packaged Windows mainline now defaults to the retained-native presenter"
    );
    assert!(
        presenter_source.contains("MICA_TERM_TERMINAL_SUBSYSTEM=scene-image"),
        "terminal presenter source should document the scene-image rollback knob while packaged Windows mainline now defaults to the retained-native presenter"
    );
    assert!(
        scene_image_source.contains("MICA_TERM_TERMINAL_SUBSYSTEM=scene-image"),
        "scene-image renderer source should document that packaged Windows mainline only uses the scene-owned image path for explicit rollback or verification runs via the scene-image override"
    );
    assert!(
        readme_source.contains("MICA_TERM_TERMINAL_SUBSYSTEM=scene-image"),
        "readme should document the subsystem override contract now that packaged Windows mainline defaults to retained-native and scene-image is the rollback path"
    );
    assert!(
        readme_source.contains("child HWND") || readme_source.contains("child host"),
        "readme should describe the retained-native Windows presenter as a dedicated child host window instead of leaving the architecture ambiguous"
    );
    assert!(
        !readme_source.contains("same-HWND native surface"),
        "readme should stop documenting the retired same-HWND DC overlay architecture now that child-host retained-native is the approved direction"
    );
}
