use anyhow::Result;
use mica_term::app::ssh::runtime::{TerminalSession, TerminalSurfaceState};
use mica_term::app::terminal_atlas::{ClusterSpriteKind, TerminalAtlasRenderer};
use mica_term::app::terminal_emoji::{
    ClusterRenderKind, EmojiFallbackReason, EmojiFontRasterizeRequest, EmojiFontResolution,
    EmojiRasterizerBackend, EmojiRenderOutcome, EmojiSprite, ResolvedEmojiFont,
    TerminalEmojiRenderer, TerminalEmojiResolver, classify_cluster_render_kind,
    recommended_emoji_font_size_px,
};
use mica_term::app::terminal_presenter::{
    PresentedTerminalFrame, TerminalPresentationOptions, TerminalPresenter, WindowsNativePresenter,
};
use slint::Rgba8Pixel;
use std::fs;
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
    }
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
        assert_eq!(request.span, 1);
        assert_eq!(request.cell_width, 10);
        assert_eq!(request.cell_height, 22);
        self.sprite.clone()
    }
}

struct FakeAtlasEmojiBackend;

impl EmojiRasterizerBackend for FakeAtlasEmojiBackend {
    fn rasterize(&self, request: EmojiFontRasterizeRequest<'_>) -> Option<EmojiSprite> {
        if request.text != "🦀" {
            return None;
        }

        let width = request.span.max(1) * request.cell_width;
        let height = request.cell_height;
        let mut rgba = vec![0u8; (width * height * 4) as usize];

        for y in 1..height.saturating_sub(1) {
            for x in 1..width.saturating_sub(1) {
                let index = ((y * width + x) * 4) as usize;
                rgba[index] = 0xf4;
                rgba[index + 1] = 0x43;
                rgba[index + 2] = 0x36;
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
