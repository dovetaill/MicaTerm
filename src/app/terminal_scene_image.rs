//! Scene-owned terminal image renderer for Windows software compatibility builds and the
//! `MICA_TERM_TERMINAL_SUBSYSTEM=scene-image` rollback path inside Slint z-order.

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::Hasher;

use anyhow::{Result, anyhow};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::app::terminal_presenter::{
    BitmapTerminalFrame, NativeImePreviewOverlay, NativeSelectionOverlay, NativeTerminalFrame,
    NativeUnderlineOverlay,
};
use crate::app::terminal_renderer::wgpu_renderer::{
    PreparedColorGlyphDraw, PreparedColorGlyphUploadPayload, PreparedMonochromeGlyphDraw,
};

#[derive(Default)]
pub struct SceneImageTerminalRenderer {
    monochrome_glyph_cache: HashMap<u32, CachedMonochromeGlyph>,
    color_glyph_cache: HashMap<u32, CachedColorGlyph>,
    last_base_fingerprint: Option<u64>,
    last_base_pixels: Option<Vec<Rgba8Pixel>>,
    last_base_bitmap_frame: Option<BitmapTerminalFrame>,
    last_bitmap_fingerprint: Option<u64>,
    last_bitmap_frame: Option<BitmapTerminalFrame>,
    base_render_count: usize,
    bitmap_render_count: usize,
    working_resize_count: usize,
    working_pixels: Vec<Rgba8Pixel>,
}

#[derive(Clone)]
struct CachedMonochromeGlyph {
    width_px: u32,
    height_px: u32,
    coverage: Vec<u8>,
}

#[derive(Clone)]
struct CachedColorGlyph {
    width_px: u32,
    height_px: u32,
    pixels: Vec<Rgba8Pixel>,
}

#[derive(Clone, Copy)]
struct PixelRect {
    start_x: u32,
    start_y: u32,
    width: u32,
    height: u32,
}

struct PixelSurface<'a> {
    pixels: &'a mut [Rgba8Pixel],
    width_px: u32,
    height_px: u32,
}

impl SceneImageTerminalRenderer {
    pub fn clear(&mut self) {
        self.monochrome_glyph_cache.clear();
        self.color_glyph_cache.clear();
        self.last_base_fingerprint = None;
        self.last_base_pixels = None;
        self.last_base_bitmap_frame = None;
        self.last_bitmap_fingerprint = None;
        self.last_bitmap_frame = None;
    }

    pub fn base_render_count(&self) -> usize {
        self.base_render_count
    }

    pub fn bitmap_render_count(&self) -> usize {
        self.bitmap_render_count
    }

    pub fn working_resize_count(&self) -> usize {
        self.working_resize_count
    }

