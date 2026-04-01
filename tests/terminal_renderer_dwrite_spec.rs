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
        source.contains("impl FontSystem for DirectWriteFontSystem"),
        "windows font backend should implement the shared font backend trait"
    );
    assert!(
        source.contains("pub fn rasterize"),
        "windows font backend should keep a concrete rasterize entrypoint for its local implementation details"
    );
    assert!(
        source.contains("fn rasterize_glyph("),
        "windows font backend should expose rasterization through the shared backend trait"
    );
}

#[test]
fn windows_text_engine_source_exposes_fallback_and_feature_contracts() {
    let font_backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let font_mod_source =
        fs::read_to_string("src/app/terminal_font/mod.rs").expect("read font mod");
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read windows dwrite font backend");
    let shaper_source =
        fs::read_to_string("src/app/terminal_layout/shaper.rs").expect("read shaper");

    assert!(
        font_backend_source.contains("pub struct FontFallbackFace"),
        "font backend should define a fallback-face contract for Windows text fallback discovery"
    );
    assert!(
        font_backend_source.contains("pub struct OpenTypeFeatureSet"),
        "font backend should expose an OpenType feature configuration contract"
    );
    assert!(
        font_backend_source.contains("pub struct ColorGlyphRaster"),
        "font backend should define a color glyph raster contract"
    );
    assert!(
        font_backend_source.contains("fn discover_fallback_faces("),
        "font backend should expose fallback chain discovery through the shared font-system trait"
    );
    assert!(
        font_backend_source.contains("fn shape_text_runs("),
        "font backend should expose glyph-run shaping that is richer than a single bundled-font output"
    );
    assert!(
        font_backend_source.contains("fn rasterize_color_glyph("),
        "font backend should expose an explicit color glyph raster contract"
    );
    assert!(
        font_mod_source.contains("FontFallbackFace")
            && font_mod_source.contains("OpenTypeFeatureSet")
            && font_mod_source.contains("ShapedGlyphRun"),
        "terminal font module should re-export the richer Windows text engine contracts"
    );
    assert!(
        dwrite_source.contains("discover_fallback_chain"),
        "Windows text backend should expose a fallback-chain discovery helper"
    );
    assert!(
        dwrite_source.contains("OpenTypeFeatureSet"),
        "Windows text backend should accept OpenType feature configuration"
    );
    assert!(
        dwrite_source.contains("allow_ligatures"),
        "Windows text backend should make ligature-aware shaping explicit in its shaping contract"
    );
    assert!(
        dwrite_source.contains("has_color_glyphs"),
        "Windows text backend should flag color glyph runs explicitly"
    );
    assert!(
        shaper_source.contains("TextShapingRequest"),
        "terminal shaper should issue structured shaping requests instead of raw text-only calls"
    );
    assert!(
        shaper_source.contains("shape_text_runs"),
        "terminal shaper should accept the richer glyph-run shaping contract"
    );
    assert!(
        shaper_source.contains("resolved_face"),
        "terminal glyph runs should record which fallback face resolved each run"
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
    assert!(
        atlas_source.contains("pub enum GlyphCacheKind"),
        "native glyph atlas should tag monochrome atlas entries explicitly"
    );
    assert!(
        atlas_source.contains("cache_kind: GlyphCacheKind"),
        "atlas entries should record whether they belong to the monochrome atlas contract"
    );
    assert!(
        renderer_source.contains("color_glyph_cache"),
        "native renderer should keep a separate cache for color glyph resources"
    );
    assert!(
        renderer_source.contains("mono_glyph_cache_entries"),
        "prepared native frames should report monochrome atlas entry counts separately"
    );
    assert!(
        renderer_source.contains("color_glyph_cache_entries"),
        "prepared native frames should report color glyph cache counts separately"
    );
    assert!(
        renderer_source.contains("run.has_color_glyphs"),
        "renderer preparation should branch explicitly between monochrome atlas and color glyph cache paths"
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
        atlas_source.contains("FontRenderProfile::bitmap_default()"),
        "bitmap atlas renderer should load a dedicated grayscale render profile instead of using raw swash mask values"
    );
    assert!(
        atlas_source.contains("map_glyph_coverage_to_alpha"),
        "bitmap atlas renderer should route swash mono masks through the shared alpha mapping helper so Windows bitmap packages can darken mid-coverage stems without letting fringe pixels glow"
    );
    assert!(
        dwrite_source.contains("map_glyph_coverage_to_alpha")
            || dwrite_source.contains("font.map_coverage_to_alpha"),
        "native font rasterization should route glyph coverage through the shared coverage mapping helper or the loaded-font wrapper around it"
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
fn terminal_presenter_threads_presentable_native_frame_state() {
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        presenter_source.contains("pub struct PresentableNativeFrame"),
        "native presenter should define a retained presentable native frame payload"
    );
    assert!(
        presenter_source.contains("pub presentable_frame: PresentableNativeFrame"),
        "native terminal frames should carry the retained presentable frame payload instead of only a frame token"
    );
    assert!(
        presenter_source.contains("NativeCursorFrameState"),
        "native terminal frames should thread cursor metadata into the presentable frame payload"
    );
    assert!(
        presenter_source.contains("NativeSelectionFrameState"),
        "native terminal frames should thread selection metadata into the presentable frame payload"
    );
    assert!(
        presenter_source.contains("underline_run_count"),
        "native terminal frames should thread underline metadata into the presentable frame payload"
    );
    assert!(
        renderer_source.contains("shaped_row_count"),
        "prepared native frames should retain shaped-row metadata from renderer preparation"
    );
    assert!(
        renderer_source.contains("glyph_run_count"),
        "prepared native frames should retain glyph-run metadata from renderer preparation"
    );
    assert!(
        renderer_source.contains("pub struct PreparedNativeRendererStats"),
        "renderer preparation should expose a structured renderer-stats payload for presentable native frames"
    );
    assert!(
        bootstrap_source.contains("frame.presentable_frame"),
        "bootstrap should thread the presentable native frame state instead of only consuming the frame token"
    );
    assert!(
        bootstrap_source.contains("surface.update_frame_state(frame);"),
        "bootstrap should still hand the full native frame state to the retained native surface bridge"
    );
}

#[test]
fn terminal_presenter_threads_native_overlay_render_contracts() {
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        presenter_source.contains("pub struct NativeSelectionOverlay"),
        "native presenter should define an explicit selection overlay payload instead of leaving selection state implicit in the bitmap path"
    );
    assert!(
        presenter_source.contains("pub rect_count: usize"),
        "selection overlays should describe how many grid-aligned rectangles the native renderer should draw"
    );
    assert!(
        presenter_source.contains("pub struct NativeUnderlineOverlay"),
        "native presenter should define an explicit underline overlay payload"
    );
    assert!(
        presenter_source.contains("pub struct NativeImePreviewOverlay"),
        "native presenter should define an explicit IME preview overlay payload"
    );
    assert!(
        presenter_source.contains("pub selection_overlay: NativeSelectionOverlay"),
        "presentable native frames should carry selection overlay data"
    );
    assert!(
        presenter_source.contains("pub underline_overlay: NativeUnderlineOverlay"),
        "presentable native frames should carry underline overlay data"
    );
    assert!(
        presenter_source.contains("pub ime_preview_overlay: NativeImePreviewOverlay"),
        "presentable native frames should carry IME preview overlay data"
    );
    assert!(
        renderer_source.contains("pub struct PreparedUnderlineOverlay"),
        "renderer preparation should expose an explicit underline overlay contract for native frame payload assembly"
    );
    assert!(
        renderer_source.contains("pub underline_overlay: PreparedUnderlineOverlay"),
        "prepared native frames should carry underline overlay metadata for presenter assembly"
    );
    assert!(
        bootstrap_source.contains("presentable_frame.cursor"),
        "bootstrap should consume cursor state from the presentable native frame payload when the native path is active"
    );
    assert!(
        bootstrap_source.contains("presentable_frame.selection_overlay"),
        "bootstrap should keep the native selection overlay payload alongside the retained frame state"
    );
    assert!(
        bootstrap_source.contains("presentable_frame.ime_preview_overlay"),
        "bootstrap should keep the native IME preview overlay payload alongside the retained frame state"
    );
}

#[test]
fn windows_presenter_installation_prefers_native_with_bitmap_fallback() {
    let runtime_profile_source =
        fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        runtime_profile_source.contains("pub fn prefers_native_terminal_renderer(self) -> bool"),
        "runtime profile should expose a helper that marks Windows mainline builds as native-first without removing the bitmap mode contract"
    );
    assert!(
        runtime_profile_source.contains("AppBuildFlavor::WindowsMainline"),
        "runtime profile should still distinguish the Windows mainline build flavor when deciding native-first terminal installation"
    );
    assert!(
        bootstrap_source.contains("profile.prefers_native_terminal_renderer()"),
        "workspace terminal presenter installation should consult the runtime-profile native preference helper instead of only looking at the raw terminal render mode"
    );
    assert!(
        bootstrap_source.contains("build_native_terminal_presenter()?"),
        "Windows presenter installation should attempt to construct the native presenter before accepting bitmap fallback"
    );
    assert!(
        bootstrap_source.contains("falling back to bitmap presenter"),
        "bitmap presenter should remain available only as the fallback path when native presenter construction fails"
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

#[test]
fn wezterm_font_backend_source_is_wired_into_the_terminal_font_stack() {
    let terminal_font_mod =
        fs::read_to_string("src/app/terminal_font/mod.rs").expect("read terminal font mod");
    let wezterm_font_source = fs::read_to_string("src/app/terminal_font/wezterm_font.rs")
        .expect("read wezterm font adapter source");

    assert!(
        std::path::Path::new("src/app/terminal_font/wezterm_font.rs").exists(),
        "phase-1 terminal font adoption should add a dedicated wezterm font adapter source file"
    );
    assert!(
        terminal_font_mod.contains("pub mod wezterm_font;"),
        "terminal font module should declare the new wezterm font adapter module"
    );
    assert!(
        terminal_font_mod.contains("pub use wezterm_font::WeztermFontSystem;"),
        "terminal font module should re-export the wezterm font adapter"
    );
    assert!(
        wezterm_font_source.contains("Phase-1 WezTerm font adapter scaffold"),
        "the phase-1 adapter file should document that it tracks the WezTerm migration path"
    );
}

#[test]
fn wezterm_font_backend_source_exposes_phase_one_migration_contracts() {
    let wezterm_font_source = fs::read_to_string("src/app/terminal_font/wezterm_font.rs")
        .expect("read wezterm font adapter source");

    assert!(
        wezterm_font_source.contains("pub enum WeztermFontIntegrationStage"),
        "the phase-1 adapter should publish its current migration stage"
    );
    assert!(
        wezterm_font_source.contains("pub fn new() -> Self"),
        "the phase-1 adapter should expose a constructor"
    );
    assert!(
        wezterm_font_source.contains("pub fn integration_stage(&self)"),
        "the phase-1 adapter should expose a stage accessor"
    );
    assert!(
        wezterm_font_source.contains("pub fn upstream_sources(&self)"),
        "the phase-1 adapter should expose the exact upstream sources it is tracking"
    );
    assert!(
        wezterm_font_source.contains("pub fn integration_blocker(&self)"),
        "the phase-1 adapter should expose the current blocker for direct cargo integration"
    );
}

#[test]
fn terminal_font_backend_owns_rustybuzz_shaping_contract_and_layout_delegates_to_it() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let shaper_source =
        fs::read_to_string("src/app/terminal_layout/shaper.rs").expect("read terminal shaper");

    assert!(
        backend_source.contains("pub struct ShapedGlyph"),
        "font backend should own the shaped glyph payload so the local WezTerm-style font stack can keep shaping and raster contracts together"
    );
    assert!(
        backend_source.contains("fn shape_text("),
        "font backend should expose a shape_text contract instead of forcing terminal_layout to parse font faces directly"
    );
    assert!(
        backend_source.contains("shape_text_with_rustybuzz"),
        "font backend should centralize the pure-Rust shaping helper so future WezTerm extraction work stays in the font module"
    );
    assert!(
        !backend_source.contains("fn face_bytes("),
        "font backend should stop exposing raw face byte accessors once shaping moves behind the backend contract"
    );
    assert!(
        !backend_source.contains("fn face_index("),
        "font backend should stop exposing raw face index accessors once shaping moves behind the backend contract"
    );
    assert!(
        shaper_source.contains("fonts.shape_text("),
        "terminal_layout should delegate shaping to the font backend instead of constructing rustybuzz faces itself"
    );
    assert!(
        !shaper_source.contains("use rustybuzz"),
        "terminal_layout should stop importing rustybuzz directly once shaping lives behind the font backend contract"
    );
    assert!(
        !shaper_source.contains("Face::from_slice"),
        "terminal_layout should stop parsing font blobs directly once the font backend owns shaping"
    );
}

#[test]
fn terminal_font_backend_owns_rasterization_contract_and_renderer_delegates_to_it() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");

    assert!(
        backend_source.contains("pub struct GlyphRasterRequest"),
        "font backend should own the glyph raster request payload so renderer and backend stay decoupled from a concrete Windows font implementation"
    );
    assert!(
        backend_source.contains("pub struct RasterizedGlyph"),
        "font backend should own the rasterized glyph payload so future WezTerm-style backends can plug into the same renderer contract"
    );
    assert!(
        backend_source.contains("fn rasterize_glyph("),
        "font backend should expose a rasterize_glyph contract instead of forcing the renderer to know about DirectWriteFontSystem"
    );
    assert!(
        renderer_source.contains("fonts: &mut dyn FontSystem"),
        "native renderer should depend on the font backend trait so the rendering seam can survive further backend extraction work"
    );
    assert!(
        renderer_source.contains("fonts.rasterize_glyph(&frame.font, glyph.glyph_id, run.style.bold)?"),
        "native renderer should rasterize through the shared font backend contract"
    );
    assert!(
        !renderer_source.contains("DirectWriteFontSystem"),
        "native renderer should stop naming DirectWriteFontSystem directly once rasterization moves behind the backend contract"
    );
}

