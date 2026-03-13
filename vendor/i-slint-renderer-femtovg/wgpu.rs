// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, rc::Rc};

use i_slint_core::{api::PhysicalSize as PhysicalWindowSize, graphics::RequestedGraphicsAPI};

use crate::{FemtoVGRenderer, GraphicsBackend, WindowSurface, wgpu::wgpu::Texture};

use wgpu_28 as wgpu;

fn wgpu_init_trace_enabled() -> bool {
    std::env::var_os("MICA_TRACE_RENDER_PIPELINE").is_some()
}

fn select_preferred_present_mode(present_modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if present_modes.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else if present_modes.contains(&wgpu::PresentMode::FifoRelaxed) {
        wgpu::PresentMode::FifoRelaxed
    } else if present_modes.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else {
        present_modes
            .first()
            .copied()
            .unwrap_or(wgpu::PresentMode::Fifo)
    }
}

fn select_preferred_alpha_mode(
    alpha_modes: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    if alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
        wgpu::CompositeAlphaMode::PreMultiplied
    } else if alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
        wgpu::CompositeAlphaMode::PostMultiplied
    } else if alpha_modes.contains(&wgpu::CompositeAlphaMode::Inherit) {
        wgpu::CompositeAlphaMode::Inherit
    } else if alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
        wgpu::CompositeAlphaMode::Opaque
    } else {
        alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestedWgpuApiSummary {
    api: &'static str,
    backends: Option<wgpu::Backends>,
}

fn summarize_requested_wgpu_api(
    requested_graphics_api: Option<&RequestedGraphicsAPI>,
) -> RequestedWgpuApiSummary {
    match requested_graphics_api {
        Some(RequestedGraphicsAPI::WGPU28(
            i_slint_core::graphics::wgpu_28::api::WGPUConfiguration::Automatic(settings),
        )) => RequestedWgpuApiSummary {
            api: "wgpu28-automatic",
            backends: Some(settings.backends),
        },
        Some(RequestedGraphicsAPI::WGPU28(
            i_slint_core::graphics::wgpu_28::api::WGPUConfiguration::Manual { .. },
        )) => RequestedWgpuApiSummary {
            api: "wgpu28-manual",
            backends: None,
        },
        Some(RequestedGraphicsAPI::WGPU28(_)) => RequestedWgpuApiSummary {
            api: "wgpu28-other",
            backends: None,
        },
        #[cfg(feature = "unstable-wgpu-27")]
        Some(RequestedGraphicsAPI::WGPU27(..)) => RequestedWgpuApiSummary {
            api: "wgpu27",
            backends: None,
        },
        Some(RequestedGraphicsAPI::OpenGL(_)) => RequestedWgpuApiSummary {
            api: "opengl",
            backends: None,
        },
        Some(RequestedGraphicsAPI::Metal) => RequestedWgpuApiSummary {
            api: "metal",
            backends: None,
        },
        Some(RequestedGraphicsAPI::Vulkan) => RequestedWgpuApiSummary {
            api: "vulkan",
            backends: None,
        },
        Some(RequestedGraphicsAPI::Direct3D) => RequestedWgpuApiSummary {
            api: "direct3d",
            backends: None,
        },
        None => RequestedWgpuApiSummary {
            api: "none",
            backends: None,
        },
    }
}

pub struct WGPUBackend {
    instance: RefCell<Option<wgpu::Instance>>,
    device: RefCell<Option<wgpu::Device>>,
    queue: RefCell<Option<wgpu::Queue>>,
    surface_config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    surface: RefCell<Option<wgpu::Surface<'static>>>,
}

pub struct WGPUWindowSurface {
    surface_texture: wgpu::SurfaceTexture,
}

impl WindowSurface<femtovg::renderer::WGPURenderer> for WGPUWindowSurface {
    fn render_surface(&self) -> &Texture {
        &self.surface_texture.texture
    }
}

impl GraphicsBackend for WGPUBackend {
    type Renderer = femtovg::renderer::WGPURenderer;
    type WindowSurface = WGPUWindowSurface;
    const NAME: &'static str = "WGPU";

    fn new_suspended() -> Self {
        Self {
            instance: Default::default(),
            device: Default::default(),
            queue: Default::default(),
            surface_config: Default::default(),
            surface: Default::default(),
        }
    }

    fn clear_graphics_context(&self) {
        self.surface_config.borrow_mut().take();
        self.surface.borrow_mut().take();
        self.queue.borrow_mut().take();
        self.device.borrow_mut().take();
    }

    fn begin_surface_rendering(
        &self,
    ) -> Result<Self::WindowSurface, Box<dyn std::error::Error + Send + Sync>> {
        let surface = self.surface.borrow();
        let surface = surface.as_ref().unwrap();
        let frame = match surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Timeout) => surface.get_current_texture()?,
            // Outdated or lost: re-configure and try again
            Err(_) => {
                let mut device = self.device.borrow_mut();
                let device = device.as_mut().unwrap();

                surface.configure(device, self.surface_config.borrow().as_ref().unwrap());
                surface.get_current_texture()?
            }
        };
        Ok(WGPUWindowSurface { surface_texture: frame })
    }

    fn submit_commands(&self, commands: <Self::Renderer as femtovg::Renderer>::CommandBuffer) {
        self.queue.borrow().as_ref().unwrap().submit(Some(commands));
    }

    fn present_surface(
        &self,
        surface: Self::WindowSurface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        surface.surface_texture.present();
        Ok(())
    }

    #[cfg(feature = "unstable-wgpu-28")]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        let instance = self.instance.borrow().clone();
        let device = self.device.borrow().clone();
        let queue = self.queue.borrow().clone();
        if let (Some(instance), Some(device), Some(queue)) = (instance, device, queue) {
            Ok(callback(Some(i_slint_core::graphics::create_graphics_api_wgpu_28(
                instance, device, queue,
            ))))
        } else {
            Ok(callback(None))
        }
    }

    #[cfg(not(feature = "unstable-wgpu-28"))]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        Ok(callback(None))
    }

    fn resize(
        &self,
        width: std::num::NonZeroU32,
        height: std::num::NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Try to get hold of the wgpu types, but if we receive the resize event while suspended, ignore it.
        let mut surface_config = self.surface_config.borrow_mut();
        let Some(surface_config) = surface_config.as_mut() else { return Ok(()) };
        let mut device = self.device.borrow_mut();
        let Some(device) = device.as_mut() else { return Ok(()) };
        let mut surface = self.surface.borrow_mut();
        let Some(surface) = surface.as_mut() else { return Ok(()) };

        surface_config.width = width.get();
        surface_config.height = height.get();

        surface.configure(device, surface_config);
        Ok(())
    }
}

