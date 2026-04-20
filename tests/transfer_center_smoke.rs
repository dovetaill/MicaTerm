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
            && content.contains("attention_action: string")
            && content.contains("attention_label: string")
            && content.contains("can_resolve_conflict: bool")
            && content.contains("can_open_workspace: bool"),
        "transfer-center rows should expose lightweight action capability flags plus attention-action copy for resume/restart/pause affordances"
    );
    assert!(
        content.contains("in property <string> active-filter: \"all\";")
            && content.contains("callback filter-toggle-requested(string);")
            && content.contains("callback transfer-row-resume-requested(string);")
            && content.contains("callback transfer-row-restart-requested(string);")
            && content.contains("callback transfer-row-pause-requested(string);")
            && content.contains("callback resolve-conflict-requested(string);")
            && content.contains("callback open-workspace-requested(string);"),
        "transfer center should expose filter and row action callbacks so the lightweight UI can drive pause/resume/restart host behavior"
    );
    assert!(
        app_window.contains("transfer-center-active-filter")
            && app_window.contains("callback transfer-center-filter-toggle-requested(string);")
            && app_window.contains("callback transfer-center-resume-requested(string);")
            && app_window.contains("callback transfer-center-restart-requested(string);")
            && app_window.contains("callback transfer-center-pause-requested(string);")
            && app_window.contains("callback transfer-center-resolve-conflict-requested(string);")
            && app_window.contains("callback transfer-center-open-workspace-requested(string);"),
        "app window should forward transfer-center filter and pause/resume/restart callbacks into bootstrap"
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
        content.contains("remove_tooltip: string,")
            && content.contains("callback open-file-requested(string);"),
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
    assert!(
        content.contains("tooltip-text: item.remove_tooltip;"),
        "completed transfer remove actions should project explicit copy so downloaded artifacts can advertise trash semantics without overloading every row with the same message"
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
    assert!(
        app_window.contains("transfer-center-outside-dismiss-hitbox := TouchArea")
            && app_window.contains("if root.transfer-center-open && !root.transfer-center-pinned")
            && app_window.contains("root.close-transfer-center-requested();"),
        "transfer center should close on outside click while unpinned so the compact utility panel behaves like a transient flyout instead of a permanent sheet"
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
            && transfer_center
                .contains("private property <length> stage-offset-y: root.open ? 0px : 8px;")
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
fn transfer_center_footer_copy_uses_adaptive_width_and_non_truncated_copy() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("private property <length> footer-copy-width: root.completed-count > 0")
            && content.contains("width: footer-strip.footer-copy-width;")
            && content.contains("wrap: word-wrap;"),
        "transfer center footer copy should reclaim width when the clear-completed action is hidden and wrap instead of truncating utility copy too aggressively"
    );
    assert!(
        content.contains("\"Completed transfers stay here until you clear them\"")
            && content.contains("\"Transfers stay close without interrupting the terminal\"")
            && content.contains("\"Ready for the next transfer\""),
        "transfer center footer should keep short, readable utility copy that fits the compact panel"
    );
}

#[test]
fn transfer_center_escape_contract_covers_panel_focus_and_right_panel_path_editor() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");
    let right_panel =
        fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel source");
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
fn completed_transfer_rows_use_compact_icon_buttons_with_disabled_state_contract() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("component TransferCenterRowActionButton inherits Rectangle")
            && content.contains("in property <image> icon-source;")
            && content.contains("in property <bool> enabled: true;"),
        "completed transfer rows should use a dedicated compact action-button component instead of raw text links so the utility panel reads like a desktop control strip"
    );
    assert!(
        content.contains("open-file-action := TransferCenterRowActionButton {")
            && content.contains("open-folder-action := TransferCenterRowActionButton {")
            && content.contains("remove-action := TransferCenterRowActionButton {"),
        "completed transfer rows should render Open File, Open Folder, and Remove through the shared action-button component"
    );
    assert!(
        content.contains("document-20-regular.svg")
            && content.contains("folder-open-20-regular.svg")
            && content.contains("dismiss-20-regular.svg"),
        "completed transfer actions should use Fluent-style document, folder-open, and dismiss glyphs so the row keeps a Windows utility-panel feel"
    );
    assert!(
        content.contains("height: 28px;")
            && content.contains("background: !root.enabled")
            && content.contains("touch.clicked();"),
        "transfer-center action buttons should expose a 28px hit target plus hover/press/disabled handling through the shared button surface contract"
    );
}

