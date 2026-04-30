use std::fs;

#[test]
fn runtime_depends_on_terminal_core_adapter_contract() {
    let source = fs::read_to_string("src/app/ssh/runtime/terminal.rs")
        .expect("read runtime terminal source");

    assert!(
        source.contains("dyn TerminalCoreAdapter"),
        "terminal runtime code should route terminal-core access through an object-safe TerminalCoreAdapter boundary rather than concrete wezterm session internals"
    );
}

#[test]
fn app_exports_terminal_core_module_and_wezterm_adapter() {
    let app_mod = fs::read_to_string("src/app/mod.rs").expect("read app mod source");
    let terminal_core_mod =
        fs::read_to_string("src/app/terminal_core/mod.rs").expect("read terminal_core mod");

    assert!(
        app_mod.contains("pub mod terminal_core;"),
        "app module should export the terminal_core namespace so the runtime and renderer can share the adapter seam"
    );
    assert!(
        terminal_core_mod.contains("WeztermTerminalCoreAdapter"),
        "terminal_core module should expose the wezterm-backed adapter implementation behind the shared adapter seam"
    );
}

#[test]
fn repository_no_longer_exposes_an_experimental_alacritty_core_path() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read cargo manifest");
    let terminal_core_mod =
        fs::read_to_string("src/app/terminal_core/mod.rs").expect("read terminal_core mod");
    let terminal_core_types =
        fs::read_to_string("src/app/terminal_core/types.rs").expect("read terminal_core types");
    let runtime_terminal =
        fs::read_to_string("src/app/ssh/runtime/terminal.rs").expect("read runtime terminal");

    assert!(
        !cargo_toml.contains("alacritty_terminal"),
        "Cargo.toml should not keep the unused alacritty_terminal dependency once the repo standardizes on the shipped WezTerm core"
    );
    assert!(
        !cargo_toml.contains("terminal-core-alacritty-experimental"),
        "Cargo.toml should not expose an Alacritty experimental feature after the runtime selection seam is collapsed"
    );
    assert!(
        !terminal_core_mod.contains("alacritty_adapter"),
        "terminal_core module should not export an Alacritty adapter after the experimental candidate path is removed"
    );
    assert!(
        !terminal_core_types.contains("AlacrittyExperimental"),
        "TerminalCoreKind should no longer advertise an AlacrittyExperimental variant after the repository chooses a single shipped core"
    );
    assert!(
        !runtime_terminal.contains("new_with_experimental_alacritty_core"),
        "runtime terminal helpers should not expose an experimental Alacritty constructor after the candidate path is deleted"
    );
}
