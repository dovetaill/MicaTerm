# SSH Quick Launch Dashboard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the blank welcome workspace with an SSH-first quick launch dashboard for saved connections, backed by local recent/favorite preferences and wired into the existing saved-asset SSH tab flow.

**Architecture:** Add a dedicated local quick-launch preferences store beside UI preferences, project saved SSH assets plus local MRU/favorites into view-model data, expose those projections through `AppWindow -> WorkspacePane -> TerminalSessionHost -> WelcomeView`, and route dashboard actions through the existing `runtime_profile_for_saved_asset(...)` + `open_session(...)` path so welcome startup stays aligned with current SSH session semantics.

**Tech Stack:** Rust, Slint, JSON preference storage, existing asset catalog and session manager, cargo tests, shell UI contract smoke scripts

---

### Task 1: Add Quick Launch Preference Storage

**Files:**
- Create: `src/app/quick_launch_preferences.rs`
- Modify: `src/app/mod.rs`
- Test: `tests/quick_launch_preferences_spec.rs`

**Step 1: Write the failing test**

Add a targeted spec that locks the minimum persisted contract:

```rust
#[test]
fn quick_launch_preferences_roundtrip_preserves_recent_and_favorites() {
    let prefs = QuickLaunchPreferences {
        favorite_asset_ids: vec!["asset-prod".into()],
        recent_asset_ids: vec!["asset-db".into(), "asset-prod".into()],
        last_selected_asset_id: Some("asset-db".into()),
    };

    store.save(&prefs).unwrap();
    assert_eq!(store.load_or_default().unwrap(), prefs);
}

#[test]
fn record_recent_moves_asset_to_front_and_caps_history() {
    let updated = record_recent_asset_id(vec!["a".into(), "b".into()], "a", 2);
    assert_eq!(updated, vec!["a", "b"]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test quick_launch_preferences_spec -q`

Expected: FAIL because `quick_launch_preferences` module and store types do not exist yet.

**Step 3: Write minimal implementation**

Create a dedicated JSON store modeled after `UiPreferencesStore`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickLaunchPreferences {
    pub favorite_asset_ids: Vec<String>,
    pub recent_asset_ids: Vec<String>,
    pub last_selected_asset_id: Option<String>,
}

pub struct QuickLaunchPreferencesStore {
    path: PathBuf,
}

impl QuickLaunchPreferencesStore {
    pub fn for_app() -> Result<Self> { /* ProjectDirs -> quick-launch-preferences.json */ }
    pub fn load_or_default(&self) -> Result<QuickLaunchPreferences> { /* read-or-default */ }
    pub fn save(&self, prefs: &QuickLaunchPreferences) -> Result<()> { /* pretty json */ }
}

pub fn record_recent_asset_id(existing: Vec<String>, asset_id: &str, cap: usize) -> Vec<String> {
    /* dedupe, push front, truncate */
}