#[test]
fn transfer_center_row_hover_hit_target_stays_behind_action_buttons() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    let row_touch_index = content
        .find("touch := TouchArea {}")
        .expect("row hover touch area should exist");
    let open_file_index = content
        .find("open-file-action := TransferCenterRowActionButton {")
        .expect("completed open-file action should exist");
    let resolve_index = content
        .find("resolve-action := TransferCenterRowActionButton {")
        .expect("conflict resolve action should exist");

    assert!(
        row_touch_index < open_file_index && row_touch_index < resolve_index,
        "the row-level hover TouchArea should stay behind action buttons so Resolve, Open File, Open Folder, and Remove remain clickable instead of being covered by a full-row hit target"
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
    assert!(
        content.contains("retry-action := TransferCenterRowActionButton {")
            && content.contains("details-action := TransferCenterRowActionButton {")
            && content.contains("remove-action := TransferCenterRowActionButton {"),
        "failed transfer rows should graduate to the same compact icon-button language as completed rows instead of mixing chips and text links"
    );
}

#[test]
fn transfer_center_rows_use_compact_primary_and_workspace_actions() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("\"Resolve\"")
            && content.contains("\"Workspace\"")
            && content.contains("resolve-action := TransferCenterRowActionButton {")
            && content.contains("open-workspace-action := TransferCenterRowActionButton {"),
        "transfer-center conflict rows should use the same compact icon-button treatment for Resolve and Workspace instead of mixing heavy chips with lighter text actions"
    );
}

#[test]
fn transfer_center_resolve_action_uses_modal_recommended_tone() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains(
            "private property <image> resolve-icon: @image-url(\"../../assets/icons/fluent/edit-20-regular.svg\");"
        ) && content.contains("resolve-action := TransferCenterRowActionButton {")
            && content.contains("recommended: true;")
            && content.contains("icon-source: root.resolve-icon;"),
        "transfer-center Resolve should reuse the modal's recommended Auto Rename tone and a clearer review/edit icon so conflicts read as a guided next step instead of another sync action"
    );
}

#[test]
fn transfer_center_footer_and_completed_badge_share_the_same_calm_button_language() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("clear-completed-action := TransferCenterRowActionButton {")
            && content.contains("label: \"Clear Completed\";")
            && content.contains("icon-source: root.close-icon;"),
        "transfer center footer should restyle Clear Completed on the shared compact action-button component so the footer no longer falls back to a text-link treatment"
    );
    assert!(
        content.contains("? ThemeTokens.status-pill-surface")
            && content.contains("? ThemeTokens.status-success-accent"),
        "completed badges should tone down their fill and rely on a calmer success foreground so the utility panel stays refined instead of overly saturated"
    );
}

#[test]
fn transfer_center_rows_gain_a_subtle_hover_lift_instead_of_only_a_border_swap() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("row-hover-shadow := Rectangle {")
            && content.contains("background: ThemeTokens.utility-panel-shadow-soft;")
            && content.contains("opacity: row.has-hover ?"),
        "transfer rows should gain a restrained hover-lift layer so the active card feels more premium than a plain border-color swap"
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

    assert!(
        modal.contains("label: root.kind == \"download\" ? \"Remote item\" : \"Incoming item\";"),
        "conflict modal should keep a dedicated incoming-item card label for remote conflicts while renaming it for download targets"
    );
    assert!(
        modal
            .contains("label: root.kind == \"download\" ? \"Local target\" : \"Existing target\";"),
        "conflict modal should keep a dedicated existing-target card label for remote conflicts while renaming it for download targets"
    );
    assert!(
        modal.contains("text: \"Folder scope\";"),
        "conflict modal should render a labeled `Folder scope` card so the narrow dialog reads like a mature transfer sheet instead of raw concatenated path text"
    );

    assert!(
        !modal.contains("text: \"Source: \" + root.source-path;")
            && !modal.contains("text: \"Target: \" + root.target-path;"),
        "conflict modal should stop rendering raw `Source:` / `Target:` lines once the labeled cards exist"
    );
    assert!(
        modal.contains("Auto Rename or Replace Existing can also be applied to the other conflict in this folder.")
            && modal.contains("Auto Rename or Replace Existing can also be applied to the other ")
            && modal.contains("Skip This Download always affects only this item.")
            && modal.contains("Apply the selected action to the other "),
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
        modal.contains("event.text == Key.Escape") && modal.contains("root.close-requested();"),
        "conflict modal should treat Escape as the explicit skip-this-download dismissal path"
    );
    assert!(
        modal.contains("public function focus-dialog()")
            && modal.contains("auto-rename-button.focus-button();")
            && !modal.contains("event.text == Key.Return"),
        "conflict modal should default focus to Auto Rename and avoid a modal-level Enter override that bypasses button focus"
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
            && modal.contains(": root.has-focus ? ThemeTokens.control-hover-surface"),
        "conflict modal batch-toggle row should reserve a distinct focus treatment instead of looking identical to the passive scope card"
    );
}

