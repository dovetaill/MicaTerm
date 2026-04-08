use std::fs;

#[test]
fn terminal_shell_no_longer_exposes_jump_to_latest_affordance() {
    let terminal_host = fs::read_to_string("ui/shell/terminal-session-host.slint")
        .expect("read terminal session host");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let theme_spec = fs::read_to_string("src/theme/spec.rs").expect("read theme spec");
    let theme_tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    assert!(
        !terminal_host.contains("jump-to-latest")
            && !workspace_pane.contains("jump-to-latest")
            && !app_window.contains("jump-to-latest")
            && !theme_spec.contains("jump_to_latest")
            && !theme_tokens.contains("jump-to-latest"),
        "jump-to-latest UI and palette plumbing should be removed from the terminal shell"
    );
}
