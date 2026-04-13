//! Source-level contract coverage for the Windows native terminal renderer stack.

use std::fs;
use std::path::Path;

#[path = "support/retired_windows_subsystem.rs"]
mod retired_windows_subsystem;

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
    assert!(
        (source.contains("Source::ColorOutline(0)")
            && source.contains("Source::ColorBitmap(StrikeWith::BestFit)"))
            || (source.contains("TerminalEmojiRenderer") && source.contains("rasterize_cluster")),
        "windows font backend should use either inline swash color glyph sources or the shared emoji rasterizer for the real emoji path"
    );
    assert!(
        !source.contains("let accent = ((glyph_id % 127) as u8).saturating_add(96);"),
        "windows font backend should stop synthesizing placeholder accent-colored emoji squares"
    );
}

#[test]
fn windows_text_engine_source_exposes_fallback_and_feature_contracts() {
    let font_backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let font_mod_source =
        fs::read_to_string("src/app/terminal_font/mod.rs").expect("read font mod");
    let locator_source =
        fs::read_to_string("src/app/terminal_font/windows_locator.rs").expect("read locator");
    let fallback_source =
        fs::read_to_string("src/app/terminal_font/windows_fallback.rs").expect("read fallback");
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
        font_backend_source.contains("source_byte_range"),
        "shaped glyph-run contracts should carry source byte ranges so layout can remap fallback subruns onto terminal cells"
    );
    assert!(
        font_backend_source.contains("Feature::from_str"),
        "font backend should parse OpenType feature tags into rustybuzz features before shaping"
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
        font_mod_source.contains("pub mod windows_locator;"),
        "terminal font module should declare the Windows locator module"
    );
    assert!(
        font_mod_source.contains("pub mod windows_fallback;"),
        "terminal font module should declare the Windows fallback module"
    );
    assert!(
        font_mod_source.contains("WindowsFontLocator")
            && font_mod_source.contains("WindowsFontFallbackResolver"),
        "terminal font module should re-export the Windows locator and fallback helpers"
    );
    assert!(
        locator_source.contains("pub struct WindowsFontLocator"),
        "Windows text stack should define a dedicated font locator"
    );
    assert!(
        locator_source.contains("pub fn from_database("),
        "Windows font locator should build from a shared system font database instead of always scanning fonts inside its constructor"
    );
    assert!(
        fallback_source.contains("pub struct WindowsFontFallbackResolver"),
        "Windows text stack should define a dedicated fallback resolver"
    );
    assert!(
        fallback_source.contains("discover_fallback_families"),
        "Windows fallback resolver should expose a helper that returns multiple families for mixed text"
    );
    assert!(
        dwrite_source.contains("discover_fallback_chain"),
        "Windows text backend should expose a fallback-chain discovery helper"
    );
    assert!(
        dwrite_source.contains("WindowsFontLocator"),
        "Windows text backend should use the locator helper instead of hard-coding fallback families inline"
    );
    assert!(
        dwrite_source.contains("WindowsFontFallbackResolver"),
        "Windows text backend should use the fallback resolver helper instead of building fallback families inline"
    );
    assert!(
        dwrite_source.contains("OpenTypeFeatureSet"),
        "Windows text backend should accept OpenType feature configuration"
    );
    assert!(
        dwrite_source.contains("TerminalEmojiRenderer")
            && dwrite_source.contains("rasterize_cluster"),
        "Windows text backend should reuse the shared emoji rasterizer for real color glyph sprites"
    );
    assert!(
        !dwrite_source.contains("let accent = ((glyph_id % 127) as u8).saturating_add(96)"),
        "Windows color glyph rasterization should stop synthesizing a flat placeholder color block from glyph ids"
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
    assert!(
        shaper_source.contains("source_byte_range") && shaper_source.contains("clusters"),
        "terminal shaper should remap fallback subrun byte ranges back onto segmented terminal clusters"
    );
}

