//! Windows native surface backend powered by a retained Direct2D draw list.

use std::collections::HashMap;

use anyhow::Result;
use slint::ComponentHandle;

use crate::AppWindow;
#[cfg(target_os = "windows")]
use crate::app::ssh::runtime::TerminalCursorShape;
#[cfg(target_os = "windows")]
use crate::app::terminal_font::{
    DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY, DEFAULT_TERMINAL_FONT_FAMILY, FontFaceKey,
};
use crate::app::terminal_renderer::diagnostics::{
    NativeTerminalSurfaceGlyphBoundsTrace, NativeTerminalSurfaceWindowsTextDiagnostics,
};
use crate::app::terminal_renderer::wgpu_renderer::{
    PreparedColorGlyphDraw, PreparedMonochromeGlyphDraw,
};
use crate::app::terminal_renderer::{
    NativeSurfaceDamage, NativeSurfaceDamageKind, NativeTerminalSurfaceDiagnostics,
    NativeTerminalSurfaceDrawCounters,
};
use crate::app::windows_frame::resolve_host_window_hwnd;

use super::backend::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
};
use super::windows_child_host::WindowsChildSurfaceHost;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{BOOL, D2DERR_RECREATE_TARGET, HWND};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_POINT_2F, D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED,
    D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    D2D1_BITMAP_PROPERTIES, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_OPACITY_MASK_CONTENT_GRAPHICS, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory,
    ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_FACE_TYPE_TRUETYPE, DWRITE_FONT_SIMULATIONS_NONE,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_GLYPH_OFFSET, DWRITE_GLYPH_RUN, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PIXEL_GEOMETRY,
    DWRITE_PIXEL_GEOMETRY_BGR, DWRITE_PIXEL_GEOMETRY_FLAT, DWRITE_PIXEL_GEOMETRY_RGB,
    DWRITE_RENDERING_MODE, DWRITE_RENDERING_MODE_ALIASED,
    DWRITE_RENDERING_MODE_CLEARTYPE_GDI_CLASSIC, DWRITE_RENDERING_MODE_CLEARTYPE_GDI_NATURAL,
    DWRITE_RENDERING_MODE_CLEARTYPE_NATURAL, DWRITE_RENDERING_MODE_CLEARTYPE_NATURAL_SYMMETRIC,
    DWRITE_RENDERING_MODE_DEFAULT, DWRITE_RENDERING_MODE_OUTLINE, DWriteCreateFactory,
    IDWriteFactory, IDWriteFactory5, IDWriteFontCollection, IDWriteFontFace, IDWriteFontFile,
    IDWriteInMemoryFontFileLoader, IDWriteRenderingParams,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
#[cfg(target_os = "windows")]
use windows::Win32::UI::HiDpi::GetDpiForWindow;
#[cfg(target_os = "windows")]
use windows::core::{Interface, PCWSTR};

#[cfg(target_os = "windows")]
const BUNDLED_JETBRAINS_MONO_FONT_BYTES: &[u8] =
    include_bytes!("../../../../assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf");
#[cfg(target_os = "windows")]
const BUNDLED_SARASA_TERM_SC_FONT_BYTES: &[u8] =
    include_bytes!("../../../../assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf");

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
    render_target: ID2D1HwndRenderTarget,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub struct WindowsDirectWriteTextRendererState {
    pub ready: bool,
    pub active_path: &'static str,
    #[cfg(target_os = "windows")]
    factory: Option<IDWriteFactory>,
    #[cfg(target_os = "windows")]
    font_collection: Option<IDWriteFontCollection>,
    #[cfg(target_os = "windows")]
    font_faces: HashMap<FontFaceKey, IDWriteFontFace>,
    #[cfg(target_os = "windows")]
    in_memory_font_file_loader: Option<IDWriteInMemoryFontFileLoader>,
    #[cfg(target_os = "windows")]
    in_memory_font_files: HashMap<FontFaceKey, IDWriteFontFile>,
    rendering_params_snapshot: Option<WindowsDirectWriteRenderingParamsSnapshot>,
}

