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
