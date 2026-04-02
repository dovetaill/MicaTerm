//! Windows native surface backend powered by a retained Direct2D draw list.

use std::collections::HashMap;

use anyhow::Result;
use slint::ComponentHandle;

use crate::AppWindow;
#[cfg(target_os = "windows")]
use crate::app::ssh::runtime::TerminalCursorShape;
use crate::app::terminal_renderer::{
    NativeSurfaceDamage, NativeSurfaceDamageKind, NativeTerminalSurfaceDiagnostics,
    NativeTerminalSurfaceDrawCounters,
};
use crate::app::terminal_renderer::wgpu_renderer::{
    PreparedColorGlyphDraw, PreparedMonochromeGlyphDraw,
};
use crate::app::windows_frame::resolve_host_window_hwnd;

use super::backend::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, RECT};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    D2D1_BITMAP_PROPERTIES, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_OPACITY_MASK_CONTENT_GRAPHICS, D2D1_RENDER_TARGET_PROPERTIES, D2D1CreateFactory,
    ID2D1Bitmap, ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::HDC;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::HWND as SysHwnd;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC};

#[derive(Default)]
pub struct WindowsD2DFactoryState {
    pub ready: bool,
    #[cfg(target_os = "windows")]
    factory: Option<ID2D1Factory>,
}

pub struct WindowsHwndRenderTargetState {
    pub hwnd: isize,
    pub generation: u64,
    #[cfg(target_os = "windows")]
    render_target: ID2D1DCRenderTarget,
}

#[derive(Default)]
pub struct WindowsBoundDcState {
    pub hwnd: isize,
    pub hdc: isize,
}

#[derive(Default)]
pub struct WindowsD2DBrushState {
    pub rgba: u32,
    generation: u64,
    #[cfg(target_os = "windows")]
    brush: Option<ID2D1SolidColorBrush>,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Default)]
pub struct WindowsMonochromeGlyphBitmapState {
    pub atlas_slot: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub bearing_x_px: i32,
    pub bearing_y_px: i32,
    pub advance_px: i32,
    pub coverage_bytes: usize,
    generation: u64,
    coverage: Vec<u8>,
    #[cfg(target_os = "windows")]
    bitmap: Option<ID2D1Bitmap>,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Default)]
pub struct WindowsColorGlyphBitmapState {
    pub cache_slot: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub rgba_bytes: usize,
    generation: u64,
    bgra: Vec<u8>,
    #[cfg(target_os = "windows")]
    bitmap: Option<ID2D1Bitmap>,
}

#[derive(Default)]
pub struct WindowsNativeSurfaceState {
    pub attached: bool,
    pub host_hwnd: Option<isize>,
    pub window_rect: NativeTerminalSurfaceRect,
    pub rect: NativeTerminalSurfaceRect,
    pub retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    pub bound_dc: Option<WindowsBoundDcState>,
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
    pub last_prepared_frame_token: u64,
    pub last_presented_frame_token: u64,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
impl WindowsNativeSurfaceState {
    fn draw_counters(&self) -> NativeTerminalSurfaceDrawCounters {
        NativeTerminalSurfaceDrawCounters {
            background_runs: self.last_drawn_background_runs,
            monochrome_glyphs: self.last_drawn_monochrome_glyphs,
            color_glyphs: self.last_drawn_color_glyphs,
            selection_rects: self.last_drawn_selection_rects,
            underline_runs: self.last_drawn_underline_runs,
            cursor_overlay_visible: self.last_drawn_cursor_overlay_visible,
            ime_preview_active: self.last_drawn_ime_preview_active,
        }
    }

    fn mark_render_target_dirty(&mut self) {
        self.render_target_dirty = true;
    }

    fn ensure_d2d_factory(&mut self) {
        #[cfg(target_os = "windows")]
        if let Err(err) = self.try_ensure_d2d_factory() {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                "failed to create Direct2D factory for Windows native terminal surface"
            );
        }
    }

    #[cfg(target_os = "windows")]
    fn try_ensure_d2d_factory(&mut self) -> Result<()> {
        if self.d2d_factory.is_some() {
            return Ok(());
        }

        let factory =
            unsafe { D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }?;
        self.d2d_factory = Some(WindowsD2DFactoryState {
            ready: true,
            factory: Some(factory),
        });
        Ok(())
    }

