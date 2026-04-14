use std::fs;

#[test]
fn font_diagnostics_module_defines_explicit_ui_and_terminal_font_contracts() {
    let app_mod = fs::read_to_string("src/app/mod.rs").expect("read app mod");
    let diagnostics_source =
        fs::read_to_string("src/app/font_diagnostics.rs").expect("read font diagnostics");

    assert!(
        app_mod.contains("pub(crate) mod font_diagnostics;")
            || app_mod.contains("pub mod font_diagnostics;"),
        "app module should expose a dedicated font diagnostics module so UI and terminal font tracing stays centralized"
    );

    for expected in [
        "pub const UI_FONT_FAMILY: &str = \"JetBrains Maple Mono\";",
        "pub const TERMINAL_PRIMARY_FAMILY: &str = \"Sarasa Term SC Nerd\";",
        "pub const TERMINAL_EMOJI_FALLBACK_FAMILY: &str = \"Segoe UI Emoji\";",
        "pub const UI_FALLBACK_FAMILIES: &[&str]",
        "pub const TERMINAL_NERD_FALLBACK_FAMILIES: &[&str]",
        "struct FontFaceMatchDiagnostic",
        "requested_family",
        "resolved_family",
        "fallback_family",
        "weight",
        "style",
        "source",
        "configure_ui_font_fallbacks",
        "log_ui_shell_font_diagnostics",
        "log_ui_text_renderer_diagnostics",
        "log_terminal_font_diagnostics",
        "terminal_letter_spacing_px",
    ] {
        assert!(
            diagnostics_source.contains(expected),
            "font diagnostics source should declare `{expected}` so packaged Windows runs stop treating font resolution as a black box"
        );
    }
}

#[test]
fn bootstrap_and_presenters_emit_explicit_font_resolution_logs() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read presenter");
    let diagnostics_source =
        fs::read_to_string("src/app/font_diagnostics.rs").expect("read font diagnostics");
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows backend");

    for expected in [
        "configure_ui_font_fallbacks();",
        "log_ui_shell_font_diagnostics(",
        "log_ui_text_renderer_diagnostics(",
        "\"ui shell font resolution established\"",
        "\"ui text renderer configuration established\"",
        "\"terminal font resolution established\"",
        "\"native terminal font chain changed\"",
    ] {
        let found = bootstrap_source.contains(expected)
            || presenter_source.contains(expected)
            || diagnostics_source.contains(expected)
            || windows_backend_source.contains(expected);
        assert!(
            found,
            "runtime sources should emit `{expected}` so Windows package logs expose real UI/terminal family matches and fallback chains"
        );
    }
}

#[test]
fn windows_terminal_symbol_fallback_waits_for_a_real_primary_glyph_miss() {
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite");
    let fallback_source =
        fs::read_to_string("src/app/terminal_font/windows_fallback.rs").expect("read fallback");

    for expected in [
        "fn primary_face_supports_text(",
        "primary_supports_text",
        "!primary_supports_text",
        "contains_symbol_text(text)",
    ] {
        let found = dwrite_source.contains(expected) || fallback_source.contains(expected);
        assert!(
            found,
            "Windows fallback code should expose `{expected}` so symbols stay on Sarasa when the primary bundled face already covers them"
        );
    }
}

#[test]
fn windows_private_use_fallback_contract_prefers_nerd_font_candidates() {
    let diagnostics_source =
        fs::read_to_string("src/app/font_diagnostics.rs").expect("read font diagnostics");
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite");
    let fallback_source =
        fs::read_to_string("src/app/terminal_font/windows_fallback.rs").expect("read fallback");

    for expected in [
        "TERMINAL_NERD_FALLBACK_FAMILIES",
        "\"Symbols Nerd Font Mono\"",
        "\"Sarasa Term SC Nerd\"",
        "\"\"",
    ] {
        let found = diagnostics_source.contains(expected)
            || dwrite_source.contains(expected)
            || fallback_source.contains(expected);
        assert!(
            found,
            "font fallback sources should expose `{expected}` so private-use prompt icons stop silently degrading into unrelated system icon fonts"
        );
    }
}

#[test]
fn native_terminal_frame_projection_trace_stays_out_of_debug_log_noise() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("tracing::trace!(")
            && bootstrap_source
                .contains("\"projecting native terminal frame into host-owned surface\""),
        "native frame projection should stay on trace-only logging so debug runs can focus on font resolution instead of per-frame surface spam"
    );
}