#[test]
fn windows_font_backend_lazy_init_contract_is_present() {
    let locator_source =
        fs::read_to_string("src/app/terminal_font/windows_locator.rs").expect("read locator");
    let emoji_source =
        fs::read_to_string("src/app/terminal_emoji.rs").expect("read terminal emoji");
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read windows dwrite font backend");

    assert!(
        dwrite_source.contains("locator: Option<WindowsFontLocator>"),
        "DirectWriteFontSystem should keep the locator lazy so startup does not eagerly scan system fonts"
    );
    assert!(
        dwrite_source.contains("emoji_renderer: Option<TerminalEmojiRenderer>"),
        "DirectWriteFontSystem should keep the emoji renderer lazy so startup does not eagerly build its font database"
    );
    assert!(
        dwrite_source.contains("system_font_database: Option<Arc<Database>>"),
        "DirectWriteFontSystem should cache a shared system font database so locator and emoji paths do not scan fonts twice"
    );
    assert!(
        dwrite_source.contains("fn ensure_system_font_database(&mut self) -> Arc<Database>"),
        "DirectWriteFontSystem should expose an on-demand system font database accessor"
    );
    assert!(
        !locator_source.contains("database.load_system_fonts();"),
        "WindowsFontLocator should stop loading system fonts internally once the shared database owns that scan"
    );
    assert!(
        emoji_source.contains("pub fn from_database("),
        "terminal emoji rendering should accept a shared font database instead of forcing an internal scan"
    );
    assert!(
        !emoji_source.contains("database.load_system_fonts();"),
        "TerminalEmojiRenderer should stop loading system fonts internally once the shared database owns that scan"
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
        atlas_source.contains("assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf"),
        "atlas renderer should point at the bundled Sarasa Term SC default face"
    );
    assert!(
        atlas_source.contains("const TERMINAL_FONT_SIZE_PX: f32 = DEFAULT_TERMINAL_FONT_SIZE_PX;"),
        "atlas renderer should derive its default font size from the shared typography contract"
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
        font_backend_source.contains("pub const DEFAULT_TERMINAL_FONT_SIZE_PX: f32 = 14.0;"),
        "shared typography defaults should move the terminal font size into the 14px Windows Terminal target range"
    );
    assert!(
        font_backend_source.contains("pub const DEFAULT_TERMINAL_LINE_HEIGHT: f32 = 1.5;"),
        "shared typography defaults should expose a slightly looser 1.5 line-height contract for dense Windows terminal text"
    );
    assert!(
        font_backend_source.contains("pub const DEFAULT_TERMINAL_LETTER_SPACING_PX: f32 = 0.0;"),
        "shared typography defaults should keep terminal letter spacing at zero"
    );
    assert!(
        font_backend_source.contains("pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = \"Medium\";"),
        "shared typography defaults should move the default terminal weight to Medium"
    );
    assert!(
        font_backend_source.contains("const GLYPH_ALPHA_GAIN: f32 = 1.0;"),
        "shared glyph coverage gain should stay neutral for regular text so grayscale edges remain crisp"
    );
    assert!(
        font_backend_source.contains("const SYNTHETIC_EMBOLDEN_STRENGTH: f32 = 0.46;"),
        "shared glyph raster settings should include a light synthetic embolden pass to thicken regular-weight strokes"
    );
    assert!(font_backend_source.contains("px_size: DEFAULT_TERMINAL_FONT_SIZE_PX,"));
    assert!(
        dwrite_source.contains("let cell_height_px = line_height.max(MIN_CELL_HEIGHT_PX);"),
        "DirectWrite metrics should honor the shared minimum terminal line-height contract across both the live and retired Windows paths"
    );
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
        presenter_source.contains("fn scaled_terminal_font_request("),
        "terminal presenters should share one font-request scaling helper so the live and retired Windows typography tuning cannot drift"
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
        bootstrap_source.contains("surface.present(frame);"),
        "bootstrap should still hand the full native frame state to the present-driver aware native surface bridge"
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
        presenter_source.contains("pub struct NativeCursorOverlay"),
        "native presenter should define an explicit cursor overlay payload instead of leaving cursor drawing implicit in the surface backend"
    );
    assert!(
        presenter_source.contains("pub struct NativeSelectionOverlay"),
        "native presenter should define an explicit selection overlay payload instead of leaving selection state implicit in the bitmap path"
    );
    assert!(
        presenter_source.contains("pub struct NativeSelectionRect"),
        "selection overlays should expose grid-aligned rectangle payloads for the retained native display list"
    );
    assert!(
        presenter_source.contains("pub rects: Vec<NativeSelectionRect>"),
        "selection overlays should carry explicit rectangle payloads instead of only a count"
    );
    assert!(
        presenter_source.contains("pub struct NativeUnderlineOverlay"),
        "native presenter should define an explicit underline overlay payload"
    );
    assert!(
        presenter_source.contains("pub struct NativeUnderlineRun"),
        "underline overlays should carry per-run payloads for retained native drawing"
    );
    assert!(
        presenter_source.contains("pub runs: Vec<NativeUnderlineRun>"),
        "underline overlays should carry explicit underline runs instead of only a count"
    );
    assert!(
        presenter_source.contains("pub struct NativeImePreviewOverlay"),
        "native presenter should define an explicit IME preview overlay payload"
    );
    assert!(
        presenter_source.contains("pub cursor_overlay: NativeCursorOverlay"),
        "presentable native frames should carry cursor overlay data"
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
        renderer_source.contains("pub runs: Vec<PreparedUnderlineRun>"),
        "prepared underline overlays should keep draw-ready underline runs for presenter assembly"
    );
    assert!(
        bootstrap_source.contains("presentable_frame.cursor_overlay"),
        "bootstrap should keep the native cursor overlay payload alongside the retained frame state"
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
fn native_renderer_sources_expose_draw_ready_text_payloads() {
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");

    assert!(
        renderer_source.contains("pub struct PreparedMonochromeGlyphDraw"),
        "native renderer should define an explicit monochrome glyph draw payload"
    );
    assert!(
        renderer_source.contains("pub struct PreparedColorGlyphDraw"),
        "native renderer should define an explicit color glyph draw payload"
    );
    assert!(
        renderer_source.contains("pub monochrome_glyph_draws: Vec<PreparedMonochromeGlyphDraw>"),
        "prepared native frames should carry retained monochrome glyph draw payloads"
    );
    assert!(
        renderer_source.contains("pub color_glyph_draws: Vec<PreparedColorGlyphDraw>"),
        "prepared native frames should carry retained color glyph draw payloads"
    );
    assert!(
        renderer_source.contains("atlas_entry: GlyphAtlasEntry"),
        "monochrome glyph draws should keep atlas entry references for native presentation"
    );
    assert!(
        renderer_source.contains("cache_entry: ColorGlyphCacheEntry"),
        "color glyph draws should keep dedicated color cache references for native presentation"
    );
    assert!(
        renderer_source.contains("pub struct PreparedMonochromeGlyphUploadPayload"),
        "native renderer should define an explicit monochrome glyph upload payload contract for backend resource creation"
    );
    assert!(
        renderer_source.contains("pub coverage: Vec<u8>"),
        "monochrome glyph upload payloads should retain the alpha mask bytes for the backend upload path"
    );
    assert!(
        renderer_source.contains("pub bearing_x_px: i32"),
        "monochrome glyph upload payloads should retain horizontal bearing metadata for backend placement"
    );
    assert!(
        renderer_source.contains("pub advance_px: i32"),
        "monochrome glyph upload payloads should retain advance metadata for backend placement"
    );
    assert!(
        renderer_source.contains("pub upload: Option<PreparedMonochromeGlyphUploadPayload>"),
        "monochrome glyph draws should expose an optional upload payload so platform backends can distinguish first upload from cache reuse"
    );
    assert!(
        renderer_source.contains("pub struct PreparedColorGlyphUploadPayload"),
        "native renderer should define an explicit color glyph upload payload contract for backend resource creation"
    );
    assert!(
        renderer_source.contains("pub rgba: Vec<u8>"),
        "color glyph upload payloads should retain RGBA bytes for backend upload"
    );
    assert!(
        renderer_source.contains("pub upload: Option<PreparedColorGlyphUploadPayload>"),
        "color glyph draws should expose an optional upload payload so platform backends can distinguish first upload from cache reuse"
    );
    assert!(
        renderer_source.contains("pub dest_x_px: i32"),
        "draw-ready glyph payloads should carry a stable destination x in pixels so the Windows backend does not need to reshape or infer pen positions"
    );
    assert!(
        renderer_source.contains("pub dest_y_px: i32"),
        "draw-ready glyph payloads should carry a stable destination y in pixels so the Windows backend can place glyph masks directly"
    );
    assert!(
        presenter_source.contains("pub background_runs: Vec<PreparedBackgroundRun>"),
        "presentable native frames should thread retained background runs alongside glyph draws"
    );
    assert!(
        presenter_source.contains("pub monochrome_glyph_draws: Vec<PreparedMonochromeGlyphDraw>"),
        "presentable native frames should thread monochrome glyph draw payloads through the presenter contract"
    );
    assert!(
        presenter_source.contains("pub color_glyph_draws: Vec<PreparedColorGlyphDraw>"),
        "presentable native frames should thread color glyph draw payloads through the presenter contract"
    );
    assert!(
        presenter_source.contains("pub default_fg_rgba: u32"),
        "presentable native frames should keep the terminal default foreground color available to platform backends"
    );
    assert!(
        presenter_source.contains("pub default_bg_rgba: u32"),
        "presentable native frames should keep the terminal default background color available to platform backends"
    );
    assert!(
        presenter_source.contains("pub row_bg_even_rgba: u32"),
        "presentable native frames should keep even-row banding colors available for blank rows and cells"
    );
    assert!(
        presenter_source.contains("pub row_bg_odd_rgba: u32"),
        "presentable native frames should keep odd-row banding colors available for blank rows and cells"
    );
    assert!(
        presenter_source.contains("pub grid_rows: u32"),
        "presentable native frames should keep grid row counts so backends can paint blank rows"
    );
    assert!(
        presenter_source.contains("pub grid_cols: u32"),
        "presentable native frames should keep grid column counts so backends can size row and overlay fills correctly"
    );
}

#[test]
fn windows_presenter_installation_prefers_native_while_packaged_contract_defaults_to_retained_native()
 {
    let runtime_profile_source =
        fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        runtime_profile_source.contains("pub fn prefers_native_terminal_renderer(self) -> bool"),
        "runtime profile should keep a helper that marks packaged profiles as native-only terminal builds"
    );
    assert!(
        runtime_profile_source.contains("AppBuildFlavor::WindowsMainline"),
        "runtime profile should still distinguish the Windows mainline build flavor when deciding native-first terminal installation"
    );
    assert!(
        runtime_profile_source.contains("retained-native presentation path"),
        "runtime profile docs should make it explicit that packaged Windows mainline keeps the native renderer on the retained-native presentation path"
    );
    assert!(
        runtime_profile_source.contains("native-first Windows software profile"),
        "runtime profile docs should describe the Linux-host software package as a native-first software path"
    );
    assert!(
        bootstrap_source.contains("build_native_terminal_presenter()"),
        "workspace terminal presenter installation should still construct the native presenter for mainline/native profiles"
    );
    assert!(
        bootstrap_source.contains("falling back to bitmap presenter"),
        "workspace terminal presenter installation should keep a bitmap presenter fallback log for software compatibility builds"
    );
    assert!(
        bootstrap_source.contains("BitmapAtlasPresenter"),
        "bootstrap should keep referencing the bitmap presenter as the internal native-renderer fallback path"
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
fn windows_platform_surface_backend_source_exposes_hwnd_and_lifecycle_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");
    let diagnostics_source = fs::read_to_string("src/app/terminal_renderer/diagnostics.rs")
        .expect("read diagnostics source");
    let native_surface_source = fs::read_to_string("src/app/terminal_renderer/native_surface.rs")
        .expect("read native surface source");
    let windows_frame_source =
        fs::read_to_string("src/app/windows_frame.rs").expect("read windows frame interop");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        windows_backend_source.contains("pub struct WindowsNativeSurfaceBackend"),
        "Windows platform layer should define a concrete native surface backend"
    );
    assert!(
        windows_backend_source.contains("pub struct WindowsNativeSurfaceState"),
        "Windows platform layer should keep a native surface state object"
    );
    assert!(
        windows_backend_source.contains("hwnd: Option<isize>"),
        "Windows backend state should track the resolved host HWND"
    );
    assert!(
        windows_backend_source
            .contains("fn resolve_host_hwnd(window: &AppWindow) -> Option<isize>"),
        "Windows backend should expose a helper that resolves the host HWND from the Slint shell"
    );
    assert!(
        windows_backend_source.contains("fn attach(&mut self, window: &AppWindow) -> Result<()>"),
        "Windows backend should expose an attach hook"
    );
    assert!(
        windows_backend_source.contains("fn present(&mut self, damage: NativeSurfaceDamage)"),
        "Windows backend should expose a damage-aware present hook"
    );
    assert!(
        windows_backend_source.contains("fn detach(&mut self)"),
        "Windows backend should expose a detach hook"
    );
    assert!(
        diagnostics_source.contains("pub struct NativeTerminalSurfaceDiagnostics"),
        "terminal renderer should define a diagnostics snapshot contract for native surface runtime state"
    );
    assert!(
        diagnostics_source.contains("pub struct NativeTerminalSurfaceDrawCounters"),
        "terminal renderer should define explicit draw counters for diagnostics snapshots"
    );
    assert!(
        diagnostics_source.contains("pub hwnd: Option<isize>"),
        "diagnostics snapshots should record the attached host HWND"
    );
    assert!(
        diagnostics_source.contains("pub render_target_generation: u64"),
        "diagnostics snapshots should record render-target generation"
    );
    assert!(
        diagnostics_source.contains("pub last_prepared_frame_token: u64"),
        "diagnostics snapshots should record the latest prepared frame token"
    );
    assert!(
        diagnostics_source.contains("pub last_presented_frame_token: u64"),
        "diagnostics snapshots should record the latest presented frame token"
    );
    assert!(
        diagnostics_source.contains("pub draw_counters: NativeTerminalSurfaceDrawCounters"),
        "diagnostics snapshots should group draw counters into a dedicated payload"
    );
    assert!(
        windows_backend_source
            .contains("fn diagnostics_snapshot(&self) -> NativeTerminalSurfaceDiagnostics"),
        "Windows backend should expose a diagnostics snapshot helper"
    );
    assert!(
        windows_backend_source.contains("last_prepared_frame_token"),
        "Windows backend should retain the last prepared frame token for diagnostics"
    );
    assert!(
        windows_backend_source.contains("NativeTerminalSurfaceDrawCounters"),
        "Windows backend should build diagnostics draw counters from the last draw pass"
    );
    assert!(
        native_surface_source.contains("latest_diagnostics: NativeTerminalSurfaceDiagnostics"),
        "native surface should retain the latest diagnostics snapshot"
    );
    assert!(
        native_surface_source
            .contains("pub fn diagnostics_snapshot(&self) -> NativeTerminalSurfaceDiagnostics"),
        "native surface should expose a diagnostics snapshot getter"
    );
    assert!(
        native_surface_source
            .contains("let mut diagnostics = state.backend.diagnostics_snapshot();")
            && native_surface_source
                .contains("diagnostics.scheduled_present_count = state.scheduled_present_count;")
            && native_surface_source.contains(
                "diagnostics.host_redraw_request_count = state.host_redraw_request_count;"
            )
            && native_surface_source
                .contains("diagnostics.host_redraw_replay_count = state.host_redraw_replay_count;")
            && native_surface_source.contains("state.latest_diagnostics = diagnostics;"),
        "native surface should refresh cached diagnostics after backend state transitions and annotate them with native present scheduling counters"
    );
    assert!(
        windows_frame_source
            .contains("pub fn resolve_host_window_hwnd(window: &AppWindow) -> Option<isize>"),
        "windows_frame should expose a reusable HWND resolution helper for the terminal backend"
    );
    assert!(
        windows_frame_source.contains("pub fn native_surface_diagnostics_hwnd("),
        "windows_frame should expose a helper that reads HWND data from native surface diagnostics snapshots"
    );
    assert!(
        bootstrap_source.contains("NativeTerminalSurface::attach(window)"),
        "bootstrap should instantiate the backend-aware native surface bridge so the Windows backend can be selected during startup"
    );
}

