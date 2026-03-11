use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLogMode {
    ErrorOnly,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLoggingConfig {
    pub mode: AppLogMode,
}

impl AppLoggingConfig {
    pub fn new(mode: AppLogMode) -> Self {
        Self { mode }
    }

    pub fn from_env() -> Self {
        let mode = match env::var("MICA_TERM_LOG").ok().as_deref() {
            Some("debug" | "trace") => AppLogMode::Debug,
            _ => AppLogMode::ErrorOnly,
        };

        Self::new(mode)
    }

    pub fn filter_directive(self) -> &'static str {
        match self.mode {
            AppLogMode::ErrorOnly => "error",
            AppLogMode::Debug => "debug",
        }
    }
}