    fn ensure_hwnd_render_target(&mut self) {
        #[cfg(target_os = "windows")]
        if let Err(err) = self.try_ensure_hwnd_render_target() {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                "failed to create or resize Direct2D HWND render target for native terminal surface"
            );
            self.clear_device_resources();
        }
    }

    #[cfg(target_os = "windows")]
    fn try_ensure_hwnd_render_target(&mut self) -> Result<()> {
        let Some(hwnd) = self.host_hwnd else {
            self.clear_device_resources();
            return Ok(());
        };
        if self.render_target_size_px().is_none() {
            self.clear_device_resources();
            return Ok(());
        }

        self.ensure_d2d_factory();
        if self.d2d_factory.is_none() {
            return Ok(());
        }

        if let Some(target_state) = self.hwnd_render_target.as_ref()
            && target_state.hwnd == hwnd
            && !self.render_target_dirty
        {
            return Ok(());
        }

        if self.render_target_dirty || self.hwnd_render_target.is_none() {
            self.clear_device_resources();
            let factory = &self
                .d2d_factory
                .as_ref()
                .and_then(|state| state.factory.as_ref())
                .expect("Direct2D factory should exist before creating a render target");
            let render_target_properties = D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let render_target = unsafe { factory.CreateDCRenderTarget(&render_target_properties) }?;
            self.render_target_generation = self.render_target_generation.saturating_add(1);
            self.hwnd_render_target = Some(WindowsHwndRenderTargetState {
                hwnd,
                generation: self.render_target_generation,
                render_target,
            });
            self.render_target_dirty = false;
        }

        Ok(())
    }

    fn clear_device_resources(&mut self) {
        self.release_bound_dc();
        self.hwnd_render_target = None;
        self.render_target_dirty = true;
        for brush in self.d2d_brushes.values_mut() {
            brush.generation = 0;
            #[cfg(target_os = "windows")]
            {
                brush.brush = None;
            }
        }
        for bitmap in self.monochrome_glyph_bitmaps.values_mut() {
            bitmap.generation = 0;
            #[cfg(target_os = "windows")]
            {
                bitmap.bitmap = None;
            }
        }
        for bitmap in self.color_glyph_bitmaps.values_mut() {
            bitmap.generation = 0;
            #[cfg(target_os = "windows")]
            {
                bitmap.bitmap = None;
            }
        }
        self.monochrome_bitmap_cache_entries = self.monochrome_glyph_bitmaps.len();
        self.color_bitmap_cache_entries = self.color_glyph_bitmaps.len();
        self.last_drawn_background_runs = 0;
        self.last_drawn_monochrome_glyphs = 0;
        self.last_drawn_color_glyphs = 0;
        self.last_drawn_selection_rects = 0;
        self.last_drawn_underline_runs = 0;
        self.last_drawn_cursor_overlay_visible = false;
        self.last_drawn_ime_preview_active = false;
    }

    fn release_bound_dc(&mut self) {
        #[cfg(target_os = "windows")]
        if let Some(bound_dc) = self.bound_dc.take() {
            unsafe {
                ReleaseDC(bound_dc.hwnd as SysHwnd, bound_dc.hdc as _);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            self.bound_dc = None;
        }
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
            .or_insert_with(|| WindowsD2DBrushState {
                rgba,
                ..Default::default()
            });

        #[cfg(target_os = "windows")]
        {
            let needs_create = self
                .d2d_brushes
                .get(&rgba)
                .map(|brush| {
                    brush.generation != self.render_target_generation || brush.brush.is_none()
                })
                .unwrap_or(false);
            if !needs_create {
                return;
            }
            let Some(render_target) = self.render_target() else {
                return;
            };
            let color = d2d_color_from_rgba(rgba);
            match unsafe { render_target.CreateSolidColorBrush(&color, None) } {
                Ok(brush) => {
                    if let Some(brush_state) = self.d2d_brushes.get_mut(&rgba) {
                        brush_state.generation = self.render_target_generation;
                        brush_state.brush = Some(brush);
                    }
                }
                Err(err) => tracing::warn!(
                    target: "app.terminal",
                    error = %err,
                    rgba = format_args!("{rgba:#010x}"),
                    "failed to create Direct2D brush for native terminal surface"
                ),
            }
        }
    }

    fn ensure_monochrome_glyph_bitmap(&mut self, draw: &PreparedMonochromeGlyphDraw) {
        let state = self
            .monochrome_glyph_bitmaps
            .entry(draw.atlas_entry.slot)
            .or_insert_with(|| WindowsMonochromeGlyphBitmapState {
                atlas_slot: draw.atlas_entry.slot,
                width_px: draw.atlas_entry.width_px,
                height_px: draw.atlas_entry.height_px,
                ..Default::default()
            });
        if let Some(upload) = draw.upload.as_ref() {
            state.width_px = upload.width_px;
            state.height_px = upload.height_px;
            state.bearing_x_px = upload.bearing_x_px;
            state.bearing_y_px = upload.bearing_y_px;
            state.advance_px = upload.advance_px;
            state.coverage_bytes = upload.coverage.len();
            state.coverage = upload.coverage.clone();
            state.generation = 0;
            #[cfg(target_os = "windows")]
            {
                state.bitmap = None;
            }
        }
        self.monochrome_bitmap_cache_entries = self.monochrome_glyph_bitmaps.len();

        #[cfg(target_os = "windows")]
        if let Err(err) = self.try_ensure_monochrome_glyph_bitmap(draw.atlas_entry.slot) {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                atlas_slot = draw.atlas_entry.slot,
                "failed to create Direct2D monochrome glyph bitmap"
            );
        }
    }

    #[cfg(target_os = "windows")]
    fn try_ensure_monochrome_glyph_bitmap(&mut self, atlas_slot: u32) -> Result<()> {
        let Some(render_target) = self.render_target() else {
            return Ok(());
        };
        let generation = self.render_target_generation;
        let needs_create = self
            .monochrome_glyph_bitmaps
            .get(&atlas_slot)
            .map(|bitmap| bitmap.generation != generation || bitmap.bitmap.is_none())
            .unwrap_or(false);
        if !needs_create {
            return Ok(());
        }

        let bitmap = {
            let Some(state) = self.monochrome_glyph_bitmaps.get(&atlas_slot) else {
                return Ok(());
            };
            if state.width_px == 0 || state.height_px == 0 || state.coverage.is_empty() {
                return Ok(());
            }
            let bitmap_properties = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };
            let size = D2D_SIZE_U {
                width: state.width_px,
                height: state.height_px,
            };
            unsafe {
                render_target.CreateBitmap(
                    size,
                    Some(state.coverage.as_ptr().cast()),
                    state.width_px,
                    &bitmap_properties,
                )
            }?
        };

        if let Some(state) = self.monochrome_glyph_bitmaps.get_mut(&atlas_slot) {
            state.generation = generation;
            state.bitmap = Some(bitmap);
        }

        Ok(())
    }

    fn ensure_color_glyph_bitmap(&mut self, draw: &PreparedColorGlyphDraw) {
        let state = self
            .color_glyph_bitmaps
            .entry(draw.cache_entry.slot)
            .or_insert_with(|| WindowsColorGlyphBitmapState {
                cache_slot: draw.cache_entry.slot,
                width_px: draw.cache_entry.width_px,
                height_px: draw.cache_entry.height_px,
                rgba_bytes: draw.cache_entry.rgba_bytes,
                ..Default::default()
            });
        if let Some(upload) = draw.upload.as_ref() {
            state.width_px = upload.width_px;
            state.height_px = upload.height_px;
            state.rgba_bytes = upload.rgba.len();
            state.bgra = premultiply_rgba_to_bgra(&upload.rgba);
            state.generation = 0;
            #[cfg(target_os = "windows")]
            {
                state.bitmap = None;
            }
        }
        self.color_bitmap_cache_entries = self.color_glyph_bitmaps.len();

        #[cfg(target_os = "windows")]
        if let Err(err) = self.try_ensure_color_glyph_bitmap(draw.cache_entry.slot) {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                cache_slot = draw.cache_entry.slot,
                "failed to create Direct2D color glyph bitmap"
            );
        }
    }

    #[cfg(target_os = "windows")]
    fn try_ensure_color_glyph_bitmap(&mut self, cache_slot: u32) -> Result<()> {
        let Some(render_target) = self.render_target() else {
            return Ok(());
        };
        let generation = self.render_target_generation;
        let needs_create = self
            .color_glyph_bitmaps
            .get(&cache_slot)
            .map(|bitmap| bitmap.generation != generation || bitmap.bitmap.is_none())
            .unwrap_or(false);
        if !needs_create {
            return Ok(());
        }

        let bitmap = {
            let Some(state) = self.color_glyph_bitmaps.get(&cache_slot) else {
                return Ok(());
            };
            if state.width_px == 0 || state.height_px == 0 || state.bgra.is_empty() {
                return Ok(());
            }
            let bitmap_properties = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };
            let size = D2D_SIZE_U {
                width: state.width_px,
                height: state.height_px,
            };
            unsafe {
                render_target.CreateBitmap(
                    size,
                    Some(state.bgra.as_ptr().cast()),
                    state.width_px.saturating_mul(4),
                    &bitmap_properties,
                )
            }?
        };

        if let Some(state) = self.color_glyph_bitmaps.get_mut(&cache_slot) {
            state.generation = generation;
            state.bitmap = Some(bitmap);
        }

        Ok(())
    }

    fn draw_background_runs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_background_runs = 0;

        #[cfg(not(target_os = "windows"))]
        let _ = frame;

        #[cfg(target_os = "windows")]
        {
            let Some(render_target) = self.render_target() else {
                return;
            };
            let presentable_frame = &frame.frame.presentable_frame;
            let clip_rect = terminal_clip_rect(frame.rect);
            self.ensure_brush(presentable_frame.default_bg_rgba);
            if let Some(brush) = self.brush_for(presentable_frame.default_bg_rgba) {
                unsafe {
                    render_target.FillRectangle(&clip_rect, &brush);
                }
            }
            for row in 0..presentable_frame.grid_rows {
                let row_rgba = if row % 2 == 0 {
                    presentable_frame.row_bg_even_rgba
                } else {
                    presentable_frame.row_bg_odd_rgba
                };
                self.ensure_brush(row_rgba);
                if let (Some(row_rect), Some(brush)) = (
                    row_background_rect(frame.rect, row, frame.frame.cell_height_px),
                    self.brush_for(row_rgba),
                ) {
                    unsafe {
                        render_target.FillRectangle(&row_rect, &brush);
                    }
                }
            }
            for run in &frame.frame.presentable_frame.background_runs {
                self.ensure_brush(run.bg_rgba);
                if let (Some(run_rect), Some(brush)) = (
                    cell_span_rect(
                        frame.rect,
                        run.row,
                        run.start_col,
                        run.end_col,
                        frame.frame.cell_width_px,
                        frame.frame.cell_height_px,
                    ),
                    self.brush_for(run.bg_rgba),
                ) {
                    unsafe {
                        render_target.FillRectangle(&run_rect, &brush);
                    }
                    self.last_drawn_background_runs =
                        self.last_drawn_background_runs.saturating_add(1);
                }
            }
        }
    }

    fn draw_monochrome_glyphs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_monochrome_glyphs = 0;

        #[cfg(not(target_os = "windows"))]
        let _ = frame;

        #[cfg(target_os = "windows")]
        {
            let Some(render_target) = self.render_target() else {
                return;
            };
            for draw in &frame.frame.presentable_frame.monochrome_glyph_draws {
                self.ensure_monochrome_glyph_bitmap(draw);
                self.ensure_brush(draw.fg_rgba);
                let Some(bitmap_state) = self.monochrome_glyph_bitmaps.get(&draw.atlas_entry.slot)
                else {
                    continue;
                };
                let Some(bitmap) = self.monochrome_bitmap_for(draw.atlas_entry.slot) else {
                    continue;
                };
                let Some(brush) = self.brush_for(draw.fg_rgba) else {
                    continue;
                };
                let Some(dest_rect) = glyph_dest_rect(
                    frame.rect,
                    draw.dest_x_px,
                    draw.dest_y_px,
                    bitmap_state.width_px,
                    bitmap_state.height_px,
                ) else {
                    continue;
                };
                let source_rect = bitmap_source_rect(bitmap_state.width_px, bitmap_state.height_px);
                unsafe {
                    render_target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
                    render_target.FillOpacityMask(
                        &bitmap,
                        &brush,
                        D2D1_OPACITY_MASK_CONTENT_GRAPHICS,
                        Some(&dest_rect),
                        Some(&source_rect),
                    );
                }
                self.last_drawn_monochrome_glyphs =
                    self.last_drawn_monochrome_glyphs.saturating_add(1);
            }
            self.monochrome_bitmap_cache_entries = self.monochrome_glyph_bitmaps.len();
        }
    }

    fn draw_color_glyphs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_color_glyphs = 0;

        #[cfg(not(target_os = "windows"))]
        let _ = frame;

        #[cfg(target_os = "windows")]
        {
            let Some(render_target) = self.render_target() else {
                return;
            };
            for draw in &frame.frame.presentable_frame.color_glyph_draws {
                self.ensure_color_glyph_bitmap(draw);
                let Some(bitmap_state) = self.color_glyph_bitmaps.get(&draw.cache_entry.slot)
                else {
                    continue;
                };
                let Some(bitmap) = self.color_bitmap_for(draw.cache_entry.slot) else {
                    continue;
                };
                let Some(dest_rect) = glyph_dest_rect(
                    frame.rect,
                    draw.dest_x_px,
                    draw.dest_y_px,
                    bitmap_state.width_px,
                    bitmap_state.height_px,
                ) else {
                    continue;
                };
                let source_rect = bitmap_source_rect(bitmap_state.width_px, bitmap_state.height_px);
                unsafe {
                    render_target.DrawBitmap(
                        &bitmap,
                        Some(&dest_rect),
                        1.0,
                        D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                        Some(&source_rect),
                    );
                }
                self.last_drawn_color_glyphs = self.last_drawn_color_glyphs.saturating_add(1);
            }
            self.color_bitmap_cache_entries = self.color_glyph_bitmaps.len();
        }
    }

    fn draw_selection_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_selection_rects = 0;

        #[cfg(not(target_os = "windows"))]
        let _ = frame;

        #[cfg(target_os = "windows")]
        {
            if !frame.frame.presentable_frame.selection_overlay.active {
                return;
            }
            let Some(render_target) = self.render_target() else {
                return;
            };
            for rect in &frame.frame.presentable_frame.selection_overlay.rects {
                self.ensure_brush(rect.overlay_rgba);
                if let (Some(selection_rect), Some(brush)) = (
                    cell_span_rect(
                        frame.rect,
                        rect.row,
                        rect.start_col,
                        rect.end_col,
                        frame.frame.cell_width_px,
                        frame.frame.cell_height_px,
                    ),
                    self.brush_for(rect.overlay_rgba),
                ) {
                    unsafe {
                        render_target.FillRectangle(&selection_rect, &brush);
                    }
                    self.last_drawn_selection_rects =
                        self.last_drawn_selection_rects.saturating_add(1);
                }
            }
        }
    }

    fn draw_underline_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_underline_runs = 0;

        #[cfg(not(target_os = "windows"))]
        let _ = frame;

        #[cfg(target_os = "windows")]
        {
            if !frame.frame.presentable_frame.underline_overlay.visible {
                return;
            }
            let Some(render_target) = self.render_target() else {
                return;
            };
            let thickness = underline_thickness(frame.frame.cell_height_px);
            for run in &frame.frame.presentable_frame.underline_overlay.runs {
                self.ensure_brush(run.fg_rgba);
                if let (Some(underline_rect), Some(brush)) = (
                    underline_rect(
                        frame.rect,
                        run.row,
                        run.start_col,
                        run.end_col,
                        frame.frame.cell_width_px,
                        frame.frame.cell_height_px,
                        thickness,
                    ),
                    self.brush_for(run.fg_rgba),
                ) {
                    unsafe {
                        render_target.FillRectangle(&underline_rect, &brush);
                    }
                    self.last_drawn_underline_runs =
                        self.last_drawn_underline_runs.saturating_add(1);
                }
            }
        }
    }

    fn draw_cursor_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_cursor_overlay_visible =
            frame.frame.presentable_frame.cursor_overlay.visible;

        #[cfg(target_os = "windows")]
        {
            let cursor = frame.frame.presentable_frame.cursor_overlay;
            if !cursor.visible {
                return;
            }
            let Some(render_target) = self.render_target() else {
                return;
            };
            self.ensure_brush(cursor.bg_rgba);
            if let (Some(cursor_rect), Some(brush)) = (
                cursor_rect(frame.rect, cursor),
                self.brush_for(cursor.bg_rgba),
            ) {
                unsafe {
                    render_target.FillRectangle(&cursor_rect, &brush);
                }
            }
        }
    }

    fn draw_ime_preview_overlay(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_drawn_ime_preview_active =
            frame.frame.presentable_frame.ime_preview_overlay.active;

        #[cfg(target_os = "windows")]
        {
            let ime = frame.frame.presentable_frame.ime_preview_overlay;
            if !ime.active {
                return;
            }
            let Some(render_target) = self.render_target() else {
                return;
            };
            let preview_rgba =
                (0x44_u32 << 24) | (frame.frame.presentable_frame.default_fg_rgba & 0x00ff_ffff);
            self.ensure_brush(preview_rgba);
            self.ensure_brush(frame.frame.presentable_frame.default_fg_rgba);
            if let (Some(preview_rect), Some(brush)) = (
                cell_span_rect(
                    frame.rect,
                    ime.row,
                    ime.start_col,
                    ime.end_col,
                    frame.frame.cell_width_px,
                    frame.frame.cell_height_px,
                ),
                self.brush_for(preview_rgba),
            ) {
                unsafe {
                    render_target.FillRectangle(&preview_rect, &brush);
                }
            }
            if let (Some(caret_rect), Some(brush)) = (
                ime_cursor_rect(
                    frame.rect,
                    ime.row,
                    ime.cursor_col,
                    frame.frame.cell_width_px,
                    frame.frame.cell_height_px,
                ),
                self.brush_for(frame.frame.presentable_frame.default_fg_rgba),
            ) {
                unsafe {
                    render_target.FillRectangle(&caret_rect, &brush);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn render_target(&self) -> Option<ID2D1DCRenderTarget> {
        self.hwnd_render_target
            .as_ref()
            .map(|state| state.render_target.clone())
    }

    #[cfg(target_os = "windows")]
    fn brush_for(&self, rgba: u32) -> Option<ID2D1SolidColorBrush> {
        self.d2d_brushes
            .get(&rgba)
            .and_then(|brush| (brush.generation == self.render_target_generation).then_some(brush))
            .and_then(|brush| brush.brush.clone())
    }

    #[cfg(target_os = "windows")]
    fn monochrome_bitmap_for(&self, atlas_slot: u32) -> Option<ID2D1Bitmap> {
        self.monochrome_glyph_bitmaps
            .get(&atlas_slot)
            .and_then(|bitmap| {
                (bitmap.generation == self.render_target_generation).then_some(bitmap)
            })
            .and_then(|bitmap| bitmap.bitmap.clone())
    }

    #[cfg(target_os = "windows")]
    fn color_bitmap_for(&self, cache_slot: u32) -> Option<ID2D1Bitmap> {
        self.color_glyph_bitmaps
            .get(&cache_slot)
            .and_then(|bitmap| {
                (bitmap.generation == self.render_target_generation).then_some(bitmap)
            })
            .and_then(|bitmap| bitmap.bitmap.clone())
    }

    #[cfg(target_os = "windows")]
    fn begin_frame(&mut self) -> bool {
        let Some(render_target) = self.render_target() else {
            return false;
        };
        let Some(host_hwnd) = self.host_hwnd else {
            return false;
        };
        let Some(bind_rect) = self.window_bind_rect() else {
            return false;
        };
        self.release_bound_dc();
        let hdc = unsafe { GetDC(host_hwnd as SysHwnd) };
        if hdc.is_null() {
            return false;
        }
        if let Err(err) = unsafe { render_target.BindDC(HDC(hdc.cast()), &bind_rect) } {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                "failed to bind Direct2D DC render target to host window DC"
            );
            unsafe {
                ReleaseDC(host_hwnd as SysHwnd, hdc);
            }
            if err.code() == D2DERR_RECREATE_TARGET {
                self.clear_device_resources();
            }
            return false;
        }
        self.bound_dc = Some(WindowsBoundDcState {
            hwnd: host_hwnd,
            hdc: hdc as isize,
        });
        let clip_rect = terminal_clip_rect(self.rect);
        unsafe {
            render_target.BeginDraw();
            render_target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
            render_target.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_ALIASED);
        }
        true
    }

    #[cfg(target_os = "windows")]
    fn end_frame(&mut self) -> bool {
        let Some(render_target) = self.render_target() else {
            self.release_bound_dc();
            return false;
        };
        unsafe {
            render_target.PopAxisAlignedClip();
        }
        if let Err(err) = unsafe { render_target.EndDraw(None, None) } {
            self.release_bound_dc();
            if err.code() == D2DERR_RECREATE_TARGET {
                self.clear_device_resources();
            } else {
                tracing::warn!(
                    target: "app.terminal",
                    error = %err,
                    "failed to finish Direct2D draw pass for native terminal surface"
                );
            }
            return false;
        }
        self.release_bound_dc();
        true
    }

    #[cfg(target_os = "windows")]
    fn window_bind_rect(&self) -> Option<RECT> {
        let right = self.window_rect.x.checked_add(self.window_rect.width)?;
        let bottom = self.window_rect.y.checked_add(self.window_rect.height)?;
        (self.window_rect.width > 0 && self.window_rect.height > 0).then_some(RECT {
            left: self.window_rect.x,
            top: self.window_rect.y,
            right,
            bottom,
        })
    }

    fn sync_host_surface_rect(&mut self) {
        if self.host_hwnd.is_none() || self.window_rect.width <= 0 || self.window_rect.height <= 0 {
            self.rect = NativeTerminalSurfaceRect::default();
        } else {
            self.rect = NativeTerminalSurfaceRect {
                x: 0,
                y: 0,
                width: self.window_rect.width,
                height: self.window_rect.height,
            };
        }
        if let Some(retained_frame) = self.retained_frame.as_mut() {
            retained_frame.rect = self.rect;
        }
    }
}

#[derive(Default)]
pub struct WindowsNativeSurfaceBackend {
    state: WindowsNativeSurfaceState,
    host_window: Option<slint::Weak<AppWindow>>,
}

impl WindowsNativeSurfaceBackend {
    fn resolve_host_hwnd(window: &AppWindow) -> Option<isize> {
        resolve_host_window_hwnd(window)
    }

    fn resolve_host_hwnd_if_needed(&mut self) {
        let next_host_hwnd = self
            .host_window
            .as_ref()
            .and_then(|window| window.upgrade())
            .and_then(|window| Self::resolve_host_hwnd(&window));

        if next_host_hwnd != self.state.host_hwnd {
            self.state.clear_device_resources();
            self.state.host_hwnd = next_host_hwnd;
        }

        self.state.sync_host_surface_rect();
    }
}

impl PlatformNativeSurfaceBackend for WindowsNativeSurfaceBackend {
    fn attach(&mut self, window: &AppWindow) -> Result<()> {
        self.state.attached = true;
        self.host_window = Some(window.as_weak());
        self.resolve_host_hwnd_if_needed();
        self.state.mark_render_target_dirty();
        Ok(())
    }

    fn update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect) {
        if !self.state.attached {
            return;
        }
        if self.state.window_rect != rect {
            self.state.window_rect = rect;
            self.resolve_host_hwnd_if_needed();
        }
    }

    fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>) {
        if !self.state.attached {
            return;
        }
        self.resolve_host_hwnd_if_needed();
        self.state.last_prepared_frame_token = frame
            .as_ref()
            .map(|retained_frame| retained_frame.frame.frame_token)
            .unwrap_or_default();
        self.state.retained_frame = frame.map(|mut retained_frame| {
            retained_frame.rect = self.state.rect;
            retained_frame
        });
    }

    fn present(&mut self, damage: NativeSurfaceDamage) {
        if !self.state.attached {
            return;
        }
        self.resolve_host_hwnd_if_needed();
        if self.state.host_hwnd.is_none() {
            return;
        }
        if self.state.rect.width <= 0 || self.state.rect.height <= 0 {
            return;
        }
        self.state.ensure_hwnd_render_target();
        let Some(frame) = self.state.retained_frame.clone() else {
            return;
        };
        let frame = &frame;

        #[cfg(target_os = "windows")]
        if !self.state.begin_frame() {
            return;
        }

        match damage.kind {
            NativeSurfaceDamageKind::OverlayOnly => {
                self.state.draw_background_runs(frame);
                self.state.draw_monochrome_glyphs(frame);
                self.state.draw_color_glyphs(frame);
                self.state.draw_selection_overlay(frame);
                self.state.draw_cursor_overlay(frame);
                self.state.draw_ime_preview_overlay(frame);
            }
            NativeSurfaceDamageKind::Full | NativeSurfaceDamageKind::None => {
                self.state.draw_background_runs(frame);
                self.state.draw_selection_overlay(frame);
                self.state.draw_monochrome_glyphs(frame);
                self.state.draw_color_glyphs(frame);
                self.state.draw_underline_overlay(frame);
                self.state.draw_cursor_overlay(frame);
                self.state.draw_ime_preview_overlay(frame);
            }
        }

        #[cfg(target_os = "windows")]
        if self.state.end_frame() {
            self.state.last_presented_frame_token = frame.frame.frame_token;
        }

        #[cfg(not(target_os = "windows"))]
        {
            self.state.last_presented_frame_token = frame.frame.frame_token;
        }
    }

    fn diagnostics_snapshot(&self) -> NativeTerminalSurfaceDiagnostics {
        NativeTerminalSurfaceDiagnostics {
            hwnd: self.state.host_hwnd,
            render_target_generation: self.state.render_target_generation,
            last_prepared_frame_token: self.state.last_prepared_frame_token,
            last_presented_frame_token: self.state.last_presented_frame_token,
            draw_counters: self.state.draw_counters(),
        }
    }

    fn detach(&mut self) {
        self.state.attached = false;
        self.state.retained_frame = None;
        self.state.clear_device_resources();
        self.state.d2d_brushes.clear();
        self.state.monochrome_glyph_bitmaps.clear();
        self.state.color_glyph_bitmaps.clear();
        self.state.d2d_factory = None;
        self.state.host_hwnd = None;
        self.host_window = None;
        self.state.window_rect = NativeTerminalSurfaceRect::default();
        self.state.rect = NativeTerminalSurfaceRect::default();
        self.state.render_target_generation = 0;
        self.state.render_target_dirty = false;
        self.state.last_prepared_frame_token = 0;
        self.state.last_presented_frame_token = 0;
    }
}

