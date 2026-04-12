//! GPU-preparation stage for TerminalRendererHost native-frame presentation.

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::Result;

use crate::app::terminal_font::{
    FontFaceKey, FontSystem, GlyphRasterRequest, LoadedFont, RasterizedGlyph,
};
use crate::app::terminal_layout::run_segmentation::RunCluster;
use crate::app::terminal_layout::{GlyphRun, ShapedRow};
use crate::app::terminal_renderer::atlas::{
    ColorGlyphCacheEntry, ColorGlyphCacheKey, GlyphAtlas, GlyphAtlasEntry,
    MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
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
    pub padding_left_px: u32,
    pub padding_right_px: u32,
    pub bearing_x_px: i32,
    pub bearing_y_px: i32,
    pub advance_px: i32,
    pub coverage: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedMonochromeGlyphVisualFit {
    BodyText,
    GridSymbol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedMonochromeGlyphDraw {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub glyph_id: u32,
    pub face_key: FontFaceKey,
    pub font_family_name: String,
    pub font_em_size_px: u32,
    pub atlas_entry: GlyphAtlasEntry,
    pub upload: Option<PreparedMonochromeGlyphUploadPayload>,
    pub advance_px: i32,
    pub visible_left_px: i32,
    pub visible_top_px: i32,
    pub visible_width_px: u32,
    pub visible_height_px: u32,
    pub x_offset_px: i32,
    pub y_offset_px: i32,
    pub dest_x_px: i32,
    pub dest_y_px: i32,
    pub fg_rgba: u32,
    pub visual_fit: PreparedMonochromeGlyphVisualFit,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GlyphCellSpanRect {
    start_x_px: i32,
    start_y_px: i32,
    width_px: u32,
    height_px: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PreparedClusterGlyphKind {
    Monochrome {
        atlas_entry: GlyphAtlasEntry,
        upload: Option<PreparedMonochromeGlyphUploadPayload>,
        fg_rgba: u32,
        face_key: FontFaceKey,
        font_family_name: String,
        font_em_size_px: u32,
        advance_px: i32,
        visible_left_px: i32,
        visible_top_px: i32,
        visible_width_px: u32,
        visible_height_px: u32,
        visual_fit: PreparedMonochromeGlyphVisualFit,
    },
    Color {
        cache_entry: ColorGlyphCacheEntry,
        upload: Option<PreparedColorGlyphUploadPayload>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreparedClusterGlyph {
    glyph_id: u32,
    start_col: u32,
    end_col: u32,
    x_offset_px: i32,
    y_offset_px: i32,
    raw_dest_x_px: i32,
    raw_dest_y_px: i32,
    width_px: u32,
    height_px: u32,
    kind: PreparedClusterGlyphKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PreparedRowArtifacts {
    row: u32,
    background_runs: Vec<PreparedBackgroundRun>,
    monochrome_glyph_draws: Vec<PreparedMonochromeGlyphDraw>,
    color_glyph_draws: Vec<PreparedColorGlyphDraw>,
    underline_runs: Vec<PreparedUnderlineRun>,
}

const DEFAULT_MONO_GLYPH_CACHE_LIMIT: usize = 1024;
const DEFAULT_COLOR_GLYPH_CACHE_LIMIT: usize = 1024;
const DEFAULT_GLYPH_RASTER_CACHE_LIMIT: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WgpuRendererCacheLimits {
    mono_glyph_cache_entries: usize,
    color_glyph_cache_entries: usize,
    glyph_raster_cache_entries: usize,
}

impl Default for WgpuRendererCacheLimits {
    fn default() -> Self {
        Self {
            mono_glyph_cache_entries: DEFAULT_MONO_GLYPH_CACHE_LIMIT,
            color_glyph_cache_entries: DEFAULT_COLOR_GLYPH_CACHE_LIMIT,
            glyph_raster_cache_entries: DEFAULT_GLYPH_RASTER_CACHE_LIMIT,
        }
    }
}

impl WgpuRendererCacheLimits {
    fn sanitized(self) -> Self {
        Self {
            mono_glyph_cache_entries: self.mono_glyph_cache_entries.max(1),
            color_glyph_cache_entries: self.color_glyph_cache_entries.max(1),
            glyph_raster_cache_entries: self.glyph_raster_cache_entries.max(1),
        }
    }

    fn exceeded_by(self, stats: WgpuRendererCacheStats) -> bool {
        stats.mono_glyph_cache_entries > self.mono_glyph_cache_entries
            || stats.color_glyph_cache_entries > self.color_glyph_cache_entries
            || stats.glyph_raster_cache_entries > self.glyph_raster_cache_entries
    }
}

#[derive(Default)]
pub struct WgpuTerminalRenderer {
    atlas: GlyphAtlas,
    color_glyph_cache: HashMap<ColorGlyphCacheKey, ColorGlyphCacheEntry>,
    glyph_raster_cache: HashMap<GlyphRasterRequest, Arc<RasterizedGlyph>>,
    previous_prepared_rows: HashMap<u64, PreparedRowArtifacts>,
    last_prepared_row_reuse_count: usize,
    last_frame_fingerprint: Option<u64>,
    next_frame_token: u64,
    next_color_slot: u32,
    cache_limits: WgpuRendererCacheLimits,
    pending_cache_reset: bool,
    cache_reset_generation: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WgpuRendererCacheStats {
    pub mono_glyph_cache_entries: usize,
    pub color_glyph_cache_entries: usize,
    pub glyph_raster_cache_entries: usize,
    pub prepared_row_cache_entries: usize,
}

impl WgpuTerminalRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_for_test() -> Result<Self> {
        Ok(Self::new())
    }

    pub fn new_with_cache_limits_for_test(
        mono_glyph_cache_entries: usize,
        color_glyph_cache_entries: usize,
        glyph_raster_cache_entries: usize,
    ) -> Result<Self> {
        let mut renderer = Self::default();
        renderer.cache_limits = WgpuRendererCacheLimits {
            mono_glyph_cache_entries,
            color_glyph_cache_entries,
            glyph_raster_cache_entries,
        }
        .sanitized();
        Ok(renderer)
    }

    pub fn glyph_raster_cache_entry_count(&self) -> usize {
        self.glyph_raster_cache.len()
    }

    pub fn last_prepared_row_reuse_count(&self) -> usize {
        self.last_prepared_row_reuse_count
    }

    pub fn prepared_row_cache_entry_count(&self) -> usize {
        self.previous_prepared_rows.len()
    }

    pub fn cache_reset_generation(&self) -> u64 {
        self.cache_reset_generation
    }

    pub fn cache_stats(&self) -> WgpuRendererCacheStats {
        WgpuRendererCacheStats {
            mono_glyph_cache_entries: self.atlas.entry_count(),
            color_glyph_cache_entries: self.color_glyph_cache.len(),
            glyph_raster_cache_entries: self.glyph_raster_cache.len(),
            prepared_row_cache_entries: self.previous_prepared_rows.len(),
        }
    }

    pub fn clear_transient_caches(&mut self) {
        self.pending_cache_reset = false;
        self.clear_glyph_caches();
    }

    fn clear_glyph_caches(&mut self) {
        self.atlas.clear();
        self.color_glyph_cache.clear();
        self.glyph_raster_cache.clear();
        self.previous_prepared_rows.clear();
        self.last_prepared_row_reuse_count = 0;
        self.last_frame_fingerprint = None;
        self.next_color_slot = 0;
    }

    fn apply_pending_cache_reset(&mut self) {
        if !self.pending_cache_reset {
            return;
        }

        self.pending_cache_reset = false;
        self.cache_reset_generation = self.cache_reset_generation.saturating_add(1);
        self.clear_glyph_caches();
    }

    pub fn prepare(
        &mut self,
        frame: &ShapedTerminalFrame,
        fonts: &mut dyn FontSystem,
    ) -> Result<PreparedNativeFrame> {
        self.apply_pending_cache_reset();
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
        let baseline_px = frame.font.metrics().baseline_px.round() as i32;
        let font_em_size_px = frame.font.px_size().max(1.0).round() as u32;
        let previous_prepared_rows = std::mem::take(&mut self.previous_prepared_rows);
        let mut next_prepared_rows = HashMap::with_capacity(frame.rows.len());
        self.last_prepared_row_reuse_count = 0;

        for row in &frame.rows {
            let row_cache_key = hash_shaped_row_cache_key(&frame.font, row);
            let cached_row = previous_prepared_rows
                .get(&row_cache_key)
                .or_else(|| next_prepared_rows.get(&row_cache_key))
                .cloned();
            let prepared_row = if let Some(cached_row) = cached_row {
                self.last_prepared_row_reuse_count =
                    self.last_prepared_row_reuse_count.saturating_add(1);
                cached_row.rebase_for_row(row.row, cell_height_px)
            } else {
                let (fresh_row, row_monochrome_glyphs_prepared, row_color_glyphs_prepared) = self
                    .prepare_row(
                    frame,
                    row,
                    fonts,
                    cell_width_px,
                    cell_height_px,
                    baseline_px,
                    font_em_size_px,
                )?;
                monochrome_glyphs_prepared =
                    monochrome_glyphs_prepared.saturating_add(row_monochrome_glyphs_prepared);
                color_glyphs_prepared =
                    color_glyphs_prepared.saturating_add(row_color_glyphs_prepared);
                fresh_row
            };

            background_runs.extend(prepared_row.background_runs.iter().copied());
            monochrome_glyph_draws.extend(prepared_row.monochrome_glyph_draws.iter().cloned());
            color_glyph_draws.extend(prepared_row.color_glyph_draws.iter().cloned());
            underline_runs.extend(prepared_row.underline_runs.iter().copied());
            next_prepared_rows
                .entry(row_cache_key)
                .or_insert_with(|| prepared_row.cache_ready_clone());
        }
        self.previous_prepared_rows = next_prepared_rows;

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
        if self.cache_limits.exceeded_by(self.cache_stats()) {
            self.pending_cache_reset = true;
        }

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

    fn cached_rasterize_glyph(
        &mut self,
        fonts: &mut dyn FontSystem,
        frame: &ShapedTerminalFrame,
        request: GlyphRasterRequest,
    ) -> Result<Arc<RasterizedGlyph>> {
        if let Some(rasterized) = self.glyph_raster_cache.get(&request) {
            return Ok(Arc::clone(rasterized));
        }

        let rasterized = Arc::new(fonts.rasterize_glyph(&frame.font, request)?);
        self.glyph_raster_cache
            .insert(request, Arc::clone(&rasterized));
        Ok(rasterized)
    }

    fn prepare_row(
        &mut self,
        frame: &ShapedTerminalFrame,
        row: &ShapedRow,
        fonts: &mut dyn FontSystem,
        cell_width_px: u32,
        cell_height_px: u32,
        baseline_px: i32,
        font_em_size_px: u32,
    ) -> Result<(PreparedRowArtifacts, usize, usize)> {
        let row_top_px = (row.row as i32).saturating_mul(cell_height_px as i32);
        let mut monochrome_glyphs_prepared = 0usize;
        let mut color_glyphs_prepared = 0usize;
        let mut artifacts = PreparedRowArtifacts {
            row: row.row,
            ..PreparedRowArtifacts::default()
        };

        for run in &row.runs {
            artifacts.background_runs.push(PreparedBackgroundRun {
                row: row.row,
                start_col: run.start_col(),
                end_col: run.end_col(),
                bg_rgba: run.style.bg_rgba,
            });

            if run.style.underline {
                artifacts.underline_runs.push(PreparedUnderlineRun {
                    row: row.row,
                    start_col: run.start_col(),
                    end_col: run.end_col(),
                    fg_rgba: run.style.fg_rgba,
                });
            }

            let mut glyph_index = 0usize;
            while glyph_index < run.glyphs.len() {
                let cluster_start = run.glyphs[glyph_index].cluster;
                let (glyph_start_col, glyph_end_col) =
                    glyph_cell_span(run, cluster_start).unwrap_or((run.start_col(), run.end_col()));
                let visual_fit = glyph_cluster_visual_fit(run, cluster_start);
                let glyph_span_rect = glyph_cell_span_rect(
                    row.row,
                    glyph_start_col,
                    glyph_end_col,
                    cell_width_px,
                    cell_height_px,
                );
                let cluster_origin_x_subpx =
                    (glyph_start_col as i32).saturating_mul(cell_width_px as i32) as f32;
                let cluster_origin_x_px = cluster_origin_x_subpx.floor() as i32;
                let mut cluster_pen_x_subpx = cluster_origin_x_subpx;
                let mut cluster_glyphs = Vec::new();

                while glyph_index < run.glyphs.len()
                    && run.glyphs[glyph_index].cluster == cluster_start
                {
                    let glyph = &run.glyphs[glyph_index];
                    if run.has_color_glyphs {
                        match fonts.rasterize_color_glyph(
                            &frame.font,
                            &run.resolved_face,
                            glyph.glyph_id,
                        )? {
                            Some(rasterized) => {
                                let (cache_entry, upload) = self.upsert_color_glyph(
                                    ColorGlyphCacheKey::new(
                                        frame.font.cache_key(),
                                        run.resolved_face.face_key,
                                        glyph.glyph_id,
                                    ),
                                    &rasterized,
                                );
                                let x_offset_px =
                                    font_design_units_to_px(&frame.font, glyph.x_offset);
                                let x_offset_subpx =
                                    font_design_units_to_px_f32(&frame.font, glyph.x_offset);
                                let y_offset_px =
                                    font_design_units_to_px(&frame.font, glyph.y_offset);
                                let glyph_origin_x_px =
                                    (cluster_pen_x_subpx + x_offset_subpx).floor() as i32;
                                cluster_glyphs.push(PreparedClusterGlyph {
                                    glyph_id: glyph.glyph_id,
                                    start_col: glyph_start_col,
                                    end_col: glyph_end_col,
                                    x_offset_px,
                                    y_offset_px,
                                    raw_dest_x_px: glyph_origin_x_px,
                                    raw_dest_y_px: row_top_px
                                        .saturating_add(center_color_glyph_in_cell(
                                            cell_height_px,
                                            rasterized.height_px,
                                        ))
                                        .saturating_add(y_offset_px),
                                    width_px: rasterized.width_px,
                                    height_px: rasterized.height_px,
                                    kind: PreparedClusterGlyphKind::Color {
                                        cache_entry,
                                        upload,
                                    },
                                });
                                cluster_pen_x_subpx += color_glyph_advance_px_f32(
                                    &frame.font,
                                    glyph.x_advance,
                                    cell_width_px,
                                    rasterized.width_px,
                                );
                                color_glyphs_prepared = color_glyphs_prepared.saturating_add(1);
                            }
                            None => {
                                let x_offset_px =
                                    font_design_units_to_px(&frame.font, glyph.x_offset);
                                let x_offset_subpx =
                                    font_design_units_to_px_f32(&frame.font, glyph.x_offset);
                                let y_offset_px =
                                    font_design_units_to_px(&frame.font, glyph.y_offset);
                                let glyph_origin_x = cluster_pen_x_subpx + x_offset_subpx;
                                let (glyph_origin_x_px, fractional_offset_x) =
                                    split_fractional_offset(glyph_origin_x);
                                let request =
                                    frame.font.raster_request_with_fractional_offset_x_for_face(
                                        run.resolved_face.face_key,
                                        glyph.glyph_id,
                                        run.style.bold,
                                        fractional_offset_x,
                                    );
                                let rasterized =
                                    self.cached_rasterize_glyph(fonts, frame, request)?;
                                let upload = (!self.atlas.contains(request)).then(|| {
                                    PreparedMonochromeGlyphUploadPayload {
                                        width_px: rasterized.width_px.saturating_add(
                                            MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX
                                                .saturating_mul(2),
                                        ),
                                        height_px: rasterized.height_px,
                                        padding_left_px: MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
                                        padding_right_px: MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
                                        bearing_x_px: rasterized.bearing_x_px,
                                        bearing_y_px: rasterized.bearing_y_px,
                                        advance_px: rasterized.advance_px,
                                        coverage: pad_monochrome_glyph_coverage(
                                            rasterized.width_px,
                                            rasterized.height_px,
                                            &rasterized.coverage,
                                            MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
                                            MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
                                        ),
                                    }
                                });
                                let atlas_entry = self.atlas.upsert(request, rasterized.as_ref());
                                cluster_glyphs.push(PreparedClusterGlyph {
                                    glyph_id: glyph.glyph_id,
                                    start_col: glyph_start_col,
                                    end_col: glyph_end_col,
                                    x_offset_px,
                                    y_offset_px,
                                    raw_dest_x_px: glyph_origin_x_px
                                        .saturating_add(rasterized.bearing_x_px)
                                        .saturating_sub(
                                            MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX as i32,
                                        ),
                                    raw_dest_y_px: row_top_px
                                        .saturating_add(baseline_px)
                                        .saturating_add(y_offset_px)
                                        .saturating_add(rasterized.bearing_y_px),
                                    width_px: rasterized.width_px,
                                    height_px: rasterized.height_px,
                                    kind: PreparedClusterGlyphKind::Monochrome {
                                        atlas_entry,
                                        upload,
                                        fg_rgba: run.style.fg_rgba,
                                        face_key: run.resolved_face.face_key,
                                        font_family_name: run.resolved_face.family_name.clone(),
                                        font_em_size_px,
                                        advance_px: rasterized.advance_px,
                                        visible_left_px: rasterized.visible_left_px,
                                        visible_top_px: rasterized.visible_top_px,
                                        visible_width_px: rasterized.visible_width_px,
                                        visible_height_px: rasterized.visible_height_px,
                                        visual_fit,
                                    },
                                });
                                cluster_pen_x_subpx += monochrome_glyph_advance_px_f32(
                                    &frame.font,
                                    glyph.x_advance,
                                    rasterized.advance_px,
                                    cell_width_px,
                                );
                                monochrome_glyphs_prepared =
                                    monochrome_glyphs_prepared.saturating_add(1);
                            }
                        }
                    } else {
                        let x_offset_px = font_design_units_to_px(&frame.font, glyph.x_offset);
                        let x_offset_subpx =
                            font_design_units_to_px_f32(&frame.font, glyph.x_offset);
                        let glyph_origin_x = cluster_pen_x_subpx + x_offset_subpx;
                        let (glyph_origin_x_px, fractional_offset_x) =
                            split_fractional_offset(glyph_origin_x);
                        let request = frame.font.raster_request_with_fractional_offset_x_for_face(
                            run.resolved_face.face_key,
                            glyph.glyph_id,
                            run.style.bold,
                            fractional_offset_x,
                        );
                        let rasterized = self.cached_rasterize_glyph(fonts, frame, request)?;
                        let upload = (!self.atlas.contains(request)).then(|| {
                            PreparedMonochromeGlyphUploadPayload {
                                width_px: rasterized.width_px.saturating_add(
                                    MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX.saturating_mul(2),
                                ),
                                height_px: rasterized.height_px,
                                padding_left_px: MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
                                padding_right_px: MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
                                bearing_x_px: rasterized.bearing_x_px,
                                bearing_y_px: rasterized.bearing_y_px,
                                advance_px: rasterized.advance_px,
                                coverage: pad_monochrome_glyph_coverage(
                                    rasterized.width_px,
                                    rasterized.height_px,
                                    &rasterized.coverage,
                                    MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
                                    MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX,
                                ),
                            }
                        });
                        let atlas_entry = self.atlas.upsert(request, rasterized.as_ref());
                        let y_offset_px = font_design_units_to_px(&frame.font, glyph.y_offset);
                        cluster_glyphs.push(PreparedClusterGlyph {
                            glyph_id: glyph.glyph_id,
                            start_col: glyph_start_col,
                            end_col: glyph_end_col,
                            x_offset_px,
                            y_offset_px,
                            raw_dest_x_px: glyph_origin_x_px
                                .saturating_add(rasterized.bearing_x_px)
                                .saturating_sub(MONOCHROME_ATLAS_HORIZONTAL_PADDING_PX as i32),
                            raw_dest_y_px: row_top_px
                                .saturating_add(baseline_px)
                                .saturating_add(y_offset_px)
                                .saturating_add(rasterized.bearing_y_px),
                            width_px: rasterized.width_px,
                            height_px: rasterized.height_px,
                            kind: PreparedClusterGlyphKind::Monochrome {
                                atlas_entry,
                                upload,
                                fg_rgba: run.style.fg_rgba,
                                face_key: run.resolved_face.face_key,
                                font_family_name: run.resolved_face.family_name.clone(),
                                font_em_size_px,
                                advance_px: rasterized.advance_px,
                                visible_left_px: rasterized.visible_left_px,
                                visible_top_px: rasterized.visible_top_px,
                                visible_width_px: rasterized.visible_width_px,
                                visible_height_px: rasterized.visible_height_px,
                                visual_fit,
                            },
                        });
                        cluster_pen_x_subpx += monochrome_glyph_advance_px_f32(
                            &frame.font,
                            glyph.x_advance,
                            rasterized.advance_px,
                            cell_width_px,
                        );
                        monochrome_glyphs_prepared = monochrome_glyphs_prepared.saturating_add(1);
                    }
                    glyph_index = glyph_index.saturating_add(1);
                }

                let cluster_offset_x_px = compute_cluster_offset_x_px(
                    cluster_origin_x_px,
                    glyph_span_rect.start_x_px,
                    &cluster_glyphs,
                );

                for glyph in cluster_glyphs {
                    let dest_x_px = glyph.raw_dest_x_px.saturating_add(cluster_offset_x_px);
                    match glyph.kind {
                        PreparedClusterGlyphKind::Monochrome {
                            atlas_entry,
                            upload,
                            fg_rgba,
                            face_key,
                            font_family_name,
                            font_em_size_px,
                            advance_px,
                            visible_left_px,
                            visible_top_px,
                            visible_width_px,
                            visible_height_px,
                            visual_fit,
                        } => artifacts
                            .monochrome_glyph_draws
                            .push(PreparedMonochromeGlyphDraw {
                                row: row.row,
                                start_col: glyph.start_col,
                                end_col: glyph.end_col,
                                glyph_id: glyph.glyph_id,
                                face_key,
                                font_family_name,
                                font_em_size_px,
                                atlas_entry,
                                upload,
                                advance_px,
                                visible_left_px,
                                visible_top_px,
                                visible_width_px,
                                visible_height_px,
                                x_offset_px: glyph.x_offset_px,
                                y_offset_px: glyph.y_offset_px,
                                dest_x_px,
                                dest_y_px: glyph.raw_dest_y_px,
                                fg_rgba,
                                visual_fit,
                            }),
                        PreparedClusterGlyphKind::Color {
                            cache_entry,
                            upload,
                        } => artifacts.color_glyph_draws.push(PreparedColorGlyphDraw {
                            row: row.row,
                            start_col: glyph.start_col,
                            end_col: glyph.end_col,
                            glyph_id: glyph.glyph_id,
                            cache_entry,
                            upload,
                            x_offset_px: glyph.x_offset_px,
                            y_offset_px: glyph.y_offset_px,
                            dest_x_px,
                            dest_y_px: glyph.raw_dest_y_px,
                        }),
                    }
                }
            }
        }

        Ok((artifacts, monochrome_glyphs_prepared, color_glyphs_prepared))
    }
}

impl PreparedRowArtifacts {
    fn cache_ready_clone(&self) -> Self {
        Self {
            row: self.row,
            background_runs: self.background_runs.clone(),
            monochrome_glyph_draws: self
                .monochrome_glyph_draws
                .iter()
                .map(|draw| rebase_monochrome_glyph_draw(draw, self.row, 0, true))
                .collect(),
            color_glyph_draws: self
                .color_glyph_draws
                .iter()
                .map(|draw| rebase_color_glyph_draw(draw, self.row, 0, true))
                .collect(),
            underline_runs: self.underline_runs.clone(),
        }
    }

    fn rebase_for_row(&self, row: u32, cell_height_px: u32) -> Self {
        let delta_y = (row as i32)
            .saturating_sub(self.row as i32)
            .saturating_mul(cell_height_px as i32);

        Self {
            row,
            background_runs: self
                .background_runs
                .iter()
                .map(|run| PreparedBackgroundRun { row, ..*run })
                .collect(),
            monochrome_glyph_draws: self
                .monochrome_glyph_draws
                .iter()
                .map(|draw| rebase_monochrome_glyph_draw(draw, row, delta_y, true))
                .collect(),
            color_glyph_draws: self
                .color_glyph_draws
                .iter()
                .map(|draw| rebase_color_glyph_draw(draw, row, delta_y, true))
                .collect(),
            underline_runs: self
                .underline_runs
                .iter()
                .map(|run| PreparedUnderlineRun { row, ..*run })
                .collect(),
        }
    }
}

fn rebase_monochrome_glyph_draw(
    draw: &PreparedMonochromeGlyphDraw,
    row: u32,
    delta_y: i32,
    strip_upload: bool,
) -> PreparedMonochromeGlyphDraw {
    PreparedMonochromeGlyphDraw {
        row,
        upload: (!strip_upload).then(|| draw.upload.clone()).flatten(),
        dest_y_px: draw.dest_y_px.saturating_add(delta_y),
        ..draw.clone()
    }
}

fn rebase_color_glyph_draw(
    draw: &PreparedColorGlyphDraw,
    row: u32,
    delta_y: i32,
    strip_upload: bool,
) -> PreparedColorGlyphDraw {
    PreparedColorGlyphDraw {
        row,
        upload: (!strip_upload).then(|| draw.upload.clone()).flatten(),
        dest_y_px: draw.dest_y_px.saturating_add(delta_y),
        ..draw.clone()
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
        row.row_hash.hash(&mut hasher);
    }

    hasher.finish()
}

fn hash_shaped_row_cache_key(font: &LoadedFont, row: &ShapedRow) -> u64 {
    let mut hasher = DefaultHasher::new();
    let metrics = font.metrics();
    font.cache_key().hash(&mut hasher);
    metrics.units_per_em.hash(&mut hasher);
    metrics.cell_width_px.to_bits().hash(&mut hasher);
    metrics.cell_height_px.to_bits().hash(&mut hasher);
    row.content_hash.hash(&mut hasher);

    hasher.finish()
}

fn font_design_units_to_px(font: &LoadedFont, value: i32) -> i32 {
    font_design_units_to_px_f32(font, value).round() as i32
}

fn font_design_units_to_px_f32(font: &LoadedFont, value: i32) -> f32 {
    let metrics = font.metrics();
    let units_per_em = metrics.units_per_em.max(1) as f32;
    let px_size = font.px_size();
    (value as f32) * (px_size / units_per_em)
}

fn split_fractional_offset(position: f32) -> (i32, f32) {
    let base = position.floor();
    (base as i32, position - base)
}

fn glyph_cell_span(run: &GlyphRun, cluster_start: u32) -> Option<(u32, u32)> {
    if run.clusters.is_empty() {
        return None;
    }

    let cluster_start = (cluster_start as usize).min(run.text.len());
    let cluster_end = next_cluster_boundary(&run.clusters, cluster_start).unwrap_or(run.text.len());
    clusters_to_cell_span(&run.clusters, cluster_start..cluster_end)
}

fn glyph_cluster_text(run: &GlyphRun, cluster_start: u32) -> Option<&str> {
    if run.clusters.is_empty() {
        return None;
    }

    let cluster_start = (cluster_start as usize).min(run.text.len());
    let cluster_end = next_cluster_boundary(&run.clusters, cluster_start).unwrap_or(run.text.len());
    run.text.get(cluster_start..cluster_end)
}

fn glyph_cluster_visual_fit(
    run: &GlyphRun,
    cluster_start: u32,
) -> PreparedMonochromeGlyphVisualFit {
    let Some(cluster_text) = glyph_cluster_text(run, cluster_start) else {
        return PreparedMonochromeGlyphVisualFit::BodyText;
    };

    if cluster_text.chars().all(is_grid_fitted_symbol) {
        PreparedMonochromeGlyphVisualFit::GridSymbol
    } else {
        PreparedMonochromeGlyphVisualFit::BodyText
    }
}

fn is_grid_fitted_symbol(ch: char) -> bool {
    matches!(
        ch,
        '\u{2500}'..='\u{257f}'
            | '\u{2580}'..='\u{259f}'
            | '\u{2800}'..='\u{28ff}'
            | '\u{e0a0}'..='\u{e0d4}'
            | '\u{ee00}'..='\u{ee0b}'
            | '\u{f5d0}'..='\u{f60d}'
            | '\u{1fb00}'..='\u{1fbff}'
    )
}

fn next_cluster_boundary(clusters: &[RunCluster], cluster_start: usize) -> Option<usize> {
    clusters
        .iter()
        .map(|cluster| cluster.byte_range.start)
        .filter(|start| *start > cluster_start)
        .min()
}

fn clusters_to_cell_span(
    clusters: &[RunCluster],
    byte_range: std::ops::Range<usize>,
) -> Option<(u32, u32)> {
    let mut start_col = None;
    let mut end_col = None;

    for cluster in clusters {
        if cluster.byte_range.start < byte_range.end && byte_range.start < cluster.byte_range.end {
            start_col.get_or_insert(cluster.cell_range.start);
            end_col = Some(cluster.cell_range.end.saturating_sub(1));
        }
    }

    match (start_col, end_col) {
        (Some(start_col), Some(end_col)) => Some((start_col, end_col)),
        _ => None,
    }
}

fn compute_cluster_offset_x_px(
    cluster_origin_x_px: i32,
    span_start_x_px: i32,
    _glyphs: &[PreparedClusterGlyph],
) -> i32 {
    span_start_x_px.saturating_sub(cluster_origin_x_px)
}

fn glyph_cell_span_rect(
    row: u32,
    start_col: u32,
    end_col: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> GlyphCellSpanRect {
    GlyphCellSpanRect {
        start_x_px: start_col.saturating_mul(cell_width_px) as i32,
        start_y_px: row.saturating_mul(cell_height_px) as i32,
        width_px: end_col
            .saturating_sub(start_col)
            .saturating_add(1)
            .saturating_mul(cell_width_px),
        height_px: cell_height_px,
    }
}

fn monochrome_glyph_advance_px_f32(
    font: &LoadedFont,
    x_advance_units: i32,
    raster_advance_px: i32,
    cell_width_px: u32,
) -> f32 {
    let shaped_advance_px = font_design_units_to_px_f32(font, x_advance_units);
    if shaped_advance_px > 0.0 {
        shaped_advance_px
    } else if raster_advance_px > 0 {
        raster_advance_px as f32
    } else {
        cell_width_px as f32
    }
}

fn color_glyph_advance_px_f32(
    font: &LoadedFont,
    x_advance_units: i32,
    cell_width_px: u32,
    raster_width_px: u32,
) -> f32 {
    let shaped_advance_px = font_design_units_to_px_f32(font, x_advance_units);
    if shaped_advance_px > 0.0 {
        shaped_advance_px
    } else {
        (cell_width_px.max(raster_width_px)) as f32
    }
}

fn center_color_glyph_in_cell(cell_height_px: u32, glyph_height_px: u32) -> i32 {
    let cell_height_px = cell_height_px as i32;
    let glyph_height_px = glyph_height_px as i32;
    (cell_height_px.saturating_sub(glyph_height_px) / 2).max(0)
}

fn pad_monochrome_glyph_coverage(
    width_px: u32,
    height_px: u32,
    coverage: &[u8],
    padding_left_px: u32,
    padding_right_px: u32,
) -> Vec<u8> {
    if width_px == 0 || height_px == 0 || (padding_left_px == 0 && padding_right_px == 0) {
        return coverage.to_vec();
    }

    let padded_width_px = width_px
        .saturating_add(padding_left_px)
        .saturating_add(padding_right_px);
    let mut padded = vec![0; (padded_width_px.saturating_mul(height_px)) as usize];

    for row in 0..height_px {
        let src_start = (row.saturating_mul(width_px)) as usize;
        let src_end = src_start.saturating_add(width_px as usize);
        let dst_start = (row
            .saturating_mul(padded_width_px)
            .saturating_add(padding_left_px)) as usize;
        let dst_end = dst_start.saturating_add(width_px as usize);
        padded[dst_start..dst_end].copy_from_slice(&coverage[src_start..src_end]);
    }

    padded
}
