use anyhow::Result;
use mica_term::app::ssh::runtime::{TerminalSurfaceState, TerminalSession};
use mica_term::app::terminal_atlas::{TerminalAtlasRenderer, TerminalAtlasSelection};
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

#[test]
fn atlas_renderer_loads_sarasa_metrics_and_emits_a_surface_image() -> Result<()> {
    let surface = render_surface(4, 12, "hello sarasa\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;

    let metrics = renderer.metrics();
    let frame = renderer.render(&surface)?;

    assert!(
        metrics.cell_width >= 9,
        "terminal cell width should grow slightly so the default Sarasa atlas no longer reads undersized"
    );
    assert!(
        metrics.cell_height >= 23,
        "terminal cell height should increase slightly so the default atlas text reads larger on standard-density displays"
    );
    assert!(
        metrics.cell_width <= 10,
        "terminal cell width should stay compact for a monospace terminal grid"
    );
    assert!(
        metrics.cell_height <= 24,
        "terminal cell height should stay compact even after increasing readability"
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
        first.image.to_rgba8().expect("rgba image before").as_slice(),
        second.image.to_rgba8().expect("rgba image after").as_slice(),
        "selection-aware rendering should visibly change the atlas pixel buffer"
    );

    Ok(())
}

#[test]
fn atlas_renderer_handles_cjk_and_nerd_font_cells_without_falling_back_to_blank_rows() -> Result<()> {
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

    assert_eq!(wide.width, 2);
    assert_eq!(icon.width, 1);
    assert!(
        buffer.as_slice().iter().any(|pixel| {
            pixel.r != default_bg.r || pixel.g != default_bg.g || pixel.b != default_bg.b
        }),
        "rendered image should contain glyph pixels beyond the default terminal background"
    );

    Ok(())
}
