use std::{fs, path::Path};

#[test]
fn startup_path_drops_legacy_terminal_font_imports() {
    let content = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        !content.contains("IosevkaTerm-Regular.ttf"),
        "startup should not import the retired Iosevka terminal font"
    );
    assert!(
        !content.contains("SarasaTermSCNerd-Regular.ttf"),
        "startup should not import the retired Sarasa terminal font"
    );
    assert!(
        !content.contains("MapleMonoNormalNL-NF-CN-Regular.ttf"),
        "the atlas renderer should own Maple font loading directly instead of routing it through Slint imports"
    );
}

#[test]
fn bundled_terminal_font_contract_uses_maple_only() {
    assert!(
        Path::new("ui/fonts/MapleMonoNormalNL-NF-CN-Regular.ttf").exists(),
        "the bundled terminal font should switch to Maple Mono Normal NL NF CN"
    );
    assert!(
        !Path::new("ui/fonts/IosevkaTerm-Regular.ttf").exists(),
        "the old Iosevka terminal font should be removed from the bundled assets"
    );
    assert!(
        !Path::new("ui/fonts/SarasaTermSCNerd-Regular.ttf").exists(),
        "the old Sarasa terminal font should be removed from the bundled assets"
    );
}

#[test]
fn terminal_host_drops_legacy_font_stack_strings() {
    let content =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        !content.contains("Sarasa Term SC Nerd"),
        "terminal host should stop advertising the Sarasa-first stack"
    );
    assert!(
        !content.contains("Iosevka Term"),
        "terminal host should stop advertising the Iosevka fallback stack"
    );
}
