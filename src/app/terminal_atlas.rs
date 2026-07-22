//! Renders the active terminal grid into a single image surface using the bundled Sarasa Term SC atlas font.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::mem::size_of;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use anyhow::{Result, anyhow};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use swash::FontRef as SwashFontRef;
use swash::scale::image::Content as SwashContent;
use swash::scale::{Render, ScaleContext, Scaler, Source};
use swash::zeno::{Format as SwashFormat, Vector as SwashVector};

use crate::app::ssh::runtime::{TerminalCellState, TerminalSurfaceState};
use crate::app::terminal_core::{TERMINAL_IMAGE_UV_SCALE, TerminalImageResource};
use crate::app::terminal_emoji::{
    ClusterRenderKind as EmojiClusterRenderKind, EmojiRenderOutcome, TerminalEmojiRenderer,
    classify_cluster_render_kind,
};
use crate::app::terminal_font::backend::{
    DEFAULT_TERMINAL_FONT_SIZE_PX, FontRenderProfile, map_glyph_coverage_to_alpha,
    terminal_cell_width_px,
};
use crate::app::terminal_model::{
    TerminalImageDrawRect, terminal_image_draw_rect, terminal_image_frame_fingerprint,
};
use crate::app::terminal_renderer::custom_grid_glyphs::{
    classify_custom_grid_glyph, generate_custom_grid_mask,
};