#[cfg(target_os = "windows")]
fn d2d_color_from_rgba(rgba: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((rgba >> 16) & 0xff) as f32 / 255.0,
        g: ((rgba >> 8) & 0xff) as f32 / 255.0,
        b: (rgba & 0xff) as f32 / 255.0,
        a: ((rgba >> 24) & 0xff) as f32 / 255.0,
    }
}

#[cfg(target_os = "windows")]
fn terminal_clip_rect(rect: NativeTerminalSurfaceRect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x as f32,
        top: rect.y as f32,
        right: rect.x.saturating_add(rect.width) as f32,
        bottom: rect.y.saturating_add(rect.height) as f32,
    }
}

#[cfg(target_os = "windows")]
fn row_background_rect(
    rect: NativeTerminalSurfaceRect,
    row: u32,
    cell_height_px: u32,
) -> Option<D2D_RECT_F> {
    let top = rect
        .y
        .saturating_add((row.saturating_mul(cell_height_px)) as i32);
    let bottom = top
        .saturating_add(cell_height_px as i32)
        .min(rect.y.saturating_add(rect.height));
    (bottom > top).then_some(D2D_RECT_F {
        left: rect.x as f32,
        top: top as f32,
        right: rect.x.saturating_add(rect.width) as f32,
        bottom: bottom as f32,
    })
}