impl Default for WindowsDirectWriteTextRendererState {
    fn default() -> Self {
        Self {
            ready: false,
            active_path: "bitmap-mask-compat",
            #[cfg(target_os = "windows")]
            factory: None,
            #[cfg(target_os = "windows")]
            font_collection: None,
            #[cfg(target_os = "windows")]
            font_faces: HashMap::new(),
            #[cfg(target_os = "windows")]
            in_memory_font_file_loader: None,
            #[cfg(target_os = "windows")]
            in_memory_font_files: HashMap::new(),
            rendering_params_snapshot: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct WindowsDirectWriteRenderingParamsSnapshot {
    source: &'static str,
    rendering_mode: &'static str,
    pixel_geometry: &'static str,
    gamma_per_mille: u32,
    enhanced_contrast_per_mille: u32,
    clear_type_level_per_mille: u32,
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
    pub surface_hwnd: Option<isize>,
    pub window_rect: NativeTerminalSurfaceRect,
    pub rect: NativeTerminalSurfaceRect,
    pub retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    pub d2d_factory: Option<WindowsD2DFactoryState>,
    pub directwrite_text_renderer: Option<WindowsDirectWriteTextRendererState>,
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
    pub last_directwrite_text_drawn: bool,
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

    fn ensure_directwrite_text_renderer(&mut self) {
        #[cfg(target_os = "windows")]
        if let Err(err) = self.try_ensure_directwrite_text_renderer() {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                "failed to create DirectWrite text renderer for native terminal surface"
            );
            if let Some(renderer) = self.directwrite_text_renderer.as_mut() {
                renderer.ready = false;
                renderer.active_path = "bitmap-mask-compat";
                renderer.rendering_params_snapshot = None;
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn try_ensure_directwrite_text_renderer(&mut self) -> Result<()> {
        if self
            .directwrite_text_renderer
            .as_ref()
            .map(|renderer| renderer.ready)
            .unwrap_or(false)
        {
            return Ok(());
        }

        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let mut font_collection = None;
        unsafe {
            factory.GetSystemFontCollection(&mut font_collection, false)?;
        }
        let Some(font_collection) = font_collection else {
            anyhow::bail!("DirectWrite returned no system font collection");
        };

        self.directwrite_text_renderer = Some(WindowsDirectWriteTextRendererState {
            ready: true,
            active_path: "directwrite-d2d",
            factory: Some(factory),
            font_collection: Some(font_collection),
            font_faces: HashMap::new(),
            in_memory_font_file_loader: None,
            in_memory_font_files: HashMap::new(),
            rendering_params_snapshot: None,
        });

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn try_ensure_hwnd_render_target(&mut self) -> Result<()> {
        let Some(surface_hwnd) = self.surface_hwnd else {
            self.clear_device_resources();
            return Ok(());
        };
        let Some((width_px, height_px)) = self.render_target_size_px() else {
            self.clear_device_resources();
            return Ok(());
        };

        self.ensure_d2d_factory();
        if self.d2d_factory.is_none() {
            return Ok(());
        }

        if let Some(target_state) = self.hwnd_render_target.as_ref()
            && target_state.hwnd == surface_hwnd
        {
            if !self.render_target_dirty {
                return Ok(());
            }

            let pixel_size = D2D_SIZE_U {
                width: width_px,
                height: height_px,
            };

            match unsafe { target_state.render_target.Resize(&pixel_size) } {
                Ok(()) => {
                    self.render_target_dirty = false;
                    return Ok(());
                }
                Err(err) if err.code() == D2DERR_RECREATE_TARGET => {
                    self.clear_device_resources();
                }
                Err(err) => return Err(err.into()),
            }
        }

        self.clear_device_resources();
        let factory = &self
            .d2d_factory
            .as_ref()
            .and_then(|state| state.factory.as_ref())
            .expect("Direct2D factory should exist before creating a render target");
        let render_target_properties = D2D1_RENDER_TARGET_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let hwnd_render_target_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: HWND(surface_hwnd as _),
            pixelSize: D2D_SIZE_U {
                width: width_px,
                height: height_px,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let render_target = unsafe {
            factory
                .CreateHwndRenderTarget(&render_target_properties, &hwnd_render_target_properties)
        }?;
        self.render_target_generation = self.render_target_generation.saturating_add(1);
        self.hwnd_render_target = Some(WindowsHwndRenderTargetState {
            hwnd: surface_hwnd,
            generation: self.render_target_generation,
            render_target,
        });
        self.render_target_dirty = false;

        Ok(())
    }

    fn clear_device_resources(&mut self) {
        self.hwnd_render_target = None;
        self.render_target_dirty = true;
        self.last_directwrite_text_drawn = false;
        for brush in self.d2d_brushes.values_mut() {
            brush.generation = 0;
            #[cfg(target_os = "windows")]
            {
                brush.brush = None;
            }
        }
        if let Some(renderer) = self.directwrite_text_renderer.as_mut() {
            renderer.active_path = if renderer.ready {
                "directwrite-d2d"
            } else {
                "bitmap-mask-compat"
            };
            renderer.rendering_params_snapshot = None;
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

    fn mark_directwrite_text_path(&mut self, active_path: &'static str) {
        if let Some(renderer) = self.directwrite_text_renderer.as_mut() {
            renderer.active_path = active_path;
            if active_path != "directwrite-d2d" {
                renderer.rendering_params_snapshot = None;
            }
        }
    }

    fn mark_directwrite_text_fallback(&mut self, reason: &'static str) {
        if self.active_text_renderer_path() != Some("bitmap-mask-compat") {
            tracing::debug!(
                target: "app.terminal",
                fallback_reason = reason,
                host_hwnd = self.host_hwnd.unwrap_or_default(),
                surface_hwnd = self.surface_hwnd.unwrap_or_default(),
                "DirectWrite text path fell back to bitmap-mask-compat"
            );
        }
        self.mark_directwrite_text_path("bitmap-mask-compat");
    }

    #[cfg(target_os = "windows")]
    fn snapshot_directwrite_rendering_params(
        source: &'static str,
        params: &IDWriteRenderingParams,
    ) -> WindowsDirectWriteRenderingParamsSnapshot {
        WindowsDirectWriteRenderingParamsSnapshot {
            source,
            rendering_mode: Self::directwrite_rendering_mode_name(unsafe {
                params.GetRenderingMode()
            }),
            pixel_geometry: Self::directwrite_pixel_geometry_name(unsafe {
                params.GetPixelGeometry()
            }),
            gamma_per_mille: Self::scale_rendering_param_to_per_mille(unsafe { params.GetGamma() }),
            enhanced_contrast_per_mille: Self::scale_rendering_param_to_per_mille(unsafe {
                params.GetEnhancedContrast()
            }),
            clear_type_level_per_mille: Self::scale_rendering_param_to_per_mille(unsafe {
                params.GetClearTypeLevel()
            }),
        }
    }

    #[cfg(target_os = "windows")]
    fn create_directwrite_rendering_params(
        factory: &IDWriteFactory,
        host_hwnd: isize,
    ) -> Option<(
        IDWriteRenderingParams,
        WindowsDirectWriteRenderingParamsSnapshot,
    )> {
        let monitor = unsafe { MonitorFromWindow(HWND(host_hwnd as _), MONITOR_DEFAULTTONEAREST) };
        let (base_params, base_source) = if monitor.0.is_null() {
            (
                unsafe { factory.CreateRenderingParams().ok()? },
                "system-default",
            )
        } else {
            unsafe { factory.CreateMonitorRenderingParams(monitor).ok() }
                .map(|params| (params, "monitor-default"))
                .or_else(|| {
                    unsafe { factory.CreateRenderingParams().ok() }
                        .map(|params| (params, "system-default"))
                })?
        };
        let gamma = unsafe { base_params.GetGamma() };
        let enhanced_contrast = unsafe { base_params.GetEnhancedContrast() };
        let clear_type_level = unsafe { base_params.GetClearTypeLevel() };
        let pixel_geometry = unsafe { base_params.GetPixelGeometry() };
        let rendering_mode = unsafe { base_params.GetRenderingMode() };
        let tuned_enhanced_contrast =
            Self::tuned_directwrite_enhanced_contrast(enhanced_contrast, pixel_geometry);

        if (tuned_enhanced_contrast - enhanced_contrast).abs() >= 0.001 {
            if let Ok(custom_params) = unsafe {
                factory.CreateCustomRenderingParams(
                    gamma,
                    tuned_enhanced_contrast,
                    clear_type_level,
                    pixel_geometry,
                    rendering_mode,
                )
            } {
                return Some((
                    custom_params.clone(),
                    Self::snapshot_directwrite_rendering_params(
                        match base_source {
                            "monitor-default" => "monitor-custom-contrast",
                            _ => "system-custom-contrast",
                        },
                        &custom_params,
                    ),
                ));
            }
        }

        Some((
            base_params.clone(),
            Self::snapshot_directwrite_rendering_params(base_source, &base_params),
        ))
    }

    #[cfg(target_os = "windows")]
    fn tuned_directwrite_enhanced_contrast(
        enhanced_contrast: f32,
        pixel_geometry: DWRITE_PIXEL_GEOMETRY,
    ) -> f32 {
        if pixel_geometry == DWRITE_PIXEL_GEOMETRY_FLAT {
            return enhanced_contrast;
        }

        // Keep monitor-provided gamma/ClearType values intact and only lift softer
        // defaults slightly so terminal stems read cleaner without changing metrics.
        enhanced_contrast.max(0.65).min(1.0)
    }

    #[cfg(target_os = "windows")]
    fn scale_rendering_param_to_per_mille(value: f32) -> u32 {
        (value.max(0.0) * 1000.0).round() as u32
    }

    #[cfg(target_os = "windows")]
    fn directwrite_pixel_geometry_name(pixel_geometry: DWRITE_PIXEL_GEOMETRY) -> &'static str {
        match pixel_geometry {
            DWRITE_PIXEL_GEOMETRY_FLAT => "flat",
            DWRITE_PIXEL_GEOMETRY_RGB => "rgb",
            DWRITE_PIXEL_GEOMETRY_BGR => "bgr",
            _ => "unknown",
        }
    }

    #[cfg(target_os = "windows")]
    fn directwrite_rendering_mode_name(rendering_mode: DWRITE_RENDERING_MODE) -> &'static str {
        match rendering_mode {
            DWRITE_RENDERING_MODE_DEFAULT => "default",
            DWRITE_RENDERING_MODE_ALIASED => "aliased",
            DWRITE_RENDERING_MODE_CLEARTYPE_GDI_CLASSIC => "cleartype-gdi-classic",
            DWRITE_RENDERING_MODE_CLEARTYPE_GDI_NATURAL => "cleartype-gdi-natural",
            DWRITE_RENDERING_MODE_CLEARTYPE_NATURAL => "cleartype-natural",
            DWRITE_RENDERING_MODE_CLEARTYPE_NATURAL_SYMMETRIC => "cleartype-natural-symmetric",
            DWRITE_RENDERING_MODE_OUTLINE => "outline",
            _ => "unknown",
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

    #[cfg(target_os = "windows")]
    fn bundled_directwrite_font_bytes(family_name: &str) -> Option<&'static [u8]> {
        if family_name.eq_ignore_ascii_case(DEFAULT_TERMINAL_FONT_FAMILY) {
            return Some(BUNDLED_JETBRAINS_MONO_FONT_BYTES);
        }

        family_name
            .eq_ignore_ascii_case(DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY)
            .then_some(BUNDLED_SARASA_TERM_SC_FONT_BYTES)
    }

    #[cfg(target_os = "windows")]
    fn try_resolve_bundled_directwrite_font_face(
        &mut self,
        face_key: FontFaceKey,
        family_name: &str,
    ) -> Option<IDWriteFontFace> {
        let font_bytes = Self::bundled_directwrite_font_bytes(family_name)?;
        let renderer = self.directwrite_text_renderer.as_mut()?;
        let factory = renderer.factory.as_ref()?.clone();

        let loader = if let Some(loader) = renderer.in_memory_font_file_loader.as_ref() {
            loader.clone()
        } else {
            let factory5: IDWriteFactory5 = factory.cast().ok()?;
            let loader = unsafe { factory5.CreateInMemoryFontFileLoader().ok()? };
            renderer.in_memory_font_file_loader = Some(loader.clone());
            loader
        };

        let font_file = if let Some(font_file) = renderer.in_memory_font_files.get(&face_key) {
            font_file.clone()
        } else {
            let font_file = unsafe {
                loader
                    .CreateInMemoryFontFileReference(
                        &factory,
                        font_bytes.as_ptr().cast(),
                        u32::try_from(font_bytes.len()).ok()?,
                        None::<&windows::core::IUnknown>,
                    )
                    .ok()?
            };
            renderer
                .in_memory_font_files
                .insert(face_key, font_file.clone());
            font_file
        };
        let font_face = unsafe {
            factory
                .CreateFontFace(
                    DWRITE_FONT_FACE_TYPE_TRUETYPE,
                    &[Some(font_file)],
                    0,
                    DWRITE_FONT_SIMULATIONS_NONE,
                )
                .ok()?
        };
        renderer.font_faces.insert(face_key, font_face.clone());
        Some(font_face)
    }

    #[cfg(target_os = "windows")]
    fn resolve_directwrite_font_face(
        &mut self,
        draw: &PreparedMonochromeGlyphDraw,
    ) -> Option<IDWriteFontFace> {
        self.ensure_directwrite_text_renderer();
        if let Some(font_face) = self
            .directwrite_text_renderer
            .as_ref()
            .and_then(|renderer| renderer.font_faces.get(&draw.face_key).cloned())
        {
            return Some(font_face);
        }

        if let Some(font_face) =
            self.try_resolve_bundled_directwrite_font_face(draw.face_key, &draw.font_family_name)
        {
            return Some(font_face);
        }

        let renderer = self.directwrite_text_renderer.as_mut()?;
        let font_collection = renderer.font_collection.as_ref()?;
        let family_name_utf16 = draw
            .font_family_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        let mut family_index = 0u32;
        let mut exists = BOOL(0);
        unsafe {
            font_collection
                .FindFamilyName(
                    PCWSTR(family_name_utf16.as_ptr()),
                    &mut family_index,
                    &mut exists,
                )
                .ok()?;
        }
        if !exists.as_bool() {
            return None;
        }

        let family = unsafe { font_collection.GetFontFamily(family_index).ok()? };
        let font = unsafe {
            family
                .GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT_REGULAR,
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                )
                .ok()?
        };
        let font_face = unsafe { font.CreateFontFace().ok()? };
        renderer.font_faces.insert(draw.face_key, font_face.clone());
        Some(font_face)
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_directwrite_font_face(&mut self, draw: &PreparedMonochromeGlyphDraw) -> Option<()> {
        let _ = draw;
        None
    }

    fn draw_directwrite_text(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        self.last_directwrite_text_drawn = false;
        self.last_drawn_monochrome_glyphs = 0;

        #[cfg(not(target_os = "windows"))]
        let _ = frame;

        #[cfg(target_os = "windows")]
        {
            self.ensure_directwrite_text_renderer();
            let Some(render_target) = self.render_target() else {
                return;
            };
            let Some(host_hwnd) = self.host_hwnd else {
                return;
            };

            let mut drawable_glyphs =
                Vec::with_capacity(frame.frame.presentable_frame.monochrome_glyph_draws.len());
            for draw in &frame.frame.presentable_frame.monochrome_glyph_draws {
                if draw.glyph_id > u16::MAX as u32 {
                    self.mark_directwrite_text_fallback("glyph-id-overflow");
                    return;
                }
                let Some(font_face) = self.resolve_directwrite_font_face(draw) else {
                    self.mark_directwrite_text_fallback("font-face-unresolved");
                    return;
                };
                drawable_glyphs.push((draw, font_face, draw.glyph_id as u16));
            }
            if drawable_glyphs.is_empty() {
                return;
            }

            let (text_rendering_params, rendering_params_snapshot) = self
                .directwrite_text_renderer
                .as_ref()
                .and_then(|renderer| renderer.factory.as_ref())
                .and_then(|factory| Self::create_directwrite_rendering_params(factory, host_hwnd))
                .map(|(params, snapshot)| (Some(params), Some(snapshot)))
                .unwrap_or((None, None));
            let text_antialias_mode = rendering_params_snapshot
                .as_ref()
                .map(|snapshot| {
                    if snapshot.pixel_geometry == "flat" || snapshot.clear_type_level_per_mille == 0
                    {
                        D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE
                    } else {
                        D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE
                    }
                })
                .unwrap_or(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);

            unsafe {
                render_target.SetTextAntialiasMode(text_antialias_mode);
                render_target.SetTextRenderingParams(text_rendering_params.as_ref());
            }
            if let Some(renderer) = self.directwrite_text_renderer.as_mut() {
                renderer.rendering_params_snapshot = rendering_params_snapshot;
            }

            for (draw, font_face, glyph_index) in drawable_glyphs {
                self.ensure_brush(draw.fg_rgba);
                let Some(brush) = self.brush_for(draw.fg_rgba) else {
                    self.mark_directwrite_text_fallback("missing-text-brush");
                    return;
                };

                let glyph_indices = [glyph_index];
                let glyph_advances = [draw.advance_px.max(0) as f32];
                let glyph_offsets = [DWRITE_GLYPH_OFFSET {
                    advanceOffset: 0.0,
                    ascenderOffset: 0.0,
                }];
                let baseline_origin = D2D_POINT_2F {
                    x: (frame.rect.x + draw.dest_x_px - draw.visible_left_px) as f32,
                    y: (frame.rect.y + draw.dest_y_px - draw.visible_top_px) as f32,
                };
                let glyph_run = DWRITE_GLYPH_RUN {
                    fontFace: core::mem::ManuallyDrop::new(Some(font_face.clone())),
                    fontEmSize: draw.font_em_size_px.max(1) as f32,
                    glyphCount: glyph_indices.len() as u32,
                    glyphIndices: glyph_indices.as_ptr(),
                    glyphAdvances: glyph_advances.as_ptr(),
                    glyphOffsets: glyph_offsets.as_ptr(),
                    isSideways: BOOL(0),
                    bidiLevel: 0,
                };

                unsafe {
                    render_target.DrawGlyphRun(
                        baseline_origin,
                        &glyph_run,
                        &brush,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
                self.last_drawn_monochrome_glyphs =
                    self.last_drawn_monochrome_glyphs.saturating_add(1);
            }

            self.last_directwrite_text_drawn = true;
            self.mark_directwrite_text_path("directwrite-d2d");
        }
    }

    fn draw_monochrome_glyphs(&mut self, frame: &RetainedNativeTerminalSurfaceFrame) {
        if self.last_directwrite_text_drawn {
            return;
        }
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
    fn render_target(&self) -> Option<ID2D1HwndRenderTarget> {
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

    fn active_text_renderer_path(&self) -> Option<&'static str> {
        self.directwrite_text_renderer
            .as_ref()
            .map(|renderer| renderer.active_path)
            .or(Some("bitmap-mask-compat"))
    }

    fn windows_text_diagnostics(&self) -> NativeTerminalSurfaceWindowsTextDiagnostics {
        NativeTerminalSurfaceWindowsTextDiagnostics {
            text_antialias_mode: Some(self.active_text_antialias_mode()),
            render_target_alpha_mode: Some(self.active_render_target_alpha_mode()),
            rendering_params_source: self.active_rendering_params_source(),
            rendering_mode: self.active_rendering_mode(),
            pixel_geometry: self.active_pixel_geometry(),
            gamma_per_mille: self.active_gamma_per_mille(),
            enhanced_contrast_per_mille: self.active_enhanced_contrast_per_mille(),
            clear_type_level_per_mille: self.active_clear_type_level_per_mille(),
            font_chain: self.active_font_chain(),
            baseline_px: self.active_baseline_px(),
            pixel_alignment: Some(self.active_pixel_alignment()),
            dpi_x: self.window_dpi().map(|(dpi_x, _)| dpi_x),
            dpi_y: self.window_dpi().map(|(_, dpi_y)| dpi_y),
            scale_factor_percent: self.active_scale_factor_percent(),
            glyph_bounds: self.active_glyph_bounds_trace(),
        }
    }

    fn active_text_antialias_mode(&self) -> &'static str {
        if self.active_text_renderer_path() != Some("directwrite-d2d") {
            return "bitmap-mask-compat";
        }

        if self.active_pixel_geometry() == Some("flat")
            || self.active_clear_type_level_per_mille() == Some(0)
        {
            "grayscale"
        } else {
            "cleartype"
        }
    }

    fn active_render_target_alpha_mode(&self) -> &'static str {
        "ignore"
    }

    fn active_rendering_params_source(&self) -> Option<&'static str> {
        if self.active_text_renderer_path() != Some("directwrite-d2d") {
            return None;
        }
        self.directwrite_text_renderer
            .as_ref()
            .and_then(|renderer| renderer.rendering_params_snapshot)
            .map(|snapshot| snapshot.source)
    }

    fn active_rendering_mode(&self) -> Option<&'static str> {
        if self.active_text_renderer_path() != Some("directwrite-d2d") {
            return None;
        }
        self.directwrite_text_renderer
            .as_ref()
            .and_then(|renderer| renderer.rendering_params_snapshot)
            .map(|snapshot| snapshot.rendering_mode)
    }

    fn active_pixel_geometry(&self) -> Option<&'static str> {
        if self.active_text_renderer_path() != Some("directwrite-d2d") {
            return None;
        }
        self.directwrite_text_renderer
            .as_ref()
            .and_then(|renderer| renderer.rendering_params_snapshot)
            .map(|snapshot| snapshot.pixel_geometry)
    }

    fn active_gamma_per_mille(&self) -> Option<u32> {
        if self.active_text_renderer_path() != Some("directwrite-d2d") {
            return None;
        }
        self.directwrite_text_renderer
            .as_ref()
            .and_then(|renderer| renderer.rendering_params_snapshot)
            .map(|snapshot| snapshot.gamma_per_mille)
    }

    fn active_enhanced_contrast_per_mille(&self) -> Option<u32> {
        if self.active_text_renderer_path() != Some("directwrite-d2d") {
            return None;
        }
        self.directwrite_text_renderer
            .as_ref()
            .and_then(|renderer| renderer.rendering_params_snapshot)
            .map(|snapshot| snapshot.enhanced_contrast_per_mille)
    }

    fn active_clear_type_level_per_mille(&self) -> Option<u32> {
        if self.active_text_renderer_path() != Some("directwrite-d2d") {
            return None;
        }
        self.directwrite_text_renderer
            .as_ref()
            .and_then(|renderer| renderer.rendering_params_snapshot)
            .map(|snapshot| snapshot.clear_type_level_per_mille)
    }

    fn active_font_chain(&self) -> Vec<String> {
        let Some(frame) = self.retained_frame.as_ref() else {
            return Vec::new();
        };

        let mut font_chain = Vec::new();
        for draw in &frame.frame.presentable_frame.monochrome_glyph_draws {
            if !font_chain.contains(&draw.font_family_name) {
                font_chain.push(draw.font_family_name.clone());
            }
        }
        font_chain
    }

    fn active_baseline_px(&self) -> Option<i32> {
        let frame = self.retained_frame.as_ref()?;
        let draw = frame
            .frame
            .presentable_frame
            .monochrome_glyph_draws
            .first()?;
        let row_top_px = (draw.row as i32).saturating_mul(frame.frame.cell_height_px as i32);
        Some(
            draw.dest_y_px
                .saturating_sub(draw.visible_top_px)
                .saturating_sub(row_top_px),
        )
    }

    fn active_pixel_alignment(&self) -> &'static str {
        "pixel-snapped"
    }

    fn active_scale_factor_percent(&self) -> Option<u32> {
        let (dpi_x, _) = self.window_dpi()?;
        Some(dpi_x.saturating_mul(100) / 96)
    }

    fn active_glyph_bounds_trace(&self) -> Vec<NativeTerminalSurfaceGlyphBoundsTrace> {
        let Some(frame) = self.retained_frame.as_ref() else {
            return Vec::new();
        };

        frame
            .frame
            .presentable_frame
            .monochrome_glyph_draws
            .iter()
            .take(6)
            .map(|draw| NativeTerminalSurfaceGlyphBoundsTrace {
                glyph_id: draw.glyph_id,
                row: draw.row,
                start_col: draw.start_col,
                end_col: draw.end_col,
                atlas_slot: draw.atlas_entry.slot,
                screen_left_px: self.window_rect.x.saturating_add(draw.dest_x_px),
                screen_top_px: self.window_rect.y.saturating_add(draw.dest_y_px),
                screen_width_px: draw.visible_width_px,
                screen_height_px: draw.visible_height_px,
                visible_left_px: draw.visible_left_px,
                visible_top_px: draw.visible_top_px,
                visible_width_px: draw.visible_width_px,
                visible_height_px: draw.visible_height_px,
            })
            .collect()
    }

    #[cfg(target_os = "windows")]
    fn window_dpi(&self) -> Option<(u32, u32)> {
        let hwnd = self.host_hwnd?;
        let dpi = unsafe { GetDpiForWindow(HWND(hwnd as _)) };
        (dpi != 0).then_some((dpi, dpi))
    }

    #[cfg(not(target_os = "windows"))]
    fn window_dpi(&self) -> Option<(u32, u32)> {
        None
    }

    #[cfg(target_os = "windows")]
    fn begin_frame(&mut self, clip_rect: NativeTerminalSurfaceRect) -> bool {
        let Some(render_target) = self.render_target() else {
            return false;
        };
        let clip_rect = terminal_clip_rect(clip_rect);
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
            return false;
        };
        unsafe {
            render_target.PopAxisAlignedClip();
        }
        if let Err(err) = unsafe { render_target.EndDraw(None, None) } {
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
        true
    }

    fn sync_surface_rect(&mut self) {
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
    child_surface_host: Option<WindowsChildSurfaceHost>,
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
            self.destroy_child_surface_window();
            self.state.clear_device_resources();
            self.state.host_hwnd = next_host_hwnd;
        }

        self.state.sync_surface_rect();
    }

    fn ensure_child_surface_window(&mut self) {
        let Some(host_hwnd) = self.state.host_hwnd else {
            self.destroy_child_surface_window();
            return;
        };
        if self.state.window_rect.width <= 0 || self.state.window_rect.height <= 0 {
            self.sync_child_surface_window_rect();
            return;
        }

        let needs_new_child = self
            .child_surface_host
            .as_ref()
            .map(|host| host.parent_hwnd != host_hwnd || host.surface_hwnd == 0)
            .unwrap_or(true);

        if needs_new_child {
            self.destroy_child_surface_window();
            match WindowsChildSurfaceHost::create(host_hwnd, self.state.window_rect) {
                Ok(child_surface_host) => {
                    tracing::debug!(
                        target: "app.terminal",
                        host_hwnd,
                        surface_hwnd = child_surface_host.surface_hwnd,
                        x = self.state.window_rect.x,
                        y = self.state.window_rect.y,
                        width = self.state.window_rect.width,
                        height = self.state.window_rect.height,
                        "created retained-native child HWND host"
                    );
                    self.state.surface_hwnd = Some(child_surface_host.surface_hwnd);
                    self.child_surface_host = Some(child_surface_host);
                    self.state.mark_render_target_dirty();
                }
                Err(err) => {
                    self.state.surface_hwnd = None;
                    tracing::warn!(
                        target: "app.terminal",
                        error = %err,
                        "failed to create retained-native child HWND host"
                    );
                }
            }
        }

        self.sync_child_surface_window_rect();
    }

    fn sync_child_surface_window_rect(&mut self) {
        self.state.sync_surface_rect();

        let should_show = self.state.attached
            && self.state.host_hwnd.is_some()
            && self.state.window_rect.width > 0
            && self.state.window_rect.height > 0;

        if !should_show {
            if let Some(child_surface_host) = self.child_surface_host.as_ref() {
                tracing::debug!(
                    target: "app.terminal",
                    host_hwnd = self.state.host_hwnd.unwrap_or_default(),
                    surface_hwnd = child_surface_host.surface_hwnd,
                    attached = self.state.attached,
                    x = self.state.window_rect.x,
                    y = self.state.window_rect.y,
                    width = self.state.window_rect.width,
                    height = self.state.window_rect.height,
                    "tearing down retained-native child HWND because the surface is not visible"
                );
            }
            self.destroy_child_surface_window();
            return;
        }

        let Some(child_surface_host) = self.child_surface_host.as_mut() else {
            self.state.surface_hwnd = None;
            return;
        };

        if let Err(err) = child_surface_host.sync_rect(self.state.window_rect) {
            tracing::warn!(
                target: "app.terminal",
                error = %err,
                "failed to sync retained-native child HWND rect"
            );
            self.destroy_child_surface_window();
            return;
        }
        child_surface_host.set_visible(true);

        self.state.surface_hwnd =
            (child_surface_host.surface_hwnd != 0).then_some(child_surface_host.surface_hwnd);
        self.state.mark_render_target_dirty();
    }

    fn destroy_child_surface_window(&mut self) {
        if let Some(mut child_surface_host) = self.child_surface_host.take() {
            tracing::debug!(
                target: "app.terminal",
                host_hwnd = self.state.host_hwnd.unwrap_or_default(),
                surface_hwnd = child_surface_host.surface_hwnd,
                "destroying retained-native child HWND host"
            );
            child_surface_host.destroy();
        }
        self.state.surface_hwnd = None;
        self.state.clear_device_resources();
        self.state.sync_surface_rect();
    }
}

impl PlatformNativeSurfaceBackend for WindowsNativeSurfaceBackend {
    fn attach(&mut self, window: &AppWindow) -> Result<()> {
        self.state.attached = true;
        self.host_window = Some(window.as_weak());
        self.resolve_host_hwnd_if_needed();
        self.ensure_child_surface_window();
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
            self.ensure_child_surface_window();
            self.sync_child_surface_window_rect();
        }
    }

    fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>) {
        if !self.state.attached {
            return;
        }
        self.resolve_host_hwnd_if_needed();
        self.ensure_child_surface_window();
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
        self.ensure_child_surface_window();
        if self.state.host_hwnd.is_none() || self.state.surface_hwnd.is_none() {
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
        let present_rect = resolved_present_rect(self.state.rect, damage);

        #[cfg(target_os = "windows")]
        if !self.state.begin_frame(present_rect) {
            return;
        }

        #[cfg(not(target_os = "windows"))]
        let _ = present_rect;

        self.state.draw_background_runs(frame);
        self.state.draw_selection_overlay(frame);
        self.state.draw_directwrite_text(frame);
        self.state.draw_monochrome_glyphs(frame);
        self.state.draw_color_glyphs(frame);
        self.state.draw_underline_overlay(frame);
        self.state.draw_cursor_overlay(frame);
        self.state.draw_ime_preview_overlay(frame);

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
            hwnd: self.state.surface_hwnd.or(self.state.host_hwnd),
            host_hwnd: self.state.host_hwnd,
            surface_hwnd: self.state.surface_hwnd,
            surface_visible: Some(
                self.state.attached
                    && self.state.surface_hwnd.is_some()
                    && self.state.window_rect.width > 0
                    && self.state.window_rect.height > 0,
            ),
            render_target_ready: Some(self.state.hwnd_render_target.is_some()),
            text_renderer_path: Some(
                self.state
                    .active_text_renderer_path()
                    .unwrap_or("bitmap-mask-compat"),
            ),
            windows_text: Some(self.state.windows_text_diagnostics()),
            render_target_generation: self.state.render_target_generation,
            last_prepared_frame_token: self.state.last_prepared_frame_token,
            last_presented_frame_token: self.state.last_presented_frame_token,
            scheduled_present_count: 0,
            host_redraw_request_count: 0,
            host_redraw_replay_count: 0,
            draw_counters: self.state.draw_counters(),
        }
    }

    fn detach(&mut self) {
        self.state.attached = false;
        self.state.retained_frame = None;
        self.destroy_child_surface_window();
        self.state.clear_device_resources();
        self.state.d2d_brushes.clear();
        // Keep CPU-side glyph payload caches across detach so a later reattach can
        // recreate D2D bitmaps even when the renderer reuses prepared rows without
        // resending upload payloads on the next frame.
        self.state.d2d_factory = None;
        self.state.directwrite_text_renderer = None;
        self.state.host_hwnd = None;
        self.state.surface_hwnd = None;
        self.host_window = None;
        self.state.window_rect = NativeTerminalSurfaceRect::default();
        self.state.rect = NativeTerminalSurfaceRect::default();
        self.state.render_target_generation = 0;
        self.state.render_target_dirty = false;
        self.state.last_prepared_frame_token = 0;
        self.state.last_presented_frame_token = 0;
    }
}

fn resolved_present_rect(
    surface_rect: NativeTerminalSurfaceRect,
    damage: NativeSurfaceDamage,
) -> NativeTerminalSurfaceRect {
    match damage.kind {
        NativeSurfaceDamageKind::OverlayOnly => intersect_present_rect(surface_rect, damage.rect),
        NativeSurfaceDamageKind::Full | NativeSurfaceDamageKind::None => surface_rect,
    }
}

fn intersect_present_rect(
    surface_rect: NativeTerminalSurfaceRect,
    damage_rect: NativeTerminalSurfaceRect,
) -> NativeTerminalSurfaceRect {
    if surface_rect.width <= 0
        || surface_rect.height <= 0
        || damage_rect.width <= 0
        || damage_rect.height <= 0
    {
        return surface_rect;
    }

    let left = surface_rect.x.max(damage_rect.x);
    let top = surface_rect.y.max(damage_rect.y);
    let right = surface_rect
        .x
        .saturating_add(surface_rect.width)
        .min(damage_rect.x.saturating_add(damage_rect.width));
    let bottom = surface_rect
        .y
        .saturating_add(surface_rect.height)
        .min(damage_rect.y.saturating_add(damage_rect.height));

    if right <= left || bottom <= top {
        surface_rect
    } else {
        NativeTerminalSurfaceRect {
            x: left,
            y: top,
            width: right.saturating_sub(left),
            height: bottom.saturating_sub(top),
        }
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
