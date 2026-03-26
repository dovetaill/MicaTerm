# SSH Create / Connect / Tabs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the confirmed SSH workspace refactor: hide the empty-state tab strip, stabilize tab close behavior, make every SSH `Open` create a new session tab, simplify the SSH asset context menu, and remove the unused modal connection-options fields.

**Architecture:** Keep the existing `AppWindow -> WorkspacePane -> TabBar / TerminalSessionHost` UI composition and the current `bootstrap -> ShellViewModel -> SessionManager -> SshSessionRuntime` runtime chain. The work is a focused semantic correction, not a runtime rewrite: first lock the new UI and context-menu contracts with failing tests, then adjust Slint geometry ownership, then switch SSH open flows from single-session reuse to multi-session-per-asset behavior, and finally trim the dead modal fields.

**Tech Stack:** Rust, Slint, Tokio, `russh`, `wezterm-term`, `termwiz`, `cargo test`, shell smoke scripts

---

## Ground Rules

- Do not restore `Connect` / `Save and Connect` into the modal footer. The modal stays on the current `Test` + `Save` action set.
- Do not reintroduce rounded corners.
- Do not expand scope into SFTP, proxy runtime wiring, environment-variable injection, or persistence-schema redesign.
- Keep changes incremental and test-driven.
- Prefer deleting obsolete contract branches instead of carrying duplicate semantics.

### Task 1: Hide the empty-state tab strip and make `WorkspacePane` own a single surface

**Files:**
- Modify: `ui/shell/workspace-pane.slint:7-80`
- Modify: `ui/shell/tabbar.slint:14-66`
- Modify: `ui/app-window.slint:428-464`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing test**

Add a new UI contract test in `tests/workspace_tabs_spec.rs` that locks the empty-state behavior:

```rust
#[test]
fn workspace_pane_only_renders_tabbar_when_tabs_exist() {
    let workspace = fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    assert!(
        workspace.contains("if root.workspace-tab-items.length > 0 : tab-strip := TabBar {"),
        "workspace pane should only render the tab strip when at least one tab exists"
    );
}
```

Update `tests/ssh_connect_tabs_ui_contract_smoke.sh` so it expects the same conditional Slint contract instead of an always-present `tab-strip := TabBar {`.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test workspace_tabs_spec workspace_pane_only_renders_tabbar_when_tabs_exist -- --exact
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- The Rust test fails because `workspace-pane.slint` still renders `TabBar` unconditionally.
- The shell smoke test fails on the old unconditional `TabBar` contract.

**Step 3: Write minimal implementation**

Change `WorkspacePane` so the tab strip is conditional and the content host continues to occupy the full height when there are no tabs:

```slint
VerticalLayout {
    width: 100%;
    height: 100%;
    spacing: 0px;

    if root.workspace-tab-items.length > 0 : tab-strip := TabBar {
        width: 100%;
        min-width: 0px;
        items: root.workspace-tab-items;
        // callbacks unchanged
    }

    content-host := Rectangle {
        width: 100%;
        min-width: 0px;
        vertical-stretch: 1;
        background: transparent;
        // TerminalSessionHost stays here
    }
}
```

Do not move the workspace responsibilities into `Sidebar`. The ownership stays inside `WorkspacePane`.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test workspace_tabs_spec workspace_pane_only_renders_tabbar_when_tabs_exist -- --exact
cargo test --test workspace_tabs_spec single_click_only_selects_saved_asset_without_opening_session -- --exact
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- The new empty-state contract test passes.
- Existing workspace tab tests still pass.
- The smoke script exits `0`.

**Step 5: Commit**

```bash
git add ui/shell/workspace-pane.slint ui/shell/tabbar.slint ui/app-window.slint tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: hide empty workspace tab strip"
```

### Task 2: Stabilize `ActiveTab` close behavior with fixed hit geometry

**Files:**
- Modify: `ui/components/active-tab.slint:5-126`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing test**

Extend the existing close-affordance contract in `tests/workspace_tabs_spec.rs`:

