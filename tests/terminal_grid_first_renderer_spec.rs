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
#[derive(Default)]
struct RecordingFontSystem {
    requests: Vec<GlyphRasterRequest>,
}

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for RecordingFontSystem {
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
        self.requests.push(request);
        let (width_px, advance_px) = match request.glyph_id {
            1 => (7, 7),
            2 => (4, 4),
            _ => (4, 4),
        };
        Ok(RasterizedGlyph {
            width_px,
            height_px: 8,
            bearing_x_px: 0,
            bearing_y_px: 0,
            visible_left_px: 0,
            visible_top_px: 0,
            visible_width_px: width_px,
            visible_height_px: 8,
            advance_px,
            coverage: vec![255; (width_px * 8) as usize],
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn grid_test_font() -> LoadedFont {
    LoadedFont::new(
        FontFaceKey(1),
        FontRequest {
            family_name: Some("Grid Test".into()),
            px_size: 8.0,
        },
        FontMetrics {
            units_per_em: 10,
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
#[test]
fn native_renderer_snaps_ascii_clusters_to_cell_origins_instead_of_shaped_pen_drift() -> Result<()>
{
    let shaped_frame = ShapedTerminalFrame {
        seqno: 1,
        font: grid_test_font(),
        rows: vec![ShapedRow {
            row: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..3,
                text: "abc".into(),
                clusters: vec![
                    RunCluster {
                        text: "a".into(),
                        cell_range: 0..1,
                        byte_range: 0..1,
                    },
                    RunCluster {
                        text: "b".into(),
                        cell_range: 1..2,
                        byte_range: 1..2,
                    },
                    RunCluster {
                        text: "c".into(),
                        cell_range: 2..3,
                        byte_range: 2..3,
                    },
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 0,
                        x_advance: 9,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 1,
                        x_advance: 9,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 2,
                        x_advance: 9,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style: style(),
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Grid Test".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = RecordingFontSystem::default();

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;

    assert_eq!(prepared.monochrome_glyph_draws.len(), 3);
    assert_eq!(
        prepared.monochrome_glyph_draws[0].dest_x_px
            + prepared.monochrome_glyph_draws[0].atlas_entry.padding_left_px as i32,
        0,
        "the first glyph should start at the first cell origin"
    );
    assert_eq!(
        prepared.monochrome_glyph_draws[1].dest_x_px
            + prepared.monochrome_glyph_draws[1].atlas_entry.padding_left_px as i32,
        8,
        "the second glyph should snap to column 1 instead of inheriting the previous glyph's 7.2px shaped advance"
    );
    assert_eq!(
        prepared.monochrome_glyph_draws[2].dest_x_px
            + prepared.monochrome_glyph_draws[2].atlas_entry.padding_left_px as i32,
        16,
        "the third glyph should stay on column 2 so cursor and selection geometry do not drift as the line grows"
    );
    assert_eq!(
        fonts.requests.iter().map(|request| request.fractional_offset_x()).collect::<Vec<_>>(),
        vec![0.0, 0.0, 0.0],
        "single-cell terminal clusters should reset their raster phase at each cell boundary instead of carrying subpixel pen drift across the row"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_advances_following_clusters_by_logical_span_after_a_wide_cell() -> Result<()> {
    let shaped_frame = ShapedTerminalFrame {
        seqno: 2,
        font: grid_test_font(),
        rows: vec![ShapedRow {
            row: 0,
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
                        glyph_id: 2,
                        cluster: 0,
                        x_advance: 10,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 3,
                        x_advance: 9,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style: style(),
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Grid Test".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = RecordingFontSystem::default();

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;

    assert_eq!(prepared.monochrome_glyph_draws.len(), 2);
    assert_eq!(
        prepared.monochrome_glyph_draws[0].start_col,
        0,
        "the wide glyph should still own the first logical column in its span"
    );
    assert_eq!(
        prepared.monochrome_glyph_draws[0].end_col,
        1,
        "the wide glyph should still report a two-cell logical ownership span"
    );
    assert_eq!(
        prepared.monochrome_glyph_draws[1].dest_x_px
            + prepared.monochrome_glyph_draws[1].atlas_entry.padding_left_px as i32,
        16,
        "the cluster after a wide glyph should start at the next logical cell after the full span instead of sliding left into the wide cell's reserved space"
    );

    Ok(())
}
