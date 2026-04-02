//! Scene-owned terminal image renderer for software builds that must stay inside Slint z-order.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::app::terminal_presenter::{BitmapTerminalFrame, NativeTerminalFrame};
use crate::app::terminal_renderer::wgpu_renderer::{
    PreparedColorGlyphDraw, PreparedColorGlyphUploadPayload, PreparedMonochromeGlyphDraw,
};

#[derive(Default)]
pub struct SceneImageTerminalRenderer {
    monochrome_glyph_cache: HashMap<u32, CachedMonochromeGlyph>,
    color_glyph_cache: HashMap<u32, CachedColorGlyph>,
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
    }

    pub fn render(&mut self, frame: &NativeTerminalFrame) -> Result<BitmapTerminalFrame> {
        let width_px = frame
            .presentable_frame
            .grid_cols
            .saturating_mul(frame.cell_width_px);
        let height_px = frame
            .presentable_frame
            .grid_rows
            .saturating_mul(frame.cell_height_px);

        if width_px == 0 || height_px == 0 {
            return Ok(BitmapTerminalFrame {
                image: Image::default(),
                grid_rows: frame.presentable_frame.grid_rows,
                grid_cols: frame.presentable_frame.grid_cols,
                cell_width_px: frame.cell_width_px,
                cell_height_px: frame.cell_height_px,
            });
        }

        let mut pixels = vec![rgba8(frame.presentable_frame.default_bg_rgba); (width_px * height_px) as usize];

        {
            let mut surface = PixelSurface {
                pixels: &mut pixels,
                width_px,
                height_px,
            };
            self.draw_row_backgrounds(&mut surface, frame);
            self.draw_background_runs(&mut surface, frame);
            self.draw_selection_overlay(&mut surface, frame);
            self.draw_monochrome_glyphs(&mut surface, frame)?;
            self.draw_color_glyphs(&mut surface, frame)?;
            self.draw_underline_overlay(&mut surface, frame);
            self.draw_ime_preview_overlay(&mut surface, frame);
        }

        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width_px, height_px);
        buffer.make_mut_slice().copy_from_slice(&pixels);

        Ok(BitmapTerminalFrame {
            image: Image::from_rgba8(buffer),
            grid_rows: frame.presentable_frame.grid_rows,
            grid_cols: frame.presentable_frame.grid_cols,
            cell_width_px: frame.cell_width_px,
            cell_height_px: frame.cell_height_px,
        })
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
            let clip_rect = cell_span_rect(
                draw.row,
                draw.start_col,
                draw.end_col,
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
            let clip_rect = cell_span_rect(
                draw.row,
                draw.start_col,
                draw.end_col,
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
    let dest_x_px = constrain_glyph_origin_x(
        dest_x_px,
        glyph.width_px,
        clip_rect,
    );
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
    let (dest_x_px, dest_y_px) = constrain_glyph_origin(
        dest_x_px,
        dest_y_px,
        glyph.width_px,
        glyph.height_px,
        clip_rect,
    );
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

fn constrain_glyph_origin(
    dest_x_px: i32,
    dest_y_px: i32,
    glyph_width_px: u32,
    glyph_height_px: u32,
    clip_rect: Option<PixelRect>,
) -> (i32, i32) {
    let Some(clip_rect) = clip_rect else {
        return (dest_x_px, dest_y_px);
    };

    (
        constrain_glyph_origin_x(dest_x_px, glyph_width_px, Some(clip_rect)),
        constrain_glyph_origin_y(dest_y_px, glyph_height_px, Some(clip_rect)),
    )
}

fn constrain_glyph_origin_x(
    dest_x_px: i32,
    glyph_width_px: u32,
    clip_rect: Option<PixelRect>,
) -> i32 {
    let Some(clip_rect) = clip_rect else {
        return dest_x_px;
    };

    constrain_axis_to_clip(dest_x_px, glyph_width_px, clip_rect.start_x, clip_rect.width)
}

fn constrain_glyph_origin_y(
    dest_y_px: i32,
    glyph_height_px: u32,
    clip_rect: Option<PixelRect>,
) -> i32 {
    let Some(clip_rect) = clip_rect else {
        return dest_y_px;
    };

    constrain_axis_to_clip(dest_y_px, glyph_height_px, clip_rect.start_y, clip_rect.height)
}

fn constrain_axis_to_clip(
    dest_px: i32,
    glyph_extent_px: u32,
    clip_start_px: u32,
    clip_extent_px: u32,
) -> i32 {
    if clip_extent_px == 0 {
        return dest_px;
    }

    let clip_start_px = clip_start_px as i32;
    let clip_end_px = clip_start_px.saturating_add(clip_extent_px as i32);
    let max_dest_px = clip_end_px.saturating_sub(glyph_extent_px as i32);

    if max_dest_px < clip_start_px {
        clip_start_px
    } else {
        dest_px.max(clip_start_px).min(max_dest_px)
    }
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