    pub fn render(&mut self, frame: &NativeTerminalFrame) -> Result<BitmapTerminalFrame> {
        let base_fingerprint = self.fingerprint_base_frame(frame)?;
        let overlay_fingerprint = self.fingerprint_overlay_frame(frame);
        let bitmap_fingerprint = combine_fingerprints(base_fingerprint, overlay_fingerprint);
        let overlay_is_noop = overlay_composition_is_noop(frame);
        if self.last_bitmap_fingerprint == Some(bitmap_fingerprint)
            && let Some(bitmap_frame) = &self.last_bitmap_frame
        {
            return Ok(bitmap_frame.clone());
        }

        let width_px = frame
            .presentable_frame
            .grid_cols
            .saturating_mul(frame.cell_width_px);
        let height_px = frame
            .presentable_frame
            .grid_rows
            .saturating_mul(frame.cell_height_px);

        if width_px == 0 || height_px == 0 {
            let bitmap_frame = BitmapTerminalFrame {
                image: Image::default(),
                grid_rows: frame.presentable_frame.grid_rows,
                grid_cols: frame.presentable_frame.grid_cols,
                cell_width_px: frame.cell_width_px,
                cell_height_px: frame.cell_height_px,
            };
            self.last_base_bitmap_frame = Some(bitmap_frame.clone());
            self.last_bitmap_fingerprint = Some(bitmap_fingerprint);
            self.last_bitmap_frame = Some(bitmap_frame.clone());
            return Ok(bitmap_frame);
        }

        let pixels = if self.last_base_fingerprint == Some(base_fingerprint) {
            if let Some(pixels) = &self.last_base_pixels {
                pixels.clone()
            } else {
                let pixels = self.render_base_pixels(frame)?;
                self.base_render_count = self.base_render_count.saturating_add(1);
                self.last_base_pixels = Some(pixels.clone());
                pixels
            }
        } else {
            let pixels = self.render_base_pixels(frame)?;
            self.base_render_count = self.base_render_count.saturating_add(1);
            self.last_base_fingerprint = Some(base_fingerprint);
            self.last_base_pixels = Some(pixels.clone());
            self.last_base_bitmap_frame = None;
            pixels
        };

        if overlay_is_noop {
            if let Some(base_frame) = &self.last_base_bitmap_frame {
                self.last_bitmap_fingerprint = Some(bitmap_fingerprint);
                self.last_bitmap_frame = Some(base_frame.clone());
                return Ok(base_frame.clone());
            }

            let bitmap_frame = self.bitmap_frame_from_pixels(frame, &pixels);
            self.bitmap_render_count = self.bitmap_render_count.saturating_add(1);
            self.last_base_bitmap_frame = Some(bitmap_frame.clone());
            self.last_bitmap_fingerprint = Some(bitmap_fingerprint);
            self.last_bitmap_frame = Some(bitmap_frame.clone());
            return Ok(bitmap_frame);
        }

        let mut pixels = std::mem::take(&mut self.working_pixels);
        let pixel_count = (width_px * height_px) as usize;
        if pixels.len() != pixel_count {
            pixels.resize(pixel_count, rgba8(frame.presentable_frame.default_bg_rgba));
            self.working_resize_count = self.working_resize_count.saturating_add(1);
        }
        if let Some(base_pixels) = &self.last_base_pixels {
            pixels.copy_from_slice(base_pixels);
        }

        {
            let mut surface = PixelSurface {
                pixels: &mut pixels,
                width_px,
                height_px,
            };
            self.draw_selection_overlay(&mut surface, frame);
            self.draw_underline_overlay(&mut surface, frame);
            self.draw_ime_preview_overlay(&mut surface, frame);
        }

        let bitmap_frame = self.bitmap_frame_from_pixels(frame, &pixels);
        self.bitmap_render_count = self.bitmap_render_count.saturating_add(1);
        self.working_pixels = pixels;
        self.last_bitmap_fingerprint = Some(bitmap_fingerprint);
        self.last_bitmap_frame = Some(bitmap_frame.clone());

        Ok(bitmap_frame)
    }

    fn fingerprint_base_frame(&mut self, frame: &NativeTerminalFrame) -> Result<u64> {
        let mut hasher = DefaultHasher::new();
        let presentable = &frame.presentable_frame;

        hash_u32(&mut hasher, frame.cell_width_px);
        hash_u32(&mut hasher, frame.cell_height_px);
        hash_u32(&mut hasher, presentable.grid_rows);
        hash_u32(&mut hasher, presentable.grid_cols);
        hash_u32(&mut hasher, presentable.default_fg_rgba);
        hash_u32(&mut hasher, presentable.default_bg_rgba);
        hash_u32(&mut hasher, presentable.row_bg_even_rgba);
        hash_u32(&mut hasher, presentable.row_bg_odd_rgba);

        hash_usize(&mut hasher, presentable.background_runs.len());
        for run in &presentable.background_runs {
            hash_background_run(&mut hasher, run);
        }

        hash_usize(&mut hasher, presentable.monochrome_glyph_draws.len());
        for draw in &presentable.monochrome_glyph_draws {
            let glyph = self.resolve_monochrome_glyph(draw)?;
            hash_monochrome_glyph_draw(&mut hasher, draw, &glyph);
        }

        hash_usize(&mut hasher, presentable.color_glyph_draws.len());
        for draw in &presentable.color_glyph_draws {
            let glyph = self.resolve_color_glyph(draw)?;
            hash_color_glyph_draw(&mut hasher, draw, &glyph);
        }

        Ok(hasher.finish())
    }

