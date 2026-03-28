use std::fs;

#[test]
fn app_window_keeps_startup_font_import_lightweight() {
    let content = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        content.contains("import \"fonts/IosevkaTerm-Regular.ttf\";"),
        "startup should keep the lighter bundled terminal font import"
    );
    assert!(
        !content.contains("SarasaTermSCNerd-Regular.ttf"),
        "the large Sarasa terminal font should not be globally imported at startup"
    );
}

#[test]
fn terminal_host_uses_startup_safe_font_fallbacks() {
    let content =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        content.contains(
            "terminal-font-family: \"Iosevka Term, Cascadia Mono, Consolas, monospace\";"
        ),
        "terminal host should default to the lighter fallback font stack"
    );
    assert!(
        !content.contains("terminal-font-family: \"Sarasa Term SC Nerd\";"),
        "terminal host should not force the large Sarasa font at startup"
    );
}