#[test]
fn transfer_center_attention_actions_expose_tooltips_and_keyboard_button_contract() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("tooltip-text: \"Review conflict options for this transfer\";")
            && content.contains("tooltip-text: \"Open this conflict in the SFTP workspace\";"),
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
        !content.contains("component TransferCenterActionChip inherits Rectangle")
            && !content.contains("component TransferCenterActionLink inherits Rectangle")
            && content.contains("component TransferCenterRowActionButton inherits Rectangle")
            && content.contains("private property <brush> chrome-border: root.has-focus")
            && content.contains("focus-halo := Rectangle {")
            && content.contains("border-color: ThemeTokens.focus-ring;"),
        "transfer-center row actions should consolidate on the shared compact button component while keeping a stronger Fluent focus-ring treatment"
    );
    assert!(
        content.contains("height: 28px;")
            && content.contains("border-width: 1px;")
            && content.contains("opacity: root.has-focus ? 0.46 : 0;"),
        "transfer-center row actions should keep the compact 28px utility-button size plus a distinct focus halo so keyboard and pointer states read differently"
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
        "conflict modal footer buttons should expose a real button contract so Tab navigation can reach Skip This Download, Auto Rename, and Replace Existing"
    );
    assert!(
        modal.contains("button-focus := FocusScope {")
            && modal.contains("event.text == \" \" || event.text == \"\\n\"")
            && modal.contains("button-touch.clicked();"),
        "conflict modal footer buttons should respond to keyboard activation instead of remaining pointer-only"
    );
    assert!(
        modal.contains("skip-button := ConflictDialogButton {")
            && modal.contains("auto-rename-button := ConflictDialogButton {")
            && modal.contains("replace-button := ConflictDialogButton {"),
        "conflict modal footer should keep all three actions in the shared focusable button component"
    );
}

#[test]
fn transfer_center_conflict_modal_exposes_download_actions() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        modal.contains("in property <string> kind: \"remote\";")
            && modal.contains("callback auto-rename-requested();")
            && modal.contains("callback skip-requested();"),
        "conflict modal should project a distinct download mode contract instead of hard-coding replace/skip semantics for every conflict type"
    );
    assert!(
        modal.contains("Skip This Download")
            && modal.contains("Auto Rename")
            && modal.contains("Replace Existing")
            && modal.contains("This affects the current download only.")
            && !modal.contains("Cancel Download"),
        "download conflict modals should expose explicit Skip This Download, Auto Rename, and Replace Existing actions with current-download scope copy"
    );
    assert!(
        app_window.contains("in-out property <string> sftp-conflict-modal-kind: \"remote\";")
            && app_window.contains("callback sftp-conflict-modal-auto-rename-requested();")
            && !app_window.contains("callback sftp-conflict-modal-cancel-download-requested();"),
        "AppWindow should thread the download conflict modal kind and keep the old cancel-download callback out of the generated Slint API"
    );
}

#[test]
fn transfer_center_conflict_modal_skip_uses_only_the_current_task() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");
    let view_model_sftp =
        fs::read_to_string("src/shell/view_model/sftp.rs").expect("read shell view-model sftp");

    assert!(
        view_model_sftp.contains("pub fn current_sftp_conflict_task(&self)")
            && view_model_sftp.contains("self.sftp_conflict_modal_state.task_id.as_deref()"),
        "ShellViewModel should expose the currently focused conflict task separately from the apply-to-batch expansion"
    );
    assert!(
        bootstrap_sftp.contains("window.on_sftp_conflict_modal_skip_requested(move || {")
            && bootstrap_sftp.contains("window.on_sftp_conflict_modal_close_requested(move || {")
            && bootstrap_sftp.contains("current_sftp_conflict_task()"),
        "skip and close handling should resolve only the current conflict instead of inheriting the apply-to-batch selection"
    );
}

