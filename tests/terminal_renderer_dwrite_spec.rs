//! Source-level contract coverage for the Windows native terminal renderer stack.

use std::fs;

#[test]
fn windows_dwrite_font_backend_source_exposes_rasterization_contract() {
    let source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read windows dwrite font backend");

    assert!(
        source.contains("pub struct DirectWriteFontSystem"),
        "windows font backend should define a DirectWriteFontSystem"
    );
    assert!(
        source.contains("pub struct GlyphRasterRequest"),
        "windows font backend should define glyph raster requests for renderer staging"
    );
    assert!(
        source.contains("pub struct RasterizedGlyph"),
        "windows font backend should define rasterized glyph payloads"
    );
    assert!(
        source.contains("pub fn rasterize"),
        "windows font backend should expose a rasterize entrypoint"
    );
}

#[test]
fn native_renderer_sources_expose_atlas_and_prepare_contracts() {
    let atlas_source =
        fs::read_to_string("src/app/terminal_renderer/atlas.rs").expect("read renderer atlas");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");

    assert!(
        atlas_source.contains("pub struct GlyphAtlas"),
        "native renderer should keep a glyph atlas abstraction"
    );
    assert!(
        atlas_source.contains("pub fn upsert"),
        "glyph atlas should support cache inserts and lookups"
    );
    assert!(
        renderer_source.contains("pub struct WgpuTerminalRenderer"),
        "native renderer module should define a WgpuTerminalRenderer"
    );
    assert!(
        renderer_source.contains("pub fn prepare"),
        "native renderer should expose a prepare method for shaped frames"
    );
    assert!(
        renderer_source.contains("glyph_cache_entries"),
        "prepared native frames should report glyph cache entry counts for reuse checks"
    );
}

#[test]
fn atlas_and_font_backend_sources_expose_tighter_typography_contract() {
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");
    let font_backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");

    assert!(
        atlas_source.contains("const TERMINAL_FONT_SIZE_PX: f32 = 18.0;"),
        "atlas renderer should slightly increase bundled font size so the regular-weight Sarasa strokes do not read too thin"
    );
    assert!(
        atlas_source.contains("const MIN_CELL_WIDTH_PX: u32 = 8;"),
        "atlas renderer should stop forcing a 9px minimum cell width for a 7px bundled mono advance"
    );
    assert!(
        atlas_source.contains("const GLYPH_ALPHA_GAIN: f32 = 1.14;"),
        "atlas renderer should increase glyph alpha gain to strengthen regular-weight strokes"
    );
    assert!(
        font_backend_source.contains("px_size: 18.0,"),
        "shared font request defaults should move to the tighter 18px contract so native and software paths stay aligned"
    );
}

#[test]
fn terminal_presenter_source_wires_windows_native_renderer() {
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");

    assert!(
        presenter_source.contains("pub struct WindowsNativePresenter"),
        "terminal presenter seam should include a Windows native presenter implementation"
    );
    assert!(
        presenter_source.contains("DirectWriteFontSystem"),
        "Windows native presenter should depend on the DirectWrite font backend"
    );
    assert!(
        presenter_source.contains("WgpuTerminalRenderer"),
        "Windows native presenter should depend on the native renderer"
    );
    assert!(
        presenter_source.contains("PresentedTerminalFrame::Native"),
        "Windows native presenter should publish native terminal frames"
    );
}
