use std::fs;

#[test]
fn transfer_center_renders_running_queued_paused_failed_completed_tabs() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    for label in ["Running", "Queued", "Paused", "Failed", "Completed"] {
        assert!(
            content.contains(&format!("status-chip_label(\"{label}\""))
                || content.contains(&format!("\"{label}\"")),
            "transfer center should expose the `{label}` tab"
        );
    }

    assert!(
        content.contains("No transfers yet"),
        "transfer center should expose a lightweight empty state"
    );
}

#[test]
fn transfer_center_exposes_live_transfer_rows_contract() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("export struct TransferCenterItem"),
        "transfer center should define a projected row contract for live transfer tasks"
    );
    assert!(
        content.contains("in property <[TransferCenterItem]> items: [];"),
        "transfer center should accept a live item list from bootstrap"
    );
    assert!(
        content.contains("for item in root.items"),
        "transfer center should render real transfer rows instead of a permanent placeholder"
    );
}

#[test]
fn transfer_center_exposes_filter_and_row_action_contracts() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window source");

    assert!(
        content.contains("can_retry: bool")
            && content.contains("can_resolve_conflict: bool")
            && content.contains("can_open_workspace: bool"),
        "transfer-center rows should expose lightweight action capability flags for retry/resolve/open-workspace affordances"
    );
    assert!(
        content.contains("in property <string> active-filter: \"all\";")
            && content.contains("callback filter-toggle-requested(string);")
            && content.contains("callback retry-requested(string);")
            && content.contains("callback resolve-conflict-requested(string);")
            && content.contains("callback open-workspace-requested(string);"),
        "transfer center should expose filter and row action callbacks so the lightweight UI can drive real host behavior"
    );
    assert!(
        app_window.contains("transfer-center-active-filter")
            && app_window.contains("callback transfer-center-filter-toggle-requested(string);")
            && app_window.contains("callback transfer-center-retry-requested(string);")
            && app_window.contains("callback transfer-center-resolve-conflict-requested(string);")
            && app_window.contains("callback transfer-center-open-workspace-requested(string);"),
        "app window should forward transfer-center filter and action callbacks into bootstrap"
    );
}

#[test]
fn transfer_center_exposes_compact_utility_panel_contract() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window source");

    assert!(
        content.contains("callback pin-toggle-requested();")
            && content.contains("callback collapse-toggle-requested();")
            && content.contains("callback open-sftp-requested();"),
        "transfer center should expose utility-panel header controls plus an empty-state Open SFTP CTA callback"
    );
    assert!(
        app_window.contains("in-out property <bool> transfer-center-pinned: false;")
            && app_window.contains("in-out property <bool> transfer-center-collapsed: false;")
            && app_window.contains("callback transfer-center-pin-toggle-requested();")
            && app_window.contains("callback transfer-center-collapse-toggle-requested();"),
        "app window should own pinned/collapsed transfer-center state and forward the new utility-panel callbacks"
    );
}

#[test]
fn transfer_center_row_projection_supports_host_direction_and_progress_bar() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("host_label: string")
            && content.contains("direction_label: string")
            && content.contains("progress_value: float"),
        "transfer rows should project host, direction, and determinate progress so the compact panel can stay dense without turning into a giant card"
    );
    assert!(
        content.contains("\"Open SFTP\""),
        "the empty state should provide an explicit Open SFTP CTA instead of leaving a large blank card"
    );
}

#[test]
fn transfer_center_contract_includes_completed_file_actions() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("callback open-file-requested(string);"),
        "transfer center should expose a completed-row file open callback"
    );
    assert!(
        content.contains("callback open-folder-requested(string);"),
        "transfer center should expose a completed-row folder open callback"
    );
    assert!(
        content.contains("callback remove-requested(string);"),
        "transfer center should expose a per-row remove callback"
    );
    assert!(
        content.contains("callback clear-completed-requested();"),
        "transfer center should expose a clear-completed toolbar callback"
    );
}

#[test]
fn app_window_exposes_transfer_center_non_modal_docked_contract() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window source");
    let bootstrap = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    assert!(
        app_window.contains("callback close-transfer-center-requested();"),
        "app window should expose a dedicated close-transfer-center callback so Escape and header close actions do not rely on toggle semantics"
    );
    assert!(
        !app_window.contains("transfer-center-dismiss-layer := TouchArea"),
        "transfer center should no longer render a full-body dismiss layer once it becomes a non-modal utility panel"
    );
    assert!(
        app_window.contains("right-panel.width")
            && app_window.contains("MotionTokens.drawer-duration")
            && bootstrap.contains("state.transfer_center_open()")
            && bootstrap.contains("!state.transfer_center_pinned()")
            && bootstrap.contains("state.close_transfer_center();"),
        "transfer center should dock against the right edge, animate like a utility drawer, and let the active terminal Escape key dismiss it when unpinned"
    );
}

