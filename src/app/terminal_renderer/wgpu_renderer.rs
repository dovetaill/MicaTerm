//! GPU-preparation stage for the staged native terminal renderer.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::app::terminal_font::{
    DirectWriteFontSystem, FontFaceKey, FontMetrics, GlyphRasterRequest,
};
use crate::app::terminal_layout::ShapedRow;
use crate::app::terminal_renderer::atlas::GlyphAtlas;

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedTerminalFrame {
    pub seqno: u64,
    pub face: FontFaceKey,
    pub px_size: f32,
    pub metrics: FontMetrics,
    pub rows: Vec<ShapedRow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedNativeFrame {
    pub frame_token: u64,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
    pub glyph_cache_entries: usize,
}

#[derive(Default)]
pub struct WgpuTerminalRenderer {
    atlas: GlyphAtlas,
    last_frame_fingerprint: Option<u64>,
    next_frame_token: u64,
}

impl WgpuTerminalRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_for_test() -> Result<Self> {
        Ok(Self::new())
    }

    pub fn prepare(
        &mut self,
        frame: &ShapedTerminalFrame,
        fonts: &mut DirectWriteFontSystem,
    ) -> Result<PreparedNativeFrame> {
        for row in &frame.rows {
            for run in &row.runs {
                for glyph in &run.glyphs {
                    let request = GlyphRasterRequest {
                        face: frame.face,
                        glyph_id: glyph.glyph_id,
                        px_size: frame.px_size,
                        bold: run.style.bold,
                    };
                    let rasterized = fonts.rasterize(request)?;
                    self.atlas.upsert(request, &rasterized);
                }
            }
        }

        let fingerprint = hash_shaped_frame(frame);
        if self.last_frame_fingerprint != Some(fingerprint) {
            self.next_frame_token = self.next_frame_token.saturating_add(1);
            self.last_frame_fingerprint = Some(fingerprint);
        }

        Ok(PreparedNativeFrame {
            frame_token: self.next_frame_token,
            cell_width_px: frame.metrics.cell_width_px.ceil() as u32,
            cell_height_px: frame.metrics.cell_height_px.ceil() as u32,
            glyph_cache_entries: self.atlas.entry_count(),
        })
    }
}

fn hash_shaped_frame(frame: &ShapedTerminalFrame) -> u64 {
    let mut hasher = DefaultHasher::new();
    frame.seqno.hash(&mut hasher);
    frame.face.hash(&mut hasher);
    frame.px_size.to_bits().hash(&mut hasher);
    frame.metrics.units_per_em.hash(&mut hasher);
    frame.metrics.cell_width_px.to_bits().hash(&mut hasher);
    frame.metrics.cell_height_px.to_bits().hash(&mut hasher);

    for row in &frame.rows {
        row.row.hash(&mut hasher);
        for run in &row.runs {
            run.row.hash(&mut hasher);
            run.cell_range.start.hash(&mut hasher);
            run.cell_range.end.hash(&mut hasher);
            run.text.hash(&mut hasher);
            run.style.fg_rgba.hash(&mut hasher);
            run.style.bold.hash(&mut hasher);
            run.style.underline.hash(&mut hasher);
            for glyph in &run.glyphs {
                glyph.glyph_id.hash(&mut hasher);
                glyph.cluster.hash(&mut hasher);
                glyph.x_advance.hash(&mut hasher);
                glyph.y_advance.hash(&mut hasher);
                glyph.x_offset.hash(&mut hasher);
                glyph.y_offset.hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}