#[cfg(target_os = "windows")]
fn cell_span_rect(
    rect: NativeTerminalSurfaceRect,
    row: u32,
    start_col: u32,
    end_col: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> Option<D2D_RECT_F> {
    if cell_width_px == 0 || cell_height_px == 0 || end_col < start_col {
        return None;
    }
    let top = rect
        .y
        .saturating_add((row.saturating_mul(cell_height_px)) as i32);
    let bottom = top
        .saturating_add(cell_height_px as i32)
        .min(rect.y.saturating_add(rect.height));
    let left = rect
        .x
        .saturating_add((start_col.saturating_mul(cell_width_px)) as i32);
    let right = rect
        .x
        .saturating_add((end_col.saturating_add(1).saturating_mul(cell_width_px)) as i32)
        .min(rect.x.saturating_add(rect.width));
    (right > left && bottom > top).then_some(D2D_RECT_F {
        left: left as f32,
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
    })
}

#[cfg(target_os = "windows")]
fn glyph_dest_rect(
    rect: NativeTerminalSurfaceRect,
    dest_x_px: i32,
    dest_y_px: i32,
    width_px: u32,
    height_px: u32,
) -> Option<D2D_RECT_F> {
    if width_px == 0 || height_px == 0 {
        return None;
    }
    let left = rect.x.saturating_add(dest_x_px);
    let top = rect.y.saturating_add(dest_y_px);
    let right = left.saturating_add(width_px as i32);
    let bottom = top.saturating_add(height_px as i32);
    (right > left && bottom > top).then_some(D2D_RECT_F {
        left: left as f32,
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
    })
}

#[cfg(target_os = "windows")]
fn bitmap_source_rect(width_px: u32, height_px: u32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: 0.0,
        top: 0.0,
        right: width_px as f32,
        bottom: height_px as f32,
    }
}