```rust
#[test]
fn close_affordance_uses_stable_hit_geometry() {
    let active_tab = fs::read_to_string("ui/components/active-tab.slint").expect("read active tab");
    assert!(
        !active_tab.contains("root.close-visible ? close-button.x : parent.width"),
        "content hit target width must not depend on close-visible hover state"
    );
    assert!(
        active_tab.contains("close-hit-target := TouchArea {"),
        "close hit target should remain a dedicated stable touch area"
    );
}
```

Also update `tests/ssh_connect_tabs_ui_contract_smoke.sh` to reject the old dynamic-width expression.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test workspace_tabs_spec close_affordance_uses_stable_hit_geometry -- --exact
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- The Rust test fails because `active-tab.slint` still contains the hover-dependent width expression.
- The smoke script fails for the same reason.

**Step 3: Write minimal implementation**

Refactor `ActiveTab` so hover only affects visuals, not geometry:

```slint
export component ActiveTab inherits Rectangle {
    private property <length> close-zone-width: 28px;

    close-button := Rectangle {
        x: parent.width - close-zone-width - 4px;
        // opacity can still depend on hover
    }

    content-hit-target := TouchArea {
        width: max(0px, close-button.x);
        height: parent.height;
    }

    close-hit-target := TouchArea {
        x: close-button.x;
        width: close-zone-width + 4px;
        height: parent.height;
    }
}
```

Keep the visual `opacity` change if desired, but do not let hover change touch geometry.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test workspace_tabs_spec close_affordance_is_modeled_separately_from_select_action -- --exact
cargo test --test workspace_tabs_spec close_affordance_uses_stable_hit_geometry -- --exact
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- Both tab-affordance tests pass.
- The smoke script exits `0`.

**Step 5: Commit**

```bash
git add ui/components/active-tab.slint tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "fix: stabilize workspace tab close hit targets"
```

### Task 3: Change SSH asset opening from single-session reuse to new-tab-per-open

**Files:**
- Modify: `src/app/ssh/session_manager.rs:79-160`
- Modify: `src/app/bootstrap.rs:1129-1413`
- Modify: `src/app/bootstrap.rs:2691-2896`
- Test: `tests/assets_explorer_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Replace the reuse-based expectations with new-tab expectations.

In `tests/assets_explorer_smoke.rs`, replace:

- `double_clicking_ssh_asset_opens_session_and_reuses_existing_tab`
- `explicit_open_context_action_opens_session_and_reuses_existing_tab`

with:

```rust
#[test]
fn double_clicking_same_ssh_asset_twice_creates_two_distinct_tabs() {
    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.clone().into());
    let first = app.get_workspace_tab_items().row_data(0).unwrap().session_id.to_string();

    app.invoke_asset_activated(ssh_id.into());
    let second = app.get_workspace_tab_items().row_data(1).unwrap().session_id.to_string();

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_ne!(first, second);
}
```

Add the same semantic contract for context-menu `Open`.

In `tests/bootstrap_smoke.rs`, add one focused regression that saved SSH assets now create a second tab when opened twice instead of reusing the first.

**Step 2: Run tests to verify it fails**

Run:

```bash
cargo test --test assets_explorer_smoke double_clicking_same_ssh_asset_twice_creates_two_distinct_tabs -- --exact
cargo test --test assets_explorer_smoke explicit_open_context_action_opens_distinct_tabs_each_time -- --exact
cargo test --test bootstrap_smoke opening_saved_ssh_asset_twice_creates_two_tabs -- --exact
```

Expected:

- The tests fail because `SessionManager::open_session()` still reuses an existing session for the same `asset_id`.

**Step 3: Write minimal implementation**

Route asset-triggered opens to “always create a new tab” behavior:

```rust
// src/app/bootstrap.rs
attempt_open_session_with_profile(
    state,
    session_bridge,
    pending_host_key_approval,
    profile,
    OpenSessionMode::ForceNewTab,
)
```

Then simplify `SessionManager::open_session()`:

```rust
pub fn open_session(&self, profile: ConnectionProfile, mode: OpenSessionMode) -> Result<SessionHandle> {
    let asset_id = profile.asset_id.clone().context("session profile requires asset_id")?;

    // remove the ActivateExisting early-return reuse branch
    // always allocate a fresh session_id for ForceNewTab callers
}
```

If `ActivateExisting` becomes unused after this task, delete the enum variant and its reuse-specific registry code in the same commit.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test assets_explorer_smoke -- --nocapture
cargo test --test workspace_tabs_spec double_click_and_open_in_new_tab_create_distinct_sessions -- --exact
cargo test --test bootstrap_smoke opening_saved_ssh_asset_twice_creates_two_tabs -- --exact
```

