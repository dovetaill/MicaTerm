#[cfg(feature = "terminal-native-renderer")]
use anyhow::{Result, bail};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_font::{
    FontFaceKey, FontMetrics, FontRenderProfile, FontRequest, FontSystem, FontFallbackFace,
    GlyphRasterRequest, LoadedFont, RasterizedGlyph,
};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::{GlyphRun, PositionedGlyph, ShapedRow, TextStyleKey};
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_layout::run_segmentation::RunCluster;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};

#[cfg(feature = "terminal-native-renderer")]
struct OverhangGlyphFontSystem;

#[cfg(feature = "terminal-native-renderer")]
struct ClusterOverhangFontSystem;

#[cfg(feature = "terminal-native-renderer")]
struct VerticalOverhangFontSystem;

#[cfg(feature = "terminal-native-renderer")]
#[derive(Default)]
struct FractionalPhaseFontSystem {
    requests: Vec<GlyphRasterRequest>,
}

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for OverhangGlyphFontSystem {
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
            advance_px: 3,
            coverage: vec![255; 8],
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for ClusterOverhangFontSystem {
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
            width_px: 2,
            height_px: 2,
            bearing_x_px: 1,
            bearing_y_px: 0,
            advance_px: 2,
            coverage: vec![255; 4],
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for VerticalOverhangFontSystem {
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
        let glyph_id = request.glyph_id;
        let height_px = if glyph_id == 1 { 6 } else { 2 };
        Ok(RasterizedGlyph {
            width_px: 2,
            height_px,
            bearing_x_px: 0,
            bearing_y_px: -2,
            advance_px: 4,
            coverage: vec![255; (2 * height_px) as usize],
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for FractionalPhaseFontSystem {
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
        Ok(RasterizedGlyph {
            width_px: 2,
            height_px: 2,
            bearing_x_px: 0,
            bearing_y_px: 0,
            advance_px: 2,
            coverage: vec![255; 4],
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn test_loaded_font() -> LoadedFont {
    LoadedFont::new(
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
    )
}

#[cfg(feature = "terminal-native-renderer")]
fn fractional_phase_test_font() -> LoadedFont {
    LoadedFont::new(
        FontFaceKey(1),
        FontRequest {
            family_name: Some("Fractional Terminal".into()),
            px_size: 4.0,
        },
        FontMetrics {
            units_per_em: 8,
            ascent_px: 3.0,
            descent_px: -1.0,
            line_gap_px: 0.0,
            baseline_px: 3.0,
            cell_width_px: 4.0,
            cell_height_px: 4.0,
        },
        FontRenderProfile::default(),
    )
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_snaps_single_glyph_origin_back_inside_its_cell_span() -> Result<()> {
    let style = TextStyleKey {
        fg_rgba: 0xffd8_dfe8,
        bg_rgba: 0xff0c_1014,
        bold: false,
        underline: false,
    };
    let shaped_frame = ShapedTerminalFrame {
        seqno: 1,
        font: test_loaded_font(),
        rows: vec![ShapedRow {
            row: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..1,
                text: "a".into(),
                clusters: vec![RunCluster {
                    text: "a".into(),
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
    let mut fonts = OverhangGlyphFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    let draw = prepared
        .monochrome_glyph_draws
        .first()
        .expect("monochrome draw");

    assert_eq!(
        draw.dest_x_px, 0,
        "renderer should pull a right-overhanging glyph back inside the cell before the frame reaches native or scene-image presentation"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_shifts_every_glyph_in_a_cluster_when_trailing_ink_overhangs() -> Result<()> {
    let style = TextStyleKey {
        fg_rgba: 0xffd8_dfe8,
        bg_rgba: 0xff0c_1014,
        bold: false,
        underline: false,
    };
    let shaped_frame = ShapedTerminalFrame {
        seqno: 1,
        font: test_loaded_font(),
        rows: vec![ShapedRow {
            row: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..1,
                text: "a".into(),
                clusters: vec![RunCluster {
                    text: "a".into(),
                    cell_range: 0..1,
                    byte_range: 0..1,
                }],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 0,
                        x_advance: 2,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 2,
                        cluster: 0,
                        x_advance: 2,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
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
    let mut fonts = ClusterOverhangFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    assert_eq!(prepared.monochrome_glyph_draws.len(), 2, "cluster should still emit two glyph draws");

    assert_eq!(
        prepared.monochrome_glyph_draws[0].dest_x_px, 0,
        "when the trailing glyph overhangs the right edge, the renderer should shift the entire cluster left instead of only clamping the last glyph"
    );
    assert_eq!(
        prepared.monochrome_glyph_draws[1].dest_x_px, 2,
        "shared cluster offset should preserve intra-cluster spacing after the shift"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_keeps_shared_row_baseline_when_a_glyph_overhangs_vertically() -> Result<()> {
    let style = TextStyleKey {
        fg_rgba: 0xffd8_dfe8,
        bg_rgba: 0xff0c_1014,
        bold: false,
        underline: false,
    };
    let shaped_frame = ShapedTerminalFrame {
        seqno: 1,
        font: test_loaded_font(),
        rows: vec![ShapedRow {
            row: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..2,
                text: "ab".into(),
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
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 0,
                        x_advance: 4,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 2,
                        cluster: 1,
                        x_advance: 4,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
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
    let mut fonts = VerticalOverhangFontSystem;

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    assert_eq!(
        prepared.monochrome_glyph_draws.len(),
        2,
        "two monochrome glyphs should produce two prepared draws for baseline inspection"
    );

    assert_eq!(
        prepared.monochrome_glyph_draws[0].dest_y_px, 1,
        "renderer should preserve the tall glyph's baseline-derived y origin and rely on later clip rects for overflow instead of pulling the glyph upward into the cell"
    );
    assert_eq!(
        prepared.monochrome_glyph_draws[1].dest_y_px, 1,
        "renderer should keep both glyphs on the same row baseline even when only one glyph overhangs the cell vertically"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_preserves_fractional_x_phase_for_hinted_monochrome_glyphs() -> Result<()> {
    let style = TextStyleKey {
        fg_rgba: 0xffd8_dfe8,
        bg_rgba: 0xff0c_1014,
        bold: false,
        underline: false,
    };
    let shaped_frame = ShapedTerminalFrame {
        seqno: 2,
        font: fractional_phase_test_font(),
        rows: vec![ShapedRow {
            row: 0,
            runs: vec![GlyphRun {
                row: 0,
                cell_range: 0..2,
                text: "ab".into(),
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
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 1,
                        cluster: 0,
                        x_advance: 3,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 2,
                        cluster: 1,
                        x_advance: 3,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style,
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Fractional Terminal".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = FractionalPhaseFontSystem::default();

    let _prepared = renderer.prepare(&shaped_frame, &mut fonts)?;

    assert_eq!(
        fonts.requests.len(),
        2,
        "renderer should rasterize both monochrome glyphs through the font backend"
    );
    assert!(
        fonts.requests[0].fractional_offset_x().abs() < 0.001,
        "the first glyph in the row should stay on the integer phase"
    );
    assert!(
        (fonts.requests[1].fractional_offset_x() - 0.5).abs() < 0.001,
        "renderer should preserve the second glyph's 0.5px fractional phase instead of snapping every hinted glyph raster request to 0.0"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn native_renderer_partitions_monochrome_glyph_cache_by_fractional_x_phase() -> Result<()> {
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
                family_name: Some("Fractional Test".into()),
                px_size: 4.0,
            },
            FontMetrics {
                units_per_em: 10,
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
                cell_range: 0..2,
                text: "aa".into(),
                clusters: vec![
                    RunCluster {
                        text: "a".into(),
                        cell_range: 0..1,
                        byte_range: 0..1,
                    },
                    RunCluster {
                        text: "a".into(),
                        cell_range: 1..2,
                        byte_range: 1..2,
                    },
                ],
                glyphs: vec![
                    PositionedGlyph {
                        glyph_id: 7,
                        cluster: 0,
                        x_advance: 11,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                    PositionedGlyph {
                        glyph_id: 7,
                        cluster: 1,
                        x_advance: 11,
                        y_advance: 0,
                        x_offset: 0,
                        y_offset: 0,
                    },
                ],
                style,
                resolved_face: FontFallbackFace {
                    face_key: FontFaceKey(1),
                    family_name: "Fractional Test".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    };
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = FractionalPhaseFontSystem::default();

    let prepared = renderer.prepare(&shaped_frame, &mut fonts)?;
    assert_eq!(
        prepared.monochrome_glyph_draws.len(),
        2,
        "two glyphs should yield two prepared monochrome draws for cache partition inspection"
    );
    assert_eq!(
        fonts.requests.len(),
        2,
        "renderer should rasterize both glyphs through explicit raster requests so the font backend can observe their fractional x phase"
    );
    assert_eq!(
        fonts.requests[0].fractional_offset_x(),
        0.0,
        "the first glyph should stay on a whole-pixel phase at the row origin"
    );
    assert!(
        (fonts.requests[1].fractional_offset_x() - 0.4).abs() < 0.001,
        "the second glyph should keep the 0.4px subpixel phase produced by the shaped advance instead of collapsing to a whole-pixel origin"
    );
    assert_ne!(
        prepared.monochrome_glyph_draws[0].atlas_entry.slot,
        prepared.monochrome_glyph_draws[1].atlas_entry.slot,
        "glyph atlas entries should stay distinct when the same glyph is rasterized at different fractional x phases"
    );

    Ok(())
}
