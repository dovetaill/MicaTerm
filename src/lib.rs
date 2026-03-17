//! Shared library entrypoint for the application modules and generated Slint bindings.

pub mod app;
pub mod shell;
pub mod theme;

slint::include_modules!();