#[test]
fn conflict_modal_close_button_exposes_fluent_icon_and_skip_tooltip_contract() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");

    assert!(
        modal.contains("component ConflictIconButton inherits Rectangle")
            && modal.contains("@image-url(\"../../assets/icons/fluent/dismiss-20-regular.svg\")")
            && modal.contains("tooltip-text: root.close-action-label();")
            && modal.contains("return \"Skip this download\";")
            && modal.contains("tooltip-source-id: \"sftp-conflict-close\";")
            && modal.contains("accessible-role: AccessibleRole.button;")
            && modal.contains("forward-focus: button-focus;"),
        "conflict modal close affordance should use the Fluent dismiss icon and explicit skip-this-download tooltip copy"
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
fn conflict_modal_uses_elevated_shell_and_separate_footer_structure() {
    let modal =
        fs::read_to_string("ui/components/sftp-conflict-modal.slint").expect("read conflict modal");
    let shell =
        fs::read_to_string("ui/components/blocking-modal-shell.slint").expect("read modal shell");
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    assert!(
        modal.contains("ModalBodyScrollArea")
            && modal.contains("ModalFooterBar")
            && modal.contains("clip: true;")
            && !modal.contains("cancel-button := ConflictDialogButton {"),
        "conflict modal should move to a clear header/body/footer structure instead of the old absolutely positioned cancel cluster"
    );
    assert!(
        shell.contains("modal-glow := Rectangle {")
            && shell.contains("modal-shadow-far := Rectangle {")
            && shell.contains("modal-shadow-near := Rectangle {"),
        "blocking modal shell should expose elevated glow and shadow layers for the redesigned conflict modal"
    );
    assert!(
        tokens.contains("out property <brush> conflict-dialog-glow:")
            && tokens.contains("out property <brush> conflict-dialog-border:")
            && tokens.contains("out property <brush> conflict-dialog-surface:"),
        "theme tokens should expose dedicated conflict-dialog visual slots so dark/light themes can stay aligned"
    );
}

#[test]
fn conflict_modal_light_theme_tokens_stay_soft_and_fluent() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    assert!(
        tokens.contains(
            "out property <brush> conflict-dialog-surface: dark-mode ? #1a2330 : #f2f6fb;"
        ) && tokens.contains(
            "out property <brush> conflict-dialog-path-surface: dark-mode ? #101822 : #f7faff;"
        ) && tokens.contains(
            "out property <brush> conflict-dialog-border: dark-mode ? #ffffff2d : #8ea4bf36;"
        ) && tokens.contains(
            "out property <brush> conflict-dialog-glow: dark-mode ? #7da8d91a : #7c9fc914;"
        ),
        "light-theme conflict dialog tokens should stay soft and elevated instead of drifting into stark white cards or noisy blue glow"
    );
}

#[test]
fn transfer_center_conflict_actions_use_clearer_workspace_and_resolve_copy() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("tooltip-text: \"Review conflict options for this transfer\";")
            && content.contains("tooltip-text: \"Open this conflict in the SFTP workspace\";")
            && !content.contains("tooltip-text: \"Resolve transfer conflict\";")
            && !content.contains("tooltip-text: \"Open task in SFTP workspace\";"),
        "transfer-center conflict actions should use clearer review/workspace wording that matches the new modal semantics"
    );
}

#[test]
fn transfer_center_row_actions_keep_a_stable_source_focus_order() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    let retry_index = content
        .find("retry-action := TransferCenterRowActionButton {")
        .expect("retry action contract");
    let resolve_index = content
        .find("resolve-action := TransferCenterRowActionButton {")
        .expect("resolve action contract");
    let workspace_index = content
        .find("open-workspace-action := TransferCenterRowActionButton {")
        .expect("workspace action contract");

    assert!(
        retry_index < resolve_index && resolve_index < workspace_index,
        "transfer-center row actions should stay declared in their intended left-to-right focus order so source-contract smoke can catch accidental reordering"
    );
    assert!(
        content.contains("x: row.action-start-x;")
            && content.contains("x: row.attention-secondary-x;")
            && content.contains("? row.attention-tertiary-x")
            && content.contains("forward-focus: action-focus;"),
        "transfer-center action affordances should keep a compact left-to-right layout backed by explicit focus forwarding"
    );
}

#[test]
fn transfer_center_attention_action_uses_dynamic_resume_restart_pause_copy() {
    let content =
        fs::read_to_string("ui/shell/transfer-center.slint").expect("read transfer center source");

    assert!(
        content.contains("label: item.attention_label;")
            && content.contains("item.attention_action == \"pause\"")
            && content.contains("root.transfer-row-pause-requested(item.id);")
            && content.contains("root.transfer-row-resume-requested(item.id);")
            && content.contains("root.transfer-row-restart-requested(item.id);"),
        "transfer-center primary attention action should use projected pause/resume/restart copy and route to explicit callbacks instead of a hard-coded retry button"
    );
    assert!(
        content.contains("private property <bool> retry-tooltip-active: false;")
            && content.contains("private property <bool> row-action-tooltip-active: self.retry-tooltip-active || self.resolve-tooltip-active || self.workspace-tooltip-active;")
            && content.contains("row.retry-tooltip-active = active;"),
        "transfer-center attention actions should stay inside the shared action-tooltip ownership model so error hover text does not compete with pause/resume/restart hints"
    );
}