#[test]
fn windows_backend_source_consumes_retained_background_and_monochrome_payloads() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("draw.upload.as_ref()"),
        "windows backend should consume retained monochrome upload payloads instead of trying to regenerate glyph masks"
    );
    assert!(
        windows_backend_source.contains("upload.coverage.len()"),
        "windows backend should retain monochrome alpha-mask bytes when creating glyph bitmap resources"
    );
    assert!(
        windows_backend_source.contains("draw.fg_rgba"),
        "windows backend should draw monochrome glyphs with the retained foreground color instead of recomputing style"
    );
    assert!(
        windows_backend_source.contains("run.bg_rgba"),
        "windows backend should draw background runs with the retained ANSI background color instead of clearing only the default background"
    );
}

#[test]
fn windows_backend_source_consumes_retained_color_glyph_and_overlay_payloads() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("draw.upload.as_ref()"),
        "windows backend should consume retained color upload payloads instead of trying to regenerate RGBA emoji bitmaps"
    );
    assert!(
        windows_backend_source.contains("upload.rgba.len()"),
        "windows backend should retain RGBA upload bytes when creating color glyph bitmap resources"
    );
    assert!(
        windows_backend_source.contains("frame.frame.presentable_frame.color_glyph_draws"),
        "windows backend should iterate retained color glyph draws instead of inferring emoji runs again"
    );
    assert!(
        windows_backend_source.contains("frame.frame.presentable_frame.selection_overlay.rects"),
        "selection overlay draw stage should consume retained selection rectangles from the presenter"
    );
    assert!(
        windows_backend_source.contains("frame.frame.presentable_frame.underline_overlay.runs"),
        "underline overlay draw stage should consume retained underline runs from the presenter"
    );
    assert!(
        windows_backend_source.contains("frame.frame.presentable_frame.cursor_overlay.visible"),
        "cursor draw stage should consume retained cursor visibility and geometry from the presenter"
    );
    assert!(
        windows_backend_source.contains("frame.frame.presentable_frame.ime_preview_overlay.active"),
        "IME draw stage should consume retained IME preview state from the presenter"
    );
}