    fn fingerprint_overlay_frame(&self, frame: &NativeTerminalFrame) -> u64 {
        let mut hasher = DefaultHasher::new();
        let presentable = &frame.presentable_frame;

        hash_u32(&mut hasher, presentable.default_fg_rgba);
        hash_selection_overlay(&mut hasher, &presentable.selection_overlay);
        hash_underline_overlay(&mut hasher, &presentable.underline_overlay);
        hash_ime_preview_overlay(&mut hasher, presentable.ime_preview_overlay);

        hasher.finish()
    }

    fn render_base_pixels(&mut self, frame: &NativeTerminalFrame) -> Result<Vec<Rgba8Pixel>> {
        let width_px = frame
            .presentable_frame
            .grid_cols
            .saturating_mul(frame.cell_width_px);
        let height_px = frame
            .presentable_frame
            .grid_rows
            .saturating_mul(frame.cell_height_px);
        let mut pixels =
            vec![rgba8(frame.presentable_frame.default_bg_rgba); (width_px * height_px) as usize];

        {
            let mut surface = PixelSurface {
                pixels: &mut pixels,
                width_px,
                height_px,
            };
            self.draw_row_backgrounds(&mut surface, frame);
            self.draw_background_runs(&mut surface, frame);
            self.draw_monochrome_glyphs(&mut surface, frame)?;
            self.draw_color_glyphs(&mut surface, frame)?;
        }

        Ok(pixels)
    }

    fn bitmap_frame_from_pixels(
        &self,
        frame: &NativeTerminalFrame,
        pixels: &[Rgba8Pixel],
    ) -> BitmapTerminalFrame {
        let width_px = frame
            .presentable_frame
            .grid_cols
            .saturating_mul(frame.cell_width_px);
        let height_px = frame
            .presentable_frame
            .grid_rows
            .saturating_mul(frame.cell_height_px);
        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width_px, height_px);
        buffer.make_mut_slice().copy_from_slice(pixels);

