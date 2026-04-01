use anyhow::Result;
use mica_term::app::ssh::runtime::{TerminalSession, TerminalSurfaceState};
use mica_term::app::terminal_atlas::{
    ClusterSpriteKind, TerminalAtlasRenderer, TerminalAtlasSelection,
};
use mica_term::app::terminal_emoji::{
    EmojiFontRasterizeRequest, EmojiFontResolution, EmojiRasterizerBackend, EmojiSprite,
    ResolvedEmojiFont, TerminalEmojiRenderer, TerminalEmojiResolver,
};
use mica_term::theme::ThemeMode;
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

fn pixel_at(image: &slint::Image, x: u32, y: u32) -> Rgba8Pixel {
    let buffer = image.to_rgba8().expect("rgba image");
    let width = buffer.width();
    buffer.as_slice()[(y * width + x) as usize]
}

fn count_non_background_pixels(image: &slint::Image, background: Rgba8Pixel) -> usize {
    image
        .to_rgba8()
        .expect("rgba image")
        .as_slice()
        .iter()
        .filter(|pixel| {
            pixel.r != background.r || pixel.g != background.g || pixel.b != background.b
        })
        .count()
}

fn renderer_with_fake_color_emoji() -> Result<TerminalAtlasRenderer> {
    TerminalAtlasRenderer::with_emoji_renderer_for_tests(TerminalEmojiRenderer::with_backend(
        TerminalEmojiResolver::from_resolution(EmojiFontResolution::Resolved(ResolvedEmojiFont {
            face_id: fontdb::ID::dummy(),
            family_name: "Noto Color Emoji".to_string(),
        })),
        Box::new(FakeAtlasEmojiBackend),
    ))
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

        for y in 2..height.saturating_sub(2) {
            for x in 2..width.saturating_sub(2) {
                let index = ((y * width + x) * 4) as usize;
                rgba[index] = 0xff;
                rgba[index + 1] = 0x7a;
                rgba[index + 2] = 0x00;
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
fn atlas_renderer_loads_sarasa_metrics_and_emits_a_surface_image() -> Result<()> {
    let surface = render_surface(4, 12, "hello sarasa\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let metrics = renderer.metrics();
    let frame = renderer.render(&surface)?;

    assert_eq!(
        metrics.cell_width, 8,
        "software atlas should stop forcing the bundled 7px Sarasa advance into a visibly loose 9px cell"
    );
    assert_eq!(
        metrics.cell_height, 20,
        "software atlas should tighten the bundled terminal line box instead of holding onto the older 22px minimum"
    );
    assert!(
        metrics.baseline_px > 0 && metrics.baseline_px < metrics.cell_height,
        "atlas metrics should expose a shared baseline so ASCII and CJK glyphs align vertically"
    );
    assert_eq!(
        frame.image.size(),
        [
            surface.cols * metrics.cell_width,
            surface.rows * metrics.cell_height,
        ]
        .into()
    );
    assert_eq!(frame.metrics, metrics);

    Ok(())
}

#[test]
fn atlas_renderer_rasterizes_bitmap_surface_at_hidpi_scale_without_changing_logical_cell_metrics()
-> Result<()> {
    let surface = render_surface(4, 12, "hidpi\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;
    let logical_metrics = renderer.metrics();

    renderer.set_raster_scale(2.0);
    let frame = renderer.render(&surface)?;

    assert_eq!(
        frame.metrics, logical_metrics,
        "bitmap atlas should keep reporting the logical cell metrics used by layout even when it rasterizes a denser hidpi backing image"
    );
    assert_eq!(
        frame.image.size(),
        [
            surface.cols * logical_metrics.cell_width * 2,
            surface.rows * logical_metrics.cell_height * 2,
        ]
        .into(),
        "hidpi bitmap atlas rendering should scale the backing image instead of leaving Slint to stretch a low-resolution terminal surface"
    );

    Ok(())
}

#[test]
fn atlas_renderer_default_background_rows_use_subtle_band_colors() -> Result<()> {
    let dark_surface = render_surface(4, 12, "");
    let mut dark_renderer = TerminalAtlasRenderer::new()?;
    let dark_frame = dark_renderer.render(&dark_surface)?;
    let dark_row0 = pixel_at(&dark_frame.image, 0, dark_frame.metrics.cell_height / 2);
    let dark_row1 = pixel_at(
        &dark_frame.image,
        0,
        dark_frame.metrics.cell_height + (dark_frame.metrics.cell_height / 2),
    );

    assert_eq!(dark_row0, unpack_rgba(0xff0c_1014));
    assert_eq!(dark_row1, unpack_rgba(0xff13_181e));

    let mut light_session = TerminalSession::new(4, 12);
    light_session.set_theme_mode(ThemeMode::Light);
    let light_surface = light_session.surface_state(Uuid::new_v4());
    let mut light_renderer = TerminalAtlasRenderer::new()?;
    let light_frame = light_renderer.render(&light_surface)?;
    let light_row0 = pixel_at(&light_frame.image, 0, light_frame.metrics.cell_height / 2);
    let light_row1 = pixel_at(
        &light_frame.image,
        0,
        light_frame.metrics.cell_height + (light_frame.metrics.cell_height / 2),
    );

    assert_eq!(light_row0, unpack_rgba(0xfffc_fdff));
    assert_eq!(light_row1, unpack_rgba(0xfff4_f8ff));

    Ok(())
}

#[test]
fn atlas_renderer_reuses_cached_sprites_for_identical_frames() -> Result<()> {
    let surface = render_surface(4, 20, "[root@host ~]# echo atlas\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let first = renderer.render(&surface)?;
    let second = renderer.render(&surface)?;

    assert_eq!(first.cache_entries, second.cache_entries);
    assert!(
        second.rerendered_rows.is_empty(),
        "identical terminal surfaces should not trigger dirty row redraws"
    );

    Ok(())
}

#[test]
fn atlas_renderer_only_redraws_rows_that_changed() -> Result<()> {
    let first_surface = render_surface(4, 24, "one\r\ntwo\r\n");
    let second_surface = render_surface(4, 24, "one\r\nTWO\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let _ = renderer.render(&first_surface)?;
    let second = renderer.render(&second_surface)?;

    assert_eq!(
        second.rerendered_rows,
        vec![1],
        "only the modified row should be rerasterized between sequential frames"
    );

    Ok(())
}

#[test]
fn atlas_renderer_selection_changes_invalidate_and_repaint_rows() -> Result<()> {
    let surface = render_surface(4, 24, "selected text\r\nnext row\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let first = renderer.render(&surface)?;
    let second = renderer.render_with_selection(
        &surface,
        Some(TerminalAtlasSelection::new(0, 0, 0, 7)),
        0x6625_63eb,
    )?;
    let third = renderer.render_with_selection(
        &surface,
        Some(TerminalAtlasSelection::new(0, 0, 0, 7)),
        0x6625_63eb,
    )?;

    assert_eq!(
        second.rerendered_rows,
        vec![0],
        "selection-only changes should invalidate just the selected row"
    );
    assert!(
        third.rerendered_rows.is_empty(),
        "re-rendering the same selected frame should reuse cached row hashes"
    );
    assert_ne!(
        first
            .image
            .to_rgba8()
            .expect("rgba image before")
            .as_slice(),
        second
            .image
            .to_rgba8()
            .expect("rgba image after")
            .as_slice(),
        "selection-aware rendering should visibly change the atlas pixel buffer"
    );

    Ok(())
}

#[test]
fn atlas_renderer_handles_cjk_and_nerd_font_cells_without_falling_back_to_blank_rows() -> Result<()>
{
    let surface = render_surface(4, 20, "界  maple\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let frame = renderer.render(&surface)?;
    let buffer = frame.image.to_rgba8().expect("rgba image");
    let default_bg = unpack_rgba(surface.default_bg_rgba);
    let wide = surface
        .cells
        .iter()
        .find(|cell| cell.text == "界")
        .expect("wide cjk cell");
    let icon = surface
        .cells
        .iter()
        .find(|cell| cell.text == "")
        .expect("nerd font icon cell");
    let rendered_icon = frame
        .rendered_clusters
        .iter()
        .find(|cluster| cluster.text == "")
        .expect("nerd font icon should be observed in the rendered cluster list");

    assert_eq!(wide.width, 2);
    assert_eq!(icon.width, 1);
    assert_eq!(
        rendered_icon.sprite_kind,
        ClusterSpriteKind::MonoAlpha,
        "private-use Nerd Font cells must remain on the Sarasa mono sprite path"
    );
    assert!(
        buffer.as_slice().iter().any(|pixel| {
            pixel.r != default_bg.r || pixel.g != default_bg.g || pixel.b != default_bg.b
        }),
        "rendered image should contain glyph pixels beyond the default terminal background"
    );

    Ok(())
}

#[test]
fn atlas_renderer_composites_color_emoji_sprites_and_preserves_them_under_selection() -> Result<()>
{
    let surface = render_surface(4, 12, "🦀\r\n");
    let mut renderer = renderer_with_fake_color_emoji()?;

    let before = renderer.render(&surface)?;
    let selected = renderer.render_with_selection(
        &surface,
        Some(TerminalAtlasSelection::new(0, 0, 0, 0)),
        0x6625_63eb,
    )?;
    let center_x = before.metrics.cell_width / 2;
    let center_y = before.metrics.cell_height / 2;
    let selected_center = pixel_at(&selected.image, center_x, center_y);

    assert!(
        before
            .rendered_clusters
            .iter()
            .any(|cluster| cluster.text == "🦀"
                && cluster.sprite_kind == ClusterSpriteKind::ColorRgba),
        "emoji clusters should be composited through the RGBA sprite path"
    );
    assert_eq!(selected.rerendered_rows, vec![0]);
    assert_ne!(
        before.image.to_rgba8().expect("before rgba").as_slice(),
        selected.image.to_rgba8().expect("selected rgba").as_slice(),
        "selection-aware rendering should still repaint around an emoji cell"
    );
    assert_eq!(
        selected_center,
        Rgba8Pixel {
            a: 255,
            r: 255,
            g: 122,
            b: 0,
        },
        "selection repainting must not erase previously composited emoji pixels"
    );

    Ok(())
}

#[test]
fn atlas_renderer_draws_underlined_cells_with_extra_baseline_ink() -> Result<()> {
    let plain_surface = render_surface(4, 12, "A\r\n");
    let underlined_surface = render_surface(4, 12, "\x1b[4mA\x1b[0m\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let plain = renderer.render(&plain_surface)?;
    let underlined = renderer.render(&underlined_surface)?;
    let baseline_y = underlined.metrics.cell_height.saturating_sub(2);
    let background = unpack_rgba(underlined_surface.default_bg_rgba);

    assert_ne!(
        plain.image.to_rgba8().expect("plain rgba").as_slice(),
        underlined
            .image
            .to_rgba8()
            .expect("underlined rgba")
            .as_slice(),
        "underlined terminal cells should paint additional pixels beyond the plain glyph sprite"
    );
    assert_ne!(
        pixel_at(
            &underlined.image,
            underlined.metrics.cell_width / 2,
            baseline_y
        ),
        background,
        "underlined cells should paint a visible underline near the baseline"
    );

    Ok(())
}

#[test]
fn atlas_renderer_draws_bold_cells_with_more_ink_than_plain_cells() -> Result<()> {
    let plain_surface = render_surface(4, 12, "A\r\n");
    let bold_surface = render_surface(4, 12, "\x1b[1mA\x1b[0m\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let plain = renderer.render(&plain_surface)?;
    let bold = renderer.render(&bold_surface)?;
    let background = unpack_rgba(bold_surface.default_bg_rgba);

    assert!(
        count_non_background_pixels(&bold.image, background)
            > count_non_background_pixels(&plain.image, background),
        "bold terminal cells should occupy more lit pixels than the plain regular-weight glyph"
    );

    Ok(())
}
