//! Segmented terminal text shaping driven by the active font backend.

use anyhow::Result;

use crate::app::terminal_font::{FontSystem, LoadedFont, ShapedGlyph};
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
            .collect::<Result<Vec<_>>>()?;

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
    ) -> Result<GlyphRun> {
        let glyphs = fonts.shape_text(font, run.text.as_str())?;

        Ok(GlyphRun {
            row: run.row,
            cell_range: run.cell_range,
            text: run.text,
            glyphs: glyphs.into_iter().map(PositionedGlyph::from).collect(),
            style: run.style,
        })
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
