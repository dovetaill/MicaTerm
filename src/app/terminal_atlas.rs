//! Renders the active terminal grid into a single image surface using the bundled Fusion JetBrains Maple Mono atlas font.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use anyhow::{Result, anyhow};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use swash::FontRef as SwashFontRef;
use swash::scale::image::Content as SwashContent;
use swash::scale::{Render, ScaleContext, Scaler, Source};
use swash::zeno::{Format as SwashFormat, Vector as SwashVector};

use crate::app::ssh::runtime::{TerminalCellState, TerminalSurfaceState};
use crate::app::terminal_font::backend::{FontRenderProfile, map_glyph_coverage_to_alpha};
use crate::app::terminal_emoji::{
    ClusterRenderKind as EmojiClusterRenderKind, EmojiRenderOutcome, TerminalEmojiRenderer,
    classify_cluster_render_kind,
};

const FUSION_JETBRAINS_MAPLE_MONO_FONT_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/Fusion-JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf");
const TERMINAL_FONT_SIZE_PX: f32 = 18.0;
const MIN_CELL_WIDTH_PX: u32 = 8;
const MIN_CELL_HEIGHT_PX: u32 = 20;
const CELL_HORIZONTAL_PADDING_PX: u32 = 0;
const CELL_VERTICAL_PADDING_PX: u32 = 0;
const ASCII_LEFT_INSET_PX: f32 = 0.0;
const MIXED_LEFT_INSET_PX: f32 = 0.0;
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
            self.start_col
        } else {
            0
        };
        let end_col = if row == self.end_row {
            self.end_col
        } else {
            cols.saturating_sub(1)
        };

        Some((
            start_col.min(cols.saturating_sub(1)),
            end_col.min(cols.saturating_sub(1)),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedCluster {
    pub row: u32,
    pub col: u32,
    pub text: String,
    pub sprite_kind: ClusterSpriteKind,
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
    cols: u32,
    default_fg_rgba: u32,
    default_bg_rgba: u32,
    row_bg_rgba: u32,
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
}

impl TerminalAtlasRenderer {
    pub fn new() -> Result<Self> {
        Self::with_emoji_renderer(TerminalEmojiRenderer::new())
    }

    pub fn with_emoji_renderer_for_tests(emoji_renderer: TerminalEmojiRenderer) -> Result<Self> {
        Self::with_emoji_renderer(emoji_renderer)
    }

    fn with_emoji_renderer(emoji_renderer: TerminalEmojiRenderer) -> Result<Self> {
        let font = FontArc::try_from_slice(FUSION_JETBRAINS_MAPLE_MONO_FONT_BYTES).map_err(
            |error| anyhow!("failed to load bundled Fusion JetBrains Maple Mono font: {error}"),
        )?;
        let swash_font = SwashFontRef::from_index(FUSION_JETBRAINS_MAPLE_MONO_FONT_BYTES, 0)
            .ok_or_else(|| {
                anyhow!("failed to load bundled Fusion JetBrains Maple Mono font into swash")
            })?;
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
        })
    }

    pub fn metrics(&self) -> TerminalAtlasMetrics {
        self.logical_metrics
    }

    pub fn raster_metrics(&self) -> TerminalAtlasMetrics {
        self.raster_metrics
    }

    pub fn set_raster_scale(&mut self, scale: f32) {
        let next_scale = sanitize_raster_scale(scale);
        if (next_scale - self.raster_scale).abs() < 0.01 {
            return;
        }

        self.raster_scale = next_scale;
        self.raster_metrics = scale_terminal_metrics(self.logical_metrics, next_scale);
        self.sprite_cache.clear();
        self.row_hashes.clear();
        self.pixels.clear();
        self.surface_width_px = 0;
        self.surface_height_px = 0;
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
        let mut rerendered_rows = Vec::new();
        let mut rendered_clusters = Vec::new();
        let resized = self.surface_width_px != width_px || self.surface_height_px != height_px;

        if resized {
            self.surface_width_px = width_px;
            self.surface_height_px = height_px;
            self.pixels = vec![rgba8(surface.row_bg_even_rgba); (width_px * height_px) as usize];
            self.row_hashes = vec![0; surface.rows as usize];
        } else if self.row_hashes.len() != surface.rows as usize {
            self.row_hashes.resize(surface.rows as usize, 0);
        }

        let mut row_cells = vec![Vec::new(); surface.rows as usize];
        for cell in &surface.cells {
            if let Some(row) = row_cells.get_mut(cell.row as usize) {
                row.push(cell);
            }
        }

        for row in 0..surface.rows {
            let row_bg_rgba = row_background_rgba(surface, row);
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
                row_bg_rgba,
                row_selection,
                row_selection_overlay_rgba,
                row_cells[row as usize].as_slice(),
            );
            if resized || self.row_hashes[row as usize] != next_hash {
                self.render_row(
                    RowRenderRequest {
                        row,
                        cols: surface.cols,
                        default_fg_rgba: surface.default_fg_rgba,
                        default_bg_rgba: surface.default_bg_rgba,
                        row_bg_rgba,
                        row_selection,
                        selection_overlay_rgba: row_selection_overlay_rgba,
                    },
                    &row_cells[row as usize],
                    &mut rendered_clusters,
                );
                self.row_hashes[row as usize] = next_hash;
                rerendered_rows.push(row);
            } else {
                self.record_rendered_clusters_from_cache(
                    &row_cells[row as usize],
                    &mut rendered_clusters,
                );
            }
        }

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

    #[allow(clippy::too_many_arguments)]
    fn render_row(
        &mut self,
        request: RowRenderRequest,
        cells: &[&TerminalCellState],
        rendered_clusters: &mut Vec<RenderedCluster>,
    ) {
        let row_y = request.row * self.raster_metrics.cell_height;
        let row_bg = rgba8(request.row_bg_rgba);
        let default_fg = rgba8(request.default_fg_rgba);
        fill_rect(
            &mut self.pixels,
            self.surface_width_px,
            self.surface_height_px,
            PixelRect {
                start_x: 0,
                start_y: row_y,
                width: request.cols * self.raster_metrics.cell_width,
                height: self.raster_metrics.cell_height,
            },
            row_bg,
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
            let cell_bg_rgba = if cell.bg_rgba == request.default_bg_rgba {
                request.row_bg_rgba
            } else {
                cell.bg_rgba
            };
            let cell_bg = rgba8(cell_bg_rgba);
            let background = if selected {
                composite_color(cell_bg, rgba8(request.selection_overlay_rgba))
            } else {
                cell_bg
            };

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
            rendered_clusters.push(RenderedCluster {
                row: cell.row,
                col: cell.col,
                text: cell.text.clone(),
                sprite_kind,
            });
        }
    }

    fn underline_thickness_px(&self) -> u32 {
        self.raster_scale.round().max(1.0) as u32
    }
}

