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
use mica_term::app::terminal_renderer::wgpu_renderer::PreparedMonochromeGlyphVisualFit;
#[cfg(feature = "terminal-native-renderer")]
use mica_term::app::terminal_renderer::{ShapedTerminalFrame, WgpuTerminalRenderer};

#[cfg(feature = "terminal-native-renderer")]
struct GlyphFitFontSystem;

#[cfg(feature = "terminal-native-renderer")]
impl FontSystem for GlyphFitFontSystem {
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
            width_px: 6,
            height_px: 8,
            bearing_x_px: 0,
            bearing_y_px: 0,
            visible_left_px: 0,
            visible_top_px: 0,
            visible_width_px: 6,
            visible_height_px: 8,
            advance_px: 7,
            coverage: vec![255; 48],
        })
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn glyph_fit_font() -> LoadedFont {
    LoadedFont::new(
        FontFaceKey(1),
        FontRequest {
            family_name: Some("Glyph Fit Terminal".into()),
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
fn ascii_frame(text: &str) -> ShapedTerminalFrame {
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
            x_advance: 7,
            y_advance: 0,
            x_offset: 0,
            y_offset: 0,
        })
        .collect::<Vec<_>>();

    ShapedTerminalFrame {
        seqno: 1,
        font: glyph_fit_font(),
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
                    family_name: "Glyph Fit Terminal".into(),
                },
                feature_set: Default::default(),
                allow_ligatures: true,
                has_color_glyphs: false,
            }],
        }],
    }
}

#[cfg(feature = "terminal-native-renderer")]
fn visual_fit_at_col(frame: &ShapedTerminalFrame, col: u32) -> Result<PreparedMonochromeGlyphVisualFit> {
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let mut fonts = GlyphFitFontSystem;
    let prepared = renderer.prepare(frame, &mut fonts)?;

    Ok(prepared
        .monochrome_glyph_draws
        .iter()
        .find(|draw| draw.start_col == col)
        .expect("draw at requested col")
        .visual_fit)
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn repeated_dash_streaks_use_grid_symbol_visual_fit() -> Result<()> {
    let frame = ascii_frame("-----");

    for col in 0..5 {
        assert_eq!(
            visual_fit_at_col(&frame, col)?,
            PreparedMonochromeGlyphVisualFit::GridSymbol,
            "repeated terminal dash streaks should use the grid-symbol visual-fit path so permission strings and separators stop looking like a single over-smoothed body-text line"
        );
    }

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn isolated_word_hyphens_stay_on_the_body_text_path() -> Result<()> {
    let long_term = ascii_frame("long-term");
    let co_op = ascii_frame("co-op");

    assert_eq!(
        visual_fit_at_col(&long_term, 4)?,
        PreparedMonochromeGlyphVisualFit::BodyText,
        "an isolated hyphen inside a normal word should stay on the body-text path"
    );
    assert_eq!(
        visual_fit_at_col(&co_op, 2)?,
        PreparedMonochromeGlyphVisualFit::BodyText,
        "short body-text compounds like co-op should not be swept into the terminal dash-streak classifier"
    );

    Ok(())
}

#[cfg(feature = "terminal-native-renderer")]
#[test]
fn permission_suffix_only_marks_the_repeated_dash_tail_as_grid_symbols() -> Result<()> {
    let frame = ascii_frame("drwx-----");

    for col in 0..4 {
        assert_eq!(
            visual_fit_at_col(&frame, col)?,
            PreparedMonochromeGlyphVisualFit::BodyText,
            "body-text permission prefixes should keep their existing visual-fit classification"
        );
    }
    for col in 4..9 {
        assert_eq!(
            visual_fit_at_col(&frame, col)?,
            PreparedMonochromeGlyphVisualFit::GridSymbol,
            "only the repeated dash tail should switch onto the grid-symbol path"
        );
    }

    Ok(())
}
