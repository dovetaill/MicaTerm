use std::fs;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use mica_term::AppWindow;
use slint::ComponentHandle;
use slint::platform::{PointerEventButton, WindowEvent};

use i_slint_backend_testing::ElementHandle;

fn slice_after<'a>(source: &'a str, marker: &str) -> &'a str {
    source
        .split_once(marker)
        .map(|(_, tail)| tail)
        .expect("marker should exist in source")
}

fn settle_modal_ui() {
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn element_center(element: &ElementHandle) -> slint::LogicalPosition {
    slint::LogicalPosition::new(
        element.absolute_position().x + element.size().width / 2.0,
        element.absolute_position().y + element.size().height / 2.0,
    )
}

fn dispatch_pointer_click(
    app: &AppWindow,
    position: slint::LogicalPosition,
    button: PointerEventButton,
) {
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    app.window()
        .dispatch_event(WindowEvent::PointerPressed { position, button });
    settle_modal_ui();
    app.window()
        .dispatch_event(WindowEvent::PointerReleased { position, button });
    settle_modal_ui();
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
fn native_terminal_paste_shortcut_scopes_view_model_borrows_to_copy_only() {
    let source = fs::read_to_string("src/app/bootstrap/windowing.rs")
        .expect("read workspace windowing binder");

    let shortcut_block = slice_after(&source, "if let Some(shortcut) = clipboard_shortcut {");
    let match_block = slice_after(shortcut_block, "match shortcut {");
    let copy_arm = slice_after(match_block, "NativeTerminalClipboardShortcut::Copy");
    let copy_arm = copy_arm
        .split_once("NativeTerminalClipboardShortcut::Paste => {")
        .map(|(arm, _)| arm)
        .expect("copy arm should precede the paste arm");
    let paste_arm = slice_after(match_block, "NativeTerminalClipboardShortcut::Paste => {");
    let paste_arm = paste_arm
        .split_once("if sftp_path_edit_shortcut {")
        .map(|(arm, _)| arm)
        .expect("paste arm should be followed by the SFTP shortcut branch");

    assert!(
        copy_arm.contains("let state = state.borrow();"),
        "the native copy path should borrow the workspace view model only inside the copy arm so paste does not inherit that borrow scope"
    );
    assert!(
        !paste_arm.contains("let state = state.borrow();"),
        "the native paste path must not hold an immutable workspace view-model borrow before invoking the shared paste callback, or Ctrl+Shift+V can panic with a RefCell borrow_mut error"
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

#[test]
fn paste_warning_editor_right_click_keeps_enter_bound_to_the_editor() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let confirm_count = Rc::new(RefCell::new(0usize));
    let confirm_count_ref = Rc::clone(&confirm_count);
    app.on_workspace_paste_warning_confirm_requested(move || {
        *confirm_count_ref.borrow_mut() += 1;
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_workspace_paste_warning_modal_open(true);
    app.set_workspace_paste_warning_editor_mode(true);
    app.set_workspace_paste_warning_text("line 1\nline 2\nline 3\nline 4".into());
    settle_modal_ui();

    let paste_input = ElementHandle::find_by_element_id(&app, "WorkspacePasteWarningModal::paste-input")
        .next()
        .expect("find paste warning editor input");
    let paste_input_position = element_center(&paste_input);

    dispatch_pointer_click(&app, paste_input_position, PointerEventButton::Left);
    assert!(
        app.get_workspace_paste_warning_editor_focused(),
        "clicking into the paste review editor should mark the editor as focused before right-click testing begins"
    );

    dispatch_pointer_click(&app, paste_input_position, PointerEventButton::Right);
    assert!(
        app.get_workspace_paste_warning_editor_focused(),
        "right-clicking inside the paste review editor should not blur it before Enter handling runs"
    );

    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "\n".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "\n".into() });
    settle_modal_ui();

    assert_eq!(
        *confirm_count.borrow(),
        0,
        "after clicking into the editor, right-click plus Enter should keep Enter in the editor path instead of re-triggering modal confirm"
    );
}
