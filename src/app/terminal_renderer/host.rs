//! Single terminal rendering entry point used by bootstrap and UI projection code.

use anyhow::Result;

use crate::app::runtime_profile::TerminalRenderMode;
use crate::app::ssh::runtime::TerminalSurfaceState;
use crate::app::terminal_atlas::TerminalAtlasSelection;
use crate::app::terminal_presenter::{
    PresentedTerminalFrame, TerminalPresentationOptions, TerminalPresenter,
    TerminalPresenterCacheStats,
};
use crate::app::terminal_semantic::OutputRuleProfile;
use crate::theme::{SearchMatchHighlightStrength, ThemeMode, ThemeVariant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalRendererHostOptions {
    pub selection: Option<TerminalAtlasSelection>,
    pub selection_overlay_rgba: u32,
    pub theme_mode: ThemeMode,
    pub theme_variant: ThemeVariant,
    pub input_highlighting_enabled: bool,
    pub output_rule_highlighting_enabled: bool,
    pub output_rule_profile: OutputRuleProfile,
    pub command_decorations_enabled: bool,
    pub overview_markers_enabled: bool,
    pub search_query: Option<String>,
    pub search_match_highlight: SearchMatchHighlightStrength,
}

impl Default for TerminalRendererHostOptions {
    fn default() -> Self {
        Self {
            selection: None,
            selection_overlay_rgba: 0,
            theme_mode: ThemeMode::Dark,
            theme_variant: ThemeVariant::PremiumDefault,
            input_highlighting_enabled: true,
            output_rule_highlighting_enabled: true,
            output_rule_profile: OutputRuleProfile::Default,
            command_decorations_enabled: true,
            overview_markers_enabled: true,
            search_query: None,
            search_match_highlight: SearchMatchHighlightStrength::Balanced,
        }
    }
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
                theme_mode: options.theme_mode,
                theme_variant: options.theme_variant,
                input_highlighting_enabled: options.input_highlighting_enabled,
                output_rule_highlighting_enabled: options.output_rule_highlighting_enabled,
                output_rule_profile: options.output_rule_profile,
                command_decorations_enabled: options.command_decorations_enabled,
                overview_markers_enabled: options.overview_markers_enabled,
                search_query: options.search_query,
                search_match_highlight: options.search_match_highlight,
                ..TerminalPresentationOptions::default()
            },
        )
    }
}
