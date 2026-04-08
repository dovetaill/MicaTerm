//! Environment-driven logging configuration shared by the binary and test harnesses.

use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLogMode {
    ErrorOnly,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLoggingConfig {
    pub mode: AppLogMode,
    pub memory_diagnostics: bool,
}

impl AppLoggingConfig {
    pub fn new(mode: AppLogMode) -> Self {
        Self {
            mode,
            memory_diagnostics: false,
        }
    }

    pub fn with_memory_diagnostics(mut self, enabled: bool) -> Self {
        self.memory_diagnostics = enabled;
        self
    }

    pub fn from_env() -> Self {
        let mode = match env::var("MICA_TERM_LOG").ok().as_deref() {
            Some("debug" | "trace") => AppLogMode::Debug,
            _ => AppLogMode::ErrorOnly,
        };
        let memory_diagnostics = matches!(
            env::var("MICA_TERM_MEMORY_DIAGNOSTICS").ok().as_deref(),
            Some("1" | "true" | "yes" | "on")
        );

        Self::new(mode).with_memory_diagnostics(memory_diagnostics)
    }

    pub fn filter_directive(self) -> &'static str {
        match self.mode {
            AppLogMode::ErrorOnly => "error",
            AppLogMode::Debug => "debug",
        }
    }

    pub fn memory_diagnostics_enabled(self) -> bool {
        self.memory_diagnostics
    }
}
