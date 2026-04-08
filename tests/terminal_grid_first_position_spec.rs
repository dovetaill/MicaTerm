#[cfg(feature = "terminal-native-renderer")]
use anyhow::{Result, bail};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::{
    FontFaceKey, FontFallbackFace, FontMetrics, FontRenderProfile, FontRequest, FontSystem,
    GlyphRasterRequest, LoadedFont, RasterizedGlyph,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::run_segmentation::RunCluster;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::{GlyphRun, PositionedGlyph, ShapedRow, TextStyleKey};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};

#[cfg(feature = "terminal-native-renderer")]
struct FixedGridFontSystem;

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for FixedGridFontSystem {
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
        request: GlyphRasterRequest,
    ) -> Result<RasterizedGlyph> {
        let (width_px, advance_px) = match request.glyph_id {
            2 => (0, 0),
            _ => (4, 8),
        };
        Ok(RasterizedGlyph {
            width_px,
            height_px: 4,
            bearing_x_px: 0,
            bearing_y_px: 0,
            visible_left_px: 0,
            visible_top_px: 0,
            visible_width_px: width_px,
            visible_height_px: 4,
            advance_px,
            coverage: vec![255; (width_px.max(1) * 4) as usize],
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn test_font() -> LoadedFont {
    LoadedFont::new(
        FontFaceKey(1),
        FontRequest {
            family_name: Some("Grid Test".into()),
            px_size: 8.0,
        },
        FontMetrics {
            units_per_em: 8,
            ascent_px: 6.0,
            descent_px: -2.0,
            line_gap_px: 0.0,
            baseline_px: 6.0,
            cell_width_px: 8.0,
            cell_height_px: 8.0,
        },
        FontRenderProfile::default(),
    )
}

#[cfg(feature = "terminal-native-renderer")]
fn style() -> TextStyleKey {
    TextStyleKey {
        fg_rgba: 0xffd8_dfe8,
        bg_rgba: 0xff0c_1014,
        bold: false,
        underline: false,
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn resolved_face() -> FontFallbackFace {
    FontFallbackFace {
        face_key: FontFaceKey(1),
        family_name: "Grid Test".into(),
    }
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn ascii_clusters_snap_to_cell_starts_instead_of_accumulating_shaped_drift() -> Result<()> {
    let shaped_frame = ShapedTerminalFrame {
        seqno: 1,
        font: test_font(),
        rows: vec![ShapedRow {
            row: 0,
            content_hash: 0,
            row_hash: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..3,
                text: "iii".into(),
                clusters: vec![
                    RunCluster {
                        text: "i".into(),
                        cell_range: 0..1,
                        byte_range: 0..1,
                    },
                    RunCluster {
                        text: "i".into(),
                        cell_range: 1..2,
                        byte_range: 1..2,
                    },
                    RunCluster {
                        text: "i".into(),
                        cell_range: 2..3,
                        byte_range: 2..3,
                    },
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 0,
                        x_advance: 7,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 1,
                        x_advance: 7,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 2,
                        x_advance: 7,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style: style(),
                resolved_face: resolved_face(),
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = FixedGridFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    let glyphs = &prepared.monochrome_glyph_draws;
    assert_eq!(
        glyphs.len(),
        3,
        "three ascii cells should produce three draws"
    );
    assert_eq!(
        glyphs[0].dest_x_px + glyphs[0].atlas_entry.padding_left_px as i32,
        0
    );
    assert_eq!(
        glyphs[1].dest_x_px + glyphs[1].atlas_entry.padding_left_px as i32,
        8
    );
    assert_eq!(
        glyphs[2].dest_x_px + glyphs[2].atlas_entry.padding_left_px as i32,
        16
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn wide_cluster_keeps_following_ascii_cell_on_its_own_grid_column() -> Result<()> {
    let shaped_frame = ShapedTerminalFrame {
        seqno: 2,
        font: test_font(),
        rows: vec![ShapedRow {
            row: 0,
            content_hash: 0,
            row_hash: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..3,
                text: "条a".into(),
                clusters: vec![
                    RunCluster {
                        text: "条".into(),
                        cell_range: 0..2,
                        byte_range: 0..3,
                    },
                    RunCluster {
                        text: "a".into(),
                        cell_range: 2..3,
                        byte_range: 3..4,
                    },
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 0,
                        x_advance: 8,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 3,
                        cluster: 3,
                        x_advance: 8,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style: style(),
                resolved_face: resolved_face(),
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = FixedGridFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    let glyphs = &prepared.monochrome_glyph_draws;
    assert_eq!(
        glyphs.len(),
        2,
        "wide char plus ascii should produce two draws"
    );
    assert_eq!(glyphs[0].start_col, 0);
    assert_eq!(glyphs[0].end_col, 1);
    assert_eq!(glyphs[1].start_col, 2);
    assert_eq!(glyphs[1].end_col, 2);
    assert_eq!(
        glyphs[1].dest_x_px + glyphs[1].atlas_entry.padding_left_px as i32,
        16
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn oversized_space_advance_does_not_expand_the_visual_gap_between_cells() -> Result<()> {
    let shaped_frame = ShapedTerminalFrame {
        seqno: 3,
        font: test_font(),
        rows: vec![ShapedRow {
            row: 0,
            content_hash: 0,
            row_hash: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..3,
                text: "a b".into(),
                clusters: vec![
                    RunCluster {
                        text: "a".into(),
                        cell_range: 0..1,
                        byte_range: 0..1,
                    },
                    RunCluster {
                        text: " ".into(),
                        cell_range: 1..2,
                        byte_range: 1..2,
                    },
                    RunCluster {
                        text: "b".into(),
                        cell_range: 2..3,
                        byte_range: 2..3,
                    },
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 0,
                        x_advance: 8,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 2,
                        cluster: 1,
                        x_advance: 32,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 3,
                        cluster: 2,
                        x_advance: 8,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style: style(),
                resolved_face: resolved_face(),
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = FixedGridFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    let glyphs = &prepared.monochrome_glyph_draws;
    assert_eq!(
        glyphs.len(),
        3,
        "space clusters should still preserve per-cell draw bookkeeping"
    );
    assert_eq!(
        glyphs[2].dest_x_px + glyphs[2].atlas_entry.padding_left_px as i32,
        16
    );

    Ok(())
}
