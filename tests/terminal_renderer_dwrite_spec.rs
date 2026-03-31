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
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read dwrite font backend");

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
        "font backend should expose a shared synthetic embolden helper for explicit bold terminal glyphs"
    );
    assert!(
        atlas_source.contains("ScaleContext"),
        "bitmap atlas renderer should keep a swash scale context for hinted mono glyph rendering"
    );
    assert!(
        atlas_source.contains(".hint(true)"),
        "bitmap atlas renderer should enable swash hinting for bundled terminal glyphs"
    );
    assert!(
        atlas_source.contains("Render::new(&[Source::Outline])"),
        "bitmap atlas renderer should rasterize mono glyphs through swash outline rendering"
    );
    assert!(
        atlas_source.contains("SwashFormat::Alpha"),
        "bitmap atlas renderer should keep the software path on a hinted grayscale mask instead of using subpixel blending that makes colored stems shimmer in the bitmap image"
    );
    assert!(
        atlas_source.contains(".offset("),
        "bitmap atlas renderer should feed fractional x positioning into swash instead of pinning every glyph to a whole-pixel origin"
    );
    assert!(
        atlas_source.contains("mono_embolden_strength"),
        "bitmap atlas renderer should centralize regular and bold faux-weight tuning in the swash raster path"
    );
    assert!(
        dwrite_source.contains("map_glyph_coverage_to_alpha"),
        "native font rasterization should route glyph coverage through the shared coverage mapping helper"
    );
    assert!(
        dwrite_source.contains("if request.bold"),
        "native font rasterization should only apply synthetic embolden when the shaped run requests bold text"
    );
    assert!(
        font_backend_source.contains("const GLYPH_ALPHA_GAIN: f32 = 1.0;"),
        "shared glyph coverage gain should stay neutral for regular text so grayscale edges remain crisp"
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
fn bitmap_terminal_pipeline_sources_plumb_window_scale_factor_into_bitmap_rasterization() {
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        presenter_source.contains("fn set_raster_scale"),
        "terminal presenter seam should expose a bitmap raster scale hook so hidpi windows can request a denser atlas backing image"
    );
    assert!(
        presenter_source.contains("self.renderer.set_raster_scale"),
        "bitmap atlas presenter should forward the window raster scale into the atlas renderer"
    );
    assert!(
        bootstrap_source.contains("presenter.set_raster_scale(window.window().scale_factor())"),
        "workspace terminal sync should push the current Slint window scale factor into the bitmap presenter before rendering so the atlas is not blurred by post-scale stretching"
    );
}

#[test]
fn atlas_source_switches_mono_glyph_rasterization_to_hinted_swash_rendering() {
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");

    assert!(
        atlas_source.contains("ScaleContext"),
        "terminal atlas should keep a swash scale context so mono glyph rendering can use the library's hinted raster path"
    );
    assert!(
        atlas_source.contains(".hint(true)"),
        "terminal atlas should enable swash hinting for mono glyph rendering instead of relying on the softer ab_glyph outline draw path"
    );
    assert!(
        atlas_source.contains("Render::new(&[Source::Outline])"),
        "terminal atlas should render mono glyphs through swash outline rendering so the bundled terminal font benefits from a sharper rasterizer"
    );
    assert!(
        atlas_source.contains("SwashFormat::Alpha"),
        "terminal atlas should keep the software renderer on hinted grayscale masks because the final bitmap is later composited by Slint rather than directly scanned out as LCD subpixels"
    );
    assert!(
        atlas_source.contains(".offset("),
        "terminal atlas should pass fractional positioning into swash so hinted mono glyphs do not collapse to inconsistent whole-pixel stems"
    );
    assert!(
        atlas_source.contains(".embolden("),
        "terminal atlas should use swash embolden control for regular and bold mono glyphs instead of the older bitmap-only coverage spread"
    );
}

#[test]
fn atlas_source_keeps_fractional_positioning_without_regular_weight_embolden() {
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");

    assert!(
        atlas_source.contains("const REGULAR_MONO_EMBOLDEN_STRENGTH: f32 = 0.0;"),
        "regular terminal text should stay on the hinted raster path without extra faux embolden so narrow glyphs like i do not turn blotchy"
    );
    assert!(
        !atlas_source.contains("scaled.kern("),
        "terminal bitmap glyph placement should not apply proportional kerning inside the fixed grid because that causes uneven cell spacing"
    );
    assert!(
        atlas_source.contains("split_fractional_offset"),
        "terminal bitmap glyph placement should preserve fractional offsets before sending them into the hinted grayscale rasterizer"
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