        BitmapTerminalFrame {
            image: Image::from_rgba8(buffer),
            grid_rows: frame.presentable_frame.grid_rows,
            grid_cols: frame.presentable_frame.grid_cols,
            cell_width_px: frame.cell_width_px,
            cell_height_px: frame.cell_height_px,
        }
    }

    fn draw_row_backgrounds(
        &self,
        surface: &mut PixelSurface<'_>,
        frame: &NativeTerminalFrame,
    ) {
        for row in 0..frame.presentable_frame.grid_rows {
            let color = if row % 2 == 0 {
                rgba8(frame.presentable_frame.row_bg_even_rgba)
            } else {
                rgba8(frame.presentable_frame.row_bg_odd_rgba)
            };
            fill_rect(
                surface,
                PixelRect {
                    start_x: 0,
                    start_y: row.saturating_mul(frame.cell_height_px),
                    width: surface.width_px,
                    height: frame.cell_height_px,
                },
                color,
            );
        }
    }

    fn draw_background_runs(
        &self,
        surface: &mut PixelSurface<'_>,
        frame: &NativeTerminalFrame,
    ) {
        for run in &frame.presentable_frame.background_runs {
            if let Some(rect) = cell_span_rect(
                run.row,
                run.start_col,
                run.end_col,
                frame.cell_width_px,
                frame.cell_height_px,
            ) {
                fill_rect(
                    surface,
                    rect,
                    rgba8(run.bg_rgba),
                );
            }
        }
    }

    fn draw_selection_overlay(
        &self,
        surface: &mut PixelSurface<'_>,
        frame: &NativeTerminalFrame,
    ) {
        if !frame.presentable_frame.selection_overlay.active {
            return;
        }

        for rect in &frame.presentable_frame.selection_overlay.rects {
            if let Some(pixel_rect) = cell_span_rect(
                rect.row,
                rect.start_col,
                rect.end_col,
                frame.cell_width_px,
                frame.cell_height_px,
            ) {
                fill_rect_alpha(
                    surface,
                    pixel_rect,
                    rgba8(rect.overlay_rgba),
                );
            }
        }
    }

    fn draw_monochrome_glyphs(
        &mut self,
        surface: &mut PixelSurface<'_>,
        frame: &NativeTerminalFrame,
    ) -> Result<()> {
        for draw in &frame.presentable_frame.monochrome_glyph_draws {
            let glyph = self.resolve_monochrome_glyph(draw)?;
            let clip_rect = row_clip_rect(
                draw.row,
                frame.presentable_frame.grid_cols,
                frame.cell_width_px,
                frame.cell_height_px,
            );
            blit_monochrome_glyph(
                surface,
                &glyph,
                draw.dest_x_px,
                draw.dest_y_px,
                rgba8(draw.fg_rgba),
                clip_rect,
            );
        }

        Ok(())
    }

    fn draw_color_glyphs(
        &mut self,
        surface: &mut PixelSurface<'_>,
        frame: &NativeTerminalFrame,
    ) -> Result<()> {
        for draw in &frame.presentable_frame.color_glyph_draws {
            let glyph = self.resolve_color_glyph(draw)?;
            let clip_rect = row_clip_rect(
                draw.row,
                frame.presentable_frame.grid_cols,
                frame.cell_width_px,
                frame.cell_height_px,
            );
            blit_color_glyph(
                surface,
                &glyph,
                draw.dest_x_px,
                draw.dest_y_px,
                clip_rect,
            );
        }

        Ok(())
    }

    fn draw_underline_overlay(
        &self,
        surface: &mut PixelSurface<'_>,
        frame: &NativeTerminalFrame,
    ) {
        if !frame.presentable_frame.underline_overlay.visible {
            return;
        }

        let thickness_px = underline_thickness(frame.cell_height_px);
        for run in &frame.presentable_frame.underline_overlay.runs {
            if let Some(mut rect) = cell_span_rect(
                run.row,
                run.start_col,
                run.end_col,
                frame.cell_width_px,
                frame.cell_height_px,
            ) {
                rect.start_y = rect.start_y.saturating_add(rect.height.saturating_sub(thickness_px));
                rect.height = thickness_px;
                fill_rect(
                    surface,
                    rect,
                    rgba8(run.fg_rgba),
                );
            }
        }
    }

    fn draw_ime_preview_overlay(
        &self,
        surface: &mut PixelSurface<'_>,
        frame: &NativeTerminalFrame,
    ) {
        let ime = frame.presentable_frame.ime_preview_overlay;
        if !ime.active {
            return;
        }

        let preview_rgba =
            (0x44_u32 << 24) | (frame.presentable_frame.default_fg_rgba & 0x00ff_ffff);
        if let Some(rect) = cell_span_rect(
            ime.row,
            ime.start_col,
            ime.end_col,
            frame.cell_width_px,
            frame.cell_height_px,
        ) {
            fill_rect_alpha(
                surface,
                rect,
                rgba8(preview_rgba),
            );
        }
        if let Some(rect) = ime_cursor_rect(
            ime.row,
            ime.cursor_col,
            frame.cell_width_px,
            frame.cell_height_px,
        ) {
            fill_rect(
                surface,
                rect,
                rgba8(frame.presentable_frame.default_fg_rgba),
            );
        }
    }

    fn resolve_monochrome_glyph(
        &mut self,
        draw: &PreparedMonochromeGlyphDraw,
    ) -> Result<CachedMonochromeGlyph> {
        if let Some(upload) = &draw.upload {
            let glyph = CachedMonochromeGlyph {
                width_px: upload.width_px,
                height_px: upload.height_px,
                coverage: upload.coverage.clone(),
            };
            self.monochrome_glyph_cache
                .insert(draw.atlas_entry.slot, glyph.clone());
            return Ok(glyph);
        }

        self.monochrome_glyph_cache
            .get(&draw.atlas_entry.slot)
            .cloned()
            .ok_or_else(|| anyhow!("missing cached monochrome glyph for slot {}", draw.atlas_entry.slot))
    }

    fn resolve_color_glyph(&mut self, draw: &PreparedColorGlyphDraw) -> Result<CachedColorGlyph> {
        if let Some(upload) = &draw.upload {
            let glyph = CachedColorGlyph {
                width_px: upload.width_px,
                height_px: upload.height_px,
                pixels: rgba_pixels_from_upload(upload),
            };
            self.color_glyph_cache
                .insert(draw.cache_entry.slot, glyph.clone());
            return Ok(glyph);
        }

        self.color_glyph_cache
            .get(&draw.cache_entry.slot)
            .cloned()
            .ok_or_else(|| anyhow!("missing cached color glyph for slot {}", draw.cache_entry.slot))
    }
}