impl FemtoVGRenderer<WGPUBackend> {
    pub fn set_window_handle(
        &self,
        window_handle: Box<dyn wgpu::WindowHandle>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let requested_wgpu_api = summarize_requested_wgpu_api(requested_graphics_api.as_ref());
        if wgpu_init_trace_enabled() {
            tracing::info!(
                target: "app.renderer",
                requested_api = requested_wgpu_api.api,
                requested_backends = ?requested_wgpu_api.backends,
                backends_to_avoid = ?wgpu::Backends::GL,
                requested_width = size.width,
                requested_height = size.height,
                "femtovg renderer received requested graphics api"
            );
        }

        let (instance, adapter, device, queue, surface) =
            i_slint_core::graphics::wgpu_28::init_instance_adapter_device_queue_surface(
                window_handle,
                requested_graphics_api,
                /* rendering artifacts :( */
                wgpu::Backends::GL,
            )?;

        let mut surface_config =
            surface.get_default_config(&adapter, size.width, size.height).unwrap();

        let adapter_info = adapter.get_info();
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        if wgpu_init_trace_enabled() {
            tracing::info!(
                target: "app.renderer",
                backend = ?adapter_info.backend,
                device_type = ?adapter_info.device_type,
                adapter_name = %adapter_info.name,
                vendor = adapter_info.vendor,
                device = adapter_info.device,
                driver = %adapter_info.driver,
                driver_info = %adapter_info.driver_info,
                "wgpu adapter initialized for femtovg renderer"
            );
            tracing::info!(
                target: "app.renderer",
                surface_formats = ?swapchain_capabilities.formats,
                present_modes = ?swapchain_capabilities.present_modes,
                alpha_modes = ?swapchain_capabilities.alpha_modes,
                requested_width = size.width,
                requested_height = size.height,
                "wgpu surface capabilities resolved for femtovg renderer"
            );
            tracing::info!(
                target: "app.renderer",
                surface_format = ?surface_config.format,
                present_mode = ?surface_config.present_mode,
                alpha_mode = ?surface_config.alpha_mode,
                width = surface_config.width,
                height = surface_config.height,
                "wgpu default surface configuration resolved for femtovg renderer"
            );
        }

        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|f| {
                matches!(f, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm)
            })
            .copied()
            .unwrap_or_else(|| swapchain_capabilities.formats[0]);
        surface_config.format = swapchain_format;
        surface_config.present_mode =
            select_preferred_present_mode(&swapchain_capabilities.present_modes);
        surface_config.alpha_mode =
            select_preferred_alpha_mode(&swapchain_capabilities.alpha_modes);
        if wgpu_init_trace_enabled() {
            tracing::info!(
                target: "app.renderer",
                surface_format = ?surface_config.format,
                present_mode = ?surface_config.present_mode,
                alpha_mode = ?surface_config.alpha_mode,
                width = surface_config.width,
                height = surface_config.height,
                "wgpu surface configured for femtovg renderer"
            );
        }
        surface.configure(&device, &surface_config);