pub fn retain_known_ssh_asset_ids(
    prefs: &QuickLaunchPreferences,
    known_asset_ids: &BTreeSet<String>,
) -> QuickLaunchPreferences {
    /* drop deleted/non-ssh ids, preserve order */
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test quick_launch_preferences_spec -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/quick_launch_preferences.rs src/app/mod.rs tests/quick_launch_preferences_spec.rs
git commit -m "feat: add quick launch preferences store"
```

### Task 2: Add Quick Launch Projection and Selection State

**Files:**
- Create: `src/shell/quick_launch.rs`
- Modify: `src/shell/mod.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/quick_launch_projection_spec.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing test**

Add projection coverage for recent/favorite/group sections and welcome selection fallback:

```rust
#[test]
fn quick_launch_recent_projection_prefers_mru_order_and_ssh_assets_only() {
    let mut view_model = seeded_view_model();
    view_model.apply_quick_launch_preferences(QuickLaunchPreferences {
        recent_asset_ids: vec!["asset-db".into(), "asset-prod".into()],
        favorite_asset_ids: vec![],
        last_selected_asset_id: None,
    });

    let recent = view_model.quick_launch_recent_items();
    assert_eq!(recent[0].asset_id, "asset-db");
}

#[test]
fn quick_launch_selection_falls_back_to_first_visible_item() {
    let mut view_model = seeded_view_model();
    view_model.ensure_quick_launch_selection();
    assert_eq!(view_model.quick_launch_selected_asset_id(), Some("asset-prod"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test quick_launch_projection_spec --test shell_view_model -q`

Expected: FAIL because quick-launch projection structs and view-model APIs do not exist yet.

**Step 3: Write minimal implementation**

Create a dedicated shell-side projection module and keep raw derivation out of `view_model.rs`:

```rust
pub struct QuickLaunchCardItem {
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub meta: String,
    pub icon_kind: String,
    pub accent_kind: String,
    pub favorite: bool,
}

pub struct QuickLaunchGroupItem {
    pub group_id: String,
    pub label: String,
    pub count: usize,
}

pub struct QuickLaunchDetailItem {
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub environment: String,
    pub auth_summary: String,
    pub proxy_summary: String,
    pub remark: String,
    pub recent_label: String,
}
```

Extend `ShellViewModel` with:

```rust
quick_launch_preferences: QuickLaunchPreferences,
quick_launch_search_query: String,
quick_launch_selected_asset_id: Option<String>,
quick_launch_active_group_id: Option<String>,
```

Add methods:

- `apply_quick_launch_preferences(...)`
- `record_recent_saved_ssh_asset(...)`
- `toggle_quick_launch_favorite(...)`
- `select_quick_launch_asset(...)`
- `set_quick_launch_search_query(...)`
- `quick_launch_recent_items()`
- `quick_launch_favorite_items()`
- `quick_launch_group_items()`
- `quick_launch_visible_group_items()`
- `quick_launch_selected_detail()`
- `ensure_quick_launch_selection()`

Projection rules:

- only include saved `SshConnection` assets
- use `title`, `user@host`, `environment`, `remark`
- map icon/accent from environment/title keywords
- keep recent/favorite order from preferences, not alphabetical order

**Step 4: Run test to verify it passes**

Run: `cargo test --test quick_launch_projection_spec --test shell_view_model -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/quick_launch.rs src/shell/mod.rs src/shell/view_model.rs tests/quick_launch_projection_spec.rs tests/shell_view_model.rs
git commit -m "feat: add quick launch view model projections"
```

### Task 3: Rebuild WelcomeView as a Quick Launch Dashboard

**Files:**
- Create: `ui/welcome/quick-launch-types.slint`
- Create: `ui/welcome/quick-launch-card.slint`
- Create: `ui/welcome/quick-launch-section.slint`
- Create: `ui/welcome/quick-launch-detail-pane.slint`
- Modify: `ui/welcome/welcome-view.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/quick_launch_ui_contract_smoke.sh`
- Modify: `tests/shell_layout_ui_contract_smoke.sh`

**Step 1: Write the failing UI contract**

Add a smoke script that asserts the new welcome shell contract exists:

```bash
grep -F 'text: "Quick Start"' "$WELCOME" >/dev/null
grep -F 'QuickLaunchSection' "$WELCOME" >/dev/null
grep -F 'QuickLaunchDetailPane' "$WELCOME" >/dev/null
grep -F 'callback welcome-quick-launch-connect-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback welcome-quick-launch-connect-in-new-tab-requested(string);' "$APP_WINDOW" >/dev/null
```

**Step 2: Run test to verify it fails**

Run: `bash tests/quick_launch_ui_contract_smoke.sh`

Expected: FAIL because welcome still renders only the legacy two-line placeholder.

**Step 3: Write minimal implementation**

Introduce shared Slint structs:

```slint
export struct QuickLaunchCardRow {
    asset_id: string,
    title: string,
    subtitle: string,
    badge: string,
    meta: string,
    icon_kind: string,
    accent_kind: string,
    favorite: bool,
    selected: bool,
}
```

Expose quick-launch properties and callbacks on the window chain:

- `ui/app-window.slint`
- `ui/shell/workspace-pane.slint`
- `ui/shell/terminal-session-host.slint`

Callbacks to add:

- `welcome-quick-launch-asset-selected(string)`
- `welcome-quick-launch-search-changed(string)`
- `welcome-quick-launch-connect-requested(string)`
- `welcome-quick-launch-connect-in-new-tab-requested(string)`
- `welcome-quick-launch-toggle-favorite-requested(string)`
- `welcome-quick-launch-reveal-in-assets-requested(string)`

Welcome UI requirements:

- replace headline-only layout with dashboard layout
- recent section first, favorites second, groups third
- detail pane pinned on the right on wide layouts, stacked below on narrow layouts
- reuse current theme tokens before adding new tokens
- use existing Fluent icon family first; no new design system

**Step 4: Run test to verify it passes**

Run: `bash tests/quick_launch_ui_contract_smoke.sh`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/welcome/quick-launch-types.slint ui/welcome/quick-launch-card.slint ui/welcome/quick-launch-section.slint ui/welcome/quick-launch-detail-pane.slint ui/welcome/welcome-view.slint ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint tests/quick_launch_ui_contract_smoke.sh tests/shell_layout_ui_contract_smoke.sh
git commit -m "feat: redesign welcome workspace as quick launch dashboard"
```

### Task 4: Wire Dashboard Actions Through Bootstrap and Persist State

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/assets_explorer_smoke.rs`

**Step 1: Write the failing bootstrap test**

Add targeted tests that exercise welcome-driven actions:

```rust
#[test]
fn quick_launch_connect_opens_saved_asset_session_and_updates_recent_order() {
    app.invoke_welcome_quick_launch_connect_requested("asset-prod".into());
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "connection-progress");
}

#[test]
fn quick_launch_reveal_in_assets_selects_console_asset() {
    app.invoke_welcome_quick_launch_reveal_in_assets_requested("asset-prod".into());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke quick_launch -- --nocapture`

Expected: FAIL because no quick-launch callbacks are bound and no store is loaded or saved.

**Step 3: Write minimal implementation**

In `bootstrap.rs`:

- create/load `QuickLaunchPreferencesStore` beside `UiPreferencesStore`
- filter persisted ids against known saved SSH asset ids after asset catalog load
- sync welcome quick-launch arrays and selected detail into window properties
- bind the new welcome callbacks
- route connect actions through existing helpers:

```rust
let profile = runtime_profile_for_saved_asset(state, asset_id)?;
let handle = bridge.manager.open_session(profile, mode)?;
state.sync_workspace_tabs(bridge.manager.ordered_sessions());
state.record_recent_saved_ssh_asset(asset_id);
save_quick_launch_preferences_if_available(...);
```

Behavior rules:

- `Connect` -> `OpenSessionMode::ActivateExisting`
- `Connect in New Tab` -> `OpenSessionMode::ForceNewTab`
- `Reveal in Assets` -> select console destination + focus asset
- `toggle favorite` -> mutate local prefs only
- search change -> update welcome filter only

**Step 4: Run test to verify it passes**

Run: `cargo test --test bootstrap_smoke quick_launch -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/assets_explorer_smoke.rs
git commit -m "feat: wire quick launch dashboard actions"
```

### Task 5: Run Full Verification and Fix Regressions

**Files:**
- Modify as needed: `src/app/bootstrap.rs`
- Modify as needed: `src/shell/view_model.rs`
- Modify as needed: `ui/welcome/*.slint`
- Verify: `tests/quick_launch_preferences_spec.rs`
- Verify: `tests/quick_launch_projection_spec.rs`
- Verify: `tests/bootstrap_smoke.rs`
- Verify: `tests/quick_launch_ui_contract_smoke.sh`

**Step 1: Run targeted verification**

Run:

```bash
cargo test --test quick_launch_preferences_spec --test quick_launch_projection_spec --test shell_view_model -q
bash tests/quick_launch_ui_contract_smoke.sh
cargo test --test bootstrap_smoke quick_launch -- --nocapture
```

Expected: PASS

**Step 2: Run broader regression checks**

Run:

```bash
bash tests/shell_layout_ui_contract_smoke.sh
cargo test --test assets_explorer_smoke --test workspace_tabs_spec -q
cargo check --workspace
```

Expected: PASS

**Step 3: Fix any fallout minimally**

If regressions appear, restrict fixes to:

- welcome property plumbing
- quick-launch projection ordering
- bootstrap callback binding
- layout contract updates caused by the new welcome host surface

Do not expand scope into:

- SFTP
- proxy runtime
- snippet UI
- terminal renderer internals

**Step 4: Re-run verification**

Run the same commands from steps 1 and 2 until all pass.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs ui/welcome tests
git commit -m "test: verify ssh quick launch dashboard"
```
