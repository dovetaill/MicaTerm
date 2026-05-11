#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::{
    FontFaceKey, FontMetrics, FontRenderProfile, FontRequest, GlyphRasterRequest, LoadedFont,
    RasterizedGlyph,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::atlas::GlyphAtlas;
use std::fs;

#[cfg(feature = "terminal-native-renderer")]
fn atlas_test_font() -> LoadedFont {
    LoadedFont::new(
        FontFaceKey(1),
        FontRequest {
            family_name: Some("Visible Bounds Test".into()),
            px_size: 14.0,
        },
        FontMetrics {
            units_per_em: 14,
            ascent_px: 10.0,
            descent_px: -4.0,
            line_gap_px: 0.0,
            baseline_px: 10.0,
            cell_width_px: 8.0,
            cell_height_px: 16.0,
        },
        FontRenderProfile::default(),
    )
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn monochrome_glyph_atlas_reserves_horizontal_safety_padding() {
    let mut atlas = GlyphAtlas::default();
    let font = atlas_test_font();
    let request = GlyphRasterRequest::new(&font, 42, false);
    let rasterized = RasterizedGlyph {
        width_px: 4,
        height_px: 2,
        bearing_x_px: 0,
        bearing_y_px: 0,
        visible_left_px: 0,
        visible_top_px: 0,
        visible_width_px: 4,
        visible_height_px: 2,
        advance_px: 4,
        coverage: vec![255; 8],
    };

    let entry = atlas.upsert(request, &rasterized);

    assert_eq!(
        entry.padding_left_px, 1,
        "compatibility atlas entries should reserve one column of safety padding on the left so right-edge overhang does not sample straight into a neighboring glyph slot"
    );
    assert_eq!(
        entry.padding_right_px, 1,
        "compatibility atlas entries should reserve one column of safety padding on the right so row-level clip can preserve the rightmost visible ink"
    );
    assert_eq!(
        entry.width_px,
        rasterized.width_px + entry.padding_left_px + entry.padding_right_px,
        "atlas entry width should include the explicit horizontal safety padding required by the compatibility bitmap path"
    );
}

#[test]
fn native_mode_keeps_host_cursor_overlay_scoped_to_bitmap_presentations() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        bootstrap_source.contains("if native_frame_presented {")
            && bootstrap_source.contains("clear_workspace_session_cursor_overlay(window);"),
        "native mode should clear any host-side cursor state once the retained native frame is presented"
    );
    assert!(
        host_source.contains(
            "if root.session-render-mode == \"bitmap\" && root.session-cursor-visible && root.cursor-blink-visible : cursor-overlay := Rectangle {"
        ),
        "host-side cursor visuals should stay scoped to bitmap mode so native mode does not double-draw the cursor overlay"
    );
}

#[test]
fn native_mode_keeps_host_surface_backdrop_opaque_behind_child_presenter() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        host_source.contains("surface-frame := Rectangle {")
            && host_source.contains("background: root.session-frame-surface;")
            && host_source.contains("blank-surface := Rectangle {")
            && host_source.contains("background: root.session-default-bg;"),
        "native mode should keep an opaque host-side terminal backdrop behind the retained child HWND so screenshot/composition gaps do not read as terminal transparency"
    );
}