#[test]
fn transfer_center_motion_aligns_with_titlebar_transfer_trigger() {
    let titlebar = fs::read_to_string("ui/shell/titlebar.slint").expect("read titlebar source");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window source");
    let transfer_center =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");
    let motion = fs::read_to_string("ui/theme/motion.slint").expect("read motion token source");

    assert!(
        titlebar.contains("layout-transfer-button-anchor-x")
            && titlebar.contains("layout-transfer-button-anchor-y")
            && titlebar.contains("transfer-summary-anchor-width"),
        "titlebar should export a transfer-trigger anchor so the compact panel can animate from the same affordance the user clicks"
    );
    assert!(
        app_window.contains("titlebar.layout-transfer-button-anchor-y - 18px")
            && app_window.contains("private property <length> transfer-center-target-width:")
            && app_window.contains("private property <length> transfer-center-closed-width:")
            && app_window.contains(
                "animate width { duration: MotionTokens.utility-panel-frame-duration; easing: ease-out; }"
            ),
        "app window should align the closed-state motion with the titlebar transfer trigger and slightly stage width as the utility drawer opens"
    );
    assert!(
        transfer_center.contains("private property <length> stage-inset: root.open ? 0px : 14px;")
            && transfer_center.contains("private property <length> stage-offset-y: root.open ? 0px : 8px;")
            && transfer_center.contains("panel-frame := Rectangle {"),
        "transfer center should slightly compress its own surface before open so the anchored motion feels like an unfolding utility panel rather than a plain slide"
    );
    assert!(
        motion.contains("out property <duration> utility-panel-frame-duration: 180ms;")
            && motion.contains("out property <duration> utility-panel-shadow-duration: 200ms;")
            && motion.contains("out property <duration> utility-panel-opacity-duration: 150ms;")
            && transfer_center.contains(
                "animate x { duration: MotionTokens.utility-panel-shadow-duration; easing: ease-out; }"
            )
            && transfer_center.contains(
                "animate opacity { duration: MotionTokens.utility-panel-opacity-duration; }"
            ),
        "utility-panel motion should be driven by shared frame/shadow/opacity timing tokens instead of hardcoded per-layer values"
    );
}

#[test]
fn transfer_center_escape_contract_covers_panel_focus_and_right_panel_path_editor() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");
    let right_panel = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel source");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window source");

    assert!(
        content.contains("focus-host := FocusScope {")
            && content.contains("event.text == Key.Escape && !root.pinned")
            && content.contains("root.close-requested();"),
        "transfer center should own a panel-level Escape focus scope so action buttons can bubble unhandled Esc back to the utility panel"
    );
    assert!(
        right_panel.contains("in property <bool> transfer-center-open: false;")
            && right_panel.contains("in property <bool> transfer-center-pinned: false;")
            && right_panel.contains("callback transfer-center-close-requested();")
            && right_panel.contains("root.transfer-center-open && !root.transfer-center-pinned"),
        "right panel should bridge plain Escape from its path editor into the transfer-center close path when the utility panel is open and unpinned"
    );
    assert!(
        app_window.contains("transfer-center-open: root.transfer-center-open;")
            && app_window.contains("transfer-center-pinned: root.transfer-center-pinned;")
            && app_window.contains("transfer-center-close-requested => {")
            && app_window.contains("root.close-transfer-center-requested();"),
        "app window should forward the right-panel Escape bridge into the existing transfer-center close callback"
    );
}

#[test]
fn completed_transfer_rows_expose_open_file_open_folder_and_remove() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window source");

    assert!(
        content.contains("can_open_file: bool")
            && content.contains("can_open_folder: bool")
            && content.contains("can_remove: bool"),
        "completed transfer rows should project dedicated open-file/open-folder/remove capability flags"
    );
    assert!(
        content.contains("\"Open File\"")
            && content.contains("\"Open Folder\"")
            && content.contains("\"Remove\""),
        "completed transfer rows should render explicit Open File / Open Folder / Remove actions instead of reusing the attention-only workspace affordance"
    );
    assert!(
        app_window.contains("callback transfer-center-open-file-requested(string);")
            && app_window.contains("callback transfer-center-open-folder-requested(string);")
            && app_window.contains("callback transfer-center-remove-requested(string);")
            && app_window.contains("callback transfer-center-clear-completed-requested();"),
        "app window should forward completed-row open/remove callbacks into bootstrap"
    );
}