#[test]
fn loaded_font_contract_moves_presenter_layout_and_renderer_off_raw_face_keys() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let shaper_source =
        fs::read_to_string("src/app/terminal_layout/shaper.rs").expect("read terminal shaper");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");

    assert!(
        backend_source.contains("pub struct LoadedFont"),
        "font backend should define a LoadedFont object so the local stack can converge on a WezTerm-style loaded-font boundary"
    );
    assert!(
        backend_source.contains("fn load_font("),
        "font backend should expose a load_font contract instead of making callers stitch together face keys and metrics manually"
    );
    assert!(
        !backend_source.contains("fn resolve_face("),
        "font backend should stop exposing resolve_face once LoadedFont becomes the shared object boundary"
    );
    assert!(
        !backend_source.contains("fn metrics(&mut self, face: FontFaceKey"),
        "font backend should stop exposing raw face-key metric lookups once LoadedFont carries the metrics"
    );
    assert!(
        presenter_source.contains("loaded_font: LoadedFont"),
        "terminal presenter should cache a LoadedFont instead of separate request, face key, and metrics fields"
    );
    assert!(
        presenter_source.contains("font_system.load_font(&request)?"),
        "terminal presenter should build its native font state through the LoadedFont contract"
    );
    assert!(
        shaper_source.contains("font: &LoadedFont"),
        "terminal layout should accept a LoadedFont handle so shaping stays backend-driven"
    );
    assert!(
        !shaper_source.contains("resolve_face("),
        "terminal layout should stop resolving raw face keys once LoadedFont is threaded through"
    );
    assert!(
        renderer_source.contains("pub font: LoadedFont"),
        "native renderer frame contracts should carry a LoadedFont instead of loose face and metric fields"
    );
    assert!(
        !renderer_source.contains("pub face: FontFaceKey"),
        "native renderer frame contracts should stop storing a raw face key once LoadedFont is available"
    );
}

