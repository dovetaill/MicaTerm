#![allow(dead_code)]

pub fn retired_kebab_name() -> String {
    ["scene", "-", "image"].concat()
}

pub fn retired_pascal_name() -> String {
    ["Scene", "Image"].concat()
}

pub fn retired_snake_name() -> String {
    ["scene", "_", "image"].concat()
}

pub fn retired_subsystem_match_expr() -> String {
    format!("Some(\"{}\")", retired_kebab_name())
}

pub fn retired_rollout_env_snippet() -> String {
    format!("MICA_TERM_TERMINAL_SUBSYSTEM={}", retired_kebab_name())
}

pub fn retired_module_name() -> String {
    format!("terminal_{}", retired_snake_name())
}

pub fn retired_module_path() -> String {
    format!("src/app/{}.rs", retired_module_name())
}

pub fn retired_mod_export() -> String {
    format!("pub mod {}", retired_module_name())
}

pub fn retired_presenter_name() -> String {
    format!("Windows{}Presenter", retired_pascal_name())
}

pub fn retired_renderer_name() -> String {
    format!("{}TerminalRenderer", retired_pascal_name())
}

pub fn retired_builder_name() -> String {
    format!("build_{}_terminal_presenter", retired_snake_name())
}

pub fn retired_font_loader_name() -> String {
    format!("load_{}_font", retired_snake_name())
}

pub fn retired_render_diagnostics_name() -> String {
    format!("{}RenderDiagnostics", retired_pascal_name())
}

pub fn retired_cache_field(suffix: &str) -> String {
    format!("{}_{}", retired_snake_name(), suffix)
}