#[test]
fn failed_transfer_rows_expose_retry_show_error_and_remove() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("can_retry: bool")
            && content.contains("can_show_error: bool")
            && content.contains("can_remove: bool"),
        "failed transfer rows should project retry/show-error/remove capability flags"
    );
    assert!(
        content.contains("\"Details\"") && content.contains("\"Remove\""),
        "failed transfer rows should surface compact Details and Remove actions alongside Retry"
    );
}

#[test]
fn transfer_center_rows_use_compact_primary_and_workspace_actions() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("\"Resolve\"")
            && content.contains("\"Workspace\"")
            && content.contains("secondary-action-touch := TouchArea"),
        "transfer-center rows should expose a compact primary action plus a lighter workspace shortcut for narrow widths"
    );
}
#[test]
fn transfer_center_rows_expose_error_summary_and_tooltip_contract() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window source");

    assert!(
        content.contains("error_summary: string"),
        "transfer center rows should expose a compact inline error summary for failed/conflict tasks"
    );
    assert!(
        content.contains("error_tooltip: string"),
        "transfer center rows should expose the full error text for hover tooltip display"
    );
    assert!(
        content.contains("show_error: bool"),
        "transfer center rows should explicitly mark whether an inline error line should render"
    );
    assert!(
        app_window.contains("transfer-center-tooltip-overlay := TitlebarTooltip"),
        "app window should host a dedicated tooltip overlay for transfer-center error hover text"
    );
}

#[test]
fn conflict_modal_exposes_destination_scoped_batch_toggle_contract() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window source");

    assert!(
        modal.contains("in property <int> batch-conflict-count: 0;")
            && modal.contains("in property <bool> apply-to-batch: false;")
            && modal.contains("callback apply-to-batch-toggled(bool);"),
        "conflict modal should expose destination-scoped batch-toggle properties and callback so the UI can drive a real multi-conflict scope"
    );
    assert!(
        modal.contains("function batch-scope-copy() -> string")
            && modal.contains("function batch-toggle-copy() -> string")
            && modal.contains("if root.batch-conflict-count > 0 : scope-card := Rectangle {"),
        "conflict modal should compute its destination-scope copy in one place and render a dedicated scope card only when other matching conflicts exist"
    );
    assert!(
        app_window.contains("in-out property <int> sftp-conflict-modal-batch-conflict-count: 0;")
            && app_window
                .contains("in-out property <bool> sftp-conflict-modal-apply-to-batch: false;")
            && app_window.contains("callback sftp-conflict-modal-apply-to-batch-toggled(bool);")
            && app_window
                .contains("batch-conflict-count: root.sftp-conflict-modal-batch-conflict-count;")
            && app_window.contains("apply-to-batch: root.sftp-conflict-modal-apply-to-batch;"),
        "app window should forward the conflict modal batch scope state and toggle callback into bootstrap"
    );
}

#[test]
fn conflict_modal_uses_labeled_cards_instead_of_raw_source_target_lines() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");

    for label in ["Incoming item", "Existing target", "Destination scope"] {
        assert!(
            modal.contains(&format!("label: \"{label}\";"))
                || modal.contains(&format!("text: \"{label}\";")),
            "conflict modal should render a labeled `{label}` card so the narrow dialog reads like a mature transfer sheet instead of raw concatenated path text"
        );
    }

    assert!(
        !modal.contains("text: \"Source: \" + root.source-path;")
            && !modal.contains("text: \"Target: \" + root.target-path;"),
        "conflict modal should stop rendering raw `Source:` / `Target:` lines once the labeled cards exist"
    );
    assert!(
        modal.contains("other conflict is waiting in this folder.")
            && modal.contains("other conflicts are waiting in this folder.")
            && modal.contains("Apply this choice to the other ")
            && modal.contains(" conflicts in this folder."),
        "conflict modal batch copy should clearly describe folder-scoped impact in both singular and plural states"
    );
}

#[test]
fn conflict_modal_exposes_keyboard_shortcuts_for_batch_toggle_and_primary_action() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");

    assert!(
        modal.contains("key-pressed(event) => {"),
        "conflict modal should own keyboard handling directly so the dialog remains usable without pointer interaction"
    );
    assert!(
        modal.contains("event.text == Key.Space")
            && modal.contains("root.apply-to-batch-toggled(!root.apply-to-batch);"),
        "conflict modal should let Space toggle the destination batch checkbox when the scope card is present"
    );
    assert!(
        modal.contains("event.text == Key.Return") && modal.contains("root.replace-requested();"),
        "conflict modal should treat Enter as the default Replace action so the dialog behaves like a mature transfer sheet"
    );
}