#[test]
fn windows_backend_source_hardens_device_loss_and_child_surface_present_contract() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("self.resolve_host_hwnd_if_needed();")
            && windows_backend_source.contains("self.ensure_child_surface_window();")
            && windows_backend_source.contains(
                "if self.state.host_hwnd.is_none() || self.state.surface_hwnd.is_none() {
            return;
        }"
            )
            && windows_backend_source.contains("self.state.ensure_hwnd_render_target();"),
        "windows backend present path should bail out once the host or child HWND disappears and otherwise recreate the dedicated child-surface Direct2D target instead of rebinding to the host window DC"
    );
    assert!(
        windows_backend_source.contains("fn end_frame(&mut self) -> bool"),
        "windows backend should report whether the Direct2D draw pass actually completed so lifecycle bookkeeping can ignore device-loss frames"
    );
    assert!(
        windows_backend_source.contains("if self.state.end_frame() {")
            && windows_backend_source
                .contains("self.state.last_presented_frame_token = frame.frame.frame_token;")
            && windows_backend_source.contains("self.state.fallback_paint_required = false;")
            && windows_backend_source
                .contains("self.set_child_surface_fallback_paint_enabled(false);"),
        "windows backend should advance last_presented_frame_token and clear child-surface fallback paint only after EndDraw succeeds"
    );
}