#[test]
fn loaded_font_object_boundary_owns_cache_identity_and_cell_metrics() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");
    let atlas_source =
        fs::read_to_string("src/app/terminal_renderer/atlas.rs").expect("read native atlas");

    assert!(
        backend_source.contains("pub struct LoadedFontKey"),
        "font backend should define a stable LoadedFontKey so cache identity belongs to the loaded-font object instead of being reconstructed in the renderer"
    );
    assert!(
        backend_source.contains("pub fn cache_key(&self) -> LoadedFontKey"),
        "LoadedFont should expose its cache identity through a method so renderer code no longer peeks into raw face and size fields"
    );
    assert!(
        backend_source.contains("pub fn cell_size_px(&self) -> (u32, u32)"),
        "LoadedFont should expose cell sizing through an object method so presenter and renderer stop reading metrics fields directly"
    );
    assert!(
        renderer_source.contains("frame.font.cache_key()"),
        "native renderer should hash the loaded-font cache key instead of rebuilding font identity from raw fields"
    );
    assert!(
        !renderer_source.contains("frame.font.face_key"),
        "native renderer should stop reading raw face keys once LoadedFont owns cache identity"
    );
    assert!(
        !renderer_source.contains("frame.font.request.px_size"),
        "native renderer should stop reading raw request size once LoadedFont owns cache identity"
    );
    assert!(
        presenter_source.contains("self.loaded_font.cell_size_px()"),
        "terminal presenter should read cell size through LoadedFont methods instead of reaching into raw metric fields"
    );
    assert!(
        atlas_source.contains("pub font_key: LoadedFontKey"),
        "glyph atlas keys should track the loaded-font cache identity instead of separate face and pixel-size fields"
    );
}

