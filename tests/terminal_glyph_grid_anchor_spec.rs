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
use mica_term::app::terminal_renderer::wgpu_renderer::{
    PreparedMonochromeGlyphDraw, PreparedMonochromeGlyphSourceKind,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};

#[cfg(feature = "terminal-native-renderer")]
struct GridAnchorFontSystem;

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for GridAnchorFontSystem {
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
            11 => (6, 7),
            22 => (14, 15),
            33 => (6, 7),
            _ => (6, 7),
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
            family_name: Some("Grid Terminal".into()),
            px_size: 10.0,
        },
        FontMetrics {
            units_per_em: 10,
            ascent_px: 8.0,
            descent_px: -2.0,
            line_gap_px: 0.0,
            baseline_px: 8.0,
            cell_width_px: 8.0,
            cell_height_px: 10.0,
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
fn ascii_frame(text: &str, x_advance: i32) -> ShapedTerminalFrame {
    let clusters = text
        .char_indices()
        .enumerate()
        .map(|(col, (byte_start, ch))| RunCluster {
            text: ch.to_string(),
            cell_range: col as u32..col as u32 + 1,
            byte_range: byte_start..byte_start + ch.len_utf8(),
        })
        .collect::<Vec<_>>();
    let glyphs = text
        .char_indices()
        .enumerate()
        .map(|(_col, (byte_start, _ch))| PositionedGlyph {
            glyph_id: 11,
            cluster: byte_start as u32,
            x_advance,
            y_advance: 0,
            x_offset: 0,
            y_offset: 0,
        })
        .collect::<Vec<_>>();

    ShapedTerminalFrame {
        seqno: 1,
        font: grid_test_font(),
        rows: vec![ShapedRow {
            row: 0,
            content_hash: 0,
            row_hash: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..text.chars().count() as u32,
                text: text.into(),
                clusters,
                glyphs,
                style: style(),
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Grid Terminal".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn visible_left(draw: &PreparedMonochromeGlyphDraw) -> i32 {
    draw.dest_x_px + draw.atlas_entry.padding_left_px as i32
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn ascii_row_anchors_each_cluster_to_declared_cell_start() -> Result<()> {
    let shaped_frame = ascii_frame("iiii", 7);
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = GridAnchorFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    println!("{:?}", prepared.monochrome_glyph_draws);
    let actual = prepared
        .monochrome_glyph_draws
        .iter()
        .map(visible_left)
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![0, 8, 16, 24],
        "single-cell ASCII clusters should be anchored from their declared terminal columns so long prompts keep glyphs, selection, and cursor geometry on the same fixed grid"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn wide_cjk_cluster_does_not_shift_following_ascii_origin_off_grid() -> Result<()> {
    let shaped_frame = ShapedTerminalFrame {
        seqno: 2,
        font: grid_test_font(),
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
                        glyph_id: 22,
                        cluster: 0,
                        x_advance: 15,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 33,
                        cluster: 3,
                        x_advance: 7,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style: style(),
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Grid Terminal".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = GridAnchorFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;

    assert_eq!(
        visible_left(&prepared.monochrome_glyph_draws[0]),
        0,
        "a double-width CJK glyph should stay anchored to the first cell in its span"
    );
    assert_eq!(
        visible_left(&prepared.monochrome_glyph_draws[1]),
        16,
        "the glyph after a width=2 cluster should start at the next terminal cell anchor instead of inheriting the previous glyph advance and leaving a phantom gap or overlap before the cursor"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn mixed_row_draw_origins_match_declared_start_cols_for_selection_grid() -> Result<()> {
    let shaped_frame = ShapedTerminalFrame {
        seqno: 3,
        font: grid_test_font(),
        rows: vec![ShapedRow {
            row: 0,
            content_hash: 0,
            row_hash: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..4,
                text: "a条b".into(),
                clusters: vec![
                    RunCluster {
                        text: "a".into(),
                        cell_range: 0..1,
                        byte_range: 0..1,
                    },
                    RunCluster {
                        text: "条".into(),
                        cell_range: 1..3,
                        byte_range: 1..4,
                    },
                    RunCluster {
                        text: "b".into(),
                        cell_range: 3..4,
                        byte_range: 4..5,
                    },
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 11,
                        cluster: 0,
                        x_advance: 7,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 22,
                        cluster: 1,
                        x_advance: 15,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 33,
                        cluster: 4,
                        x_advance: 7,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style: style(),
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Grid Terminal".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = GridAnchorFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;

    for draw in &prepared.monochrome_glyph_draws {
        assert_eq!(
            visible_left(draw),
            draw.start_col as i32 * prepared.cell_width_px as i32,
            "prepared glyph draws should share the same horizontal cell grid as selection and hit-testing instead of drifting off the declared start column"
        );
    }

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn generated_grid_masks_anchor_to_the_cell_rect_instead_of_font_baseline_bearings() -> Result<()> {
    let shaped_frame = ShapedTerminalFrame {
        seqno: 4,
        font: grid_test_font(),
        rows: vec![ShapedRow {
            row: 1,
            content_hash: 0,
            row_hash: 0,
            runs: vec![GlyphRun {
                row: 1,
                cell_range: 0..2,
                text: "╭╮".into(),
                clusters: vec![
                    RunCluster {
                        text: "╭".into(),
                        cell_range: 0..1,
                        byte_range: 0..3,
                    },
                    RunCluster {
                        text: "╮".into(),
                        cell_range: 1..2,
                        byte_range: 3..6,
                    },
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 11,
                        cluster: 0,
                        x_advance: 7,
                        y_advance: 0,
                        x_offset: 2,
                        y_offset: -1,
                    },
                    PositionedGlyph {
                        glyph_id: 11,
                        cluster: 3,
                        x_advance: 7,
                        y_advance: 0,
                        x_offset: 2,
                        y_offset: -1,
                    },
                ],
                style: style(),
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Grid Terminal".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = GridAnchorFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;

    assert_eq!(
        prepared.monochrome_glyph_draws.len(),
        2,
        "generated box glyph routing should still emit one prepared draw per visible cell"
    );
    for draw in &prepared.monochrome_glyph_draws {
        assert_eq!(
            draw.source_kind,
            PreparedMonochromeGlyphSourceKind::GeneratedMask,
            "Task 5 should mark generated box glyphs explicitly so the native renderer can keep them off the DirectWrite body-text path"
        );
        assert_eq!(
            visible_left(draw),
            draw.start_col as i32 * prepared.cell_width_px as i32,
            "generated box glyphs should sit flush on the declared cell anchor instead of inheriting font bearing offsets"
        );
        assert_eq!(
            draw.dest_y_px,
            draw.row as i32 * prepared.cell_height_px as i32,
            "generated box glyphs should anchor vertically to the row's snapped cell rect instead of the body-text baseline"
        );
    }

    Ok(())
}