#[test]
fn native_terminal_pipeline_sources_keep_bitmap_raster_scale_hooks() {
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        presenter_source.contains("BitmapAtlasPresenter"),
        "terminal presenter seam should keep the bitmap atlas presenter for the software compatibility pipeline"
    );
    assert!(
        presenter_source.contains("PresentedTerminalFrame::Bitmap"),
        "terminal presenter seam should keep the bitmap frame variant for the software compatibility pipeline"
    );
    assert!(
        presenter_source.contains("fn set_raster_scale"),
        "terminal presenter seam should keep the bitmap raster scale hook so the atlas image path can respect HiDPI scale"
    );
    assert!(
        bootstrap_source.contains("host.set_raster_scale(scale_factor)"),
        "workspace terminal sync should keep pushing Slint window scale through the renderer host so the bitmap raster path still tracks HiDPI scale after the presenter-host seam extraction"
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
fn windows_native_renderer_source_threads_fractional_x_phase_into_raster_requests() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite backend");
    let atlas_source =
        fs::read_to_string("src/app/terminal_renderer/atlas.rs").expect("read glyph atlas");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");

    assert!(
        backend_source.contains("fractional_offset_x_bits"),
        "glyph raster requests should carry a fractional x phase bucket so hinted glyph rasters stop being incorrectly reused across different subpixel positions"
    );
    assert!(
        backend_source.contains("raster_request_with_fractional_offset_x"),
        "loaded fonts should expose a dedicated raster request helper for fractional x phases instead of forcing native-quality glyph rendering through whole-pixel origins"
    );
    assert!(
        dwrite_source.contains(".offset(SwashVector::new(request.fractional_offset_x(), 0.0))"),
        "windows dwrite rasterization should feed the renderer-provided fractional x phase into swash so hinted grayscale glyphs stay sharp across both the live and retired Windows paths"
    );
    assert!(
        renderer_source.contains("split_fractional_offset")
            && renderer_source.contains("raster_request_with_fractional_offset_x"),
        "native renderer should preserve fractional x positioning long enough to build phase-aware glyph raster requests"
    );
    assert!(
        atlas_source.contains("fractional_offset_x_bits"),
        "glyph atlas keys should partition monochrome glyph cache entries by fractional x phase so the renderer does not reuse the wrong hinted bitmap"
    );
}

