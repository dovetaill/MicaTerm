use anyhow::Result;
use mica_term::app::ssh::runtime::{TerminalSession, TerminalSurfaceState};
use mica_term::app::terminal_atlas::{ClusterSpriteKind, TerminalAtlasRenderer};
use mica_term::app::terminal_emoji::{
    ClusterRenderKind, EmojiFallbackReason, EmojiFontRasterizeRequest, EmojiRasterizerBackend,
    EmojiRenderOutcome, EmojiSprite, EmojiFontResolution, ResolvedEmojiFont,
    TerminalEmojiRenderer, TerminalEmojiResolver, classify_cluster_render_kind,
};
use slint::Rgba8Pixel;
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
        TerminalEmojiResolver::from_resolution(EmojiFontResolution::Resolved(
            ResolvedEmojiFont {
                face_id: fontdb::ID::dummy(),
                family_name: "Noto Color Emoji".to_string(),
            },
        )),
        Box::new(FakeAtlasEmojiBackend),
    )
}

fn atlas_renderer_with_fake_color_emoji() -> Result<TerminalAtlasRenderer> {
    TerminalAtlasRenderer::with_emoji_renderer_for_tests(fake_color_emoji_renderer())
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
fn emoji_rasterizer_returns_rgba_sprite_data_from_backend() {
    let renderer = TerminalEmojiRenderer::with_backend(
        TerminalEmojiResolver::from_resolution(EmojiFontResolution::Resolved(
            ResolvedEmojiFont {
                face_id: fontdb::ID::dummy(),
                family_name: "Noto Color Emoji".to_string(),
            },
        )),
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
        TerminalEmojiResolver::from_resolution(EmojiFontResolution::Resolved(
            ResolvedEmojiFont {
                face_id: fontdb::ID::dummy(),
                family_name: "Noto Color Emoji".to_string(),
            },
        )),
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
