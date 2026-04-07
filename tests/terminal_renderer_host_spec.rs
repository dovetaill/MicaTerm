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

#[test]
fn bootstrap_test_hooks_can_install_requested_workspace_presenter_paths() {
    let source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    assert!(
        source.contains("TEST_WORKSPACE_TERMINAL_HOST_FACTORY")
            && source.contains("resolve_workspace_terminal_presenter("),
        "test builds should expose an injectable workspace terminal presenter seam so presenter selection tests can exercise requested presenter paths instead of always forcing BitmapAtlasPresenter"
    );
}

#[test]
fn tests_can_install_requested_workspace_presenter_path() {
    let source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    assert!(
        source.contains("TEST_WORKSPACE_TERMINAL_HOST_FACTORY"),
        "tests should be able to override the workspace terminal presenter path instead of hard-coding a bitmap-only presenter install"
    );
    assert!(
        source.contains("resolve_workspace_terminal_presenter(profile)"),
        "workspace presenter installation should resolve through one shared builder path so tests can exercise the same selection logic as packaged runtime code"
    );
}
