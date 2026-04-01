//! GPU-preparation stage for the staged native terminal renderer.

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::app::terminal_font::{FontSystem, LoadedFont};
use crate::app::terminal_layout::ShapedRow;
use crate::app::terminal_renderer::atlas::{
    ColorGlyphCacheEntry, ColorGlyphCacheKey, GlyphAtlas,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedTerminalFrame {
    pub seqno: u64,
    pub font: LoadedFont,
    pub rows: Vec<ShapedRow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedNativeRendererStats {
    pub glyph_cache_entries: usize,
    pub mono_glyph_cache_entries: usize,
    pub color_glyph_cache_entries: usize,
    pub monochrome_glyphs_prepared: usize,
    pub color_glyphs_prepared: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparedUnderlineOverlay {
    pub visible: bool,
    pub run_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedNativeFrame {
    pub frame_token: u64,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
    pub glyph_cache_entries: usize,
    pub mono_glyph_cache_entries: usize,
    pub color_glyph_cache_entries: usize,
    pub monochrome_glyphs_prepared: usize,
    pub color_glyphs_prepared: usize,
    pub shaped_row_count: usize,
    pub glyph_run_count: usize,
    pub glyph_count: usize,
    pub underline_run_count: usize,
    pub underline_overlay: PreparedUnderlineOverlay,
    pub renderer_stats: PreparedNativeRendererStats,
}

#[derive(Default)]
pub struct WgpuTerminalRenderer {
    atlas: GlyphAtlas,
    color_glyph_cache: HashMap<ColorGlyphCacheKey, ColorGlyphCacheEntry>,
    last_frame_fingerprint: Option<u64>,
    next_frame_token: u64,
    next_color_slot: u32,
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
        fonts: &mut dyn FontSystem,
    ) -> Result<PreparedNativeFrame> {
        let mut monochrome_glyphs_prepared = 0usize;
        let mut color_glyphs_prepared = 0usize;
        let shaped_row_count = frame.rows.len();
        let glyph_run_count = frame.rows.iter().map(|row| row.runs.len()).sum::<usize>();
        let glyph_count = frame
            .rows
            .iter()
            .flat_map(|row| row.runs.iter())
            .map(|run| run.glyphs.len())
            .sum::<usize>();
        let underline_run_count = frame
            .rows
            .iter()
            .flat_map(|row| row.runs.iter())
            .filter(|run| run.style.underline)
            .count();
        let underline_overlay = PreparedUnderlineOverlay {
            visible: underline_run_count > 0,
            run_count: underline_run_count,
        };

        for row in &frame.rows {
            for run in &row.runs {
                for glyph in &run.glyphs {
                    if run.has_color_glyphs {
                        match fonts.rasterize_color_glyph(&frame.font, glyph.glyph_id)? {
                            Some(rasterized) => {
                                self.upsert_color_glyph(
                                    ColorGlyphCacheKey::new(frame.font.cache_key(), glyph.glyph_id),
                                    rasterized,
                                );
                                color_glyphs_prepared = color_glyphs_prepared.saturating_add(1);
                            }
                            None => {
                                let request = frame.font.raster_request(glyph.glyph_id, run.style.bold);
                                let rasterized = fonts
                                    .rasterize_glyph(&frame.font, glyph.glyph_id, run.style.bold)?;
                                self.atlas.upsert(request, &rasterized);
                                monochrome_glyphs_prepared = monochrome_glyphs_prepared.saturating_add(1);
                            }
                        }
                    } else {
                        let request = frame.font.raster_request(glyph.glyph_id, run.style.bold);
                        let rasterized =
                            fonts.rasterize_glyph(&frame.font, glyph.glyph_id, run.style.bold)?;
                        self.atlas.upsert(request, &rasterized);
                        monochrome_glyphs_prepared = monochrome_glyphs_prepared.saturating_add(1);
                    }
                }
            }
        }

        let fingerprint = hash_shaped_frame(frame);
        if self.last_frame_fingerprint != Some(fingerprint) {
            self.next_frame_token = self.next_frame_token.saturating_add(1);
            self.last_frame_fingerprint = Some(fingerprint);
        }
        let (cell_width_px, cell_height_px) = frame.font.cell_size_px();
        let mono_glyph_cache_entries = self.atlas.entry_count();
        let color_glyph_cache_entries = self.color_glyph_cache.len();
        let renderer_stats = PreparedNativeRendererStats {
            glyph_cache_entries: mono_glyph_cache_entries + color_glyph_cache_entries,
            mono_glyph_cache_entries,
            color_glyph_cache_entries,
            monochrome_glyphs_prepared,
            color_glyphs_prepared,
        };

        Ok(PreparedNativeFrame {
            frame_token: self.next_frame_token,
            cell_width_px,
            cell_height_px,
            glyph_cache_entries: renderer_stats.glyph_cache_entries,
            mono_glyph_cache_entries: renderer_stats.mono_glyph_cache_entries,
            color_glyph_cache_entries: renderer_stats.color_glyph_cache_entries,
            monochrome_glyphs_prepared: renderer_stats.monochrome_glyphs_prepared,
            color_glyphs_prepared: renderer_stats.color_glyphs_prepared,
            shaped_row_count,
            glyph_run_count,
            glyph_count,
            underline_run_count,
            underline_overlay,
            renderer_stats,
        })
    }

    fn upsert_color_glyph(
        &mut self,
        key: ColorGlyphCacheKey,
        rasterized: crate::app::terminal_font::ColorGlyphRaster,
    ) -> ColorGlyphCacheEntry {
        if let Some(entry) = self.color_glyph_cache.get(&key) {
            return *entry;
        }

        let entry = ColorGlyphCacheEntry {
            slot: self.next_color_slot,
            width_px: rasterized.width_px,
            height_px: rasterized.height_px,
            rgba_bytes: rasterized.rgba.len(),
        };
        self.next_color_slot = self.next_color_slot.saturating_add(1);
        self.color_glyph_cache.insert(key, entry);
        entry
    }
}

fn hash_shaped_frame(frame: &ShapedTerminalFrame) -> u64 {
    let mut hasher = DefaultHasher::new();
    let metrics = frame.font.metrics();
    frame.seqno.hash(&mut hasher);
    frame.font.cache_key().hash(&mut hasher);
    metrics.units_per_em.hash(&mut hasher);
    metrics.cell_width_px.to_bits().hash(&mut hasher);
    metrics.cell_height_px.to_bits().hash(&mut hasher);

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