#[test]
fn loaded_font_object_boundary_carries_render_profile_into_cache_and_rasterization() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read dwrite backend");
    let atlas_source =
        fs::read_to_string("src/app/terminal_renderer/atlas.rs").expect("read native atlas");

    assert!(
        backend_source.contains("pub struct FontRenderProfile"),
        "font backend should define a FontRenderProfile so raster tuning lives with the loaded-font object instead of leaking into the renderer"
    );
    assert!(
        backend_source.contains("render_profile: FontRenderProfile"),
        "LoadedFont should carry a render profile so a loaded font fully describes how glyphs are rasterized"
    );
    assert!(
        backend_source.contains("pub fn render_profile(&self) -> FontRenderProfile"),
        "LoadedFont should expose its render profile through an object method so backend-owned tuning stays queryable without reopening raw struct fields"
    );
    assert!(
        backend_source.contains("render_profile: FontRenderProfileKey"),
        "LoadedFontKey should include the render profile cache identity so glyph atlas entries split when raster tuning changes"
    );
    assert!(
        atlas_source.contains("pub font_key: LoadedFontKey"),
        "glyph atlas should continue keying entries by LoadedFontKey so render-profile changes naturally partition cache entries"
    );
    assert!(
        dwrite_source.contains("font.map_coverage_to_alpha"),
        "Windows native rasterization should route glyph coverage through the loaded-font render profile instead of a hard-coded global transform"
    );
    assert!(
        dwrite_source.contains("font.apply_synthetic_embolden"),
        "Windows native rasterization should pull synthetic embolden strength from the loaded-font render profile"
    );
}