#[test]
fn conflict_modal_batch_toggle_row_exposes_focus_and_checkbox_accessibility_contract() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");

    assert!(
        modal.contains("component ConflictBatchToggleRow inherits Rectangle")
            && modal.contains("accessible-role: AccessibleRole.checkbox;")
            && modal.contains("accessible-checkable: true;")
            && modal.contains("accessible-checked: root.checked;")
            && modal.contains("forward-focus: toggle-focus;"),
        "conflict modal batch-toggle row should behave like a real checkbox row so keyboard users can discover and activate the folder-scope toggle"
    );
    assert!(
        modal.contains("border-color: root.has-focus ? ThemeTokens.focus-ring")
            && modal
                .contains("background: toggle-touch.pressed ? ThemeTokens.control-pressed-surface")
            && modal.contains(": root.has-focus ? ThemeTokens.control-active-surface"),
        "conflict modal batch-toggle row should reserve a distinct focus treatment instead of looking identical to the passive scope card"
    );
}

#[test]
fn transfer_center_attention_actions_expose_tooltips_and_keyboard_button_contract() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("tooltip-text: \"Resolve transfer conflict\";")
            && content.contains("tooltip-text: \"Open task in SFTP workspace\";"),
        "transfer-center resolve/workspace affordances should expose explicit tooltip copy so narrow action labels stay understandable"
    );
    assert!(
        content.contains("accessible-role: AccessibleRole.button;")
            && content.contains("accessible-action-default => { touch.clicked(); }")
            && content.contains("forward-focus: action-focus;")
            && content.contains("event.text == \" \" || event.text == \"\\n\""),
        "transfer-center row actions should expose a button-like keyboard contract instead of remaining pointer-only chips"
    );
    assert!(
        content.contains("if self.tooltip-active {")
            && content.contains("root.tooltip-open-requested(")
            && content.contains("root.tooltip-close-requested(root.tooltip-source-id);"),
        "transfer-center action affordances should surface their tooltip on hover or focus and close it again once attention leaves"
    );
}

#[test]
fn transfer_center_row_actions_expose_fluent_focus_ring_contract() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("component TransferCenterActionChip inherits Rectangle")
            && content.contains("component TransferCenterActionLink inherits Rectangle")
            && content.contains("private property <brush> chrome-border: root.has-focus")
            && content.contains("focus-halo := Rectangle {")
            && content.contains("border-color: ThemeTokens.focus-ring;"),
        "transfer-center row actions should expose a stronger Fluent focus-ring treatment instead of relying on minimal border changes alone"
    );
    assert!(
        content.contains("border-width: root.has-focus || root.has-hover ? 1px : 0px;")
            && content.contains("opacity: root.has-focus ? 0.44 : 0;")
            && content.contains("opacity: root.has-focus ? 0.42 : 0;"),
        "transfer-center row actions should keep a subtle hover contour and a separate focus halo so keyboard and pointer states read differently"
    );
}

#[test]
fn transfer_center_header_buttons_use_focusable_sidebar_toolbar_contract() {
    let button = fs::read_to_string("ui/components/sidebar-toolbar-icon-button.slint")
        .expect("read sidebar toolbar icon button source");
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        button.contains("accessible-role: AccessibleRole.button;")
            && button.contains("accessible-action-default => { touch.clicked(); }")
            && button.contains("forward-focus: button-focus;")
            && button.contains("button-focus := FocusScope {"),
        "sidebar toolbar icon buttons should expose a real keyboard focus contract so compact utility-panel header actions are not pointer-only"
    );
    assert!(
        button.contains("event.text == \" \" || event.text == \"\\n\"")
            && button.contains("out property <bool> has-focus: button-focus.has-focus;")
            && button.contains("changed tooltip-active => {")
            && button.contains("private property <brush> chrome-border: root.has-focus")
            && button.contains("focus-halo := Rectangle {")
            && button.contains("border-color: ThemeTokens.focus-ring;"),
        "sidebar toolbar icon buttons should support keyboard activation, a visible focus ring, and tooltip visibility on focus"
    );
    assert!(
        content.contains("collapse-button := SidebarToolbarIconButton {")
            && content.contains("pin-button := SidebarToolbarIconButton {")
            && content.contains("close-button := SidebarToolbarIconButton {"),
        "transfer center header should keep Collapse, Pin, and Close on the shared focusable toolbar button component"
    );
}

