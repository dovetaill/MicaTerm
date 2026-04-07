use std::fs;

#[test]
fn bootstrap_routes_terminal_presentation_through_renderer_host() {
    let source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    assert!(
        source.contains("TerminalRendererHost"),
        "bootstrap should route terminal presentation through a TerminalRendererHost seam instead of holding presenter variants directly"
    );
}

#[test]
fn terminal_renderer_module_exports_renderer_host() {
    let source = fs::read_to_string("src/app/terminal_renderer/mod.rs")
        .expect("read terminal renderer mod source");

    assert!(
        source.contains("pub mod host;") && source.contains("pub use host::TerminalRendererHost;"),
        "terminal_renderer module should export the shared TerminalRendererHost seam"
    );
}