fn rgba_pixels_from_upload(upload: &PreparedColorGlyphUploadPayload) -> Vec<Rgba8Pixel> {
    upload
        .rgba
        .chunks_exact(4)
        .map(|rgba| Rgba8Pixel {
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
            a: rgba[3],
        })
        .collect()
}

fn rgba8(color: u32) -> Rgba8Pixel {
    Rgba8Pixel {
        a: ((color >> 24) & 0xff) as u8,
        r: ((color >> 16) & 0xff) as u8,
        g: ((color >> 8) & 0xff) as u8,
        b: (color & 0xff) as u8,
    }
}

fn fill_rect(surface: &mut PixelSurface<'_>, rect: PixelRect, color: Rgba8Pixel) {
    let end_x = (rect.start_x + rect.width).min(surface.width_px);
    let end_y = (rect.start_y + rect.height).min(surface.height_px);

    for y in rect.start_y.min(surface.height_px)..end_y {
        for x in rect.start_x.min(surface.width_px)..end_x {
            surface.pixels[(y * surface.width_px + x) as usize] = color;
        }
    }
}

fn fill_rect_alpha(surface: &mut PixelSurface<'_>, rect: PixelRect, color: Rgba8Pixel) {
    let end_x = (rect.start_x + rect.width).min(surface.width_px);
    let end_y = (rect.start_y + rect.height).min(surface.height_px);

    for y in rect.start_y.min(surface.height_px)..end_y {
        for x in rect.start_x.min(surface.width_px)..end_x {
            blend(&mut surface.pixels[(y * surface.width_px + x) as usize], color, color.a);
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

fn blit_monochrome_glyph(
    surface: &mut PixelSurface<'_>,
    glyph: &CachedMonochromeGlyph,
    dest_x_px: i32,
    dest_y_px: i32,
    color: Rgba8Pixel,
    clip_rect: Option<PixelRect>,
) {
    for source_y in 0..glyph.height_px {
        let target_y = dest_y_px.saturating_add(source_y as i32);
        if target_y < 0 || target_y >= surface.height_px as i32 {
            continue;
        }

        for source_x in 0..glyph.width_px {
            let target_x = dest_x_px.saturating_add(source_x as i32);
            if target_x < 0 || target_x >= surface.width_px as i32 {
                continue;
            }
            if !pixel_in_clip_rect(target_x as u32, target_y as u32, clip_rect) {
                continue;
            }

            let source_index = (source_y * glyph.width_px + source_x) as usize;
            if let Some(alpha) = glyph.coverage.get(source_index).copied()
                && alpha > 0
            {
                let dest_index = (target_y as u32 * surface.width_px + target_x as u32) as usize;
                blend(&mut surface.pixels[dest_index], color, alpha);
            }
        }
    }
}

fn blit_color_glyph(
    surface: &mut PixelSurface<'_>,
    glyph: &CachedColorGlyph,
    dest_x_px: i32,
    dest_y_px: i32,
    clip_rect: Option<PixelRect>,
) {
    for source_y in 0..glyph.height_px {
        let target_y = dest_y_px.saturating_add(source_y as i32);
        if target_y < 0 || target_y >= surface.height_px as i32 {
            continue;
        }

        for source_x in 0..glyph.width_px {
            let target_x = dest_x_px.saturating_add(source_x as i32);
            if target_x < 0 || target_x >= surface.width_px as i32 {
                continue;
            }
            if !pixel_in_clip_rect(target_x as u32, target_y as u32, clip_rect) {
                continue;
            }

            let source_index = (source_y * glyph.width_px + source_x) as usize;
            if let Some(color) = glyph.pixels.get(source_index).copied()
                && color.a > 0
            {
                let dest_index = (target_y as u32 * surface.width_px + target_x as u32) as usize;
                blend(&mut surface.pixels[dest_index], color, color.a);
            }
        }
    }
}

fn pixel_in_clip_rect(x: u32, y: u32, clip_rect: Option<PixelRect>) -> bool {
    let Some(clip_rect) = clip_rect else {
        return true;
    };

    x >= clip_rect.start_x
        && y >= clip_rect.start_y
        && x < clip_rect.start_x.saturating_add(clip_rect.width)
        && y < clip_rect.start_y.saturating_add(clip_rect.height)
}

fn cell_span_rect(
    row: u32,
    start_col: u32,
    end_col: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> Option<PixelRect> {
    if cell_width_px == 0 || cell_height_px == 0 || end_col < start_col {
        return None;
    }

    Some(PixelRect {
        start_x: start_col.saturating_mul(cell_width_px),
        start_y: row.saturating_mul(cell_height_px),
        width: end_col
            .saturating_sub(start_col)
            .saturating_add(1)
            .saturating_mul(cell_width_px),
        height: cell_height_px,
    })
}

fn row_clip_rect(
    row: u32,
    grid_cols: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> Option<PixelRect> {
    if grid_cols == 0 {
        return None;
    }

    cell_span_rect(
        row,
        0,
        grid_cols.saturating_sub(1),
        cell_width_px,
        cell_height_px,
    )
}

fn underline_thickness(cell_height_px: u32) -> u32 {
    (cell_height_px / 12).max(1)
}

fn ime_cursor_rect(
    row: u32,
    cursor_col: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> Option<PixelRect> {
    let mut rect = cell_span_rect(row, cursor_col, cursor_col, cell_width_px, cell_height_px)?;
    rect.width = (cell_width_px.max(1) / 10).max(1);
    Some(rect)
}

fn hash_background_run(hasher: &mut impl Hasher, run: &crate::app::terminal_renderer::wgpu_renderer::PreparedBackgroundRun) {
    hash_u32(hasher, run.row);
    hash_u32(hasher, run.start_col);
    hash_u32(hasher, run.end_col);
    hash_u32(hasher, run.bg_rgba);
}

fn hash_selection_overlay(hasher: &mut impl Hasher, overlay: &NativeSelectionOverlay) {
    hash_bool(hasher, overlay.active);
    if !overlay.active {
        return;
    }

    hash_u32(hasher, overlay.start_row);
    hash_u32(hasher, overlay.start_col);
    hash_u32(hasher, overlay.end_row);
    hash_u32(hasher, overlay.end_col);
    hash_u32(hasher, overlay.overlay_rgba);
    hash_usize(hasher, overlay.rects.len());
    for rect in &overlay.rects {
        hash_u32(hasher, rect.row);
        hash_u32(hasher, rect.start_col);
        hash_u32(hasher, rect.end_col);
        hash_u32(hasher, rect.overlay_rgba);
    }
}

fn hash_underline_overlay(hasher: &mut impl Hasher, overlay: &NativeUnderlineOverlay) {
    hash_bool(hasher, overlay.visible);
    if !overlay.visible {
        return;
    }

    hash_usize(hasher, overlay.runs.len());
    for run in &overlay.runs {
        hash_u32(hasher, run.row);
        hash_u32(hasher, run.start_col);
        hash_u32(hasher, run.end_col);
        hash_u32(hasher, run.fg_rgba);
    }
}

fn hash_ime_preview_overlay(hasher: &mut impl Hasher, overlay: NativeImePreviewOverlay) {
    hash_bool(hasher, overlay.active);
    if !overlay.active {
        return;
    }

    hash_u32(hasher, overlay.row);
    hash_u32(hasher, overlay.start_col);
    hash_u32(hasher, overlay.end_col);
    hash_u32(hasher, overlay.cursor_col);
}

fn hash_monochrome_glyph_draw(
    hasher: &mut impl Hasher,
    draw: &PreparedMonochromeGlyphDraw,
    glyph: &CachedMonochromeGlyph,
) {
    hash_u32(hasher, draw.row);
    hash_u32(hasher, draw.start_col);
    hash_u32(hasher, draw.end_col);
    hash_i32(hasher, draw.dest_x_px);
    hash_i32(hasher, draw.dest_y_px);
    hash_u32(hasher, draw.fg_rgba);
    hash_u32(hasher, glyph.width_px);
    hash_u32(hasher, glyph.height_px);
    hash_usize(hasher, glyph.coverage.len());
    hasher.write(&glyph.coverage);
}

fn hash_color_glyph_draw(
    hasher: &mut impl Hasher,
    draw: &PreparedColorGlyphDraw,
    glyph: &CachedColorGlyph,
) {
    hash_u32(hasher, draw.row);
    hash_u32(hasher, draw.start_col);
    hash_u32(hasher, draw.end_col);
    hash_i32(hasher, draw.dest_x_px);
    hash_i32(hasher, draw.dest_y_px);
    hash_u32(hasher, glyph.width_px);
    hash_u32(hasher, glyph.height_px);
    hash_usize(hasher, glyph.pixels.len());
    for pixel in &glyph.pixels {
        hasher.write(&[pixel.r, pixel.g, pixel.b, pixel.a]);
    }
}

fn hash_bool(hasher: &mut impl Hasher, value: bool) {
    hasher.write(&[u8::from(value)]);
}

fn hash_u32(hasher: &mut impl Hasher, value: u32) {
    hasher.write(&value.to_le_bytes());
}

fn hash_i32(hasher: &mut impl Hasher, value: i32) {
    hasher.write(&value.to_le_bytes());
}

fn hash_usize(hasher: &mut impl Hasher, value: usize) {
    hasher.write(&(value as u64).to_le_bytes());
}

fn hash_u64(hasher: &mut impl Hasher, value: u64) {
    hasher.write(&value.to_le_bytes());
}

fn combine_fingerprints(base_fingerprint: u64, overlay_fingerprint: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_u64(&mut hasher, base_fingerprint);
    hash_u64(&mut hasher, overlay_fingerprint);
    hasher.finish()
}

fn overlay_composition_is_noop(frame: &NativeTerminalFrame) -> bool {
    let presentable = &frame.presentable_frame;
    !presentable.selection_overlay.active
        && !presentable.underline_overlay.visible
        && !presentable.ime_preview_overlay.active
}