Expected:

- Every asset open now produces a distinct `session_id`.
- The old reuse-based tests are either updated or removed.
- Workspace projection still handles multiple tabs correctly.

**Step 5: Commit**

```bash
git add src/app/ssh/session_manager.rs src/app/bootstrap.rs tests/assets_explorer_smoke.rs tests/workspace_tabs_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: open a new ssh tab for every asset open action"
```

### Task 4: Simplify the SSH asset context menu to a single `Open`

**Files:**
- Modify: `src/shell/context_menu.rs:225-333`
- Modify: `src/shell/view_model.rs:1410-1493`
- Modify: `src/app/bootstrap.rs:2810-2905`
- Test: `tests/assets_context_menu_spec.rs`
- Test: `tests/assets_context_menu_smoke.rs`
- Test: `tests/assets_explorer_smoke.rs`

**Step 1: Write the failing tests**

Update `tests/assets_context_menu_spec.rs` so SSH actions now expect:

```rust
#[test]
fn ssh_context_menu_exposes_only_one_open_action() {
    let actions = resolve_action_tree(
        ContextTargetKind::SshConnection,
        &SelectionContext {
            selected_ids: vec!["ssh-prod-01".into()],
            clipboard_has_asset_payload: false,
            target_mutable: true,
            target_has_active_connection: false,
        },
    );

    let ids: Vec<_> = actions.iter().map(|action| action.id).collect();
    assert!(ids.contains(&"open-connection"));
    assert!(!ids.contains(&"open-in-new-tab"));
    assert!(!ids.contains(&"close-connection"));
}
```

Remove or replace the smoke tests that assert `close-connection` enablement.

In `tests/assets_context_menu_smoke.rs`, add a direct behavior test:

```rust
#[test]
fn invoking_open_from_ssh_context_menu_creates_a_new_session_tab() {
    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);
    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_context_menu_requested(ssh_id.clone().into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("open-connection".into());
    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("open-connection".into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
}
```

**Step 2: Run tests to verify it fails**

Run:

```bash
cargo test --test assets_context_menu_spec ssh_context_menu_exposes_only_one_open_action -- --exact
cargo test --test assets_context_menu_smoke invoking_open_from_ssh_context_menu_creates_a_new_session_tab -- --exact
```

Expected:

- The spec test fails because `resolve_ssh_actions()` still returns `open-in-new-tab` and `close-connection`.
- The smoke test may fail because the old context menu logic still has duplicate action branches.

**Step 3: Write minimal implementation**

Trim the menu tree and remove obsolete state branches:

```rust
fn resolve_ssh_actions(selection: &SelectionContext) -> Vec<ContextMenuActionNode> {
    let mut actions = vec![
        action_with_state(
            "open-connection",
            "Open",
            "window-console",
            selection_state(selection),
            false,
        ),
    ];
    actions.extend(create_actions(true));
    actions.extend([
        action_with_state("edit-connection", "Edit", "edit", selection_state(selection), true),
        action_with_state("batch-edit", "Batch Edit", "edit", selection_state(selection), false),
        action_with_state("clone-connection", "Clone", "copy", selection_state(selection), false),
        action_with_state("copy-host", "Copy Host", "copy", selection_state(selection), false),
        action_with_state("delete-asset", "Delete", "delete", mutable_selection_state(selection), true),
        action_with_state("rename-asset", "Rename", "edit", mutable_selection_state(selection), false),
    ]);
    actions
}
```

Also remove view-model logic that only existed to compute `target_has_active_connection` for the deleted `close-connection` branch.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test assets_context_menu_spec -- --nocapture
cargo test --test assets_context_menu_smoke -- --nocapture
cargo test --test assets_explorer_smoke explicit_open_context_action_opens_distinct_tabs_each_time -- --exact
```

Expected:

- No spec or smoke test expects `open-in-new-tab` or `close-connection`.
- `open-connection` remains the only SSH open action and creates a new session each time.

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs src/shell/view_model.rs src/app/bootstrap.rs tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs tests/assets_explorer_smoke.rs
git commit -m "refactor: simplify ssh asset context menu to a single open action"
```