#[test]
fn terminal_style_contract_threads_bold_and_underline_across_runtime_model_and_layout() {
    let runtime_contracts_source =
        fs::read_to_string("src/app/ssh/runtime/contracts.rs").expect("read runtime contracts");
    let wezterm_adapter_source = fs::read_to_string("src/app/terminal_core/wezterm_adapter.rs")
        .expect("read wezterm adapter");
    let model_source =
        fs::read_to_string("src/app/terminal_model.rs").expect("read terminal model");
    let segmentation_source = fs::read_to_string("src/app/terminal_layout/run_segmentation.rs")
        .expect("read run segmentation");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");

    assert!(
        runtime_contracts_source.contains("pub bold: bool"),
        "runtime terminal cells should expose the bold SGR state"
    );
    assert!(
        runtime_contracts_source.contains("pub underline: bool"),
        "runtime terminal cells should expose the underline SGR state"
    );
    assert!(
        wezterm_adapter_source.contains("attrs.intensity()"),
        "wezterm adapter projection should derive bold state from wezterm cell intensity before the runtime surface snapshot crosses the core adapter boundary"
    );
    assert!(
        wezterm_adapter_source.contains("attrs.underline()"),
        "wezterm adapter projection should derive underline state from wezterm cell underline metadata before the runtime surface snapshot reaches the renderer/model stack"
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
        model_source.contains("pub bg_rgba: u32"),
        "terminal model cells should preserve background color state from runtime snapshots"
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
        segmentation_source.contains("pub bg_rgba: u32"),
        "text style keys should include background color so retained native frames can draw ANSI background runs"
    );
    assert!(
        model_source.contains("cell.bold.hash(hasher);"),
        "terminal model row hashing should include bold state so style-only changes still invalidate downstream renderer fingerprints"
    );
    assert!(
        model_source.contains("cell.underline.hash(hasher);"),
        "terminal model row hashing should include underline state so decoration-only changes still invalidate downstream renderer fingerprints"
    );
    assert!(
        model_source.contains("cell.bg_rgba.hash(hasher);"),
        "terminal model row hashing should include background color so background-only style changes still invalidate downstream renderer fingerprints"
    );
    assert!(
        renderer_source.contains("row.row_hash.hash(&mut hasher);"),
        "native frame fingerprints should consume the model-layer row hash so the renderer can keep style-sensitive invalidation without re-hashing every run and glyph on each frame"
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
        renderer_source.contains("fonts.rasterize_glyph(&frame.font, request)?")
            || renderer_source.contains("fonts.rasterize_glyph(font, request)?"),
        "native renderer should rasterize through the shared font backend contract even when prepare-path helpers abstract the direct call site"
    );
    assert!(
        !renderer_source.contains("DirectWriteFontSystem"),
        "native renderer should stop naming DirectWriteFontSystem directly once rasterization moves behind the backend contract"
    );
    assert!(
        backend_source.contains("pub face_key: FontFaceKey"),
        "glyph raster requests should carry the resolved fallback face key so monochrome fallback runs stop rasterizing through the primary face by accident"
    );
    assert!(
        renderer_source.contains("run.resolved_face.face_key"),
        "native renderer should pass the resolved fallback face key into glyph raster requests so fallback glyphs can be rasterized from their actual face data"
    );
    assert!(
        backend_source.contains("pub visible_left_px: i32")
            && backend_source.contains("pub visible_top_px: i32")
            && backend_source.contains("pub visible_width_px: u32")
            && backend_source.contains("pub visible_height_px: u32"),
        "rasterized glyph contracts should carry visible bounds metadata so later Windows native drawing stages can consume overhang data without reverse-engineering it from coverage buffers"
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
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite backend");
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
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite backend");

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
fn windows_native_font_loading_contract_drops_retired_windows_entrypoint() {
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite backend");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");

    assert!(
        !dwrite_source.contains(&format!(
            "pub fn {}",
            retired_windows_subsystem::retired_font_loader_name()
        )),
        "windows dwrite backend should stop exposing a dedicated retired Windows font-loading entrypoint once retained-native is the only supported Windows path"
    );
    assert!(
        !presenter_source.contains(&format!(
            "font_system.{}(&request)?",
            retired_windows_subsystem::retired_font_loader_name()
        )),
        "terminal presenter source should stop loading fonts through a retired Windows entrypoint once that subsystem is deleted"
    );
}

#[test]
fn windows_native_font_backend_source_switches_to_directwrite_face_resolution_and_metrics() {
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite backend");

    assert!(
        dwrite_source.contains("DWriteCreateFactory"),
        "Windows native font backend should create a real DirectWrite factory instead of resolving every face from bundled font bytes"
    );
    assert!(
        dwrite_source.contains("IDWriteFactory"),
        "Windows native font backend should keep a DirectWrite factory handle for system face lookup"
    );
    assert!(
        dwrite_source.contains("GetSystemFontCollection"),
        "Windows native font backend should read from the Windows system font collection when resolving terminal fallback faces"
    );
    assert!(
        dwrite_source.contains("FindFamilyName"),
        "Windows native font backend should resolve requested font families through the DirectWrite font collection instead of only matching metadata tags"
    );
    assert!(
        dwrite_source.contains("GetFirstMatchingFont"),
        "Windows native font backend should ask DirectWrite for the actual matching face inside each family"
    );
    assert!(
        dwrite_source.contains("CreateFontFace"),
        "Windows native font backend should materialize DirectWrite font faces instead of keeping fallback families as metadata-only labels"
    );
    assert!(
        dwrite_source.contains("GetMetrics"),
        "Windows native font backend should pull ascent, descent, and line metrics from DirectWrite instead of only relying on ab_glyph heuristics"
    );
    assert!(
        dwrite_source.contains("GetFiles"),
        "Windows native font backend should enumerate DirectWrite font files so fallback runs can bind to actual per-face font data"
    );
    assert!(
        dwrite_source.contains("MapCharacters"),
        "Windows native font backend should use DirectWrite fallback mapping so mixed-script text no longer collapses to a single fake face"
    );
    assert!(
        !dwrite_source.contains("self.font_bytes"),
        "Windows native font backend should stop hard-binding every fallback run to one bundled font byte slice"
    );
}

#[test]
fn native_font_metrics_source_exposes_explicit_baseline_contract() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite backend");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");

    assert!(
        backend_source.contains("pub baseline_px: f32"),
        "font metrics should expose an explicit baseline_px field so the live and retired Windows paths stop reverse-engineering row baselines from raw ascent metrics"
    );
    assert!(
        dwrite_source.contains("let baseline_px =") && dwrite_source.contains("baseline_px,"),
        "windows dwrite font loading should compute and store an explicit row baseline instead of leaving the renderer to infer one from ascent rounding"
    );
    assert!(
        renderer_source.contains("frame.font.metrics().baseline_px.round() as i32"),
        "native renderer should read the loaded-font baseline contract directly so hinted glyph placement stays stable across the live and retired Windows composition paths"
    );
    assert!(
        !renderer_source.contains("frame.font.metrics().ascent_px.round() as i32"),
        "native renderer should stop deriving row baselines from raw ascent rounding once the font backend exposes explicit baseline metrics"
    );
}

