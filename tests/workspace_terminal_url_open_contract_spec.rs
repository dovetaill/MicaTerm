use std::fs;

fn windows_dependency_block(cargo_toml: &str) -> &str {
    let start = cargo_toml
        .find("[target.'cfg(target_os = \"windows\")'.dependencies]")
        .expect("windows target dependency section");
    let rest = &cargo_toml[start..];
    let end = rest.find("\n[build-dependencies]").unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn workspace_terminal_url_open_path_enables_windows_shell_feature() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read Cargo.toml");
    let url_open_source = fs::read_to_string("src/app/url_open.rs").expect("read url_open");
    let windows_target_dependencies = windows_dependency_block(&cargo_toml);

    assert!(
        url_open_source.contains("ShellExecuteW("),
        "workspace terminal URL-open path should use ShellExecuteW on Windows via the shared url_open helper"
    );
    assert!(
        windows_target_dependencies.contains("\"Win32_UI_Shell\""),
        "windows crate features must include Win32_UI_Shell when the shared url_open helper imports ShellExecuteW"
    );
}

#[test]
fn workspace_terminal_url_open_path_keeps_non_windows_launchers_cfg_local() {
    let url_open_source = fs::read_to_string("src/app/url_open.rs").expect("read url_open");

    assert!(
        url_open_source.contains("std::process::Command::new(\"open\")"),
        "macOS browser launching should stay local to the cfg block in the shared url_open helper so Windows builds do not carry an unused Command import"
    );
    assert!(
        url_open_source.contains("std::process::Command::new(\"xdg-open\")"),
        "Linux browser launching should stay local to the cfg block in the shared url_open helper so Windows builds do not carry an unused Command import"
    );
    assert!(
        !url_open_source.contains("use std::process::Command;"),
        "shared url_open helper should not keep a top-level Command import once only cfg-gated branches need it"
    );
}
