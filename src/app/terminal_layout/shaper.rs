//! HarfBuzz-backed shaping for segmented terminal text runs.

use anyhow::Result;
use harfbuzz_rs::{Face, Font, GlyphBuffer, UnicodeBuffer, shape};

use crate::app::terminal_font::{FontRequest, FontSystem};
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
        fonts: &mut dyn FontSystem,
    ) -> Result<ShapedRow>;
}

#[derive(Default)]
pub struct HarfBuzzTextShaper {
    request: FontRequest,
}

impl TextShaper for HarfBuzzTextShaper {
    fn shape_row(
        &mut self,
        row: &TerminalModelRow,
        fonts: &mut dyn FontSystem,
    ) -> Result<ShapedRow> {
        let runs = segment_row(row)
            .into_iter()
            .map(|run| self.shape_segment(run, fonts))
            .collect::<Result<Vec<_>>>()?;

        Ok(ShapedRow {
            row: row.row_index,
            runs,
        })
    }
}

impl HarfBuzzTextShaper {
    fn shape_segment(&mut self, run: SegmentedRun, fonts: &mut dyn FontSystem) -> Result<GlyphRun> {
        let face_key = fonts.resolve_face(&self.request)?;
        let _metrics = fonts.metrics(face_key, self.request.px_size)?;
        let face = Face::from_bytes(fonts.face_bytes(face_key)?, fonts.face_index(face_key));
        let font = Font::new(face);
        let buffer = UnicodeBuffer::new()
            .add_str(run.text.as_str())
            .guess_segment_properties();
        let glyph_buffer = shape(&font, buffer, &[]);

        Ok(GlyphRun {
            row: run.row,
            cell_range: run.cell_range,
            text: run.text,
            glyphs: positioned_glyphs(&glyph_buffer),
            style: run.style,
        })
    }
}

fn positioned_glyphs(buffer: &GlyphBuffer) -> Vec<PositionedGlyph> {
    buffer
        .get_glyph_infos()
        .iter()
        .zip(buffer.get_glyph_positions())
        .map(|(info, position)| PositionedGlyph {
            glyph_id: info.codepoint,
            cluster: info.cluster,
            x_advance: position.x_advance,
            y_advance: position.y_advance,
            x_offset: position.x_offset,
            y_offset: position.y_offset,
        })
        .collect()
}

pub fn shape_row(row: &TerminalModelRow, fonts: &mut dyn FontSystem) -> Result<ShapedRow> {
    HarfBuzzTextShaper::default().shape_row(row, fonts)
}
