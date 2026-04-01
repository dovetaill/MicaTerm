//! Phase-1 WezTerm font adapter scaffold for the terminal rendering stack.

/// Captures where the local WezTerm font adoption work currently stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeztermFontIntegrationStage {
    /// The local adapter exists, but the repo still uses the legacy shaping path.
    Scaffold,
    /// Cargo can resolve the upstream workspace, but the dependency stack is still too heavy.
    PendingDependencyPruning,
}

/// Placeholder adapter for the first WezTerm font adoption phase.
///
/// Later tasks will replace this with a concrete wrapper around `wezterm-font`
/// shaping and rasterization primitives once the existing HarfBuzz dependency
/// path has been replaced.
#[derive(Debug, Default)]
pub struct WeztermFontSystem;

impl WeztermFontSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn integration_stage(&self) -> WeztermFontIntegrationStage {
        WeztermFontIntegrationStage::PendingDependencyPruning
    }

    pub fn upstream_sources(&self) -> &'static [&'static str] {
        &[
            "wezterm-font/Cargo.toml",
            "wezterm-font/src/lib.rs",
            "wezterm-font/src/shaper/mod.rs",
            "wezterm-font/src/rasterizer/mod.rs",
            "wezterm-gui/src/termwindow/render/mod.rs",
            "wezterm-gui/src/renderstate.rs",
            "wezterm-gui/src/termwindow/webgpu.rs",
        ]
    }

    pub fn integration_blocker(&self) -> &'static str {
        "direct cargo integration now resolves through git, but the upstream crate still pulls in cairo-rs, freetype, harfbuzz, and a large wezterm workspace dependency graph that is too heavy for this repo's current Windows-first embedding target"
    }
}

#[cfg(test)]
mod tests {
    use super::{WeztermFontIntegrationStage, WeztermFontSystem};

    #[test]
    fn scaffold_reports_current_migration_stage_and_blocker() {
        let adapter = WeztermFontSystem::new();

        assert_eq!(
            adapter.integration_stage(),
            WeztermFontIntegrationStage::PendingDependencyPruning
        );
        assert!(adapter.integration_blocker().contains("cairo-rs"));
        assert!(
            adapter
                .upstream_sources()
                .iter()
                .any(|path| path.contains("wezterm-font/Cargo.toml"))
        );
        assert!(
            adapter
                .upstream_sources()
                .iter()
                .any(|path| path.contains("wezterm-font/src/lib.rs"))
        );
    }
}
