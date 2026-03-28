use std::fs;

#[test]
fn app_window_keeps_sarasa_out_of_startup_imports() {
    let content = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        !content.contains("SarasaTermSCNerd-Regular.ttf"),
        "Sarasa should stay out of the global startup import path"
    );
}

#[test]
fn terminal_host_font_contract_prefers_sarasa() {
    let content =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        content.contains(
            "in property <string> terminal-font-family: \"Sarasa Term SC Nerd, Iosevka Term, Cascadia Mono, Consolas, monospace\";"
        ),
        "terminal host should expose Sarasa-first terminal font contract"
    );
}

#[test]
fn bootstrap_registers_terminal_font_when_terminal_host_is_visible() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        content.contains("crate::app::terminal_font::ensure_terminal_font_registered()"),
        "bootstrap should trigger lazy terminal font registration"
    );
}

#[test]
fn terminal_font_module_embeds_sarasa_and_uses_shared_fontique_collection() {
    let content = fs::read_to_string("src/app/terminal_font.rs").expect("read terminal_font");

    assert!(content.contains("SarasaTermSCNerd-Regular.ttf"));
    assert!(content.contains("slint::fontique_07::shared_collection()"));
    assert!(content.contains("fontique::Blob::new"));
}
