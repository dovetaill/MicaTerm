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
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite font backend");

    assert!(
        atlas_source.contains("const TERMINAL_FONT_SIZE_PX: f32 = 18.0;"),
        "atlas renderer should slightly increase bundled font size so the regular-weight Sarasa strokes do not read too thin"
    );
    assert!(
        atlas_source.contains("const MIN_CELL_WIDTH_PX: u32 = 8;"),
        "atlas renderer should stop forcing a 9px minimum cell width for a 7px bundled mono advance"
    );
    assert!(
        font_backend_source.contains("pub(crate) fn map_glyph_coverage_to_alpha"),
        "font backend should expose a shared glyph coverage mapping helper so bitmap and native renderers do not drift"
    );
    assert!(
        font_backend_source.contains("pub(crate) fn apply_synthetic_embolden"),
        "font backend should expose a shared synthetic embolden helper for regular-weight terminal glyphs"
    );
    assert!(
        atlas_source.contains("map_glyph_coverage_to_alpha"),
        "atlas renderer should route glyph coverage through the shared coverage mapping helper"
    );
    assert!(
        atlas_source.contains("apply_synthetic_embolden(&mut alpha"),
        "atlas renderer should apply the shared embolden pass to its mono alpha mask"
    );
    assert!(
        dwrite_source.contains("map_glyph_coverage_to_alpha"),
        "native font rasterization should route glyph coverage through the shared coverage mapping helper"
    );
    assert!(
        dwrite_source.contains("apply_synthetic_embolden(&mut coverage"),
        "native font rasterization should apply the same embolden pass as the bitmap atlas path"
    );
    assert!(
        font_backend_source.contains("const GLYPH_ALPHA_GAIN: f32 = 1.26;"),
        "shared glyph coverage gain should be stronger than the previous lighter-weight default"
    );
    assert!(
        font_backend_source.contains("const SYNTHETIC_EMBOLDEN_STRENGTH: f32 = 0.46;"),
        "shared glyph raster settings should include a light synthetic embolden pass to thicken regular-weight strokes"
    );
    assert!(font_backend_source.contains("px_size: 18.0,"));
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

#[test]
fn terminal_style_contract_threads_bold_and_underline_across_runtime_model_and_layout() {
    let runtime_source = fs::read_to_string("src/app/ssh/runtime.rs").expect("read runtime");
    let model_source =
        fs::read_to_string("src/app/terminal_model.rs").expect("read terminal model");
    let segmentation_source = fs::read_to_string("src/app/terminal_layout/run_segmentation.rs")
        .expect("read run segmentation");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");

    assert!(
        runtime_source.contains("pub bold: bool"),
        "runtime terminal cells should expose the bold SGR state"
    );
    assert!(
        runtime_source.contains("pub underline: bool"),
        "runtime terminal cells should expose the underline SGR state"
    );
    assert!(
        runtime_source.contains("attrs.intensity()"),
        "surface projection should derive bold state from wezterm cell intensity"
    );
    assert!(
        runtime_source.contains("attrs.underline()"),
        "surface projection should derive underline state from wezterm cell underline metadata"
    );
    assert!(
        model_source.contains("pub bold: bool"),
        "terminal model cells should preserve bold state from runtime snapshots"
    );
    assert!(
        model_source.contains("pub underline: bool"),
        "terminal model cells should preserve underline state from runtime snapshots"
    );
    assert!(
        segmentation_source.contains("pub bold: bool"),
        "text style keys should include bold state so shaped runs can split correctly"
    );
    assert!(
        segmentation_source.contains("pub underline: bool"),
        "text style keys should include underline state so renderer prep does not drop decorations"
    );
    assert!(
        renderer_source.contains("run.style.bold.hash(&mut hasher);"),
        "native frame fingerprints should include bold state so style-only changes trigger redraws"
    );
    assert!(
        renderer_source.contains("run.style.underline.hash(&mut hasher);"),
        "native frame fingerprints should include underline state so decoration changes trigger redraws"
    );
}