fn row_background_rgba(surface: &TerminalSurfaceState, row: u32) -> u32 {
    if row.is_multiple_of(2) {
        surface.row_bg_even_rgba
    } else {
        surface.row_bg_odd_rgba
    }
}

fn compute_terminal_metrics(font: &FontArc, px_size: f32) -> TerminalAtlasMetrics {
    let scaled = font.as_scaled(PxScale::from(px_size));
    let mono_advance = scaled
        .h_advance(scaled.glyph_id('M'))
        .max(scaled.h_advance(scaled.glyph_id('0')))
        .max(scaled.h_advance(scaled.glyph_id('W')))
        .max(scaled.h_advance(scaled.glyph_id('界')) / 2.0);
    let cell_width = mono_advance.ceil() as u32 + CELL_HORIZONTAL_PADDING_PX;
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
        if let Some(image) = render_hinted_mono_glyph(
            &mut scaler,
            glyph_id.0,
            embolden,
            offset_x,
            render_profile,
        ) {
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
        };
    }

    let content_advance = pen_x.ceil().max(0.0);
    let offset_x = compute_cluster_offset_x(
        classify_cluster_layout(text, span),
        width as u32,
        min_x,
        max_x,
        content_advance,
    );

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
    row_bg_rgba: u32,
    selection: Option<(u32, u32)>,
    selection_overlay_rgba: u32,
    cells: &[&TerminalCellState],
) -> u64 {
    let mut hasher = DefaultHasher::new();
    cols.hash(&mut hasher);
    default_fg_rgba.hash(&mut hasher);
    default_bg_rgba.hash(&mut hasher);
    row_bg_rgba.hash(&mut hasher);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClusterLayout {
    Ascii,
    Wide,
    Mixed,
}

fn classify_cluster_layout(text: &str, span: u32) -> ClusterLayout {
    if span > 1 {
        ClusterLayout::Wide
    } else if text.is_ascii() {
        ClusterLayout::Ascii
    } else {
        ClusterLayout::Mixed
    }
}

fn compute_cluster_offset_x(
    layout: ClusterLayout,
    width_px: u32,
    min_x: f32,
    max_x: f32,
    content_advance: f32,
) -> f32 {
    let width = width_px as f32;
    let left_padding = match layout {
        ClusterLayout::Ascii => ASCII_LEFT_INSET_PX,
        ClusterLayout::Wide => ((width - content_advance).max(0.0) / 2.0).floor(),
        ClusterLayout::Mixed => ((width - content_advance).max(0.0) / 2.0).floor() + MIXED_LEFT_INSET_PX,
    };
    let mut offset_x = left_padding + (-min_x).max(0.0);
    let right_overflow = max_x + offset_x - width;
    if right_overflow > 0.0 {
        offset_x -= right_overflow;
    }
    let left_overflow = min_x + offset_x;
    if left_overflow < 0.0 {
        offset_x -= left_overflow;
    }
    offset_x
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
    use super::{ClusterLayout, compute_cluster_offset_x};

    #[test]
    fn ascii_cluster_offset_clamps_right_overhang_back_inside_the_cell() {
        let offset = compute_cluster_offset_x(ClusterLayout::Ascii, 8, 1.0, 8.4, 7.0);

        assert!(
            1.0 + offset >= -0.001,
            "placement should keep the left edge inside the cell after clamping the glyph bounds"
        );
        assert!(
            8.4 + offset <= 8.001,
            "placement should pull right-overhanging glyph ink back inside the fixed terminal cell instead of letting the right edge get clipped"
        );
        assert!(
            offset < 0.0,
            "a right-overhanging ASCII glyph should shift slightly left when the raw placement would clip the right edge"
        );
    }

    #[test]
    fn wide_cluster_offset_keeps_center_alignment_when_no_clamp_is_needed() {
        let offset = compute_cluster_offset_x(ClusterLayout::Wide, 16, 0.0, 8.0, 8.0);

        assert_eq!(offset, 4.0);
    }
}