#[test]
fn windows_native_font_backend_uses_a_non_neutral_render_profile() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read dwrite backend");

    assert!(
        backend_source.contains("pub fn windows_native_default() -> Self"),
        "font backend should define a dedicated Windows-native render profile instead of only exposing the neutral default profile"
    );
    assert!(
        dwrite_source.contains("FontRenderProfile::windows_native_default()"),
        "Windows native font loading should use the dedicated Windows profile so native glyph masks are darker and less washed out than the neutral baseline"
    );
    assert!(
        !dwrite_source.contains("FontRenderProfile::default()"),
        "Windows native font loading should stop using the neutral render profile once the Windows-specific tuning path exists"
    );
}

#[test]
fn font_backend_source_exposes_bitmap_render_profile_defaults() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");

    assert!(
        backend_source.contains("pub fn bitmap_default() -> Self"),
        "font backend should define a dedicated bitmap render profile so software atlas packages can tune grayscale masks separately from the Windows native path"
    );
}

#[test]
fn windows_native_font_backend_source_switches_to_hinted_swash_rasterization() {
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read dwrite backend");

    assert!(
        dwrite_source.contains("SwashFontRef"),
        "Windows native font backend should keep a swash font handle so it can reuse the hinted grayscale rasterizer instead of the softer ab_glyph outline draw path"
    );
    assert!(
        dwrite_source.contains("ScaleContext"),
        "Windows native font backend should keep a swash scale context for hinted glyph rasterization"
    );
    assert!(
        dwrite_source.contains(".hint(true)"),
        "Windows native font backend should enable swash hinting so mono terminal glyphs stop looking soft on Windows"
    );
    assert!(
        dwrite_source.contains("Render::new(&[Source::Outline])"),
        "Windows native font backend should rasterize through swash outline rendering instead of ab_glyph outline drawing"
    );
    assert!(
        dwrite_source.contains("SwashFormat::Alpha"),
        "Windows native font backend should keep the native path on grayscale glyph masks instead of LCD subpixel masks"
    );
}
