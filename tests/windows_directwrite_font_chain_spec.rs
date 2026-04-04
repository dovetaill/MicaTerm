use std::collections::BTreeSet;
use std::fs;

#[cfg(feature = "terminal-native-renderer")]
use anyhow::Result;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::{DirectWriteFontSystem, FontRequest, FontSystem, TextShapingRequest};

#[test]
fn cargo_manifest_enables_windows_directwrite_bindings_for_terminal_font_work() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read workspace manifest");

    assert!(
        cargo_toml.contains("\"Win32_Graphics_DirectWrite\""),
        "Task 3 should enable the windows crate's DirectWrite bindings so the Windows terminal font backend can call the native text stack"
    );
}

#[test]
fn windows_directwrite_source_uses_native_collection_and_fallback_mapping_contracts() {
    let source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read windows dwrite source");

    for expected in [
        "DWriteCreateFactory",
        "IDWriteFactory",
        "GetSystemFontCollection",
        "FindFamilyName",
        "GetFirstMatchingFont",
        "CreateFontFace",
        "GetMetrics",
        "GetFiles",
        "MapCharacters",
    ] {
        assert!(
            source.contains(expected),
            "windows dwrite backend should reference `{expected}` so face loading, metrics, and fallback mapping come from DirectWrite"
        );
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn mixed_script_monochrome_fallback_runs_rasterize_through_resolved_face_keys() -> Result<()> {
    let mut fonts = DirectWriteFontSystem::new()?;
    let loaded_font = fonts.load_font(&FontRequest::default())?;
    let fallback_faces = fonts.discover_fallback_faces(&loaded_font, "A⌘界")?;
    let primary_face_key = fallback_faces
        .first()
        .map(|face| face.face_key)
        .expect("fallback discovery should always include a primary face");
    let shaped_runs = fonts.shape_text_runs(&loaded_font, &TextShapingRequest::new("A⌘界"))?;
    let fallback_run = shaped_runs
        .iter()
        .find(|run| !run.has_color_glyphs && run.resolved_face.face_key != primary_face_key)
        .expect("mixed monochrome text should produce a non-primary fallback run");
    let glyph_id = fallback_run
        .glyphs
        .first()
        .map(|glyph| glyph.glyph_id)
        .expect("fallback run should carry at least one glyph");
    let raster = fonts.rasterize_glyph(
        &loaded_font,
        loaded_font.raster_request_with_fractional_offset_x_for_face(
            fallback_run.resolved_face.face_key,
            glyph_id,
            false,
            0.0,
        ),
    )?;
    let distinct_faces = shaped_runs
        .iter()
        .map(|run| run.resolved_face.face_key.0)
        .collect::<BTreeSet<_>>();

    assert!(
        distinct_faces.len() >= 2,
        "mixed Latin/symbol/CJK text should resolve to multiple face keys instead of one synthetic fallback label"
    );
    assert!(
        raster.advance_px != 0 || !raster.coverage.is_empty(),
        "a non-primary monochrome fallback run should rasterize from its resolved face instead of failing back to the primary bundled font only"
    );

    Ok(())
}
