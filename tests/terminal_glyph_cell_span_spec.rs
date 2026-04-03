#[cfg(feature = "terminal-native-renderer")]
use anyhow::Result;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::mock::mock_font_system;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::{FontRequest, FontSystem};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::shape_row;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_model::{TerminalModelCell, TerminalModelRow};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};

#[cfg(feature = "terminal-native-renderer")]
fn build_ascii_row(text: &str) -> TerminalModelRow {
    TerminalModelRow {
        row_index: 0,
        text: text.into(),
        wrapped: false,
        row_hash: 0,
        cells: text
            .chars()
            .enumerate()
            .map(|(col, ch)| TerminalModelCell {
                row: 0,
                col: col as u32,
                width: 1,
                text: ch.to_string(),
                bold: false,
                underline: false,
                fg_rgba: 0xffd8_dfe8,
                bg_rgba: 0xff0c_1014,
            })
            .collect(),
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_assigns_monochrome_glyph_draws_to_cluster_cell_spans() -> Result<()> {
    let row = build_ascii_row("ab");
    let mut fonts = mock_font_system();
    let loaded_font = fonts.load_font(&FontRequest::default())?;
    let shaped_row = shape_row(&row, &loaded_font, &mut fonts)?;
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;

    let prepared = renderer.prepare(
        &ShapedTerminalFrame {
            seqno: 1,
            font: loaded_font,
            rows: vec![shaped_row],
        },
        &mut fonts,
    )?;

    assert!(
        prepared.monochrome_glyph_draws.len() >= 2,
        "ascii pair should prepare at least two monochrome glyph draws"
    );
    assert_eq!(prepared.monochrome_glyph_draws[0].start_col, 0);
    assert_eq!(prepared.monochrome_glyph_draws[0].end_col, 0);
    assert_eq!(prepared.monochrome_glyph_draws[1].start_col, 1);
    assert_eq!(prepared.monochrome_glyph_draws[1].end_col, 1);

    Ok(())
}
