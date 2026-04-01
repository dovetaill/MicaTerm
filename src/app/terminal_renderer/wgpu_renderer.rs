//! GPU-preparation stage for the staged native terminal renderer.

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::app::terminal_font::{FontSystem, LoadedFont};
use crate::app::terminal_layout::ShapedRow;
use crate::app::terminal_renderer::atlas::{
    ColorGlyphCacheEntry, ColorGlyphCacheKey, GlyphAtlas, GlyphAtlasEntry,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedTerminalFrame {
    pub seqno: u64,
    pub font: LoadedFont,
    pub rows: Vec<ShapedRow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedBackgroundRun {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub bg_rgba: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedMonochromeGlyphUploadPayload {
    pub width_px: u32,
    pub height_px: u32,
    pub bearing_x_px: i32,
    pub bearing_y_px: i32,
    pub advance_px: i32,
    pub coverage: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedMonochromeGlyphDraw {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub glyph_id: u32,
    pub atlas_entry: GlyphAtlasEntry,
    pub upload: Option<PreparedMonochromeGlyphUploadPayload>,
    pub x_offset_px: i32,
    pub y_offset_px: i32,
    pub dest_x_px: i32,
    pub dest_y_px: i32,
    pub fg_rgba: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedColorGlyphUploadPayload {
    pub width_px: u32,
    pub height_px: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedColorGlyphDraw {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub glyph_id: u32,
    pub cache_entry: ColorGlyphCacheEntry,
    pub upload: Option<PreparedColorGlyphUploadPayload>,
    pub x_offset_px: i32,
    pub y_offset_px: i32,
    pub dest_x_px: i32,
    pub dest_y_px: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedUnderlineRun {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub fg_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedNativeRendererStats {
    pub glyph_cache_entries: usize,
    pub mono_glyph_cache_entries: usize,
    pub color_glyph_cache_entries: usize,
    pub monochrome_glyphs_prepared: usize,
    pub color_glyphs_prepared: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparedUnderlineOverlay {
    pub visible: bool,
    pub run_count: usize,
    pub runs: Vec<PreparedUnderlineRun>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
    pub background_runs: Vec<PreparedBackgroundRun>,
    pub monochrome_glyph_draws: Vec<PreparedMonochromeGlyphDraw>,
    pub color_glyph_draws: Vec<PreparedColorGlyphDraw>,
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
        let mut background_runs = Vec::new();
        let mut monochrome_glyph_draws = Vec::new();
        let mut color_glyph_draws = Vec::new();
        let mut underline_runs = Vec::new();
        let shaped_row_count = frame.rows.len();
        let glyph_run_count = frame.rows.iter().map(|row| row.runs.len()).sum::<usize>();
        let glyph_count = frame
            .rows
            .iter()
            .flat_map(|row| row.runs.iter())
            .map(|run| run.glyphs.len())
            .sum::<usize>();
        let (cell_width_px, cell_height_px) = frame.font.cell_size_px();
        let baseline_px = frame.font.metrics().ascent_px.round() as i32;

        for row in &frame.rows {
            let row_top_px = (row.row as i32).saturating_mul(cell_height_px as i32);
            for run in &row.runs {
                let mut pen_x_px = (run.start_col() as i32).saturating_mul(cell_width_px as i32);
                background_runs.push(PreparedBackgroundRun {
                    row: row.row,
                    start_col: run.start_col(),
                    end_col: run.end_col(),
                    bg_rgba: run.style.bg_rgba,
                });

                if run.style.underline {
                    underline_runs.push(PreparedUnderlineRun {
                        row: row.row,
                        start_col: run.start_col(),
                        end_col: run.end_col(),
                        fg_rgba: run.style.fg_rgba,
                    });
                }

                for glyph in &run.glyphs {
                    if run.has_color_glyphs {
                        match fonts.rasterize_color_glyph(&frame.font, glyph.glyph_id)? {
                            Some(rasterized) => {
                                let (cache_entry, upload) = self.upsert_color_glyph(
                                    ColorGlyphCacheKey::new(frame.font.cache_key(), glyph.glyph_id),
                                    &rasterized,
                                );
                                let x_offset_px =
                                    font_design_units_to_px(&frame.font, glyph.x_offset);
                                let y_offset_px =
                                    font_design_units_to_px(&frame.font, glyph.y_offset);
                                let dest_x_px = pen_x_px.saturating_add(x_offset_px);
                                let dest_y_px = row_top_px
                                    .saturating_add(center_color_glyph_in_cell(
                                        cell_height_px,
                                        rasterized.height_px,
                                    ))
                                    .saturating_add(y_offset_px);
                                color_glyph_draws.push(PreparedColorGlyphDraw {
                                    row: row.row,
                                    start_col: run.start_col(),
                                    end_col: run.end_col(),
                                    glyph_id: glyph.glyph_id,
                                    cache_entry,
                                    upload,
                                    x_offset_px,
                                    y_offset_px,
                                    dest_x_px,
                                    dest_y_px,
                                });
                                pen_x_px = pen_x_px.saturating_add(color_glyph_advance_px(
                                    &frame.font,
                                    glyph.x_advance,
                                    cell_width_px,
                                    rasterized.width_px,
                                ));
                                color_glyphs_prepared = color_glyphs_prepared.saturating_add(1);
                            }
                            None => {
                                let request =
                                    frame.font.raster_request(glyph.glyph_id, run.style.bold);
                                let rasterized = fonts.rasterize_glyph(
                                    &frame.font,
                                    glyph.glyph_id,
                                    run.style.bold,
                                )?;
                                let upload = (!self.atlas.contains(request)).then(|| {
                                    PreparedMonochromeGlyphUploadPayload {
                                        width_px: rasterized.width_px,
                                        height_px: rasterized.height_px,
                                        bearing_x_px: rasterized.bearing_x_px,
                                        bearing_y_px: rasterized.bearing_y_px,
                                        advance_px: rasterized.advance_px,
                                        coverage: rasterized.coverage.clone(),
                                    }
                                });
                                let atlas_entry = self.atlas.upsert(request, &rasterized);
                                let x_offset_px =
                                    font_design_units_to_px(&frame.font, glyph.x_offset);
                                let y_offset_px =
                                    font_design_units_to_px(&frame.font, glyph.y_offset);
                                let dest_x_px = pen_x_px
                                    .saturating_add(x_offset_px)
                                    .saturating_add(rasterized.bearing_x_px);
                                let dest_y_px = row_top_px
                                    .saturating_add(baseline_px)
                                    .saturating_add(y_offset_px)
                                    .saturating_add(rasterized.bearing_y_px);
                                monochrome_glyph_draws.push(PreparedMonochromeGlyphDraw {
                                    row: row.row,
                                    start_col: run.start_col(),
                                    end_col: run.end_col(),
                                    glyph_id: glyph.glyph_id,
                                    atlas_entry,
                                    upload,
                                    x_offset_px,
                                    y_offset_px,
                                    dest_x_px,
                                    dest_y_px,
                                    fg_rgba: run.style.fg_rgba,
                                });
                                pen_x_px = pen_x_px.saturating_add(monochrome_glyph_advance_px(
                                    &frame.font,
                                    glyph.x_advance,
                                    rasterized.advance_px,
                                    cell_width_px,
                                ));
                                monochrome_glyphs_prepared =
                                    monochrome_glyphs_prepared.saturating_add(1);
                            }
                        }
                    } else {
                        let request = frame.font.raster_request(glyph.glyph_id, run.style.bold);
                        let rasterized =
                            fonts.rasterize_glyph(&frame.font, glyph.glyph_id, run.style.bold)?;
                        let upload = (!self.atlas.contains(request)).then(|| {
                            PreparedMonochromeGlyphUploadPayload {
                                width_px: rasterized.width_px,
                                height_px: rasterized.height_px,
                                bearing_x_px: rasterized.bearing_x_px,
                                bearing_y_px: rasterized.bearing_y_px,
                                advance_px: rasterized.advance_px,
                                coverage: rasterized.coverage.clone(),
                            }
                        });
                        let atlas_entry = self.atlas.upsert(request, &rasterized);
                        let x_offset_px = font_design_units_to_px(&frame.font, glyph.x_offset);
                        let y_offset_px = font_design_units_to_px(&frame.font, glyph.y_offset);
                        let dest_x_px = pen_x_px
                            .saturating_add(x_offset_px)
                            .saturating_add(rasterized.bearing_x_px);
                        let dest_y_px = row_top_px
                            .saturating_add(baseline_px)
                            .saturating_add(y_offset_px)
                            .saturating_add(rasterized.bearing_y_px);
                        monochrome_glyph_draws.push(PreparedMonochromeGlyphDraw {
                            row: row.row,
                            start_col: run.start_col(),
                            end_col: run.end_col(),
                            glyph_id: glyph.glyph_id,
                            atlas_entry,
                            upload,
                            x_offset_px,
                            y_offset_px,
                            dest_x_px,
                            dest_y_px,
                            fg_rgba: run.style.fg_rgba,
                        });
                        pen_x_px = pen_x_px.saturating_add(monochrome_glyph_advance_px(
                            &frame.font,
                            glyph.x_advance,
                            rasterized.advance_px,
                            cell_width_px,
                        ));
                        monochrome_glyphs_prepared = monochrome_glyphs_prepared.saturating_add(1);
                    }
                }
            }
        }

        let underline_overlay = PreparedUnderlineOverlay {
            visible: !underline_runs.is_empty(),
            run_count: underline_runs.len(),
            runs: underline_runs,
        };
        let underline_run_count = underline_overlay.run_count;
        let fingerprint = hash_shaped_frame(frame);
        if self.last_frame_fingerprint != Some(fingerprint) {
            self.next_frame_token = self.next_frame_token.saturating_add(1);
            self.last_frame_fingerprint = Some(fingerprint);
        }
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
            background_runs,
            monochrome_glyph_draws,
            color_glyph_draws,
            underline_run_count,
            underline_overlay,
            renderer_stats,
        })
    }

    fn upsert_color_glyph(
        &mut self,
        key: ColorGlyphCacheKey,
        rasterized: &crate::app::terminal_font::ColorGlyphRaster,
    ) -> (
        ColorGlyphCacheEntry,
        Option<PreparedColorGlyphUploadPayload>,
    ) {
        if let Some(entry) = self.color_glyph_cache.get(&key) {
            return (*entry, None);
        }

        let entry = ColorGlyphCacheEntry {
            slot: self.next_color_slot,
            width_px: rasterized.width_px,
            height_px: rasterized.height_px,
            rgba_bytes: rasterized.rgba.len(),
        };
        self.next_color_slot = self.next_color_slot.saturating_add(1);
        self.color_glyph_cache.insert(key, entry);

        (
            entry,
            Some(PreparedColorGlyphUploadPayload {
                width_px: rasterized.width_px,
                height_px: rasterized.height_px,
                rgba: rasterized.rgba.clone(),
            }),
        )
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
            run.style.bg_rgba.hash(&mut hasher);
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

fn font_design_units_to_px(font: &LoadedFont, value: i32) -> i32 {
    let metrics = font.metrics();
    let units_per_em = metrics.units_per_em.max(1) as f32;
    let px_size = font.px_size();
    ((value as f32) * (px_size / units_per_em)).round() as i32
}

fn monochrome_glyph_advance_px(
    font: &LoadedFont,
    x_advance_units: i32,
    raster_advance_px: i32,
    cell_width_px: u32,
) -> i32 {
    let shaped_advance_px = font_design_units_to_px(font, x_advance_units);
    if shaped_advance_px > 0 {
        shaped_advance_px
    } else if raster_advance_px > 0 {
        raster_advance_px
    } else {
        cell_width_px as i32
    }
}

fn color_glyph_advance_px(
    font: &LoadedFont,
    x_advance_units: i32,
    cell_width_px: u32,
    raster_width_px: u32,
) -> i32 {
    let shaped_advance_px = font_design_units_to_px(font, x_advance_units);
    if shaped_advance_px > 0 {
        shaped_advance_px
    } else {
        (cell_width_px.max(raster_width_px)) as i32
    }
}

fn center_color_glyph_in_cell(cell_height_px: u32, glyph_height_px: u32) -> i32 {
    let cell_height_px = cell_height_px as i32;
    let glyph_height_px = glyph_height_px as i32;
    (cell_height_px.saturating_sub(glyph_height_px) / 2).max(0)
}
