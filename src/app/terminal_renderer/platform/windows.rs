//! Windows native surface backend scaffold.

use std::collections::HashMap;

use anyhow::Result;

use crate::AppWindow;
use crate::app::terminal_renderer::wgpu_renderer::{
    PreparedColorGlyphDraw, PreparedMonochromeGlyphDraw,
};
use crate::app::windows_frame::resolve_host_window_hwnd;

use super::backend::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsD2DFactoryState {
    pub ready: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsHwndRenderTargetState {
    pub hwnd: isize,
    pub width_px: u32,
    pub height_px: u32,
    pub generation: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsD2DBrushState {
    pub rgba: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsMonochromeGlyphBitmapState {
    pub atlas_slot: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub bearing_x_px: i32,
    pub bearing_y_px: i32,
    pub advance_px: i32,
    pub coverage_bytes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsColorGlyphBitmapState {
    pub cache_slot: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub rgba_bytes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsNativeSurfaceState {
    pub hwnd: Option<isize>,
    pub rect: NativeTerminalSurfaceRect,
    pub retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    pub d2d_factory: Option<WindowsD2DFactoryState>,
    pub hwnd_render_target: Option<WindowsHwndRenderTargetState>,
    pub render_target_generation: u64,
    pub render_target_dirty: bool,
    pub d2d_brushes: HashMap<u32, WindowsD2DBrushState>,
    pub monochrome_glyph_bitmaps: HashMap<u32, WindowsMonochromeGlyphBitmapState>,
    pub color_glyph_bitmaps: HashMap<u32, WindowsColorGlyphBitmapState>,
    pub monochrome_bitmap_cache_entries: usize,
    pub color_bitmap_cache_entries: usize,
    pub last_drawn_background_runs: usize,
    pub last_drawn_monochrome_glyphs: usize,
    pub last_drawn_color_glyphs: usize,
    pub last_drawn_selection_rects: usize,
    pub last_drawn_underline_runs: usize,
    pub last_drawn_cursor_overlay_visible: bool,
    pub last_drawn_ime_preview_active: bool,
    pub last_presented_frame_token: u64,
}

impl WindowsNativeSurfaceState {
    fn mark_render_target_dirty(&mut self) {
        self.render_target_dirty = true;
    }

    fn ensure_d2d_factory(&mut self) {
        if self.d2d_factory.is_none() {
            self.d2d_factory = Some(WindowsD2DFactoryState { ready: true });
        }
    }

    fn ensure_hwnd_render_target(&mut self) {
        let Some(hwnd) = self.hwnd else {
            self.clear_device_resources();
            return;
        };
        let Some((width_px, height_px)) = self.render_target_size_px() else {
            self.clear_device_resources();
            return;
        };

        let needs_recreate = self.render_target_dirty
            || self
                .hwnd_render_target
                .as_ref()
                .map(|target| {
                    target.hwnd != hwnd
                        || target.width_px != width_px
                        || target.height_px != height_px
                })
                .unwrap_or(true);
        if !needs_recreate {
            return;
        }

        self.ensure_d2d_factory();
        self.clear_device_resources();
        self.render_target_generation = self.render_target_generation.saturating_add(1);
        self.hwnd_render_target = Some(WindowsHwndRenderTargetState {
            hwnd,
            width_px,
            height_px,
            generation: self.render_target_generation,
        });
        self.render_target_dirty = false;
    }

    fn clear_device_resources(&mut self) {
        self.hwnd_render_target = None;
        self.d2d_brushes.clear();
        self.monochrome_glyph_bitmaps.clear();
        self.color_glyph_bitmaps.clear();
        self.monochrome_bitmap_cache_entries = 0;
        self.color_bitmap_cache_entries = 0;
        self.last_drawn_background_runs = 0;
        self.last_drawn_monochrome_glyphs = 0;
        self.last_drawn_color_glyphs = 0;
        self.last_drawn_selection_rects = 0;
        self.last_drawn_underline_runs = 0;
        self.last_drawn_cursor_overlay_visible = false;
        self.last_drawn_ime_preview_active = false;
    }

    fn render_target_size_px(&self) -> Option<(u32, u32)> {
        let width_px = u32::try_from(self.rect.width).ok()?;
        let height_px = u32::try_from(self.rect.height).ok()?;
        if width_px == 0 || height_px == 0 {
            None
        } else {
            Some((width_px, height_px))
        }
    }

    fn ensure_brush(&mut self, rgba: u32) {
        self.d2d_brushes
            .entry(rgba)
            .or_insert(WindowsD2DBrushState { rgba });
    }

    fn ensure_monochrome_glyph_bitmap(&mut self, draw: &PreparedMonochromeGlyphDraw) {
        if let Some(upload) = draw.upload.as_ref() {
            self.monochrome_glyph_bitmaps.insert(
                draw.atlas_entry.slot,
                WindowsMonochromeGlyphBitmapState {
                    atlas_slot: draw.atlas_entry.slot,
                    width_px: upload.width_px,
                    height_px: upload.height_px,
                    bearing_x_px: upload.bearing_x_px,
                    bearing_y_px: upload.bearing_y_px,
                    advance_px: upload.advance_px,
                    coverage_bytes: upload.coverage.len(),
                },
            );
            return;
        }

        self.monochrome_glyph_bitmaps
            .entry(draw.atlas_entry.slot)
            .or_insert_with(|| WindowsMonochromeGlyphBitmapState {
                atlas_slot: draw.atlas_entry.slot,
                width_px: draw.atlas_entry.width_px,
                height_px: draw.atlas_entry.height_px,
                ..Default::default()
            });
    }

    fn ensure_color_glyph_bitmap(&mut self, draw: &PreparedColorGlyphDraw) {
        if let Some(upload) = draw.upload.as_ref() {
            self.color_glyph_bitmaps.insert(
                draw.cache_entry.slot,
                WindowsColorGlyphBitmapState {
                    cache_slot: draw.cache_entry.slot,
                    width_px: upload.width_px,
                    height_px: upload.height_px,
                    rgba_bytes: upload.rgba.len(),
                },
            );
            return;
        }

        self.color_glyph_bitmaps
            .entry(draw.cache_entry.slot)
            .or_insert_with(|| WindowsColorGlyphBitmapState {
                cache_slot: draw.cache_entry.slot,
                width_px: draw.cache_entry.width_px,
                height_px: draw.cache_entry.height_px,
                rgba_bytes: draw.cache_entry.rgba_bytes,
            });
    }

    fn draw_background_runs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_background_runs = 0;
        for run in &frame.frame.presentable_frame.background_runs {
            self.ensure_brush(run.bg_rgba);
            self.last_drawn_background_runs = self.last_drawn_background_runs.saturating_add(1);
        }
    }

    fn draw_monochrome_glyphs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_monochrome_glyphs = 0;
        for draw in &frame.frame.presentable_frame.monochrome_glyph_draws {
            self.ensure_monochrome_glyph_bitmap(draw);
            self.ensure_brush(draw.fg_rgba);
            if self
                .monochrome_glyph_bitmaps
                .contains_key(&draw.atlas_entry.slot)
            {
                self.last_drawn_monochrome_glyphs =
                    self.last_drawn_monochrome_glyphs.saturating_add(1);
            }
        }
        self.monochrome_bitmap_cache_entries = self.monochrome_glyph_bitmaps.len();
    }

    fn draw_color_glyphs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_color_glyphs = 0;
        for draw in &frame.frame.presentable_frame.color_glyph_draws {
            self.ensure_color_glyph_bitmap(draw);
            if self.color_glyph_bitmaps.contains_key(&draw.cache_entry.slot) {
                self.last_drawn_color_glyphs = self.last_drawn_color_glyphs.saturating_add(1);
            }
        }
        self.color_bitmap_cache_entries = self.color_glyph_bitmaps.len();
    }

    fn draw_selection_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_selection_rects = 0;
        for rect in &frame.frame.presentable_frame.selection_overlay.rects {
            self.ensure_brush(rect.overlay_rgba);
            self.last_drawn_selection_rects = self.last_drawn_selection_rects.saturating_add(1);
        }
    }

    fn draw_underline_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_underline_runs = 0;
        for run in &frame.frame.presentable_frame.underline_overlay.runs {
            self.ensure_brush(run.fg_rgba);
            self.last_drawn_underline_runs = self.last_drawn_underline_runs.saturating_add(1);
        }
    }

    fn draw_cursor_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_cursor_overlay_visible =
            frame.frame.presentable_frame.cursor_overlay.visible;
        if frame.frame.presentable_frame.cursor_overlay.visible {
            self.ensure_brush(frame.frame.presentable_frame.cursor_overlay.fg_rgba);
            self.ensure_brush(frame.frame.presentable_frame.cursor_overlay.bg_rgba);
        }
    }

    fn draw_ime_preview_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_ime_preview_active =
            frame.frame.presentable_frame.ime_preview_overlay.active;
    }
}

