use std::fs;

#[test]
fn vendored_slint_skia_item_renderer_switches_shell_text_aa_by_host_opacity() {
    let item_renderer_source = fs::read_to_string("vendor/i-slint-renderer-skia/itemrenderer.rs")
        .expect("read skia item renderer");

    for expected in [
        "MICA_TERM_FORCE_OPAQUE_HOST_WINDOW",
        "font.set_subpixel(true);",
        "font.set_edging(skia_safe::font::Edging::SubpixelAntiAlias);",
        "font.set_edging(skia_safe::font::Edging::AntiAlias);",
        "font.set_hinting(skia_safe::FontHinting::Slight);",
        "font.set_hinting(skia_safe::FontHinting::Normal);",
    ] {
        assert!(
            item_renderer_source.contains(expected),
            "vendored skia item renderer should keep `{expected}` so Windows shell text can switch between an opaque-host LCD path and a transparent-host grayscale fallback instead of hard-wiring one AA mode for every host window"
        );
    }
}

#[test]
fn vendored_slint_skia_item_renderer_preserves_lcd_text_inside_opacity_layers() {
    let item_renderer_source = fs::read_to_string("vendor/i-slint-renderer-skia/itemrenderer.rs")
        .expect("read skia item renderer");

    for expected in [
        "let mut layer_paint = skia_safe::Paint::default();",
        "layer_paint.set_alpha_f(opacity.clamp(0.0, 1.0));",
        "skia_safe::canvas::SaveLayerRec::default()",
        ".paint(&layer_paint)",
        "skia_safe::canvas::SaveLayerFlags::PRESERVE_LCD_TEXT",
        "self.canvas.save_layer(&layer_rec);",
        "let layer_surface_props = self.canvas.top_props();",
        "self.canvas.new_surface(&image_info, Some(&layer_surface_props))?",
    ] {
        assert!(
            item_renderer_source.contains(expected),
            "vendored skia item renderer should keep `{expected}` so text-bearing shell subtrees rendered through opacity layers do not automatically drop back to the fuzzier non-LCD path on Windows"
        );
    }
}

#[test]
fn vendored_slint_d3d_surface_switches_pixel_geometry_by_host_opacity() {
    let d3d_surface_source = fs::read_to_string("vendor/i-slint-renderer-skia/d3d_surface.rs")
        .expect("read skia d3d surface");

    for expected in [
        "MICA_TERM_FORCE_OPAQUE_HOST_WINDOW",
        "fn shell_text_surface_props_flags() -> SurfacePropsFlags",
        "fn shell_text_surface_pixel_geometry(",
        "fn shell_text_surface_props(",
        "SurfaceProps::new_with_text_properties(",
        "shell_text_rendering_params(hwnd)",
        "let surface_color_space = skia_safe::ColorSpace::new_srgb();",
        "let surface_props = shell_text_surface_props(hwnd);",
        "DWRITE_PIXEL_GEOMETRY_RGB",
        "DWRITE_PIXEL_GEOMETRY_BGR",
        "SurfacePropsFlags::empty()",
        "SurfacePropsFlags::USE_DEVICE_INDEPENDENT_FONTS",
        "PixelGeometry::RGBH",
        "PixelGeometry::BGRH",
        "PixelGeometry::Unknown",
        "Some(surface_color_space)",
        "Some(&surface_props)",
    ] {
        assert!(
            d3d_surface_source.contains(expected),
            "vendored skia d3d surface should keep `{expected}` so Windows UI text rendering uses the monitor's real LCD stripe order instead of hard-wiring RGB geometry for every panel"
        );
    }
}

#[test]
fn ui_renderer_logs_dynamic_shell_text_policy() {
    let diagnostics_source =
        fs::read_to_string("src/app/font_diagnostics.rs").expect("read font diagnostics");

    for expected in [
        "ui_host_window_transparent",
        "ui_system_pixel_geometry",
        "ui_text_antialias_mode = ui_text_antialias_mode()",
        "ui_text_subpixel_positioning = ui_text_subpixel_positioning()",
        "ui_surface_pixel_geometry = ui_surface_pixel_geometry()",
        "ui_surface_color_space = ui_surface_color_space()",
        "ui_surface_uses_device_independent_fonts = ui_surface_uses_device_independent_fonts()",
        "ui_text_contrast = ui_text_contrast()",
        "ui_text_gamma = ui_text_gamma()",
        "ui_text_rendering_policy = ui_text_rendering_policy()",
        "ui_chrome_font_weight",
        "\"ui text renderer configuration established\"",
    ] {
        assert!(
            diagnostics_source.contains(expected),
            "ui renderer diagnostics should expose `{expected}` so packaged Windows runs can see whether the shell is on the opaque-host LCD path or the transparent-host grayscale fallback instead of guessing from screenshots"
        );
    }
}

#[test]
fn vendored_slint_skia_layer_blending_keeps_cached_text_sharp_at_native_scale() {
    let item_renderer_source = fs::read_to_string("vendor/i-slint-renderer-skia/itemrenderer.rs")
        .expect("read skia item renderer");

    assert!(
        item_renderer_source.contains(
            "self.canvas.draw_image_with_sampling_options(\n                layer_image,\n                skia_safe::Point::default(),\n                skia_safe::sampling_options::FilterMode::Nearest,"
        ),
        "vendored skia item renderer should draw cached shell layers back with nearest sampling so 1:1 text snapshots do not get softened by an unnecessary linear filter"
    );
}
