//! Single terminal rendering entry point used by bootstrap and UI projection code.

use anyhow::Result;

use crate::app::runtime_profile::TerminalRenderMode;
use crate::app::ssh::runtime::TerminalSurfaceState;
use crate::app::terminal_atlas::TerminalAtlasSelection;
use crate::app::terminal_presenter::{
    PresentedTerminalFrame, TerminalPresentationOptions, TerminalPresenter,
    TerminalPresenterCacheStats,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalRendererHostOptions {
    pub selection: Option<TerminalAtlasSelection>,
    pub selection_overlay_rgba: u32,
}

pub struct TerminalRendererHost {
    presenter: Box<dyn TerminalPresenter>,
    render_mode: TerminalRenderMode,
}

impl TerminalRendererHost {
    pub fn new(presenter: Box<dyn TerminalPresenter>, render_mode: TerminalRenderMode) -> Self {
        Self {
            presenter,
            render_mode,
        }
    }

    pub fn render_mode(&self) -> TerminalRenderMode {
        self.render_mode
    }

    pub fn render_mode_label(&self) -> &'static str {
        self.render_mode.as_str()
    }

    pub fn set_raster_scale(&mut self, scale_factor: f32) {
        self.presenter.set_raster_scale(scale_factor);
    }

    pub fn default_cell_size(&self) -> (u32, u32) {
        self.presenter.default_cell_size()
    }

    pub fn cache_stats(&self) -> TerminalPresenterCacheStats {
        self.presenter.cache_stats()
    }

    pub fn cache_reset_generation(&self) -> u64 {
        self.presenter.cache_reset_generation()
    }

    pub fn clear_transient_caches(&mut self) {
        self.presenter.clear_transient_caches();
    }

    pub fn present(
        &mut self,
        surface: &TerminalSurfaceState,
        options: TerminalRendererHostOptions,
    ) -> Result<PresentedTerminalFrame> {
        self.present_surface_update(surface, options)
    }

    pub fn present_surface_update(
        &mut self,
        surface: &TerminalSurfaceState,
        options: TerminalRendererHostOptions,
    ) -> Result<PresentedTerminalFrame> {
        self.presenter.present(
            surface,
            TerminalPresentationOptions {
                selection: options.selection,
                selection_overlay_rgba: options.selection_overlay_rgba,
                ..TerminalPresentationOptions::default()
            },
        )
    }
}
