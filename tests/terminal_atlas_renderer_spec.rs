use anyhow::Result;
use mica_term::app::ssh::runtime::{
    TerminalCellState, TerminalCursorShape, TerminalCursorState, TerminalRowState, TerminalSession,
    TerminalSurfaceState,
};
use mica_term::app::terminal_atlas::{
    ClusterSpriteKind, RenderedClusterSourceKind, TerminalAtlasRenderer, TerminalAtlasSelection,
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

fn manual_surface(rows: u32, cols: u32, cells: Vec<TerminalCellState>) -> TerminalSurfaceState {
    let mut visible_lines = vec![String::new(); rows as usize];
    for cell in &cells {
        if let Some(line) = visible_lines.get_mut(cell.row as usize) {
            line.push_str(&cell.text);
        }
    }
    let visible_rows = visible_lines
        .iter()
        .enumerate()
        .map(|(index, text)| TerminalRowState {
            index: index as u32,
            text: text.clone(),
            wrapped: false,
        })
        .collect();

    TerminalSurfaceState {
        session_id: Uuid::new_v4(),
        seqno: 1,
        rows,
        cols,
        default_fg_rgba: 0xffd8_dfe8,
        default_bg_rgba: 0xff0c_1014,
        row_bg_even_rgba: 0xff0c_1014,
        row_bg_odd_rgba: 0xff0c_1014,
        viewport_offset_lines: 0,
        viewport_max_offset_lines: 0,
        viewport_at_bottom: true,
        visible_rows,
        visible_lines,
        cells,
        cursor: TerminalCursorState {
            row: 0,
            col: 0,
            visible: false,
            blinking: false,
            shape: TerminalCursorShape::Block,
            fg_rgba: 0xffd8_dfe8,
            bg_rgba: 0xff0c_1014,
        },
        alternate_screen_active: false,
        mouse_grabbed: false,
        application_cursor_keys: false,
        bracketed_paste_enabled: false,
        shell_integration: Default::default(),
    }
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

fn color_distance(a: Rgba8Pixel, b: Rgba8Pixel) -> u16 {
    let dr = (i16::from(a.r) - i16::from(b.r)).unsigned_abs();
    let dg = (i16::from(a.g) - i16::from(b.g)).unsigned_abs();
    let db = (i16::from(a.b) - i16::from(b.b)).unsigned_abs();

    dr + dg + db
}

fn count_non_row_background_pixels(image: &slint::Image) -> usize {
    let buffer = image.to_rgba8().expect("rgba image");
    let width = buffer.width() as usize;
    if width == 0 {
        return 0;
    }

    let pixels = buffer.as_slice();
    pixels
        .chunks(width)
        .map(|row| {
            let background = row.last().copied().expect("row background");
            row.iter()
                .filter(|pixel| {
                    pixel.r != background.r || pixel.g != background.g || pixel.b != background.b
                })
                .count()
        })
        .sum()
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
fn atlas_renderer_loads_sarasa_term_metrics_and_emits_a_surface_image() -> Result<()> {
    let surface = render_surface(4, 12, "hello cascadia\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let metrics = renderer.metrics();
    let frame = renderer.render(&surface)?;

    assert_eq!(
        metrics.cell_width, 8,
        "software atlas should expose the bundled Sarasa Term SC atlas font on its measured 8px logical cell"
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
fn atlas_renderer_viewport_background_is_quiet_and_row_band_free() -> Result<()> {
    let dark_surface = render_surface(4, 12, "");
    let mut dark_renderer = TerminalAtlasRenderer::new()?;
    let dark_frame = dark_renderer.render(&dark_surface)?;
    let dark_top = pixel_at(&dark_frame.image, 0, dark_frame.metrics.cell_height / 2);
    let dark_row1 = pixel_at(
        &dark_frame.image,
        0,
        dark_frame.metrics.cell_height + (dark_frame.metrics.cell_height / 2),
    );
    let dark_bottom = pixel_at(
        &dark_frame.image,
        0,
        dark_frame
            .image
            .size()
            .height
            .saturating_sub(dark_frame.metrics.cell_height / 2 + 1),
    );

    assert!(
        color_distance(dark_top, dark_row1) <= 6,
        "dark viewport background should not alternate into visible row banding between adjacent rows"
    );
    assert!(
        color_distance(dark_top, dark_bottom) <= 2,
        "dark viewport background should stay effectively flat for the Ayu default preset instead of drifting into a renderer-only gradient"
    );

    let mut light_session = TerminalSession::new(4, 12);
    light_session.set_theme_mode(ThemeMode::Light);
    let light_surface = light_session.surface_state(Uuid::new_v4());
    let mut light_renderer = TerminalAtlasRenderer::new()?;
    let light_frame = light_renderer.render(&light_surface)?;
    let light_top = pixel_at(&light_frame.image, 0, light_frame.metrics.cell_height / 2);
    let light_row1 = pixel_at(
        &light_frame.image,
        0,
        light_frame.metrics.cell_height + (light_frame.metrics.cell_height / 2),
    );
    let light_bottom = pixel_at(
        &light_frame.image,
        0,
        light_frame
            .image
            .size()
            .height
            .saturating_sub(light_frame.metrics.cell_height / 2 + 1),
    );

    assert_eq!(
        light_top.a, 255,
        "light viewport fill should stay fully opaque so empty terminal space reads like the same solid surface as text rows"
    );
    assert!(
        color_distance(light_top, light_row1) <= 6,
        "light viewport background should not reintroduce parity-based banding between adjacent empty rows"
    );
    assert!(
        color_distance(light_top, light_bottom) <= 2,
        "light viewport background should stay effectively flat for the Ayu default preset instead of drifting into a renderer-only gradient"
    );

    Ok(())
}

#[test]
fn atlas_renderer_preserves_explicit_cell_backgrounds_over_unified_viewport_fill() -> Result<()> {
    let mut surface = render_surface(2, 4, "");
    let explicit_bg = 0xffc8_6414;
    surface.cells.push(TerminalCellState {
        row: 0,
        col: 1,
        width: 1,
        text: " ".to_string(),
        bold: false,
        underline: false,
        fg_rgba: surface.default_fg_rgba,
        bg_rgba: explicit_bg,
    });

    let mut renderer = TerminalAtlasRenderer::new()?;
    let frame = renderer.render(&surface)?;
    let explicit_pixel = pixel_at(
        &frame.image,
        frame.metrics.cell_width + (frame.metrics.cell_width / 2),
        frame.metrics.cell_height / 2,
    );
    let neighbor_pixel = pixel_at(
        &frame.image,
        (frame.metrics.cell_width * 2) + (frame.metrics.cell_width / 2),
        frame.metrics.cell_height / 2,
    );

    assert_eq!(
        explicit_pixel,
        unpack_rgba(explicit_bg),
        "explicit ANSI/cell background colors should still paint on top of the unified viewport background layer"
    );
    assert_ne!(
        explicit_pixel, neighbor_pixel,
        "only cells with an explicit background should diverge from the shared viewport fill"
    );

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
fn atlas_renderer_word_selection_repaints_only_the_token_row() -> Result<()> {
    let surface = render_surface(4, 32, "hello-world/path.txt\r\nnext row\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let _ = renderer.render(&surface)?;
    let selected = renderer.render_with_selection(
        &surface,
        Some(TerminalAtlasSelection::new(0, 0, 0, 20)),
        0x6625_63eb,
    )?;

    assert_eq!(
        selected.rerendered_rows,
        vec![0],
        "double-click token selection should repaint only the affected visual row instead of dirtifying unrelated rows"
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
        "private-use Nerd Font cells must remain on a Sarasa-family-owned monochrome sprite path"
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
        Some(TerminalAtlasSelection::new(0, 0, 0, 1)),
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
    let plain_count = count_non_row_background_pixels(&plain.image);
    let bold_count = count_non_row_background_pixels(&bold.image);

    assert!(
        bold_count > plain_count,
        "bold terminal cells should occupy more lit pixels than the plain regular-weight glyph"
    );

    Ok(())
}

#[test]
fn atlas_renderer_generated_box_masks_eliminate_seam_gaps_and_preserve_body_text_sources()
-> Result<()> {
    let mut renderer = TerminalAtlasRenderer::new()?;
    let mut surface = manual_surface(
        3,
        8,
        vec![
            cell(0, 0, "╭"),
            cell(0, 1, "─"),
            cell(0, 2, "─"),
            cell(0, 3, "─"),
            cell(0, 4, "─"),
            cell(0, 5, "╮"),
            cell(1, 0, "│"),
            cell(1, 1, "C"),
            cell(1, 2, "o"),
            cell(1, 3, "d"),
            cell(1, 4, "e"),
            cell(1, 5, "x"),
            cell(1, 6, "│"),
            cell(2, 0, "╰"),
            cell(2, 1, "─"),
            cell(2, 2, "─"),
            cell(2, 3, "─"),
            cell(2, 4, "─"),
            cell(2, 5, "╯"),
        ],
    );

    let frame = renderer.render(&surface)?;
    assert_eq!(
        frame
            .rendered_clusters
            .iter()
            .find(|cluster| cluster.text == "╭")
            .expect("box glyph cluster")
            .source_kind,
        RenderedClusterSourceKind::GeneratedMask,
        "box-drawing glyphs should be observable as generated masks once Task 4 switches bitmap atlas rendering onto the shared generator"
    );
    assert_eq!(
        frame
            .rendered_clusters
            .iter()
            .find(|cluster| cluster.text == "C")
            .expect("body text cluster")
            .source_kind,
        RenderedClusterSourceKind::FontRaster,
        "body text inside the same bitmap frame should stay on the font-raster path"
    );
    assert_eq!(
        seam_gap_count_for_verticals(&frame.image, &frame.raster_metrics, &surface),
        0,
        "generated vertical box strokes should not leave background leaks on row seams"
    );
    assert_eq!(
        seam_gap_count_for_horizontals(
            &frame.image,
            &frame.raster_metrics,
            &frame.rendered_clusters,
            &surface
        ),
        0,
        "generated horizontal box strokes should not leave background leaks on column seams"
    );

    surface = manual_surface(
        1,
        8,
        vec![
            cell(0, 0, "A"),
            cell(0, 1, "╭"),
            cell(0, 2, "─"),
            cell(0, 3, "╮"),
            cell(0, 4, "中"),
        ],
    );
    let mixed = renderer.render(&surface)?;
    for (text, expected_source) in [
        ("A", RenderedClusterSourceKind::FontRaster),
        ("╭", RenderedClusterSourceKind::GeneratedMask),
        ("─", RenderedClusterSourceKind::GeneratedMask),
        ("╮", RenderedClusterSourceKind::GeneratedMask),
        ("中", RenderedClusterSourceKind::FontRaster),
    ] {
        assert_eq!(
            mixed
                .rendered_clusters
                .iter()
                .find(|cluster| cluster.text == text)
                .expect("mixed cluster should exist")
                .source_kind,
            expected_source,
            "mixed atlas rendering should keep `{text}` on the expected source path"
        );
    }

    Ok(())
}

#[test]
fn atlas_renderer_generated_block_masks_keep_expected_fill_ratios_across_scales() -> Result<()> {
    let surface = manual_surface(
        1,
        8,
        vec![
            cell(0, 0, "█"),
            cell(0, 1, "▀"),
            cell(0, 2, "▄"),
            cell(0, 3, "▌"),
            cell(0, 4, "▐"),
        ],
    );
    let mut renderer = TerminalAtlasRenderer::new()?;

    for scale in [1.0, 1.25, 2.0] {
        renderer.set_raster_scale(scale);
        let frame = renderer.render(&surface)?;

        assert_eq!(
            rendered_fill_ratio(&frame, "█"),
            1.0,
            "full block should stay fully filled at scale {scale}"
        );
        assert_half_fill(
            rendered_fill_ratio(&frame, "▀"),
            "upper half block should stay half filled at scale {scale}",
        );
        assert_half_fill(
            rendered_fill_ratio(&frame, "▄"),
            "lower half block should stay half filled at scale {scale}",
        );
        assert_half_fill(
            rendered_fill_ratio(&frame, "▌"),
            "left half block should stay half filled at scale {scale}",
        );
        assert_half_fill(
            rendered_fill_ratio(&frame, "▐"),
            "right half block should stay half filled at scale {scale}",
        );
    }

    Ok(())
}

fn rendered_fill_ratio(
    frame: &mica_term::app::terminal_atlas::TerminalSurfaceFrame,
    text: &str,
) -> f32 {
    let cluster = frame
        .rendered_clusters
        .iter()
        .find(|cluster| cluster.text == text)
        .expect("cluster should exist");
    let buffer = frame.image.to_rgba8().expect("rgba image");
    let background = unpack_rgba(0xff0c_1014);
    let start_x = cluster.col * frame.raster_metrics.cell_width;
    let start_y = cluster.row * frame.raster_metrics.cell_height;
    let mut lit = 0usize;
    let mut total = 0usize;

    for y in start_y..start_y + frame.raster_metrics.cell_height {
        for x in start_x..start_x + frame.raster_metrics.cell_width {
            let pixel = buffer.as_slice()[(y * buffer.width() + x) as usize];
            total += 1;
            if pixel != background {
                lit += 1;
            }
        }
    }

    (lit as f32 / total as f32 * 100.0).round() / 100.0
}

fn seam_gap_count_for_verticals(
    image: &slint::Image,
    metrics: &mica_term::app::terminal_atlas::TerminalAtlasMetrics,
    surface: &TerminalSurfaceState,
) -> usize {
    let background = unpack_rgba(surface.default_bg_rgba);
    let buffer = image.to_rgba8().expect("rgba image");
    let x = metrics.cell_width / 2;
    let mut gaps = 0usize;

    for seam_row in 1..surface.rows.saturating_sub(1) {
        let y = seam_row * metrics.cell_height;
        let pixel = buffer.as_slice()[(y * buffer.width() + x) as usize];
        if pixel == background {
            gaps += 1;
        }
    }

    gaps
}

fn seam_gap_count_for_horizontals(
    image: &slint::Image,
    metrics: &mica_term::app::terminal_atlas::TerminalAtlasMetrics,
    rendered_clusters: &[mica_term::app::terminal_atlas::RenderedCluster],
    surface: &TerminalSurfaceState,
) -> usize {
    let background = unpack_rgba(surface.default_bg_rgba);
    let buffer = image.to_rgba8().expect("rgba image");
    let y = metrics.cell_height / 2;
    let mut gaps = 0usize;

    let seam_cols = rendered_clusters
        .iter()
        .filter(|cluster| {
            cluster.row == 0 && cluster.source_kind == RenderedClusterSourceKind::GeneratedMask
        })
        .map(|cluster| cluster.col)
        .collect::<std::collections::BTreeSet<_>>();

    for seam_col in seam_cols.into_iter().skip(1) {
        let x = seam_col * metrics.cell_width;
        let pixel = buffer.as_slice()[(y * buffer.width() + x) as usize];
        if pixel == background {
            gaps += 1;
        }
    }

    gaps
}

fn cell(row: u32, col: u32, text: &str) -> TerminalCellState {
    TerminalCellState {
        row,
        col,
        width: 1,
        text: text.to_string(),
        bold: false,
        underline: false,
        fg_rgba: 0xffd8_dfe8,
        bg_rgba: 0xff0c_1014,
    }
}

fn assert_half_fill(ratio: f32, message: &str) {
    assert!(
        (ratio - 0.5).abs() <= 0.05,
        "{message}: observed ratio {ratio}"
    );
}