const SARASA_TERM_SC_FONT_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf");
const TERMINAL_FONT_SIZE_PX: f32 = DEFAULT_TERMINAL_FONT_SIZE_PX;
const MIN_CELL_WIDTH_PX: u32 = 8;
const MIN_CELL_HEIGHT_PX: u32 = 20;
const CELL_HORIZONTAL_PADDING_PX: u32 = 0;
const CELL_VERTICAL_PADDING_PX: u32 = 0;
const REGULAR_MONO_EMBOLDEN_STRENGTH: f32 = 0.0;
const BOLD_MONO_EMBOLDEN_STRENGTH: f32 = 0.52;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalAtlasMetrics {
    pub cell_width: u32,
    pub cell_height: u32,
    pub baseline_px: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterSpriteKind {
    MonoAlpha,
    ColorRgba,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderedClusterSourceKind {
    FontRaster,
    GeneratedMask,
    ColorEmoji,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TerminalAtlasSelection {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

impl TerminalAtlasSelection {
    pub fn new(start_row: u32, start_col: u32, end_row: u32, end_col: u32) -> Self {
        if (start_row, start_col) <= (end_row, end_col) {
            Self {
                start_row,
                start_col,
                end_row,
                end_col,
            }
        } else {
            Self {
                start_row: end_row,
                start_col: end_col,
                end_row: start_row,
                end_col: start_col,
            }
        }
    }

    fn row_bounds(self, row: u32, cols: u32) -> Option<(u32, u32)> {
        if cols == 0 || row < self.start_row || row > self.end_row {
            return None;
        }

        let start_col = if row == self.start_row {
            self.start_col.min(cols)
        } else {
            0
        };
        let end_col_exclusive = if row == self.end_row {
            self.end_col.min(cols)
        } else {
            cols
        };
        if start_col >= end_col_exclusive {
            return None;
        }

        Some((
            start_col.min(cols.saturating_sub(1)),
            end_col_exclusive
                .saturating_sub(1)
                .min(cols.saturating_sub(1)),
        ))
    }
}

#[derive(Clone, Debug)]
pub struct TerminalSurfaceFrame {
    pub image: Image,
    pub metrics: TerminalAtlasMetrics,
    pub raster_metrics: TerminalAtlasMetrics,
    pub cache_entries: usize,
    pub rerendered_rows: Vec<u32>,
    pub rendered_clusters: Vec<RenderedCluster>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalAtlasCacheStats {
    pub sprite_cache_entries: usize,
    pub row_hash_entries: usize,
    pub surface_pixel_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedCluster {
    pub row: u32,
    pub col: u32,
    pub text: String,
    pub sprite_kind: ClusterSpriteKind,
    pub source_kind: RenderedClusterSourceKind,
}

pub struct TerminalAtlasRenderer {
    font: FontArc,
    swash_font: SwashFontRef<'static>,
    mono_scale_context: ScaleContext,
    mono_render_profile: FontRenderProfile,
    emoji_renderer: TerminalEmojiRenderer,
    logical_metrics: TerminalAtlasMetrics,
    raster_metrics: TerminalAtlasMetrics,
    raster_scale: f32,
    sprite_cache: HashMap<ClusterKey, CachedClusterSprite>,
    row_hashes: Vec<u64>,
    pixels: Vec<Rgba8Pixel>,
    surface_width_px: u32,
    surface_height_px: u32,
    viewport_background_signature: Option<(u32, u32, u32)>,
    image_fingerprint: Option<u64>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct ClusterKey {
    text: String,
    span: u32,
    bold: bool,
}

#[derive(Clone, Copy)]
struct RowRenderRequest {
    row: u32,
    default_fg_rgba: u32,
    default_bg_rgba: u32,
    viewport_bg_top_rgba: u32,
    viewport_bg_bottom_rgba: u32,
    row_selection: Option<(u32, u32)>,
    selection_overlay_rgba: u32,
}

#[derive(Clone, Copy)]
struct PixelRect {
    start_x: u32,
    start_y: u32,
    width: u32,
    height: u32,
}

#[derive(Clone, Debug)]
enum CachedClusterSprite {
    MonoAlpha {
        width: u32,
        height: u32,
        alpha: Vec<u8>,
        source_kind: RenderedClusterSourceKind,
    },
    ColorRgba {
        width: u32,
        height: u32,
        rgba: Vec<Rgba8Pixel>,
    },
}

struct RenderedMonoGlyph {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

struct MonoRasterRequest<'a> {
    font: &'a FontArc,
    swash_font: SwashFontRef<'a>,
    scale_context: &'a mut ScaleContext,
    metrics: TerminalAtlasMetrics,
    px_size: f32,
    text: &'a str,
    span: u32,
    bold: bool,
    render_profile: FontRenderProfile,
}

impl CachedClusterSprite {
    fn kind(&self) -> ClusterSpriteKind {
        match self {
            Self::MonoAlpha { .. } => ClusterSpriteKind::MonoAlpha,
            Self::ColorRgba { .. } => ClusterSpriteKind::ColorRgba,
        }
    }

    fn source_kind(&self) -> RenderedClusterSourceKind {
        match self {
            Self::MonoAlpha { source_kind, .. } => *source_kind,
            Self::ColorRgba { .. } => RenderedClusterSourceKind::ColorEmoji,
        }
    }
}

impl TerminalAtlasRenderer {
    pub fn new() -> Result<Self> {
        Self::with_emoji_renderer(TerminalEmojiRenderer::new())
    }

    pub fn with_emoji_renderer_for_tests(emoji_renderer: TerminalEmojiRenderer) -> Result<Self> {
        Self::with_emoji_renderer(emoji_renderer)
    }

    fn with_emoji_renderer(emoji_renderer: TerminalEmojiRenderer) -> Result<Self> {
        let font = FontArc::try_from_slice(SARASA_TERM_SC_FONT_BYTES)
            .map_err(|error| anyhow!("failed to load bundled Sarasa Term SC font: {error}"))?;
        let swash_font = SwashFontRef::from_index(SARASA_TERM_SC_FONT_BYTES, 0)
            .ok_or_else(|| anyhow!("failed to load bundled Sarasa Term SC font into swash"))?;
        let logical_metrics = compute_terminal_metrics(&font, TERMINAL_FONT_SIZE_PX);
        Ok(Self {
            font,
            swash_font,
            mono_scale_context: ScaleContext::new(),
            mono_render_profile: FontRenderProfile::bitmap_default(),
            emoji_renderer,
            logical_metrics,
            raster_metrics: logical_metrics,
            raster_scale: 1.0,
            sprite_cache: HashMap::new(),
            row_hashes: Vec::new(),
            pixels: Vec::new(),
            surface_width_px: 0,
            surface_height_px: 0,
            viewport_background_signature: None,
            image_fingerprint: None,
        })
    }

    pub fn metrics(&self) -> TerminalAtlasMetrics {
        self.logical_metrics
    }

    pub fn raster_metrics(&self) -> TerminalAtlasMetrics {
        self.raster_metrics
    }

    pub fn cache_stats(&self) -> TerminalAtlasCacheStats {
        TerminalAtlasCacheStats {
            sprite_cache_entries: self.sprite_cache.len(),
            row_hash_entries: self.row_hashes.len(),
            surface_pixel_bytes: self.pixels.len().saturating_mul(size_of::<Rgba8Pixel>()),
        }
    }

    pub fn clear_transient_caches(&mut self) {
        self.sprite_cache.clear();
        self.row_hashes.clear();
        self.pixels.clear();
        self.surface_width_px = 0;
        self.surface_height_px = 0;
        self.viewport_background_signature = None;
        self.image_fingerprint = None;
    }

    pub fn set_raster_scale(&mut self, scale: f32) {
        let next_scale = sanitize_raster_scale(scale);
        if (next_scale - self.raster_scale).abs() < 0.01 {
            return;
        }

        self.raster_scale = next_scale;
        self.raster_metrics = scale_terminal_metrics(self.logical_metrics, next_scale);
        self.clear_transient_caches();
    }

    pub fn render(&mut self, surface: &TerminalSurfaceState) -> Result<TerminalSurfaceFrame> {
        self.render_with_selection(surface, None, 0)
    }

    pub fn render_with_selection(
        &mut self,
        surface: &TerminalSurfaceState,
        selection: Option<TerminalAtlasSelection>,
        selection_overlay_rgba: u32,
    ) -> Result<TerminalSurfaceFrame> {
        let width_px = surface.cols.max(1) * self.raster_metrics.cell_width;
        let height_px = surface.rows.max(1) * self.raster_metrics.cell_height;
        let viewport_background_signature = (
            surface.default_bg_rgba,
            surface.row_bg_even_rgba,
            surface.row_bg_odd_rgba,
        );
        let mut rerendered_rows = Vec::new();
        let mut rendered_clusters = Vec::new();
        let resized = self.surface_width_px != width_px || self.surface_height_px != height_px;
        let viewport_background_changed =
            resized || self.viewport_background_signature != Some(viewport_background_signature);
        let image_fingerprint =
            terminal_image_frame_fingerprint(&surface.image_resources, &surface.image_placements);
        let force_image_recompose = !surface.image_placements.is_empty()
            || self.image_fingerprint != Some(image_fingerprint);

        if resized {
            self.surface_width_px = width_px;
            self.surface_height_px = height_px;
            self.pixels = vec![rgba8(surface.default_bg_rgba); (width_px * height_px) as usize];
            self.row_hashes = vec![0; surface.rows as usize];
        } else if self.row_hashes.len() != surface.rows as usize {
            self.row_hashes.resize(surface.rows as usize, 0);
        }
        if viewport_background_changed {
            fill_viewport_background_span(
                &mut self.pixels,
                self.surface_width_px,
                self.surface_height_px,
                0,
                self.surface_height_px,
                rgba8(surface.row_bg_even_rgba),
                rgba8(surface.row_bg_odd_rgba),
            );
            self.viewport_background_signature = Some(viewport_background_signature);
        }

        let mut row_cells = vec![Vec::new(); surface.rows as usize];
        for cell in &surface.cells {
            if let Some(row) = row_cells.get_mut(cell.row as usize) {
                row.push(cell);
            }
        }

        let mut row_render_data = Vec::with_capacity(surface.rows as usize);
        for row in 0..surface.rows {
            let row_selection = selection.and_then(|value| value.row_bounds(row, surface.cols));
            let row_selection_overlay_rgba = if row_selection.is_some() {
                selection_overlay_rgba
            } else {
                0
            };
            let next_hash = hash_row(
                surface.cols,
                surface.default_fg_rgba,
                surface.default_bg_rgba,
                surface.row_bg_even_rgba,
                surface.row_bg_odd_rgba,
                row_selection,
                row_selection_overlay_rgba,
                row_cells[row as usize].as_slice(),
            );
            row_render_data.push((
                RowRenderRequest {
                    row,
                    default_fg_rgba: surface.default_fg_rgba,
                    default_bg_rgba: surface.default_bg_rgba,
                    viewport_bg_top_rgba: surface.row_bg_even_rgba,
                    viewport_bg_bottom_rgba: surface.row_bg_odd_rgba,
                    row_selection,
                    selection_overlay_rgba: row_selection_overlay_rgba,
                },
                next_hash,
                resized || force_image_recompose || self.row_hashes[row as usize] != next_hash,
            ));
        }

        if force_image_recompose {
            for (request, next_hash, _) in &row_render_data {
                self.render_row_background(*request, &row_cells[request.row as usize]);
                self.row_hashes[request.row as usize] = *next_hash;
                rerendered_rows.push(request.row);
            }
            self.render_terminal_images(surface, |z_index| z_index < 0);
            for (request, _, _) in &row_render_data {
                self.render_row_foreground(
                    *request,
                    &row_cells[request.row as usize],
                    &mut rendered_clusters,
                );
            }
            self.render_terminal_images(surface, |z_index| z_index >= 0);
        } else {
            for (request, next_hash, dirty) in &row_render_data {
                if *dirty {
                    self.render_row_background(*request, &row_cells[request.row as usize]);
                    self.render_row_foreground(
                        *request,
                        &row_cells[request.row as usize],
                        &mut rendered_clusters,
                    );
                    self.row_hashes[request.row as usize] = *next_hash;
                    rerendered_rows.push(request.row);
                } else {
                    self.record_rendered_clusters_from_cache(
                        &row_cells[request.row as usize],
                        &mut rendered_clusters,
                    );
                }
            }
        }
        self.image_fingerprint = Some(image_fingerprint);

        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width_px, height_px);
        buffer
            .make_mut_slice()
            .copy_from_slice(self.pixels.as_slice());

        Ok(TerminalSurfaceFrame {
            image: Image::from_rgba8(buffer),
            metrics: self.logical_metrics,
            raster_metrics: self.raster_metrics,
            cache_entries: self.sprite_cache.len(),
            rerendered_rows,
            rendered_clusters,
        })
    }

    fn render_row_background(&mut self, request: RowRenderRequest, cells: &[&TerminalCellState]) {
        let row_y = request.row * self.raster_metrics.cell_height;
        // Restore the dirty row from the shared viewport background layer before repainting
        // selection, explicit cell backgrounds, and glyphs on top.
        fill_viewport_background_span(
            &mut self.pixels,
            self.surface_width_px,
            self.surface_height_px,
            row_y,
            self.raster_metrics.cell_height,
            rgba8(request.viewport_bg_top_rgba),
            rgba8(request.viewport_bg_bottom_rgba),
        );
        let row_bg = viewport_gradient_color(
            rgba8(request.viewport_bg_top_rgba),
            rgba8(request.viewport_bg_bottom_rgba),
            row_y + (self.raster_metrics.cell_height / 2),
            self.surface_height_px,
        );

        if let Some((start_col, end_col)) = request.row_selection {
            fill_rect(
                &mut self.pixels,
                self.surface_width_px,
                self.surface_height_px,
                PixelRect {
                    start_x: start_col * self.raster_metrics.cell_width,
                    start_y: row_y,
                    width: (end_col - start_col + 1) * self.raster_metrics.cell_width,
                    height: self.raster_metrics.cell_height,
                },
                composite_color(row_bg, rgba8(request.selection_overlay_rgba)),
            );
        }

        for cell in cells {
            let cell_x = cell.col * self.raster_metrics.cell_width;
            let span = cell.width.max(1);
            let span_width_px = span * self.raster_metrics.cell_width;
            let selected = request
                .row_selection
                .is_some_and(|value| selection_overlaps_cell(value, cell.col, span));
            let uses_default_background = cell.bg_rgba == request.default_bg_rgba;
            let cell_bg = if uses_default_background {
                row_bg
            } else {
                rgba8(cell.bg_rgba)
            };
            let background = if selected {
                composite_color(cell_bg, rgba8(request.selection_overlay_rgba))
            } else {
                cell_bg
            };

            if selected || !uses_default_background {
                fill_rect(
                    &mut self.pixels,
                    self.surface_width_px,
                    self.surface_height_px,
                    PixelRect {
                        start_x: cell_x,
                        start_y: row_y,
                        width: span_width_px,
                        height: self.raster_metrics.cell_height,
                    },
                    background,
                );
            }
        }
    }

    fn render_row_foreground(
        &mut self,
        request: RowRenderRequest,
        cells: &[&TerminalCellState],
        rendered_clusters: &mut Vec<RenderedCluster>,
    ) {
        let row_y = request.row * self.raster_metrics.cell_height;
        let row_bg = viewport_gradient_color(
            rgba8(request.viewport_bg_top_rgba),
            rgba8(request.viewport_bg_bottom_rgba),
            row_y + (self.raster_metrics.cell_height / 2),
            self.surface_height_px,
        );
        let default_fg = rgba8(request.default_fg_rgba);
        for cell in cells {
            let cell_x = cell.col * self.raster_metrics.cell_width;
            let span = cell.width.max(1);
            let span_width_px = span * self.raster_metrics.cell_width;
            let selected = request
                .row_selection
                .is_some_and(|value| selection_overlaps_cell(value, cell.col, span));
            let cell_bg = if cell.bg_rgba == request.default_bg_rgba {
                row_bg
            } else {
                rgba8(cell.bg_rgba)
            };
            let background = if selected {
                composite_color(cell_bg, rgba8(request.selection_overlay_rgba))
            } else {
                cell_bg
            };
            let foreground = if selected {
                resolve_selected_foreground(rgba8(cell.fg_rgba), cell_bg, default_fg, background)
            } else {
                rgba8(cell.fg_rgba)
            };
            let underline_height = self.underline_thickness_px();
            if cell.underline {
                fill_rect(
                    &mut self.pixels,
                    self.surface_width_px,
                    self.surface_height_px,
                    PixelRect {
                        start_x: cell_x,
                        start_y: row_y
                            + self
                                .raster_metrics
                                .cell_height
                                .saturating_sub(underline_height + 1),
                        width: span_width_px,
                        height: underline_height,
                    },
                    foreground,
                );
            }

            if cell.text.chars().all(char::is_whitespace) {
                continue;
            }

            let key = self.ensure_cluster_sprite(&cell.text, span, cell.bold);
            let sprite = self
                .sprite_cache
                .get(&key)
                .expect("sprite cache entry must exist after insertion");
            rendered_clusters.push(RenderedCluster {
                row: cell.row,
                col: cell.col,
                text: cell.text.clone(),
                sprite_kind: sprite.kind(),
                source_kind: sprite.source_kind(),
            });
            blit_cached_sprite(
                &mut self.pixels,
                self.surface_width_px,
                self.surface_height_px,
                cell_x,
                row_y,
                sprite,
                foreground,
            );
        }
    }

    fn render_terminal_images(
        &mut self,
        surface: &TerminalSurfaceState,
        matches_layer: impl Fn(i32) -> bool,
    ) {
        let resources = surface
            .image_resources
            .iter()
            .map(|resource| (resource.content_hash, resource.as_ref()))
            .collect::<HashMap<_, _>>();
        let mut placements = surface
            .image_placements
            .iter()
            .filter(|placement| matches_layer(placement.z_index))
            .collect::<Vec<_>>();
        placements.sort_by_key(|placement| (placement.z_index, placement.protocol_order));
        for placement in placements {
            let Some(resource) = resources.get(&placement.resource_key) else {
                continue;
            };
            let Some(draw_rect) = terminal_image_draw_rect(
                placement,
                surface.rows,
                surface.cols,
                self.raster_metrics.cell_width,
                self.raster_metrics.cell_height,
            ) else {
                continue;
            };
            blit_terminal_image(
                &mut self.pixels,
                self.surface_width_px,
                self.surface_height_px,
                resource,
                draw_rect,
            );
        }
    }

    fn ensure_cluster_sprite(&mut self, text: &str, span: u32, bold: bool) -> ClusterKey {
        let key = ClusterKey {
            text: text.to_string(),
            span,
            bold,
        };

        if !self.sprite_cache.contains_key(&key) {
            let sprite = self.rasterize_cluster_sprite(text, span, bold);
            self.sprite_cache.insert(key.clone(), sprite);
        }

        key
    }

    fn rasterize_cluster_sprite(
        &mut self,
        text: &str,
        span: u32,
        bold: bool,
    ) -> CachedClusterSprite {
        if classify_cluster_render_kind(text) == EmojiClusterRenderKind::Emoji {
            match self.emoji_renderer.rasterize_cluster(
                text,
                span,
                self.raster_metrics.cell_width,
                self.raster_metrics.cell_height,
            ) {
                EmojiRenderOutcome::Sprite(sprite) => {
                    return CachedClusterSprite::ColorRgba {
                        width: sprite.width,
                        height: sprite.height,
                        rgba: rgba_pixels_from_bytes(&sprite.rgba),
                    };
                }
                EmojiRenderOutcome::VisibleFallback {
                    replacement_text,
                    reason,
                } => {
                    tracing::warn!(
                        ?reason,
                        text = %text,
                        "terminal emoji rasterization fell back to a visible mono replacement"
                    );
                    return rasterize_mono_cluster_sprite(MonoRasterRequest {
                        font: &self.font,
                        swash_font: self.swash_font,
                        scale_context: &mut self.mono_scale_context,
                        render_profile: self.mono_render_profile,
                        metrics: self.raster_metrics,
                        px_size: TERMINAL_FONT_SIZE_PX * self.raster_scale,
                        text: &replacement_text,
                        span,
                        bold,
                    });
                }
            }
        }

        if let Some(kind) = classify_custom_grid_glyph(text, span) {
            let mask = generate_custom_grid_mask(
                kind,
                self.raster_metrics.cell_width.saturating_mul(span.max(1)),
                self.raster_metrics.cell_height,
                self.raster_scale,
            );
            return CachedClusterSprite::MonoAlpha {
                width: mask.width_px,
                height: mask.height_px,
                alpha: mask.alpha,
                source_kind: RenderedClusterSourceKind::GeneratedMask,
            };
        }

        rasterize_mono_cluster_sprite(MonoRasterRequest {
            font: &self.font,
            swash_font: self.swash_font,
            scale_context: &mut self.mono_scale_context,
            render_profile: self.mono_render_profile,
            metrics: self.raster_metrics,
            px_size: TERMINAL_FONT_SIZE_PX * self.raster_scale,
            text,
            span,
            bold,
        })
    }

    fn record_rendered_clusters_from_cache(
        &mut self,
        cells: &[&TerminalCellState],
        rendered_clusters: &mut Vec<RenderedCluster>,
    ) {
        for cell in cells {
            if cell.text.chars().all(char::is_whitespace) {
                continue;
            }

            let key = self.ensure_cluster_sprite(&cell.text, cell.width.max(1), cell.bold);
            let sprite_kind = self
                .sprite_cache
                .get(&key)
                .map(CachedClusterSprite::kind)
                .unwrap_or(ClusterSpriteKind::MonoAlpha);
            let source_kind = self
                .sprite_cache
                .get(&key)
                .map(CachedClusterSprite::source_kind)
                .unwrap_or(RenderedClusterSourceKind::FontRaster);
            rendered_clusters.push(RenderedCluster {
                row: cell.row,
                col: cell.col,
                text: cell.text.clone(),
                sprite_kind,
                source_kind,
            });
        }
    }

    fn underline_thickness_px(&self) -> u32 {
        self.raster_scale.round().max(1.0) as u32
    }
}

fn compute_terminal_metrics(font: &FontArc, px_size: f32) -> TerminalAtlasMetrics {
    let scaled = font.as_scaled(PxScale::from(px_size));
    let mono_advance = scaled
        .h_advance(scaled.glyph_id('M'))
        .max(scaled.h_advance(scaled.glyph_id('0')))
        .max(scaled.h_advance(scaled.glyph_id('W')))
        .max(scaled.h_advance(scaled.glyph_id('界')) / 2.0);
    let cell_width = terminal_cell_width_px(mono_advance) as u32 + CELL_HORIZONTAL_PADDING_PX;
    let line_height = (scaled.ascent() - scaled.descent() + scaled.line_gap()).ceil() as u32;
    let cell_height = (line_height + CELL_VERTICAL_PADDING_PX).max(MIN_CELL_HEIGHT_PX);
    let top_padding = cell_height.saturating_sub(line_height) / 2;
    let baseline_px = top_padding + scaled.ascent().ceil() as u32;

    TerminalAtlasMetrics {
        cell_width: cell_width.max(MIN_CELL_WIDTH_PX),
        cell_height,
        baseline_px,
    }
}

fn rasterize_mono_cluster_sprite(request: MonoRasterRequest<'_>) -> CachedClusterSprite {
    let MonoRasterRequest {
        font,
        swash_font,
        scale_context,
        metrics,
        px_size,
        text,
        span,
        bold,
        render_profile,
    } = request;
    let width = (span.max(1) * metrics.cell_width) as usize;
    let height = metrics.cell_height as usize;
    let mut alpha = vec![0u8; width * height];
    let scaled = font.as_scaled(PxScale::from(px_size));
    let baseline = metrics.baseline_px as f32;
    let mut pen_x = 0.0f32;
    let mut scaler = scale_context
        .builder(swash_font)
        .size(px_size)
        .hint(true)
        .build();
    let embolden = mono_embolden_strength(bold);
    let mut rendered_glyphs = Vec::new();
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;

    for ch in text.chars() {
        if ch.is_control() {
            continue;
        }

        let glyph_id = scaled.glyph_id(ch);

        let (origin_x, offset_x) = split_fractional_offset(pen_x);
        if let Some(image) =
            render_hinted_mono_glyph(&mut scaler, glyph_id.0, embolden, offset_x, render_profile)
        {
            let left = origin_x + image.left;
            min_x = min_x.min(left as f32);
            max_x = max_x.max(left as f32 + image.width as f32);
            rendered_glyphs.push(RenderedMonoGlyph {
                left,
                top: baseline.round() as i32 - image.top,
                width: image.width,
                height: image.height,
                alpha: image.alpha,
            });
        }

        pen_x += scaled.h_advance(glyph_id);
    }

    if rendered_glyphs.is_empty() {
        return CachedClusterSprite::MonoAlpha {
            width: width as u32,
            height: height as u32,
            alpha,
            source_kind: RenderedClusterSourceKind::FontRaster,
        };
    }

    let offset_x = compute_cluster_offset_x(min_x);

    for glyph in rendered_glyphs {
        blit_alpha_mask(
            &mut alpha,
            width as u32,
            height as u32,
            glyph.left + offset_x.round() as i32,
            glyph.top,
            glyph.width,
            glyph.height,
            &glyph.alpha,
        );
    }

    CachedClusterSprite::MonoAlpha {
        width: width as u32,
        height: height as u32,
        alpha,
        source_kind: RenderedClusterSourceKind::FontRaster,
    }
}

fn mono_embolden_strength(bold: bool) -> f32 {
    if bold {
        BOLD_MONO_EMBOLDEN_STRENGTH
    } else {
        REGULAR_MONO_EMBOLDEN_STRENGTH
    }
}

fn render_hinted_mono_glyph(
    scaler: &mut Scaler<'_>,
    glyph_id: u16,
    embolden: f32,
    offset_x: f32,
    render_profile: FontRenderProfile,
) -> Option<RenderedMonoGlyph> {
    let mut renderer = Render::new(&[Source::Outline]);
    renderer
        .format(SwashFormat::Alpha)
        .offset(SwashVector::new(offset_x, 0.0))
        .embolden(embolden);
    let image = renderer.render(scaler, glyph_id)?;
    if image.placement.width == 0 || image.placement.height == 0 {
        return None;
    }

    let alpha = match image.content {
        SwashContent::Mask => image
            .data
            .into_iter()
            .map(|value| map_glyph_coverage_to_alpha(f32::from(value) / 255.0, render_profile))
            .collect(),
        SwashContent::SubpixelMask => rgba_pixels_from_bytes(&image.data)
            .into_iter()
            .map(|pixel| pixel.g.max(pixel.r).max(pixel.b))
            .map(|value| map_glyph_coverage_to_alpha(f32::from(value) / 255.0, render_profile))
            .collect(),
        _ => return None,
    };

    Some(RenderedMonoGlyph {
        left: image.placement.left,
        top: image.placement.top,
        width: image.placement.width,
        height: image.placement.height,
        alpha,
    })
}

#[allow(clippy::too_many_arguments)]
fn blit_alpha_mask(
    dst: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    dest_x: i32,
    dest_y: i32,
    src_width: u32,
    src_height: u32,
    src_alpha: &[u8],
) {
    let start_x = dest_x.max(0) as u32;
    let start_y = dest_y.max(0) as u32;
    let end_x = (dest_x + src_width as i32).min(dst_width as i32).max(0) as u32;
    let end_y = (dest_y + src_height as i32).min(dst_height as i32).max(0) as u32;

    for y in start_y..end_y {
        let src_y = (y as i32 - dest_y) as usize;
        for x in start_x..end_x {
            let src_x = (x as i32 - dest_x) as usize;
            let src_index = src_y * src_width as usize + src_x;
            let dst_index = y as usize * dst_width as usize + x as usize;
            if let (Some(source), Some(target)) = (src_alpha.get(src_index), dst.get_mut(dst_index))
            {
                *target = (*target).max(*source);
            }
        }
    }
}

fn split_fractional_offset(position: f32) -> (i32, f32) {
    let base = position.floor();
    (base as i32, position - base)
}

fn sanitize_raster_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.max(1.0)
    } else {
        1.0
    }
}

fn scale_terminal_metrics(metrics: TerminalAtlasMetrics, scale: f32) -> TerminalAtlasMetrics {
    TerminalAtlasMetrics {
        cell_width: ((metrics.cell_width as f32) * scale).round().max(1.0) as u32,
        cell_height: ((metrics.cell_height as f32) * scale).round().max(1.0) as u32,
        baseline_px: ((metrics.baseline_px as f32) * scale).round().max(1.0) as u32,
    }
}

fn hash_row(
    cols: u32,
    default_fg_rgba: u32,
    default_bg_rgba: u32,
    viewport_bg_top_rgba: u32,
    viewport_bg_bottom_rgba: u32,
    selection: Option<(u32, u32)>,
    selection_overlay_rgba: u32,
    cells: &[&TerminalCellState],
) -> u64 {
    let mut hasher = DefaultHasher::new();
    cols.hash(&mut hasher);
    default_fg_rgba.hash(&mut hasher);
    default_bg_rgba.hash(&mut hasher);
    viewport_bg_top_rgba.hash(&mut hasher);
    viewport_bg_bottom_rgba.hash(&mut hasher);
    selection.hash(&mut hasher);
    selection_overlay_rgba.hash(&mut hasher);
    for cell in cells {
        cell.col.hash(&mut hasher);
        cell.width.hash(&mut hasher);
        cell.text.hash(&mut hasher);
        cell.bold.hash(&mut hasher);
        cell.underline.hash(&mut hasher);
        cell.fg_rgba.hash(&mut hasher);
        cell.bg_rgba.hash(&mut hasher);
    }
    hasher.finish()
}

fn fill_viewport_background_span(
    pixels: &mut [Rgba8Pixel],
    surface_width_px: u32,
    surface_height_px: u32,
    start_y: u32,
    height: u32,
    top: Rgba8Pixel,
    bottom: Rgba8Pixel,
) {
    let end_y = (start_y + height).min(surface_height_px);

    for y in start_y.min(surface_height_px)..end_y {
        let color = viewport_gradient_color(top, bottom, y, surface_height_px);
        for x in 0..surface_width_px {
            pixels[(y * surface_width_px + x) as usize] = color;
        }
    }
}

fn viewport_gradient_color(
    top: Rgba8Pixel,
    bottom: Rgba8Pixel,
    y: u32,
    total_height: u32,
) -> Rgba8Pixel {
    if total_height <= 1 || top == bottom {
        return top;
    }

    let ratio = y.min(total_height - 1) as f32 / (total_height - 1) as f32;
    lerp_rgba(top, bottom, ratio)
}

fn lerp_rgba(top: Rgba8Pixel, bottom: Rgba8Pixel, ratio: f32) -> Rgba8Pixel {
    fn lerp_channel(start: u8, end: u8, ratio: f32) -> u8 {
        let start = f32::from(start);
        let end = f32::from(end);
        (start + ((end - start) * ratio.clamp(0.0, 1.0))).round() as u8
    }

    Rgba8Pixel {
        a: 255,
        r: lerp_channel(top.r, bottom.r, ratio),
        g: lerp_channel(top.g, bottom.g, ratio),
        b: lerp_channel(top.b, bottom.b, ratio),
    }
}

fn rgba8(color: u32) -> Rgba8Pixel {
    Rgba8Pixel {
        a: ((color >> 24) & 0xff) as u8,
        r: ((color >> 16) & 0xff) as u8,
        g: ((color >> 8) & 0xff) as u8,
        b: (color & 0xff) as u8,
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    pixels: &mut [Rgba8Pixel],
    surface_width_px: u32,
    surface_height_px: u32,
    rect: PixelRect,
    color: Rgba8Pixel,
) {
    let end_x = (rect.start_x + rect.width).min(surface_width_px);
    let end_y = (rect.start_y + rect.height).min(surface_height_px);

    for y in rect.start_y.min(surface_height_px)..end_y {
        for x in rect.start_x.min(surface_width_px)..end_x {
            pixels[(y * surface_width_px + x) as usize] = color;
        }
    }
}

fn blend(dst: &mut Rgba8Pixel, fg: Rgba8Pixel, alpha: u8) {
    let alpha = alpha as u16;
    let inv_alpha = 255 - alpha;

    dst.r = ((fg.r as u16 * alpha + dst.r as u16 * inv_alpha) / 255) as u8;
    dst.g = ((fg.g as u16 * alpha + dst.g as u16 * inv_alpha) / 255) as u8;
    dst.b = ((fg.b as u16 * alpha + dst.b as u16 * inv_alpha) / 255) as u8;
    dst.a = 255;
}

fn composite_color(base: Rgba8Pixel, overlay: Rgba8Pixel) -> Rgba8Pixel {
    let mut color = base;
    blend(&mut color, overlay, overlay.a);
    color
}

fn selection_overlaps_cell(selection: (u32, u32), col: u32, width: u32) -> bool {
    let cell_end = col + width.saturating_sub(1);
    cell_end >= selection.0 && col <= selection.1
}

fn resolve_selected_foreground(
    original_fg: Rgba8Pixel,
    original_bg: Rgba8Pixel,
    default_fg: Rgba8Pixel,
    selected_bg: Rgba8Pixel,
) -> Rgba8Pixel {
    if rgb_triplet(original_fg) == rgb_triplet(original_bg) {
        return selected_bg;
    }

    let candidates = [
        original_fg,
        default_fg,
        Rgba8Pixel {
            a: 255,
            r: 255,
            g: 255,
            b: 255,
        },
        Rgba8Pixel {
            a: 255,
            r: 0,
            g: 0,
            b: 0,
        },
    ];

    candidates
        .into_iter()
        .max_by(|left, right| {
            contrast_ratio(*left, selected_bg)
                .partial_cmp(&contrast_ratio(*right, selected_bg))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(original_fg)
}

fn rgb_triplet(color: Rgba8Pixel) -> (u8, u8, u8) {
    (color.r, color.g, color.b)
}

fn contrast_ratio(fg: Rgba8Pixel, bg: Rgba8Pixel) -> f32 {
    let fg_luma = relative_luminance(fg);
    let bg_luma = relative_luminance(bg);
    let (lighter, darker) = if fg_luma >= bg_luma {
        (fg_luma, bg_luma)
    } else {
        (bg_luma, fg_luma)
    };

    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(color: Rgba8Pixel) -> f32 {
    let channel = |value: u8| {
        let normalized = f32::from(value) / 255.0;
        if normalized <= 0.04045 {
            normalized / 12.92
        } else {
            ((normalized + 0.055) / 1.055).powf(2.4)
        }
    };

    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

fn compute_cluster_offset_x(min_x: f32) -> f32 {
    if !min_x.is_finite() {
        return 0.0;
    }

    // The terminal grid owns horizontal placement. Keep compatibility sprites
    // anchored to their first cell and only shift right when glyph ink would
    // otherwise start left of that cell origin.
    (-min_x).max(0.0)
}

fn blit_cached_sprite(
    pixels: &mut [Rgba8Pixel],
    surface_width_px: u32,
    surface_height_px: u32,
    cell_x: u32,
    row_y: u32,
    sprite: &CachedClusterSprite,
    fg: Rgba8Pixel,
) {
    match sprite {
        CachedClusterSprite::MonoAlpha {
            width,
            height,
            alpha,
            ..
        } => blit_mono_alpha(
            pixels,
            surface_width_px,
            surface_height_px,
            cell_x,
            row_y,
            *width,
            *height,
            alpha,
            fg,
        ),
        CachedClusterSprite::ColorRgba {
            width,
            height,
            rgba,
        } => {
            let start_x = cell_x.min(surface_width_px);
            let start_y = row_y.min(surface_height_px);
            let end_x = (start_x + *width).min(surface_width_px);
            let end_y = (start_y + *height).min(surface_height_px);

            for y in start_y..end_y {
                let sprite_y = (y - start_y) as usize;
                for x in start_x..end_x {
                    let sprite_x = (x - start_x) as usize;
                    let source = rgba[sprite_y * *width as usize + sprite_x];
                    if source.a == 0 {
                        continue;
                    }
                    let pixel = &mut pixels[(y * surface_width_px + x) as usize];
                    blend(pixel, source, source.a);
                }
            }
        }
    }
}

fn blit_terminal_image(
    pixels: &mut [Rgba8Pixel],
    surface_width_px: u32,
    surface_height_px: u32,
    resource: &TerminalImageResource,
    draw_rect: TerminalImageDrawRect,
) {
    let Some(expected_bytes) = usize::try_from(resource.width)
        .ok()
        .and_then(|width| {
            usize::try_from(resource.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return;
    };
    if resource.width == 0
        || resource.height == 0
        || resource.rgba.len() < expected_bytes
        || draw_rect.dest_width_px == 0
        || draw_rect.dest_height_px == 0
    {
        return;
    }

    let uv_width = u64::from(draw_rect.uv.right.saturating_sub(draw_rect.uv.left));
    let uv_height = u64::from(draw_rect.uv.bottom.saturating_sub(draw_rect.uv.top));
    let uv_scale = u64::from(TERMINAL_IMAGE_UV_SCALE);
    for dest_y in 0..draw_rect.dest_height_px {
        let sample_v = u64::from(draw_rect.uv.top)
            + uv_height * u64::from(dest_y.saturating_mul(2).saturating_add(1))
                / u64::from(draw_rect.dest_height_px.saturating_mul(2));
        let source_y = (sample_v * u64::from(resource.height) / uv_scale)
            .min(u64::from(resource.height.saturating_sub(1))) as u32;
        let target_y = draw_rect.dest_y_px.saturating_add(dest_y);
        if target_y >= surface_height_px {
            continue;
        }
        for dest_x in 0..draw_rect.dest_width_px {
            let sample_u = u64::from(draw_rect.uv.left)
                + uv_width * u64::from(dest_x.saturating_mul(2).saturating_add(1))
                    / u64::from(draw_rect.dest_width_px.saturating_mul(2));
            let source_x = (sample_u * u64::from(resource.width) / uv_scale)
                .min(u64::from(resource.width.saturating_sub(1))) as u32;
            let target_x = draw_rect.dest_x_px.saturating_add(dest_x);
            if target_x >= surface_width_px {
                continue;
            }

            let source_index =
                (source_y as usize * resource.width as usize + source_x as usize) * 4;
            let source = Rgba8Pixel {
                r: resource.rgba[source_index],
                g: resource.rgba[source_index + 1],
                b: resource.rgba[source_index + 2],
                a: resource.rgba[source_index + 3],
            };
            if source.a == 0 {
                continue;
            }
            let target = &mut pixels[(target_y * surface_width_px + target_x) as usize];
            blend(target, source, source.a);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_mono_alpha(
    pixels: &mut [Rgba8Pixel],
    surface_width_px: u32,
    surface_height_px: u32,
    cell_x: u32,
    row_y: u32,
    width: u32,
    height: u32,
    alpha: &[u8],
    fg: Rgba8Pixel,
) {
    let start_x = cell_x.min(surface_width_px);
    let start_y = row_y.min(surface_height_px);
    let end_x = (start_x + width).min(surface_width_px);
    let end_y = (start_y + height).min(surface_height_px);

    for y in start_y..end_y {
        let sprite_y = (y - start_y) as usize;
        for x in start_x..end_x {
            let sprite_x = (x - start_x) as usize;
            let alpha = alpha[sprite_y * width as usize + sprite_x];
            if alpha == 0 {
                continue;
            }
            let pixel = &mut pixels[(y * surface_width_px + x) as usize];
            blend(pixel, fg, alpha);
        }
    }
}

fn rgba_pixels_from_bytes(bytes: &[u8]) -> Vec<Rgba8Pixel> {
    bytes
        .chunks_exact(4)
        .map(|chunk| Rgba8Pixel {
            r: chunk[0],
            g: chunk[1],
            b: chunk[2],
            a: chunk[3],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use uuid::Uuid;

    use super::{TerminalAtlasRenderer, compute_cluster_offset_x};
    use crate::app::ssh::runtime::{SurfaceState, TerminalCellState};
    use crate::app::terminal_core::{
        TERMINAL_IMAGE_UV_SCALE, TerminalImagePlacement, TerminalImageResource, TerminalImageUvRect,
    };

    fn image_surface(z_index: i32, alpha: u8) -> SurfaceState {
        let mut surface =
            SurfaceState::from_visible_lines(Uuid::new_v4(), 1, 1, 1, vec!["M".to_string()]);
        surface.default_fg_rgba = 0xffff_ffff;
        surface.default_bg_rgba = 0xff00_0000;
        surface.row_bg_even_rgba = 0xff00_0000;
        surface.row_bg_odd_rgba = 0xff00_0000;
        surface.cells = vec![TerminalCellState {
            row: 0,
            col: 0,
            width: 1,
            text: "M".to_string(),
            bold: false,
            underline: false,
            fg_rgba: 0xffff_ffff,
            bg_rgba: 0xff00_0000,
        }];
        surface.image_resources = vec![Arc::new(TerminalImageResource {
            content_hash: [3; 32],
            width: 1,
            height: 1,
            rgba: Arc::<[u8]>::from([255, 0, 0, alpha]),
        })];
        surface.image_placements = vec![TerminalImagePlacement {
            resource_key: [3; 32],
            row: 0,
            col: 0,
            row_span: 1,
            col_span: 1,
            uv: TerminalImageUvRect {
                left: 0,
                top: 0,
                right: TERMINAL_IMAGE_UV_SCALE,
                bottom: TERMINAL_IMAGE_UV_SCALE,
            },
            padding_left_px: 0,
            padding_top_px: 0,
            padding_right_px: 0,
            padding_bottom_px: 0,
            z_index,
            image_id: None,
            placement_id: None,
            protocol_order: 0,
        }];
        surface
    }

    #[test]
    fn ascii_cluster_offset_keeps_the_cluster_anchored_to_the_cell_origin() {
        let offset = compute_cluster_offset_x(1.0);

        assert_eq!(
            offset, 0.0,
            "bitmap compatibility sprites should stay anchored to the owning cell instead of shifting left to hide right-edge overhang, otherwise cursor and selection geometry drift away from the text grid"
        );
    }

    #[test]
    fn wide_cluster_offset_stays_on_the_first_cell_in_the_span() {
        let offset = compute_cluster_offset_x(0.0);

        assert_eq!(
            offset, 0.0,
            "double-width clusters in the bitmap atlas path should stay anchored to the first cell in their logical span instead of being visually centered"
        );
    }

    #[test]
    fn mixed_cluster_offset_does_not_center_single_cell_punctuation() {
        let offset = compute_cluster_offset_x(0.0);

        assert_eq!(
            offset, 0.0,
            "single-cell mixed clusters should not be optically centered because the terminal grid, not glyph advance, owns horizontal placement"
        );
    }

    #[test]
    fn bitmap_images_alpha_composite_over_the_terminal_surface() {
        let mut renderer = TerminalAtlasRenderer::new().expect("atlas renderer");
        let mut surface = image_surface(0, 128);
        surface.cells[0].text = " ".to_string();
        let frame = renderer.render(&surface).expect("render image frame");
        let pixels = frame.image.to_rgba8().expect("rgba image");

        assert!(
            pixels
                .as_slice()
                .iter()
                .all(|pixel| { pixel.r == 128 && pixel.g == 0 && pixel.b == 0 && pixel.a == 255 })
        );
    }

    #[test]
    fn bitmap_image_z_index_controls_whether_text_is_visible() {
        let mut renderer = TerminalAtlasRenderer::new().expect("atlas renderer");
        let below = renderer
            .render(&image_surface(-1, 255))
            .expect("render below-text image")
            .image
            .to_rgba8()
            .expect("rgba below image");
        let above = renderer
            .render(&image_surface(0, 255))
            .expect("render above-text image")
            .image
            .to_rgba8()
            .expect("rgba above image");

        assert!(
            below
                .as_slice()
                .iter()
                .any(|pixel| pixel.r != 255 || pixel.g != 0 || pixel.b != 0),
            "negative-z images should be composited before terminal glyphs"
        );
        assert!(
            above
                .as_slice()
                .iter()
                .all(|pixel| pixel.r == 255 && pixel.g == 0 && pixel.b == 0),
            "non-negative-z images should be composited after terminal glyphs"
        );
    }
}
