# SSH New Tab Productized Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Productize the MCA TERM right-side New Tab surface into a mature SSH launcher with real recent connection data, a natural saved SSH entry, and restrained terminal visuals.

**Architecture:** Keep the existing workspace launcher and OpenSavedSshModal flow. Add real recent timestamps to quick launch preferences, project them into the existing Slint recent row model, and replace the current welcome dashboard styling with a focused intro + recent list layout.

**Tech Stack:** Rust, Slint, serde, chrono, existing MCA TERM ShellViewModel and AppWindow bindings.

---

### Task 1: Recent Data Model

**Files:**
- Modify: `src/app/quick_launch_preferences.rs`
- Modify: `src/shell/quick_launch.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/quick_launch.rs`
- Test: `tests/quick_launch_preferences_spec.rs`
- Test: `tests/quick_launch_projection_spec.rs`

**Steps:**
1. Add `QuickLaunchRecentAsset` with `asset_id` and `opened_at_unix_seconds`.
2. Keep backward compatibility with existing `recent_asset_ids` JSON arrays of strings.
3. Change `record_recent_saved_ssh_asset` to store `Utc::now().timestamp()`.
4. Cap recent list at 7.
5. Add tests for compatibility, cap, and non-empty time labels.

### Task 2: New Tab Row Projection

**Files:**
- Modify: `src/shell/quick_launch.rs`
- Modify: `src/shell/view_model/quick_launch.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/welcome/quick-launch-types.slint`
- Test: `tests/quick_launch_projection_spec.rs`

**Steps:**
1. Add `time_label` to `QuickLaunchCardItem` and `QuickLaunchCardRow`.
2. Compute time labels from recent `opened_at_unix_seconds`.
3. Keep existing quick launch callbacks and selectors intact.
4. Verify recent row order and row count still work.

### Task 3: New Tab Visual Surface

**Files:**
- Modify: `ui/welcome/welcome-view.slint`
- Modify: `ui/welcome/quick-launch-card.slint`
- Modify: `ui/welcome/quick-launch-section.slint`
- Create: `assets/icons/new-tab/open-saved-ssh.svg`
- Create: `assets/icons/new-tab/server-stack.svg`
- Test: `tests/quick_launch_ui_contract_smoke.sh`

**Steps:**
1. Replace the heavy header card with a compact intro block.
2. Add a restrained right-side terminal/connection motif using Slint shapes.
3. Render `Open Saved SSH` as an integrated action, not a detached large dashboard button.
4. Convert recent entries into modern list rows with icon, title, subtitle, time label, and chevron.
5. Simplify empty state while keeping the saved SSH action visible.

### Task 4: Interaction Verification

**Files:**
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/quick_launch_ui_contract_smoke.sh`

**Steps:**
1. Confirm clicking a recent row still invokes `welcome_quick_launch_connect_requested`.
2. Confirm clicking Open Saved SSH opens `OpenSavedSshModal`.
3. Confirm activating a saved SSH from the modal replaces launcher tab with a real session tab.
4. Confirm recent data updates after connection.

### Task 5: Final Verification

**Commands:**
- `cargo test quick_launch_preferences_spec --test quick_launch_preferences_spec`
- `cargo test quick_launch_projection_spec --test quick_launch_projection_spec`
- `cargo test launcher_recent_connection_replaces_launcher_tab_with_real_session_tab --test bootstrap_smoke`
- `cargo test launcher_picker_activation_replaces_launcher_tab_and_closes_modal --test bootstrap_smoke`
- `bash tests/quick_launch_ui_contract_smoke.sh`
- `cargo check`