### Task 5: Remove the dead SSH modal connection-options fields

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint:593-623`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing test**

Add or update a modal-contract test so the file no longer contains the dead group and labels:

```rust
#[test]
fn ssh_modal_no_longer_renders_dead_connection_options_group() {
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");
    assert!(!ssh_modal.contains("text: \"Connection Options\""));
    assert!(!ssh_modal.contains("label: \"Proxy Method\""));
    assert!(!ssh_modal.contains("label: \"Session Environment\""));
}
```

Update `tests/assets_modal_ui_contract_smoke.sh` to reject those same strings with `! grep -F`.

**Step 2: Run tests to verify it fails**

Run:

```bash
cargo test --test assets_modal_smoke ssh_modal_no_longer_renders_dead_connection_options_group -- --exact
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- The Rust test fails because the modal still contains the connection-options group.
- The shell smoke test fails on the old labels.

**Step 3: Write minimal implementation**

Delete the dead UI group only. Do not expand scope into persistence or runtime cleanup in this task:

```slint
// ui/components/assets-ssh-connection-modal.slint
// remove:
// - Text { text: "Connection Options"; ... }
// - FormField { label: "Proxy Method"; ... }
// - FormField { label: "Session Environment"; ... }
```

Keep the remaining sections and footer untouched.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- The modal contract tests pass.
- `Test` and `Save` remain the only visible footer actions.

**Step 5: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "refactor: remove dead ssh modal connection options fields"
```

### Task 6: Run the focused regression suite before closing the branch

**Files:**
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/assets_explorer_smoke.rs`
- Test: `tests/assets_context_menu_spec.rs`
- Test: `tests/assets_context_menu_smoke.rs`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`
- Test: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Run the focused Rust regression suite**

Run:

```bash
cargo test --test workspace_tabs_spec --test assets_explorer_smoke --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_modal_smoke --test bootstrap_smoke
```

Expected:

- All targeted Rust tests pass.
- Any old test that still expects session reuse or `open-in-new-tab` is updated or deleted in the earlier tasks.

**Step 2: Run the focused shell smoke scripts**

Run:

```bash
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- Both scripts exit `0`.

**Step 3: Inspect git diff before handing off**

Run:

```bash
git diff --stat
git diff -- docs/plans/2026-03-26-ssh-create-connect-tabs-design.md ui/components/active-tab.slint ui/shell/workspace-pane.slint src/app/ssh/session_manager.rs src/app/bootstrap.rs src/shell/context_menu.rs ui/components/assets-ssh-connection-modal.slint tests/workspace_tabs_spec.rs tests/assets_explorer_smoke.rs tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs tests/assets_modal_smoke.rs tests/bootstrap_smoke.rs tests/ssh_connect_tabs_ui_contract_smoke.sh tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- The diff is limited to the confirmed scope.
- No unrelated runtime, SFTP, or persistence redesign changes appear.

**Step 4: Inspect final branch status**

Run:

```bash
git status --short
```

Expected:

- The working tree is either clean because each earlier task commit already landed, or it only contains intentional follow-up edits discovered during the verification pass.
- There are no unrelated modified files.

**Step 5: Commit only if verification required a final follow-up fix**

If Steps 1-4 uncovered a small regression and you changed files to fix it, run:

```bash
git add tests/workspace_tabs_spec.rs tests/assets_explorer_smoke.rs tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs tests/assets_modal_smoke.rs tests/bootstrap_smoke.rs tests/ssh_connect_tabs_ui_contract_smoke.sh tests/assets_modal_ui_contract_smoke.sh
git commit -m "test: lock ssh workspace shell and modal regression coverage"
```

Expected:

- If no additional edits were needed, skip this step and leave the branch on the commits created in Tasks 1-5.
- If a follow-up fix was needed, there is exactly one final commit that contains only the verification-driven adjustments.