#[cfg(target_os = "windows")]
fn underline_thickness(cell_height_px: u32) -> u32 {
    (cell_height_px / 12).max(1)
}

#[cfg(target_os = "windows")]
fn underline_rect(
    rect: NativeTerminalSurfaceRect,
    row: u32,
    start_col: u32,
    end_col: u32,
    cell_width_px: u32,
    cell_height_px: u32,
    thickness_px: u32,
) -> Option<D2D_RECT_F> {
    let mut base_rect =
        cell_span_rect(rect, row, start_col, end_col, cell_width_px, cell_height_px)?;
    base_rect.top = (base_rect.bottom - thickness_px as f32).max(base_rect.top);
    Some(base_rect)
}

#[cfg(target_os = "windows")]
fn cursor_rect(
    rect: NativeTerminalSurfaceRect,
    cursor: crate::app::terminal_presenter::NativeCursorOverlay,
) -> Option<D2D_RECT_F> {
    let base_rect = cell_span_rect(
        rect,
        cursor.row,
        cursor.col,
        cursor.col,
        cursor.cell_width_px,
        cursor.cell_height_px,
    )?;
    match cursor.shape {
        TerminalCursorShape::Block => Some(base_rect),
        TerminalCursorShape::Underline => {
            let mut underline = base_rect;
            underline.top = (underline.bottom - (cursor.cell_height_px.max(1) / 8).max(1) as f32)
                .max(underline.top);
            Some(underline)
        }
        TerminalCursorShape::Bar => {
            let mut bar = base_rect;
            bar.right =
                (bar.left + (cursor.cell_width_px.max(1) / 8).max(1) as f32).min(base_rect.right);
            Some(bar)
        }
    }
}

#[cfg(target_os = "windows")]
fn ime_cursor_rect(
    rect: NativeTerminalSurfaceRect,
    row: u32,
    cursor_col: u32,
    cell_width_px: u32,
    cell_height_px: u32,
) -> Option<D2D_RECT_F> {
    let mut bar = cell_span_rect(
        rect,
        row,
        cursor_col,
        cursor_col,
        cell_width_px,
        cell_height_px,
    )?;
    bar.right = (bar.left + (cell_width_px.max(1) / 10).max(1) as f32).min(bar.right);
    Some(bar)
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn premultiply_rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|pixel| {
            let r = u16::from(pixel[0]);
            let g = u16::from(pixel[1]);
            let b = u16::from(pixel[2]);
            let a = u16::from(pixel[3]);
            let premultiply = |value: u16| -> u8 { ((value * a + 127) / 255) as u8 };
            [premultiply(b), premultiply(g), premultiply(r), a as u8]
        })
        .collect()
}
