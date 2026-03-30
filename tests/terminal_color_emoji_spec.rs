use anyhow::Result;
use mica_term::app::ssh::runtime::{TerminalSession, TerminalSurfaceState};
use mica_term::app::terminal_atlas::{ClusterSpriteKind, TerminalAtlasRenderer};
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
fn emoji_clusters_are_not_treated_as_blank_terminal_cells() -> Result<()> {
    let surface = render_surface(4, 12, "🦀\r\n");
    let mut renderer = TerminalAtlasRenderer::new()?;
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
    let mut renderer = TerminalAtlasRenderer::new()?;

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