#[test]
fn windows_backend_source_hardens_detach_and_device_loss_contracts() {
    assert!(
        Path::new("src/app/terminal_renderer/damage.rs").exists(),
        "Task 7 should add the shared native surface damage tracker module before wiring lifecycle hardening into the Windows backend"
    );

    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");
    let windows_frame_source =
        fs::read_to_string("src/app/windows_frame.rs").expect("read windows frame interop");

    assert!(
        windows_backend_source.contains("pub attached: bool"),
        "Windows backend state should retain an explicit attachment guard so detach blocks later present work"
    );
    assert!(
        windows_backend_source.contains("self.state.attached = true;"),
        "Windows backend attach should mark the backend attached before later draw scheduling runs"
    );
    assert!(
        windows_backend_source.contains("self.state.attached = false;"),
        "Windows backend detach should flip the attachment guard before clearing retained resources"
    );
    assert!(
        windows_backend_source.contains("if !self.state.attached {"),
        "Windows backend present and update hooks should bail out once detach or shutdown has invalidated the surface"
    );
    assert!(
        windows_backend_source.contains("if err.code() == D2DERR_RECREATE_TARGET {")
            && windows_backend_source.contains("self.clear_device_resources();"),
        "Windows backend should clear stale Direct2D resources when target loss forces device recreation"
    );
    assert!(
        !windows_backend_source.contains("self.state.monochrome_glyph_bitmaps.clear();")
            && !windows_backend_source.contains("self.state.color_glyph_bitmaps.clear();"),
        "Windows backend detach should preserve CPU-side glyph payload caches so prepared-row reuse can survive a later surface reattach without requiring fresh upload payloads"
    );
    assert!(
        windows_frame_source.contains("pub fn native_surface_is_attached("),
        "windows_frame should expose a helper for reading attachment state from native surface diagnostics during shutdown and recovery diagnostics"
    );
    assert!(
        windows_frame_source.contains("diagnostics.hwnd.is_some()"),
        "windows_frame attachment helper should derive attachment from the latest native surface diagnostics snapshot"
    );
}
