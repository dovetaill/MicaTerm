//! Segmented terminal text shaping driven by the active font backend.

use anyhow::Result;

use crate::app::terminal_font::{
    FontFallbackFace, FontSystem, LoadedFont, OpenTypeFeatureSet, ShapedGlyph,
    TextShapingRequest,
};
use crate::app::terminal_layout::run_segmentation::{SegmentedRun, TextStyleKey, segment_row};
use crate::app::terminal_model::TerminalModelRow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionedGlyph {
    pub glyph_id: u32,
    pub cluster: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphRun {
    pub row: u32,
    pub cell_range: std::ops::Range<u32>,
    pub text: String,
    pub glyphs: Vec<PositionedGlyph>,
    pub style: TextStyleKey,
    pub resolved_face: FontFallbackFace,
    pub feature_set: OpenTypeFeatureSet,
    pub allow_ligatures: bool,
    pub has_color_glyphs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapedRow {
    pub row: u32,
    pub runs: Vec<GlyphRun>,
}

pub trait TextShaper {
    fn shape_row(
        &mut self,
        row: &TerminalModelRow,
        font: &LoadedFont,
        fonts: &mut dyn FontSystem,
    ) -> Result<ShapedRow>;
}

#[derive(Default)]
pub struct TerminalTextShaper;

impl TextShaper for TerminalTextShaper {
    fn shape_row(
        &mut self,
        row: &TerminalModelRow,
        font: &LoadedFont,
        fonts: &mut dyn FontSystem,
    ) -> Result<ShapedRow> {
        let runs = segment_row(row)
            .into_iter()
            .map(|run| self.shape_segment(run, font, fonts))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        Ok(ShapedRow {
            row: row.row_index,
            runs,
        })
    }
}

impl TerminalTextShaper {
    fn shape_segment(
        &mut self,
        run: SegmentedRun,
        font: &LoadedFont,
        fonts: &mut dyn FontSystem,
    ) -> Result<Vec<GlyphRun>> {
        let cell_range = run.cell_range.clone();
        let row = run.row;
        let style = run.style;
        let shaping_request = TextShapingRequest::new(run.text);
        // 真实 shaping 仍然由字体后端统一负责，`fonts.shape_text(...)` 的底层契约没有回流到 layout 层。
        let shaped_runs = fonts.shape_text_runs(font, &shaping_request)?;

        Ok(shaped_runs
            .into_iter()
            .map(|shaped_run| GlyphRun {
                row,
                cell_range: cell_range.clone(),
                text: shaped_run.text,
                glyphs: shaped_run
                    .glyphs
                    .into_iter()
                    .map(PositionedGlyph::from)
                    .collect(),
                style,
                resolved_face: shaped_run.resolved_face,
                feature_set: shaped_run.feature_set,
                allow_ligatures: shaped_run.allow_ligatures,
                has_color_glyphs: shaped_run.has_color_glyphs,
            })
            .collect())
    }
}

impl From<ShapedGlyph> for PositionedGlyph {
    fn from(glyph: ShapedGlyph) -> Self {
        Self {
            glyph_id: glyph.glyph_id,
            cluster: glyph.cluster,
            x_advance: glyph.x_advance,
            y_advance: glyph.y_advance,
            x_offset: glyph.x_offset,
            y_offset: glyph.y_offset,
        }
    }
}

pub fn shape_row(
    row: &TerminalModelRow,
    font: &LoadedFont,
    fonts: &mut dyn FontSystem,
) -> Result<ShapedRow> {
    TerminalTextShaper.shape_row(row, font, fonts)
}
