//! Renders the active terminal grid into a single image surface using a Sarasa-backed sprite atlas.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont, point};
use anyhow::{Result, anyhow};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::app::ssh::runtime::{TerminalCellState, TerminalSurfaceState};
use crate::app::terminal_emoji::{
    ClusterRenderKind as EmojiClusterRenderKind, EmojiRenderOutcome, TerminalEmojiRenderer,
    classify_cluster_render_kind,
};
use crate::app::terminal_font::backend::{
    apply_synthetic_embolden, map_glyph_coverage_to_alpha,
};

const SARASA_FONT_BYTES: &[u8] = include_bytes!("../../ui/fonts/SarasaTermSCNerd-Regular.ttf");
const TERMINAL_FONT_SIZE_PX: f32 = 18.0;
const MIN_CELL_WIDTH_PX: u32 = 8;
const MIN_CELL_HEIGHT_PX: u32 = 20;
const CELL_HORIZONTAL_PADDING_PX: u32 = 0;
const CELL_VERTICAL_PADDING_PX: u32 = 0;
const ASCII_LEFT_INSET_PX: f32 = 0.0;
const MIXED_LEFT_INSET_PX: f32 = 0.0;

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
    emoji_renderer: TerminalEmojiRenderer,
    metrics: TerminalAtlasMetrics,
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
        let font = FontArc::try_from_slice(SARASA_FONT_BYTES)
            .map_err(|error| anyhow!("failed to load bundled Sarasa terminal font: {error}"))?;
        let metrics = compute_terminal_metrics(&font);
        Ok(Self {
            font,
            emoji_renderer,
            metrics,
            sprite_cache: HashMap::new(),
            row_hashes: Vec::new(),
            pixels: Vec::new(),
            surface_width_px: 0,
            surface_height_px: 0,
        })
    }

    pub fn metrics(&self) -> TerminalAtlasMetrics {
        self.metrics
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
        let width_px = surface.cols.max(1) * self.metrics.cell_width;
        let height_px = surface.rows.max(1) * self.metrics.cell_height;
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
            metrics: self.metrics,
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
        let row_y = request.row * self.metrics.cell_height;
        let row_bg = rgba8(request.row_bg_rgba);
        let default_fg = rgba8(request.default_fg_rgba);
        fill_rect(
            &mut self.pixels,
            self.surface_width_px,
            self.surface_height_px,
            PixelRect {
                start_x: 0,
                start_y: row_y,
                width: request.cols * self.metrics.cell_width,
                height: self.metrics.cell_height,
            },
            row_bg,
        );

        if let Some((start_col, end_col)) = request.row_selection {
            fill_rect(
                &mut self.pixels,
                self.surface_width_px,
                self.surface_height_px,
                PixelRect {
                    start_x: start_col * self.metrics.cell_width,
                    start_y: row_y,
                    width: (end_col - start_col + 1) * self.metrics.cell_width,
                    height: self.metrics.cell_height,
                },
                composite_color(row_bg, rgba8(request.selection_overlay_rgba)),
            );
        }

        for cell in cells {
            let cell_x = cell.col * self.metrics.cell_width;
            let span = cell.width.max(1);
            let span_width_px = span * self.metrics.cell_width;
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
                    height: self.metrics.cell_height,
                },
                background,
            );

            let foreground = if selected {
                resolve_selected_foreground(rgba8(cell.fg_rgba), cell_bg, default_fg, background)
            } else {
                rgba8(cell.fg_rgba)
            };
            if cell.underline {
                fill_rect(
                    &mut self.pixels,
                    self.surface_width_px,
                    self.surface_height_px,
                    PixelRect {
                        start_x: cell_x,
                        start_y: row_y + self.metrics.cell_height.saturating_sub(2),
                        width: span_width_px,
                        height: 1,
                    },
                    foreground,
                );
            }

            if cell.text.chars().all(char::is_whitespace) {
                continue;
            }

            let key = self.ensure_cluster_sprite(&cell.text, span);
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
            if cell.bold
                && let CachedClusterSprite::MonoAlpha {
                    width,
                    height,
                    alpha,
                } = sprite
            {
                let mut bold_alpha = alpha.clone();
                apply_synthetic_embolden(&mut bold_alpha, *width, *height);
                blit_mono_alpha(
                    &mut self.pixels,
                    self.surface_width_px,
                    self.surface_height_px,
                    cell_x,
                    row_y,
                    *width,
                    *height,
                    &bold_alpha,
                    foreground,
                );
                continue;
            }

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

    fn ensure_cluster_sprite(&mut self, text: &str, span: u32) -> ClusterKey {
        let key = ClusterKey {
            text: text.to_string(),
            span,
        };

        if !self.sprite_cache.contains_key(&key) {
            let sprite = self.rasterize_cluster_sprite(text, span);
            self.sprite_cache.insert(key.clone(), sprite);
        }

        key
    }

    fn rasterize_cluster_sprite(&self, text: &str, span: u32) -> CachedClusterSprite {
        if classify_cluster_render_kind(text) == EmojiClusterRenderKind::Emoji {
            match self.emoji_renderer.rasterize_cluster(
                text,
                span,
                self.metrics.cell_width,
                self.metrics.cell_height,
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
                    return rasterize_mono_cluster_sprite(
                        &self.font,
                        self.metrics,
                        &replacement_text,
                        span,
                    );
                }
            }
        }

        rasterize_mono_cluster_sprite(&self.font, self.metrics, text, span)
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

            let key = self.ensure_cluster_sprite(&cell.text, cell.width.max(1));
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
}

