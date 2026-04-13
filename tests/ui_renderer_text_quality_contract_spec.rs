use std::fs;

#[test]
fn vendored_slint_skia_item_renderer_prefers_grayscale_for_windows_shell_text() {
    let item_renderer_source = fs::read_to_string("vendor/i-slint-renderer-skia/itemrenderer.rs")
        .expect("read skia item renderer");

    for expected in [
        "font.set_subpixel(true);",
        "font.set_edging(skia_safe::font::Edging::AntiAlias);",
        "font.set_hinting(skia_safe::FontHinting::Normal);",
    ] {
        assert!(
            item_renderer_source.contains(expected),
            "vendored skia item renderer should keep `{expected}` so Windows shell chrome text stays on grayscale AA with subpixel positioning instead of forcing LCD color fringing on the composited dark-mode shell"
        );
    }
}

#[test]
fn vendored_slint_d3d_surface_pins_srgb_surface_props_for_ui_text() {
    let d3d_surface_source = fs::read_to_string("vendor/i-slint-renderer-skia/d3d_surface.rs")
        .expect("read skia d3d surface");

    for expected in [
        "let surface_color_space = skia_safe::ColorSpace::new_srgb();",
        "let surface_props = SurfaceProps::new(",
        "SurfacePropsFlags::USE_DEVICE_INDEPENDENT_FONTS",
        "PixelGeometry::Unknown",
        "Some(surface_color_space)",
        "Some(&surface_props)",
    ] {
        assert!(
            d3d_surface_source.contains(expected),
            "vendored skia d3d surface should keep `{expected}` so Windows UI text rendering stays on an explicit sRGB compositing path without pretending the composited shell is a safe RGB LCD target"
        );
    }
}

#[test]
fn ui_renderer_logs_explicit_shell_text_policy() {
    let diagnostics_source =
        fs::read_to_string("src/app/font_diagnostics.rs").expect("read font diagnostics");

    for expected in [
        "ui_text_antialias_mode = UI_TEXT_ANTIALIAS_MODE",
        "ui_text_subpixel_positioning",
        "ui_surface_color_space = UI_SURFACE_COLOR_SPACE",
        "ui_chrome_font_weight",
        "\"ui text renderer configuration established\"",
    ] {
        assert!(
            diagnostics_source.contains(expected),
            "ui renderer diagnostics should expose `{expected}` so packaged Windows runs can correlate shell sharpness changes with the active Windows shell text policy instead of guessing from screenshots"
        );
    }
}