        *self.graphics_backend.instance.borrow_mut() = Some(instance.clone());
        *self.graphics_backend.device.borrow_mut() = Some(device.clone());
        *self.graphics_backend.queue.borrow_mut() = Some(queue.clone());
        *self.graphics_backend.surface_config.borrow_mut() = Some(surface_config);
        *self.graphics_backend.surface.borrow_mut() = Some(surface);

        let wgpu_renderer = femtovg::renderer::WGPURenderer::new(device, queue);
        let femtovg_canvas = femtovg::Canvas::new_with_text_context(
            wgpu_renderer,
            crate::font_cache::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();

        let canvas = Rc::new(RefCell::new(femtovg_canvas));
        self.reset_canvas(canvas);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        select_preferred_alpha_mode, select_preferred_present_mode, summarize_requested_wgpu_api,
    };
    use i_slint_core::graphics::RequestedGraphicsAPI;
    use wgpu_28 as wgpu;

    #[test]
    fn selects_fifo_present_mode_when_available() {
        let selected = select_preferred_present_mode(&[
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::FifoRelaxed,
        ]);

        assert_eq!(selected, wgpu::PresentMode::Fifo);
    }

    #[test]
    fn selects_blended_alpha_mode_before_opaque() {
        let selected = select_preferred_alpha_mode(&[
            wgpu::CompositeAlphaMode::Opaque,
            wgpu::CompositeAlphaMode::PreMultiplied,
        ]);

        assert_eq!(selected, wgpu::CompositeAlphaMode::PreMultiplied);
    }

    #[test]
    fn falls_back_to_opaque_alpha_when_no_blended_mode_exists() {
        let selected = select_preferred_alpha_mode(&[wgpu::CompositeAlphaMode::Opaque]);

        assert_eq!(selected, wgpu::CompositeAlphaMode::Opaque);
    }

    #[test]
    fn summarizes_absent_requested_graphics_api() {
        let summary = summarize_requested_wgpu_api(None);

        assert_eq!(summary.api, "none");
        assert_eq!(summary.backends, None);
    }

    #[test]
    fn summarizes_wgpu28_automatic_requested_backends() {
        let mut settings = i_slint_core::graphics::wgpu_28::api::WGPUSettings::default();
        settings.backends = wgpu::Backends::DX12;
        let requested =
            RequestedGraphicsAPI::WGPU28(i_slint_core::graphics::wgpu_28::api::WGPUConfiguration::Automatic(settings));

        let summary = summarize_requested_wgpu_api(Some(&requested));

        assert_eq!(summary.api, "wgpu28-automatic");
        assert_eq!(summary.backends, Some(wgpu::Backends::DX12));
    }
}