#[test]
fn transfer_center_row_error_tooltip_yields_to_action_tooltips() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("private property <bool> row-action-tooltip-active:")
            && content.contains("changed row-action-tooltip-active => {"),
        "transfer-center rows should explicitly track whether an inline action currently owns tooltip attention"
    );
    assert!(
        content.contains("if self.row-action-tooltip-active {")
            && content.contains("root.queue-tooltip-close(item.id);"),
        "transfer-center rows should close the error tooltip while resolve/workspace actions are hovered or focused"
    );
    assert!(
        content.contains(
            "!self.row-action-tooltip-active && self.has-hover && item.error_tooltip != \"\""
        ),
        "transfer-center rows should only schedule the inline error tooltip when no action tooltip is active"
    );
}

#[test]
fn conflict_modal_footer_buttons_expose_focusable_button_contract() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");

    assert!(
        modal.contains("accessible-role: AccessibleRole.button;")
            && modal.contains("accessible-action-default => { button-touch.clicked(); }")
            && modal.contains("forward-focus: button-focus;")
            && modal.contains("out property <bool> has-focus: button-focus.has-focus;"),
        "conflict modal footer buttons should expose a real button contract so Tab navigation can reach Cancel, Skip, and Replace"
    );
    assert!(
        modal.contains("button-focus := FocusScope {")
            && modal.contains("event.text == \" \" || event.text == \"\\n\"")
            && modal.contains("button-touch.clicked();"),
        "conflict modal footer buttons should respond to keyboard activation instead of remaining pointer-only"
    );
    assert!(
        modal.contains("cancel-button := ConflictDialogButton {")
            && modal.contains("skip-button := ConflictDialogButton {")
            && modal.contains("replace-button := ConflictDialogButton {"),
        "conflict modal footer should keep all three actions in the shared focusable button component"
    );
}

#[test]
fn conflict_modal_close_button_exposes_focus_and_tooltip_contract() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");

    assert!(
        modal.contains("component ConflictIconButton inherits Rectangle")
            && modal.contains("tooltip-text: \"Close conflict dialog\";")
            && modal.contains("tooltip-source-id: \"sftp-conflict-close\";")
            && modal.contains("accessible-role: AccessibleRole.button;")
            && modal.contains("forward-focus: button-focus;"),
        "conflict modal close affordance should graduate from a bare rectangle to a real focusable icon button with explicit close tooltip copy"
    );
    assert!(
        modal.contains("close-tooltip-overlay := TitlebarTooltip {")
            && modal.contains("text: root.tooltip-text-value;")
            && modal.contains("tooltip-visible: root.tooltip-visible-value;")
            && modal.contains("function schedule-tooltip("),
        "conflict modal should own a lightweight tooltip state machine so the close affordance can show the same kind of explicit hint as other shell actions"
    );
}

#[test]
fn transfer_center_row_actions_keep_a_stable_source_focus_order() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    let retry_index = content
        .find("retry-action := TransferCenterActionChip {")
        .expect("retry action contract");
    let resolve_index = content
        .find("resolve-action := TransferCenterActionChip {")
        .expect("resolve action contract");
    let workspace_index = content
        .find("open-workspace-action := TransferCenterActionLink {")
        .expect("workspace action contract");

    assert!(
        retry_index < resolve_index && resolve_index < workspace_index,
        "transfer-center row actions should stay declared in their intended left-to-right focus order so source-contract smoke can catch accidental reordering"
    );
    assert!(
        content.contains("x: parent.width - 188px;")
            && content.contains("x: parent.width - 104px;")
            && content.contains("forward-focus: action-focus;"),
        "transfer-center action affordances should keep a compact left-to-right layout backed by explicit focus forwarding"
    );
}

#[test]
fn transfer_center_retry_action_matches_attention_tooltip_contract() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("tooltip-text: \"Retry failed transfer\";")
            && content.contains("tooltip-source-id: item.id + \"-retry\";"),
        "transfer-center retry should expose explicit tooltip copy so failed-task recovery reads as clearly as resolve/workspace actions"
    );
    assert!(
        content.contains("private property <bool> retry-tooltip-active: false;")
            && content.contains("private property <bool> row-action-tooltip-active: self.retry-tooltip-active || self.resolve-tooltip-active || self.workspace-tooltip-active;")
            && content.contains("row.retry-tooltip-active = active;"),
        "transfer-center retry should participate in the shared action-tooltip ownership model so error hover text does not compete with the retry hint"
    );
}