fn row_background_rgba(surface: &TerminalSurfaceState, row: u32) -> u32 {
    if row.is_multiple_of(2) {
        surface.row_bg_even_rgba
    } else {
        surface.row_bg_odd_rgba
    }
}

fn compute_terminal_metrics(font: &FontArc) -> TerminalAtlasMetrics {
    let scaled = font.as_scaled(PxScale::from(TERMINAL_FONT_SIZE_PX));
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

fn rasterize_mono_cluster_sprite(
    font: &FontArc,
    metrics: TerminalAtlasMetrics,
    text: &str,
    span: u32,
) -> CachedClusterSprite {
    let width = (span.max(1) * metrics.cell_width) as usize;
    let height = metrics.cell_height as usize;
    let mut alpha = vec![0u8; width * height];
    let scaled = font.as_scaled(PxScale::from(TERMINAL_FONT_SIZE_PX));
    let baseline = metrics.baseline_px as f32;
    let mut pen_x = 0.0f32;
    let mut previous_id = None::<GlyphId>;
    let mut outlined_glyphs = Vec::new();
    let mut min_x = f32::MAX;

    for ch in text.chars() {
        if ch.is_control() {
            continue;
        }

        let glyph_id = scaled.glyph_id(ch);
        if let Some(prev) = previous_id {
            pen_x += scaled.kern(prev, glyph_id);
        }

        let glyph = glyph_id
            .with_scale_and_position(PxScale::from(TERMINAL_FONT_SIZE_PX), point(pen_x, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            min_x = min_x.min(bounds.min.x);
            outlined_glyphs.push(outlined);
        }

        pen_x += scaled.h_advance(glyph_id);
        previous_id = Some(glyph_id);
    }

    if outlined_glyphs.is_empty() {
        return CachedClusterSprite::MonoAlpha {
            width: width as u32,
            height: height as u32,
            alpha,
        };
    }

    let content_advance = pen_x.ceil().max(0.0);
    let left_padding = match classify_cluster_layout(text, span) {
        ClusterLayout::Ascii => ASCII_LEFT_INSET_PX,
        ClusterLayout::Wide => ((width as f32 - content_advance).max(0.0) / 2.0).floor(),
        ClusterLayout::Mixed => {
            ((width as f32 - content_advance).max(0.0) / 2.0).floor() + MIXED_LEFT_INSET_PX
        }
    };
    let offset_x = left_padding + (-min_x).max(0.0);

    for outlined in outlined_glyphs {
        let bounds = outlined.px_bounds();
        let glyph_origin_x = (offset_x + bounds.min.x).round() as i32;
        let glyph_origin_y = bounds.min.y.round() as i32;
        outlined.draw(|x, y, coverage| {
            let target_x = glyph_origin_x + x as i32;
            let target_y = glyph_origin_y + y as i32;
            if !(0..width as i32).contains(&target_x) || !(0..height as i32).contains(&target_y) {
                return;
            }

            let mask_index = target_y as usize * width + target_x as usize;
            let next_alpha = map_glyph_coverage_to_alpha(coverage);
            alpha[mask_index] = alpha[mask_index].max(next_alpha);
        });
    }
    apply_synthetic_embolden(&mut alpha, width as u32, height as u32);

    CachedClusterSprite::MonoAlpha {
        width: width as u32,
        height: height as u32,
        alpha,
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