#[derive(Debug, Default)]
pub struct WindowsNativeSurfaceBackend {
    state: WindowsNativeSurfaceState,
}

impl WindowsNativeSurfaceBackend {
    fn resolve_host_hwnd(window: &AppWindow) -> Option<isize> {
        resolve_host_window_hwnd(window)
    }
}

impl PlatformNativeSurfaceBackend for WindowsNativeSurfaceBackend {
    fn attach(&mut self, window: &AppWindow) -> Result<()> {
        self.state.hwnd = Self::resolve_host_hwnd(window);
        self.state.mark_render_target_dirty();
        Ok(())
    }

    fn update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect) {
        if self.state.rect != rect {
            self.state.rect = rect;
            self.state.mark_render_target_dirty();
        }
    }

    fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>) {
        if self.state.retained_frame != frame {
            self.state.retained_frame = frame;
            self.state.mark_render_target_dirty();
        }
    }

    fn present(&mut self) {
        self.state.ensure_hwnd_render_target();
        if let Some(frame) = self.state.retained_frame.clone() {
            let frame = &frame;
            self.state.draw_background_runs(frame);
            self.state.draw_selection_overlay(frame);
            self.state.draw_monochrome_glyphs(frame);
            self.state.draw_color_glyphs(frame);
            self.state.draw_underline_overlay(frame);
            self.state.draw_cursor_overlay(frame);
            self.state.draw_ime_preview_overlay(frame);
            self.state.last_presented_frame_token = frame.frame.frame_token;
        }
    }

    fn detach(&mut self) {
        self.state.retained_frame = None;
        self.state.clear_device_resources();
        self.state.d2d_factory = None;
        self.state.hwnd = None;
        self.state.rect = NativeTerminalSurfaceRect::default();
        self.state.render_target_generation = 0;
        self.state.render_target_dirty = false;
        self.state.last_presented_frame_token = 0;
    }
}
