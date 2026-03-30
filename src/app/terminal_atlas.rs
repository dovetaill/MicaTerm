//! Renders the active terminal grid into a single image surface using a Sarasa-backed sprite atlas.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont, point};
use anyhow::{Result, anyhow};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::app::ssh::runtime::{TerminalCellState, TerminalSurfaceState};

const SARASA_FONT_BYTES: &[u8] = include_bytes!("../../ui/fonts/SarasaTermSCNerd-Unhinted.ttf");
const TERMINAL_FONT_SIZE_PX: f32 = 16.0;
const MIN_CELL_WIDTH_PX: u32 = 8;
const CELL_HORIZONTAL_PADDING_PX: u32 = 0;
const CELL_VERTICAL_PADDING_PX: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalAtlasMetrics {
    pub cell_width: u32,
    pub cell_height: u32,
    pub baseline_px: u32,
}

#[derive(Clone, Debug)]
pub struct TerminalSurfaceFrame {
    pub image: Image,
    pub metrics: TerminalAtlasMetrics,
    pub cache_entries: usize,
    pub rerendered_rows: Vec<u32>,
}

pub struct TerminalAtlasRenderer {
    font: FontArc,
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

#[derive(Clone, Debug)]
struct CachedClusterSprite {
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

impl TerminalAtlasRenderer {
    pub fn new() -> Result<Self> {
        let font = FontArc::try_from_slice(SARASA_FONT_BYTES)
            .map_err(|error| anyhow!("failed to load bundled Sarasa terminal font: {error}"))?;
        let metrics = compute_terminal_metrics(&font);
        Ok(Self {
            font,
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
        let width_px = surface.cols.max(1) * self.metrics.cell_width;
        let height_px = surface.rows.max(1) * self.metrics.cell_height;
        let mut rerendered_rows = Vec::new();
        let resized = self.surface_width_px != width_px || self.surface_height_px != height_px;

        if resized {
            self.surface_width_px = width_px;
            self.surface_height_px = height_px;
            self.pixels = vec![rgba8(surface.default_bg_rgba); (width_px * height_px) as usize];
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
            let next_hash =
                hash_row(surface.cols, surface.default_bg_rgba, row_cells[row as usize].as_slice());
            if resized || self.row_hashes[row as usize] != next_hash {
                self.render_row(row, surface.cols, surface.default_bg_rgba, &row_cells[row as usize]);
                self.row_hashes[row as usize] = next_hash;
                rerendered_rows.push(row);
            }
        }

        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width_px, height_px);
        buffer.make_mut_slice().copy_from_slice(self.pixels.as_slice());

        Ok(TerminalSurfaceFrame {
            image: Image::from_rgba8(buffer),
            metrics: self.metrics,
            cache_entries: self.sprite_cache.len(),
            rerendered_rows,
        })
    }

    fn render_row(
        &mut self,
        row: u32,
        cols: u32,
        default_bg_rgba: u32,
        cells: &[&TerminalCellState],
    ) {
        let row_y = row * self.metrics.cell_height;
        let row_bg = rgba8(default_bg_rgba);
        for y in row_y..row_y + self.metrics.cell_height {
            for x in 0..cols * self.metrics.cell_width {
                self.pixels[(y * self.surface_width_px + x) as usize] = row_bg;
            }
        }

        for cell in cells {
            let cell_x = cell.col * self.metrics.cell_width;
            let span = cell.width.max(1);
            let span_width_px = span * self.metrics.cell_width;
            let background = rgba8(cell.bg_rgba);

            for y in row_y..row_y + self.metrics.cell_height {
                for x in cell_x..(cell_x + span_width_px).min(self.surface_width_px) {
                    self.pixels[(y * self.surface_width_px + x) as usize] = background;
                }
            }

            if cell.text.chars().all(char::is_whitespace) {
                continue;
            }

            let key = self.ensure_cluster_sprite(&cell.text, span);
            let sprite = self
                .sprite_cache
                .get(&key)
                .expect("sprite cache entry must exist after insertion");
            blit_sprite(
                &mut self.pixels,
                self.surface_width_px,
                self.surface_height_px,
                cell_x,
                row_y,
                sprite,
                rgba8(cell.fg_rgba),
            );
        }
    }

    fn ensure_cluster_sprite(&mut self, text: &str, span: u32) -> ClusterKey {
        let key = ClusterKey {
            text: text.to_string(),
            span,
        };

        if !self.sprite_cache.contains_key(&key) {
            let sprite = rasterize_cluster_sprite(&self.font, self.metrics, text, span);
            self.sprite_cache.insert(key.clone(), sprite);
        }

        key
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
    let cell_height = line_height + CELL_VERTICAL_PADDING_PX;
    let top_padding = cell_height.saturating_sub(line_height) / 2;
    let baseline_px = top_padding + scaled.ascent().ceil() as u32;

    TerminalAtlasMetrics {
        cell_width: cell_width.max(MIN_CELL_WIDTH_PX),
        cell_height: cell_height.max(TERMINAL_FONT_SIZE_PX.ceil() as u32 + 4),
        baseline_px,
    }
}

fn rasterize_cluster_sprite(
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

        let glyph = glyph_id.with_scale_and_position(PxScale::from(TERMINAL_FONT_SIZE_PX), point(pen_x, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            min_x = min_x.min(bounds.min.x);
            outlined_glyphs.push(outlined);
        }

        pen_x += scaled.h_advance(glyph_id);
        previous_id = Some(glyph_id);
    }

    if outlined_glyphs.is_empty() {
        return CachedClusterSprite {
            width: width as u32,
            height: height as u32,
            alpha,
        };
    }

    let content_advance = pen_x.ceil().max(0.0);
    let left_padding = ((width as f32 - content_advance).max(0.0) / 2.0).floor();
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
            let next_alpha = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
            alpha[mask_index] = alpha[mask_index].max(next_alpha);
        });
    }

