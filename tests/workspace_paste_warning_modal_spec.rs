use std::fs;

fn slice_after<'a>(source: &'a str, marker: &str) -> &'a str {
    source
        .split_once(marker)
        .map(|(_, tail)| tail)
        .expect("marker should exist in source")
}

#[test]
fn native_terminal_shortcut_routes_paste_through_workspace_callback() {
    let source = fs::read_to_string("src/app/bootstrap/windowing.rs")
        .expect("read workspace windowing binder");

    assert!(
        source.contains("window.invoke_workspace_session_paste_requested();"),
        "the Windows native Ctrl+Shift+V path should reuse the workspace-session paste callback so shortcut paste and right-click paste share one pipeline"
    );
}

#[test]
fn workspace_paste_requests_are_ignored_while_the_review_modal_is_open() {
    let source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap paste handler");

    assert!(
        source.contains("window.get_workspace_paste_warning_modal_open()")
            && source.contains(
                "ignored workspace paste request because the paste review modal is already open"
            ),
        "the shared paste callback should refuse re-entrant paste requests while the review modal is open so draft edits are not overwritten"
    );
}

#[test]
fn paste_warning_modal_uses_scrollable_editor_and_local_fluent_sections() {
    let source = fs::read_to_string("ui/components/workspace-paste-warning-modal.slint")
        .expect("read workspace paste warning modal");

    assert!(
        source.contains("import { ScrollView } from \"std-widgets.slint\";")
            && source.contains("header := Rectangle")
            && source.contains("body := Rectangle")
            && source.contains("footer := Rectangle")
            && source.contains("editor-scroll := ScrollView")
            && source.contains("viewport-width:")
            && source.contains("viewport-height:"),
        "the paste warning modal should keep a local Fluent-style header/body/footer structure and host the multiline editor inside a real ScrollView"
    );
}

#[test]
fn paste_warning_modal_separates_review_enter_from_editor_enter() {
    let modal_source = fs::read_to_string("ui/components/workspace-paste-warning-modal.slint")
        .expect("read workspace paste warning modal");
    let shell_source = fs::read_to_string("ui/components/blocking-modal-shell.slint")
        .expect("read blocking modal shell");
    let app_window_source =
        fs::read_to_string("ui/app-window.slint").expect("read app window paste modal shell");
    let paste_modal_shell = slice_after(
        &app_window_source,
        "if root.workspace-paste-warning-modal-open : workspace-paste-warning-modal-shell := BlockingModalShell {",
    );

    assert!(
        modal_source.contains("Enter to paste  •  Esc to cancel")
            && modal_source.contains("Enter inserts a newline while editing.")
            && modal_source.contains("review-key-anchor := TextInput")
            && modal_source.contains("changed focus-sequence => {")
            && modal_source.contains("review-key-anchor.focus();")
            && modal_source.contains("changed has-focus => {")
            && shell_source.contains("in property <bool> enter-enabled: false;")
            && shell_source.contains("callback enter-requested();")
            && paste_modal_shell
                .contains("enter-enabled: !root.workspace-paste-warning-editor-focused;")
            && paste_modal_shell.contains("enter-requested => {"),
        "the review mode should keep Enter wired to paste by default, but hand Enter back to the editor once the draft field gains focus"
    );
}
