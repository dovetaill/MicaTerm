use std::fs;

#[test]
fn vendored_slint_skia_item_renderer_enables_windows_subpixel_text_quality() {
    let item_renderer_source = fs::read_to_string("vendor/i-slint-renderer-skia/itemrenderer.rs")
        .expect("read skia item renderer");

    for expected in [
        "font.set_subpixel(true);",
        "font.set_edging(skia_safe::font::Edging::SubpixelAntiAlias);",
        "font.set_hinting(skia_safe::FontHinting::Slight);",
    ] {
        assert!(
            item_renderer_source.contains(expected),
            "vendored skia item renderer should keep `{expected}` so Windows shell chrome text stays on the explicit subpixel AA path instead of silently drifting back to default grayscale rasterization"
        );
    }
}

#[test]
fn vendored_slint_d3d_surface_pins_rgb_surface_props_for_ui_text() {
    let d3d_surface_source = fs::read_to_string("vendor/i-slint-renderer-skia/d3d_surface.rs")
        .expect("read skia d3d surface");

    for expected in [
        "let surface_props = SurfaceProps::new(",
        "SurfacePropsFlags::USE_DEVICE_INDEPENDENT_FONTS",
        "PixelGeometry::RGBH",
        "Some(&surface_props)",
    ] {
        assert!(
            d3d_surface_source.contains(expected),
            "vendored skia d3d surface should keep `{expected}` so Windows UI text rendering remains locked to an explicit RGB LCD surface contract"
        );
    }
}