    CachedClusterSprite {
        width: width as u32,
        height: height as u32,
        alpha,
    }
}

fn hash_row(cols: u32, default_bg_rgba: u32, cells: &[&TerminalCellState]) -> u64 {
    let mut hasher = DefaultHasher::new();
    cols.hash(&mut hasher);
    default_bg_rgba.hash(&mut hasher);
    for cell in cells {
        cell.col.hash(&mut hasher);
        cell.width.hash(&mut hasher);
        cell.text.hash(&mut hasher);
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

fn blend(dst: &mut Rgba8Pixel, fg: Rgba8Pixel, alpha: u8) {
    let alpha = alpha as u16;
    let inv_alpha = 255 - alpha;

    dst.r = ((fg.r as u16 * alpha + dst.r as u16 * inv_alpha) / 255) as u8;
    dst.g = ((fg.g as u16 * alpha + dst.g as u16 * inv_alpha) / 255) as u8;
    dst.b = ((fg.b as u16 * alpha + dst.b as u16 * inv_alpha) / 255) as u8;
    dst.a = 255;
}

fn blit_sprite(
    pixels: &mut [Rgba8Pixel],
    surface_width_px: u32,
    surface_height_px: u32,
    cell_x: u32,
    row_y: u32,
    sprite: &CachedClusterSprite,
    fg: Rgba8Pixel,
) {
    let start_x = cell_x.min(surface_width_px);
    let start_y = row_y.min(surface_height_px);
    let end_x = (start_x + sprite.width).min(surface_width_px);
    let end_y = (start_y + sprite.height).min(surface_height_px);

    for y in start_y..end_y {
        let sprite_y = (y - start_y) as usize;
        for x in start_x..end_x {
            let sprite_x = (x - start_x) as usize;
            let alpha = sprite.alpha[sprite_y * sprite.width as usize + sprite_x];
            if alpha == 0 {
                continue;
            }
            let pixel = &mut pixels[(y * surface_width_px + x) as usize];
            blend(pixel, fg, alpha);
        }
    }
}
