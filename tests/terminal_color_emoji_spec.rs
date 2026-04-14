use anyhow::Result;
use mica_term::app::ssh::runtime::{TerminalSession, TerminalSurfaceState};
use mica_term::app::terminal_atlas::{ClusterSpriteKind, TerminalAtlasRenderer};
use mica_term::app::terminal_emoji::{
    ClusterRenderKind, EmojiFallbackReason, EmojiFontRasterizeRequest, EmojiFontResolution,
    EmojiRasterizerBackend, EmojiRenderOutcome, EmojiSprite, ResolvedEmojiFont,
    TerminalEmojiRenderer, TerminalEmojiResolver, classify_cluster_render_kind,
    recommended_emoji_font_size_px,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::mock::mock_font_system;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::{
    ColorGlyphRaster, DirectWriteFontSystem, FontFaceKey, FontFallbackFace, FontRequest,
    FontSystem, GlyphRasterRequest, LoadedFont, RasterizedGlyph, TextShapingRequest,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::run_segmentation::RunCluster;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::{GlyphRun, PositionedGlyph, ShapedRow, TextStyleKey};
use mica_term::app::terminal_presenter::{
    PresentedTerminalFrame, TerminalPresentationOptions, TerminalPresenter, WindowsNativePresenter,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};
use slint::Rgba8Pixel;
use std::fs;
#[path = "support/retired_windows_subsystem.rs"]
mod retired_windows_subsystem;
use uuid::Uuid;

fn render_surface(rows: usize, cols: usize, text: &str) -> TerminalSurfaceState {
    let mut session = TerminalSession::new(rows, cols);
    session.apply_remote_bytes(text.as_bytes());
    session.surface_state(Uuid::new_v4())
}

fn unpack_rgba(color: u32) -> Rgba8Pixel {
    Rgba8Pixel {
        a: ((color >> 24) & 0xff) as u8,
        r: ((color >> 16) & 0xff) as u8,
        g: ((color >> 8) & 0xff) as u8,
        b: (color & 0xff) as u8,
    }
}

fn fake_color_emoji_renderer() -> TerminalEmojiRenderer {
    TerminalEmojiRenderer::with_backend(
        TerminalEmojiResolver::from_resolution(EmojiFontResolution::Resolved(ResolvedEmojiFont {
            face_id: fontdb::ID::dummy(),
            family_name: "Noto Color Emoji".to_string(),
        })),
        Box::new(FakeAtlasEmojiBackend),
    )
}

fn atlas_renderer_with_fake_color_emoji() -> Result<TerminalAtlasRenderer> {
    TerminalAtlasRenderer::with_emoji_renderer_for_tests(fake_color_emoji_renderer())
}

fn present_native_frame(
    presenter: &mut WindowsNativePresenter,
    surface: &TerminalSurfaceState,
) -> Result<mica_term::app::terminal_presenter::NativeTerminalFrame> {
    match presenter.present(surface, TerminalPresentationOptions::default())? {
        PresentedTerminalFrame::Native(frame) => Ok(*frame),
        PresentedTerminalFrame::Bitmap(_) => {
            panic!("WindowsNativePresenter should never emit bitmap frames in native tests")
        }
    }
}

#[test]
fn native_presenter_reloads_font_metrics_when_raster_scale_changes() -> Result<()> {
    let mut presenter = WindowsNativePresenter::new()?;
    let (base_cell_width, base_cell_height) = presenter.default_cell_size();

    presenter.set_raster_scale(2.0);
    let (scaled_cell_width, scaled_cell_height) = presenter.default_cell_size();

    assert!(
        scaled_cell_width > base_cell_width,
        "native presenter should reload a larger device-pixel font when raster scale increases"
    );
    assert!(
        scaled_cell_height > base_cell_height,
        "native presenter should reload a taller device-pixel line box when raster scale increases"
    );

    Ok(())
}

#[test]
fn windows_native_presenter_keeps_a_more_readable_text_line_box() -> Result<()> {
    let native = WindowsNativePresenter::new()?;

    assert!(
        native.default_cell_size().1 >= 24,
        "Windows terminal defaults should reserve at least a 24px row box so the larger 16px-class Semibold body text still keeps vertical breathing room"
    );
    assert!(
        native.default_cell_size().0 >= 8,
        "Windows terminal defaults should preserve at least the shared 8px Sarasa Term SC Nerd SemiBold column box so dense prompt output does not collapse below the current terminal typography contract"
    );

    Ok(())
}

#[test]
fn windows_terminal_sources_remove_legacy_windows_presenter_plumbing() {
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let mod_source = fs::read_to_string("src/app/mod.rs").expect("read app mod");

    assert!(
        !presenter_source.contains(&retired_windows_subsystem::retired_presenter_name()),
        "terminal presenter source should stop defining the retired Windows software presenter once retained-native is the only live Windows path"
    );
    assert!(
        !bootstrap_source.contains(&retired_windows_subsystem::retired_builder_name()),
        "bootstrap should stop referencing the retired Windows presenter builder once retained-native is the only live Windows path"
    );
    assert!(
        !bootstrap_source.contains(&format!(
            "TerminalCompositionMode::{}",
            retired_windows_subsystem::retired_pascal_name()
        )),
        "bootstrap should stop branching on the retired Windows composition mode once subsystem switching is removed"
    );
    assert!(
        !mod_source.contains(&retired_windows_subsystem::retired_mod_export()),
        "app module exports should stop exposing the retired Windows renderer module once that subsystem is deleted"
    );
}

#[test]
fn emoji_clusters_are_not_treated_as_blank_terminal_cells() -> Result<()> {
    let surface = render_surface(4, 12, "🦀\r\n");
    let mut renderer = atlas_renderer_with_fake_color_emoji()?;
    let frame = renderer.render(&surface)?;
    let buffer = frame.image.to_rgba8().expect("rgba image");
    let default_bg = unpack_rgba(surface.default_bg_rgba);
    let crab = frame
        .rendered_clusters
        .iter()
        .find(|cluster| cluster.text == "🦀")
        .expect("emoji cell should be observed in rendered clusters");

    assert_eq!(
        crab.sprite_kind,
        ClusterSpriteKind::ColorRgba,
        "emoji-presenting terminal cells should route to the color sprite path"
    );
    assert!(
        buffer.as_slice().iter().any(|pixel| {
            pixel.r != default_bg.r || pixel.g != default_bg.g || pixel.b != default_bg.b
        }),
        "emoji-presenting cells should paint visible pixels instead of disappearing into the row background"
    );

    Ok(())
}

#[test]
fn repeated_emoji_cells_use_cached_color_sprite_entries() -> Result<()> {
    let surface = render_surface(4, 12, "🦀🦀\r\n");
    let mut renderer = atlas_renderer_with_fake_color_emoji()?;

    let first = renderer.render(&surface)?;
    let second = renderer.render(&surface)?;
    let repeated_emoji = second
        .rendered_clusters
        .iter()
        .filter(|cluster| cluster.text == "🦀")
        .collect::<Vec<_>>();

    assert!(
        !repeated_emoji.is_empty(),
        "the repeated emoji cluster should be reported for inspection"
    );
    assert!(
        repeated_emoji
            .iter()
            .all(|cluster| cluster.sprite_kind == ClusterSpriteKind::ColorRgba),
        "repeated emoji cells should keep using the color sprite path"
    );
    assert_eq!(
        first.cache_entries, second.cache_entries,
        "re-rendering the same emoji frame should reuse cached sprite entries"
    );
    assert!(
        second.rerendered_rows.is_empty(),
        "re-rendering an identical emoji frame should not dirty any rows"
    );

    Ok(())
}

#[test]
fn classify_cluster_render_kind_routes_emoji_clusters_to_color_rendering() {
    for text in ["🦀", "📦", "🌐", "❤️", "👨‍💻"] {
        assert_eq!(
            classify_cluster_render_kind(text),
            ClusterRenderKind::Emoji,
            "`{text}` should classify as an emoji-rendered cluster"
        );
    }
}

#[test]
fn classify_cluster_render_kind_keeps_private_use_and_ascii_clusters_on_mono_path() {
    for text in ["", "ascii", "git"] {
        assert_eq!(
            classify_cluster_render_kind(text),
            ClusterRenderKind::Mono,
            "`{text}` should stay on the mono terminal atlas path"
        );
    }
}

#[test]
fn classify_cluster_render_kind_keeps_plain_keycap_bases_on_mono_path() {
    for text in ["0", "1", "2", "#", "*"] {
        assert_eq!(
            classify_cluster_render_kind(text),
            ClusterRenderKind::Mono,
            "`{text}` should stay on the mono terminal atlas path"
        );
    }
}

#[test]
fn classify_cluster_render_kind_routes_explicit_keycap_sequences_to_color_rendering() {
    for text in ["1️⃣", "#️⃣", "*️⃣"] {
        assert_eq!(
            classify_cluster_render_kind(text),
            ClusterRenderKind::Emoji,
            "`{text}` should classify as an emoji-rendered cluster"
        );
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn windows_text_engine_keeps_plain_symbols_off_the_color_emoji_path() -> Result<()> {
    let mut fonts = DirectWriteFontSystem::new()?;
    let font = fonts.load_font(&FontRequest::default())?;

    for text in ["⚙", "✖", "☁"] {
        let runs = fonts.shape_text_runs(&font, &TextShapingRequest::new(text))?;
        assert_eq!(runs.len(), 1, "`{text}` should shape as a single run");
        assert!(
            !runs[0].has_color_glyphs,
            "`{text}` should stay on the mono/symbol path instead of being forced through the color emoji fallback pipeline"
        );
    }

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn windows_text_engine_still_marks_real_emoji_clusters_as_color_glyphs() -> Result<()> {
    let mut fonts = DirectWriteFontSystem::new()?;
    fonts.set_emoji_renderer_for_tests(fake_color_emoji_renderer());
    let font = fonts.load_font(&FontRequest::default())?;

    for text in ["🦀", "🙂", "⚙️"] {
        let runs = fonts.shape_text_runs(&font, &TextShapingRequest::new(text))?;
        assert_eq!(runs.len(), 1, "`{text}` should shape as a single run");
        assert!(
            runs[0].has_color_glyphs,
            "`{text}` should remain on the color emoji path"
        );
    }

    Ok(())
}

#[test]
fn windows_text_engine_splits_leading_emoji_from_ascii_tail() {
    let source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read windows dwrite source");

    assert!(
        source.contains("split_shaping_subruns_by_family("),
        "the Windows shaper should split leading emoji into their own subruns before rasterization so Segoe UI Emoji never receives mixed emoji-plus-ASCII text"
    );
    assert!(
        source.contains("let primary_family = font")
            || source.contains("let primary_family = font\n"),
        "the Windows shaper should derive per-grapheme fallback resolution from the requested terminal family instead of the first fallback face encountered in the run"
    );
    assert!(
        !source.contains("let primary_family = fallback_faces"),
        "the Windows shaper should stop using the first fallback face as the primary family because an emoji-leading run would otherwise smear the ASCII tail onto Segoe UI Emoji"
    );
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn windows_text_engine_demotes_failed_color_emoji_rasterization_to_mono_runs() -> Result<()> {
    let mut fonts = DirectWriteFontSystem::new()?;
    fonts.set_emoji_renderer_for_tests(TerminalEmojiRenderer::with_backend(
        TerminalEmojiResolver::from_resolution(EmojiFontResolution::Resolved(ResolvedEmojiFont {
            face_id: fontdb::ID::dummy(),
            family_name: "Segoe UI Emoji".to_string(),
        })),
        Box::new(FakeEmojiRasterizerBackend { sprite: None }),
    ));
    let font = fonts.load_font(&FontRequest::default())?;
    let runs = fonts.shape_text_runs(&font, &TextShapingRequest::new("🦀"))?;

    assert_eq!(runs.len(), 1, "emoji probe should stay within a single run");
    assert!(
        !runs[0].has_color_glyphs,
        "when the Windows color emoji rasterizer cannot produce a sprite, the renderer should fall back to the monochrome glyph path instead of caching a synthetic placeholder block"
    );
    assert!(
        runs[0]
            .glyphs
            .first()
            .map(|glyph| glyph.glyph_id < 0x8000_0000)
            .unwrap_or(false),
        "the fallback run should keep real shaped glyph ids instead of switching to a synthetic placeholder sprite id"
    );

    Ok(())
}

#[test]
fn recommended_emoji_font_size_stays_within_terminal_cell_bounds() {
    let single_cell_size = recommended_emoji_font_size_px(1, 10, 22);
    let double_cell_size = recommended_emoji_font_size_px(2, 10, 22);

    assert!(
        single_cell_size <= 10.0,
        "single-cell emoji should fit the cell width"
    );
    assert!(
        single_cell_size < 22.0,
        "single-cell emoji should keep vertical padding inside the cell"
    );
    assert!(
        double_cell_size <= 20.0,
        "double-cell emoji should stay within the available span width"
    );
    assert!(
        double_cell_size < 22.0,
        "double-cell emoji should keep vertical padding inside the cell"
    );
}

#[test]
fn emoji_resolver_reports_a_visible_fallback_when_no_preferred_font_is_available() {
    let resolver = TerminalEmojiResolver::from_database(fontdb::Database::new());

    assert_eq!(
        resolver.resolve_preferred_font(),
        EmojiFontResolution::VisibleFallback {
            replacement_text: "�".to_string(),
            reason: EmojiFallbackReason::MissingPreferredFont,
        }
    );
}

#[derive(Clone)]
struct FakeEmojiRasterizerBackend {
    sprite: Option<EmojiSprite>,
}

impl EmojiRasterizerBackend for FakeEmojiRasterizerBackend {
    fn rasterize(&self, request: EmojiFontRasterizeRequest<'_>) -> Option<EmojiSprite> {
        assert_eq!(request.text, "🦀");
        assert!(request.span >= 1);
        assert!(request.cell_width > 0);
        assert!(request.cell_height > 0);
        self.sprite.clone()
    }
}

struct FakeAtlasEmojiBackend;

impl EmojiRasterizerBackend for FakeAtlasEmojiBackend {
    fn rasterize(&self, request: EmojiFontRasterizeRequest<'_>) -> Option<EmojiSprite> {
        let width = request.span.max(1) * request.cell_width;
        let height = request.cell_height;
        let mut rgba = vec![0u8; (width * height * 4) as usize];

        for y in 1..height.saturating_sub(1) {
            for x in 1..width.saturating_sub(1) {
                let index = ((y * width + x) * 4) as usize;
                let (r, g, b) = if x * 2 >= width {
                    (0xf4, 0xb4, 0x00)
                } else {
                    (0xf4, 0x43, 0x36)
                };
                rgba[index] = r;
                rgba[index + 1] = g;
                rgba[index + 2] = b;
                rgba[index + 3] = 0xff;
            }
        }

        Some(EmojiSprite {
            width,
            height,
            rgba,
        })
    }
}

#[test]
fn first_monochrome_native_frame_carries_upload_payload_then_reuses_cache() -> Result<()> {
    let surface = render_surface(4, 12, "A\r\n");
    let mut presenter = WindowsNativePresenter::new()?;

    let first = present_native_frame(&mut presenter, &surface)?;
    let second = present_native_frame(&mut presenter, &surface)?;

    assert!(
        first
            .presentable_frame
            .monochrome_glyph_draws
            .iter()
            .any(|draw| draw.upload.as_ref().is_some_and(|upload| {
                !upload.coverage.is_empty()
                    && upload.width_px > 0
                    && upload.height_px > 0
                    && upload.advance_px >= 0
            })),
        "the first monochrome native frame should carry glyph upload payload bytes for backend resource creation"
    );
    assert!(
        second
            .presentable_frame
            .monochrome_glyph_draws
            .iter()
            .all(|draw| draw.upload.is_none()),
        "re-presenting the same monochrome frame should reuse cached glyph resources instead of resending upload payloads"
    );

    Ok(())
}

#[test]
fn first_color_native_frame_carries_upload_payload_then_reuses_cache() -> Result<()> {
    let surface = render_surface(4, 12, "🦀\r\n");
    let mut presenter = WindowsNativePresenter::new()?;
    presenter.set_emoji_renderer_for_tests(fake_color_emoji_renderer());

    let first = present_native_frame(&mut presenter, &surface)?;
    let second = present_native_frame(&mut presenter, &surface)?;

    assert!(
        first
            .presentable_frame
            .color_glyph_draws
            .iter()
            .any(|draw| draw.upload.as_ref().is_some_and(|upload| {
                !upload.rgba.is_empty() && upload.width_px > 0 && upload.height_px > 0
            })),
        "the first color native frame should carry RGBA upload payload bytes for backend resource creation"
    );
    assert!(
        second
            .presentable_frame
            .color_glyph_draws
            .iter()
            .all(|draw| draw.upload.is_none()),
        "re-presenting the same color frame should reuse cached color glyph resources instead of resending upload payloads"
    );

    Ok(())
}

#[test]
fn monochrome_native_frame_draws_expose_stable_pixel_destinations() -> Result<()> {
    let surface = render_surface(
        4, 12, "AB
",
    );
    let mut presenter = WindowsNativePresenter::new()?;

    let frame = present_native_frame(&mut presenter, &surface)?;
    let draws = &frame.presentable_frame.monochrome_glyph_draws;

    assert!(
        draws.len() >= 2,
        "a two-character monochrome row should expose at least two monochrome glyph draws for placement inspection"
    );
    assert!(
        draws[1].dest_x_px > draws[0].dest_x_px,
        "monochrome glyph destinations should advance across the row instead of collapsing to one origin"
    );
    assert!(
        draws.iter().all(|draw| draw.dest_y_px >= 0),
        "monochrome glyph destinations should keep non-negative pixel baselines inside the terminal surface"
    );

    Ok(())
}

#[test]
fn presentable_native_frame_threads_palette_contract_and_color_destinations() -> Result<()> {
    let surface = render_surface(
        4, 12, "🦀
",
    );
    let mut presenter = WindowsNativePresenter::new()?;
    presenter.set_emoji_renderer_for_tests(fake_color_emoji_renderer());

    let frame = present_native_frame(&mut presenter, &surface)?;
    let presentable = &frame.presentable_frame;

    assert_eq!(presentable.default_fg_rgba, surface.default_fg_rgba);
    assert_eq!(presentable.default_bg_rgba, surface.default_bg_rgba);
    assert_eq!(presentable.row_bg_even_rgba, surface.row_bg_even_rgba);
    assert_eq!(presentable.row_bg_odd_rgba, surface.row_bg_odd_rgba);
    assert_eq!(presentable.grid_rows, surface.rows);
    assert_eq!(presentable.grid_cols, surface.cols);
    assert!(
        presentable
            .color_glyph_draws
            .iter()
            .any(|draw| draw.dest_x_px >= 0 && draw.dest_y_px >= 0),
        "color glyph draws should expose stable destination pixels for the backend bitmap path"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn dwrite_color_glyph_raster_is_not_a_flat_placeholder_block() -> Result<()> {
    let mut fonts = DirectWriteFontSystem::new()?;
    fonts.set_emoji_renderer_for_tests(fake_color_emoji_renderer());
    let loaded_font = fonts.load_font(&FontRequest::default())?;
    let shaped_runs = fonts.shape_text_runs(&loaded_font, &TextShapingRequest::new("🦀"))?;
    let color_run = shaped_runs
        .iter()
        .find(|run| run.has_color_glyphs)
        .expect("emoji shaping should produce a color glyph run");
    let glyph_id = color_run
        .glyphs
        .first()
        .map(|glyph| glyph.glyph_id)
        .expect("color glyph run should expose at least one glyph");
    let raster = fonts
        .rasterize_color_glyph(&loaded_font, &color_run.resolved_face, glyph_id)?
        .expect("emoji glyph should rasterize into RGBA data");
    let visible_colors = raster
        .rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[3] != 0)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        raster.rgba.len(),
        (raster.width_px * raster.height_px * 4) as usize,
        "color glyph raster should expose tightly packed RGBA bytes"
    );
    assert!(
        visible_colors.len() >= 2,
        "real color glyph rasters should contain multiple visible colors instead of a flat placeholder fill"
    );

    Ok(())
}

#[test]
fn windows_dwrite_color_glyph_source_prefers_real_emoji_rasterization_over_inline_blocks() {
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read windows dwrite font backend");

    assert!(
        dwrite_source.contains("TerminalEmojiRenderer"),
        "windows color glyph path should route through the shared terminal emoji rasterizer instead of an inline placeholder block"
    );
    assert!(
        dwrite_source.contains("rasterize_cluster"),
        "windows color glyph path should call the emoji cluster rasterizer when preparing color glyph sprites"
    );
    assert!(
        !dwrite_source.contains("glyph_id % 127"),
        "windows color glyph rasterization should stop synthesizing fake RGBA colors from glyph IDs"
    );
}

#[test]
fn native_renderer_color_glyph_contract_uses_separate_cache_state() {
    let atlas_source =
        fs::read_to_string("src/app/terminal_renderer/atlas.rs").expect("read renderer atlas");
    let renderer_source = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read native renderer");
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read windows dwrite font backend");
    let shaper_source =
        fs::read_to_string("src/app/terminal_layout/shaper.rs").expect("read terminal shaper");

    assert!(
        atlas_source.contains("GlyphCacheKind::Monochrome"),
        "native atlas contract should reserve the atlas path for monochrome glyph entries"
    );
    assert!(
        atlas_source.contains("pub struct ColorGlyphCacheEntry"),
        "native renderer should expose a dedicated color glyph cache entry contract"
    );
    assert!(
        renderer_source.contains("color_glyph_cache"),
        "native renderer should keep color glyphs in a dedicated cache state instead of the monochrome atlas"
    );
    assert!(
        renderer_source.contains("rasterize_color_glyph"),
        "renderer preparation should call the explicit color glyph raster contract for emoji runs"
    );
    assert!(
        renderer_source.contains("pub color_glyph_draws: Vec<PreparedColorGlyphDraw>"),
        "prepared native frames should expose dedicated color glyph draw payloads"
    );
    assert!(
        renderer_source.contains("pub monochrome_glyph_draws: Vec<PreparedMonochromeGlyphDraw>"),
        "prepared native frames should keep monochrome atlas draw payloads separate from color glyph draws"
    );
    assert!(
        renderer_source.contains("cache_entry: ColorGlyphCacheEntry"),
        "color glyph draw payloads should retain color cache references instead of reusing monochrome atlas entries"
    );
    assert!(
        renderer_source.contains("atlas_entry: GlyphAtlasEntry"),
        "monochrome glyph draw payloads should keep atlas entry references for non-color runs"
    );
    assert!(
        dwrite_source.contains("ColorGlyphRaster"),
        "Windows DWrite backend should expose RGBA color glyph rasters for emoji presentation"
    );
    assert!(
        shaper_source.contains("has_color_glyphs"),
        "text shaping should keep color glyph intent explicit so the renderer can separate cache paths"
    );
    assert!(
        renderer_source.contains("color_glyphs_prepared"),
        "prepared frame metadata should count color glyph work separately from monochrome atlas uploads"
    );
}

#[test]
fn windows_backend_source_keeps_color_glyphs_separate_from_overlay_stages() {
    let windows_backend_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows platform backend");

    assert!(
        windows_backend_source.contains("pub last_drawn_color_glyphs: usize"),
        "windows backend should track color glyph draw counts separately from monochrome glyph draws"
    );
    assert!(
        windows_backend_source.contains("pub last_drawn_selection_rects: usize"),
        "windows backend should track selection overlay draws separately from text draws"
    );
    assert!(
        windows_backend_source.contains("pub last_drawn_underline_runs: usize"),
        "windows backend should track underline overlay draws separately from text draws"
    );
    assert!(
        windows_backend_source.contains("pub last_drawn_cursor_overlay_visible: bool"),
        "windows backend should track whether the cursor overlay draw stage ran"
    );
    assert!(
        windows_backend_source.contains("pub last_drawn_ime_preview_active: bool"),
        "windows backend should track whether the IME preview draw stage ran"
    );
}

#[test]
fn semantic_output_overlay_contract_stays_in_display_list_layer() {
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read terminal presenter");
    let semantic_source = fs::read_to_string("src/app/terminal_semantic/output_blocks.rs")
        .expect("read terminal semantic output blocks");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        presenter_source.contains("pub semantic_overlays: Vec<SemanticOutputOverlay>"),
        "presentable native frames should carry semantic overlays as retained display-list payloads"
    );
    assert!(
        presenter_source.contains("detect_output_block_overlays(&frame_model)"),
        "native presenter should derive semantic overlays from the terminal model instead of rewriting terminal colors"
    );
    assert!(
        semantic_source.contains("TerminalModelFrame"),
        "semantic output detection should analyze the shared terminal model layer"
    );
    assert!(
        !semantic_source.contains("TerminalCellState"),
        "semantic overlays should not mutate the runtime terminal cell state or ANSI truth"
    );
    assert!(
        bootstrap_source.contains("presentable_frame.semantic_overlays"),
        "bootstrap should thread semantic overlays alongside the retained native frame payload"
    );
}

#[test]
fn emoji_rasterizer_returns_rgba_sprite_data_from_backend() {
    let renderer = TerminalEmojiRenderer::with_backend(
        TerminalEmojiResolver::from_resolution(EmojiFontResolution::Resolved(ResolvedEmojiFont {
            face_id: fontdb::ID::dummy(),
            family_name: "Noto Color Emoji".to_string(),
        })),
        Box::new(FakeEmojiRasterizerBackend {
            sprite: Some(EmojiSprite {
                width: 2,
                height: 1,
                rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
            }),
        }),
    );

    assert_eq!(
        renderer.rasterize_cluster("🦀", 1, 10, 22),
        EmojiRenderOutcome::Sprite(EmojiSprite {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
        })
    );
}

#[test]
fn emoji_rasterizer_returns_visible_fallback_on_backend_failure() {
    let renderer = TerminalEmojiRenderer::with_backend(
        TerminalEmojiResolver::from_resolution(EmojiFontResolution::Resolved(ResolvedEmojiFont {
            face_id: fontdb::ID::dummy(),
            family_name: "Noto Color Emoji".to_string(),
        })),
        Box::new(FakeEmojiRasterizerBackend { sprite: None }),
    );

    assert_eq!(
        renderer.rasterize_cluster("🦀", 1, 10, 22),
        EmojiRenderOutcome::VisibleFallback {
            replacement_text: "�".to_string(),
            reason: EmojiFallbackReason::RasterizationFailed,
        }
    );
}

#[cfg(feature = "terminal-native-renderer")]
struct DistinctColorFaceFontSystem;

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for DistinctColorFaceFontSystem {
    fn load_font(&mut self, _request: &FontRequest) -> Result<LoadedFont> {
        unreachable!("test prepares a loaded font up front")
    }

    fn shape_text(
        &mut self,
        _font: &LoadedFont,
        _text: &str,
    ) -> Result<Vec<mica_term::app::terminal_font::ShapedGlyph>> {
        unreachable!("renderer prepare should not call shape_text in this cache-key test")
    }

    fn rasterize_glyph(
        &mut self,
        _font: &LoadedFont,
        _request: GlyphRasterRequest,
    ) -> Result<RasterizedGlyph> {
        unreachable!("renderer prepare should stay on the color path in this cache-key test")
    }

    fn rasterize_color_glyph(
        &mut self,
        _font: &LoadedFont,
        _resolved_face: &FontFallbackFace,
        glyph_id: u32,
    ) -> Result<Option<ColorGlyphRaster>> {
        Ok(Some(ColorGlyphRaster {
            width_px: 4,
            height_px: 4,
            rgba: vec![glyph_id as u8; 4 * 4 * 4],
        }))
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_keeps_color_cache_entries_distinct_per_fallback_face() -> Result<()> {
    let mut font_system = mock_font_system();
    let loaded_font = font_system.load_font(&FontRequest::default())?;
    let style = TextStyleKey {
        fg_rgba: 0xffd8_dfe8,
        bg_rgba: 0xff0c_1014,
        bold: false,
        underline: false,
    };
    let shaped_frame = ShapedTerminalFrame {
        seqno: 1,
        font: loaded_font,
        rows: vec![ShapedRow {
            row: 0,
            content_hash: 0,
            row_hash: 0,
            runs: vec![
                GlyphRun {
                    row: 0,
                    cell_range: 0..2,
                    text: "🙂".into(),
                    clusters: vec![RunCluster {
                        text: "🙂".into(),
                        cell_range: 0..2,
                        byte_range: 0..4,
                    }],
                    glyphs: vec![PositionedGlyph {
                        glyph_id: 77,
                        cluster: 0,
                        x_advance: 0,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    }],
                    style,
                    resolved_face: FontFallbackFace {
                        face_key: FontFaceKey(100),
                        family_name: "Segoe UI Emoji".into(),
                    },
                    feature_set: Default::default(),
                    allow_ligatures: true,
                    has_color_glyphs: true,
                },
                GlyphRun {
                    row: 0,
                    cell_range: 2..4,
                    text: "🦀".into(),
                    clusters: vec![RunCluster {
                        text: "🦀".into(),
                        cell_range: 2..4,
                        byte_range: 0..4,
                    }],
                    glyphs: vec![PositionedGlyph {
                        glyph_id: 77,
                        cluster: 0,
                        x_advance: 0,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    }],
                    style,
                    resolved_face: FontFallbackFace {
                        face_key: FontFaceKey(200),
                        family_name: "Noto Color Emoji".into(),
                    },
                    feature_set: Default::default(),
                    allow_ligatures: true,
                    has_color_glyphs: true,
                },
            ],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut stub_fonts = DistinctColorFaceFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut stub_fonts)?;

    assert_eq!(
        prepared.color_glyph_cache_entries, 2,
        "color glyph cache keys should stay distinct when the same glyph id is resolved by different fallback faces"
    );
    assert!(
        prepared
            .color_glyph_draws
            .iter()
            .all(|draw| draw.upload.is_some()),
        "the first frame should upload separate color bitmap payloads for distinct fallback-face cache keys"
    );

    Ok(())
}
