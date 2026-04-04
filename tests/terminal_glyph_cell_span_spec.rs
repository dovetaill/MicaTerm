#[cfg(feature = "terminal-native-renderer")]
use anyhow::Result;
#[cfg(feature = "terminal-native-renderer")]
use anyhow::bail;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::mock::mock_font_system;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::{
    FontFaceKey, FontMetrics, FontRenderProfile, FontRequest, FontSystem, FontFallbackFace,
    GlyphRasterRequest, LoadedFont, RasterizedGlyph,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::{
    GlyphRun, PositionedGlyph, ShapedRow, TextStyleKey, shape_row,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::run_segmentation::RunCluster;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_model::{TerminalModelCell, TerminalModelRow};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};

#[cfg(feature = "terminal-native-renderer")]
struct OverhangCellSpanFontSystem;

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for OverhangCellSpanFontSystem {
    fn load_font(&mut self, _request: &FontRequest) -> Result<LoadedFont> {
        bail!("load_font is not used in this test")
    }

    fn shape_text(
        &mut self,
        _font: &LoadedFont,
        _text: &str,
    ) -> Result<Vec<mica_term::app::terminal_font::ShapedGlyph>> {
        bail!("shape_text is not used in this test")
    }

    fn rasterize_glyph(
        &mut self,
        _font: &LoadedFont,
        _request: GlyphRasterRequest,
    ) -> Result<RasterizedGlyph> {
        Ok(RasterizedGlyph {
            width_px: 4,
            height_px: 2,
            bearing_x_px: 2,
            bearing_y_px: 0,
            visible_left_px: 2,
            visible_top_px: 0,
            visible_width_px: 4,
            visible_height_px: 2,
            advance_px: 3,
            coverage: vec![255; 8],
        })
    }
}

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

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_keeps_logical_cell_span_even_when_visible_bounds_extend_past_it() -> Result<()> {
    let style = TextStyleKey {
        fg_rgba: 0xffd8_dfe8,
        bg_rgba: 0xff0c_1014,
        bold: false,
        underline: false,
    };
    let shaped_frame = ShapedTerminalFrame {
        seqno: 1,
        font: LoadedFont::new(
            FontFaceKey(1),
            FontRequest {
                family_name: Some("Test Terminal".into()),
                px_size: 4.0,
            },
            FontMetrics {
                units_per_em: 4,
                ascent_px: 3.0,
                descent_px: -1.0,
                line_gap_px: 0.0,
                baseline_px: 3.0,
                cell_width_px: 4.0,
                cell_height_px: 4.0,
            },
            FontRenderProfile::default(),
        ),
        rows: vec![ShapedRow {
            row: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..1,
                text: "W".into(),
                clusters: vec![RunCluster {
                    text: "W".into(),
                    cell_range: 0..1,
                    byte_range: 0..1,
                }],
                glyphs: vec![PositionedGlyph {
                    glyph_id: 1,
                    cluster: 0,
                    x_advance: 3,
                    y_advance: 0,
                    x_offset: 0,
                    y_offset: 0,
                }],
                style,
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Test Terminal".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = OverhangCellSpanFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    let draw = prepared
        .monochrome_glyph_draws
        .first()
        .expect("monochrome draw");

    assert_eq!(draw.start_col, 0);
    assert_eq!(draw.end_col, 0);
    assert!(
        draw.dest_x_px + draw.atlas_entry.padding_left_px as i32 > 0,
        "renderer should keep the logical cell ownership for hit-testing while allowing visible bounds to extend beyond the cell span"
    );

    Ok(())
}
