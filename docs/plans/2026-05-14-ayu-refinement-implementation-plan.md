# Ayu Light/Dark Refinement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refine MicaTerm's existing Ayu Light and Ayu Dark implementation so the shell and terminal read as one Ayu product, with softer light-mode surfaces, subtler dark-mode separators, and list selection that uses subtle fill plus an accent rail instead of hard orange boxed borders.

**Architecture:** Keep `src/theme/spec.rs` as the only authored color source, keep `src/app/terminal_theme.rs` as the runtime projection layer, and keep `src/app/bootstrap.rs` plus `src/app/bootstrap/shell_chrome.rs` as the publishing path into `AppWindow`. Update Slint consumers to prefer runtime-projected shell/session properties, and only add a new semantic field if the existing `selected` / `border` / `focus` fields cannot express the row-state rewrite without regressions.

**Tech Stack:** Rust, Slint, existing MicaTerm theme projection code, Rust source-contract tests, bootstrap smoke tests.

---

### Task 1: Freeze the screenshot-informed Ayu targets in tests

**Files:**
- Modify: `tests/terminal_theme_selection_spec.rs`
- Modify: `tests/theme_terminal_redesign_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/theme_semantic_token_contract_spec.rs`
- Modify: `tests/ui_preferences.rs`
- Reference: `docs/plans/2026-05-14-ayu-refinement-design.md`

**Step 1: Write the failing terminal-palette assertions**

Add or update assertions for the Premium Default preset:

```rust
assert_eq!(light.background, 0xf8_f9fa);
assert_eq!(light.foreground, 0x5c_6166);
assert_eq!(light.cursor_bg, 0xff_aa33);
assert_eq!(light.cursor_fg, 0xf8_f9fa);
assert_eq!(light.scrollbar_track, (0xf4, 0xf6, 0xf8));
assert_eq!(light.scrollbar_thumb, (0xd6, 0xdc, 0xe3));
assert_eq!(light.scrollbar_thumb_active, (0xc6, 0xcd, 0xd6));
```

Also add or update dark assertions that prove the dark terminal family stays in
its current Ayu range rather than being redesigned.

**Step 2: Write the failing shell-neighborhood assertions**

In `tests/theme_terminal_redesign_spec.rs`, lock the refined shell ladder:

- light:
  - `app_background = 0xf8_f9fa`
  - `titlebar_background = 0xf8_f9fa`
  - `tabbar_background = 0xf8_f9fa`
  - `sidebar_background = 0xf8_f9fa`
  - `sidebar_panel_background = 0xf6_f8fa`
  - `right_panel_background = 0xf6_f8fa`
  - `terminal_frame_background = 0xfc_fcfc`
  - `border = 0xe5_e9ef`
- dark stays close to the current Ayu dark family

**Step 3: Add the row-state contract assertions**

Add source-contract assertions that fail until the selected-row rewrite exists:

- `ui/components/asset-node-row.slint` should not rely on a full selected border
- `ui/components/sidebar-nav-button.slint` should not rely on a full active box
- `ui/shell/right-panel.slint` selected rows should share the same subtle-fill
  and accent-rail direction

Use string assertions such as:

```rust
assert!(!asset_row.contains("border-width: root.focused || root.selected ? 1px : 0px;"));
```

and replace them with positive assertions for an accent rail rectangle and a
selected fill.

**Step 4: Run the focused tests to confirm they fail**

Run:

```bash
cargo test terminal_theme -- --nocapture
cargo test theme_terminal_redesign_spec -- --nocapture
cargo test bootstrap_smoke -- --nocapture
cargo test theme_semantic_token_contract_spec -- --nocapture
cargo test ui_preferences -- --nocapture
```

Expected: FAIL because the current values and row-state consumers still reflect
older shell-ladder and boxed-border behavior.

**Step 5: Commit**

```bash
git add tests/terminal_theme_selection_spec.rs tests/theme_terminal_redesign_spec.rs tests/bootstrap_smoke.rs tests/theme_semantic_token_contract_spec.rs tests/ui_preferences.rs
git commit -m "test: freeze ayu refinement targets"
```

### Task 2: Update the authored Ayu shell and terminal values in `src/theme/spec.rs`

**Files:**
- Modify: `src/theme/spec.rs`
- Test: `tests/terminal_theme_selection_spec.rs`
- Test: `tests/theme_terminal_redesign_spec.rs`

**Step 1: Update the light terminal values**

Change the Premium Default light terminal authored values to:

```rust
pub const TERMINAL_BG_BASE_LIGHT: u32 = 0xf8_f9fa;
pub const TERMINAL_BG_GRADIENT_TOP_LIGHT: u32 = 0xf8_f9fa;
pub const TERMINAL_BG_GRADIENT_BOTTOM_LIGHT: u32 = 0xf8_f9fa;
```

and update the light scrollbar values to the refined set from Task 1.

**Step 2: Update the light shell ladder**

In `premium_shell_light()`, set the authored values to the refined unified
surface ladder from the design doc.

At minimum, update:

```rust
app_background: 0xf8_f9fa,
titlebar_background: 0xf8_f9fa,
tabbar_background: 0xf8_f9fa,
sidebar_background: 0xf8_f9fa,
sidebar_panel_background: 0xf6_f8fa,
right_panel_background: 0xf6_f8fa,
terminal_frame_background: 0xfc_fcfc,
border: 0xe5_e9ef,
separator: 0xe5_e9ef,
```

and keep text/accent values in the Ayu light family.

**Step 3: Polish dark without redesigning it**

Keep dark terminal values stable. Only soften shell-neighborhood fields if the
new tests require it, and do not brighten the dark terminal foreground to pure
white.

**Step 4: Run the focused authored-value tests**

Run:

```bash
cargo test terminal_theme -- --nocapture
cargo test theme_terminal_redesign_spec -- --nocapture
```

Expected: PASS for tests that only depend on authored values.

**Step 5: Commit**

```bash
git add src/theme/spec.rs tests/terminal_theme_selection_spec.rs tests/theme_terminal_redesign_spec.rs
git commit -m "feat: refine authored ayu shell and terminal values"
```

### Task 3: Decide whether the existing row-state semantics are sufficient

**Files:**
- Reference: `src/theme/spec.rs`
- Reference: `src/app/terminal_theme.rs`
- Reference: `src/app/bootstrap.rs`
- Reference: `ui/app-window.slint`
- Reference: `ui/components/asset-node-row.slint`
- Reference: `ui/components/sidebar-nav-button.slint`
- Reference: `ui/shell/right-panel.slint`

**Step 1: Try the no-new-field implementation on paper**

Use the existing fields with this mapping:

- `sidebar_item_hover` => hover fill
- `sidebar_item_selected` => selected fill
- `sidebar_item_selected_border` => selected accent rail
- `focus_ring` => actual keyboard focus only

Do not add code yet. Confirm that all three target consumers can express:

- selected subtle fill
- selected accent rail
- keyboard focus without reintroducing a selected box

**Step 2: If the mapping is sufficient, record that no new field is needed**

Proceed directly to Task 4.

**Step 3: If the mapping is not sufficient, add one minimal semantic field**

Add only one new field to `ShellChromeTheme` and the projected runtime preset.
Suggested shape:

```rust
pub list_focus_outline: u32,
```

Then thread it through:

- `src/theme/spec.rs`
- `src/app/terminal_theme.rs`
- `src/app/bootstrap.rs`
- `ui/app-window.slint`

Do not add more fields unless a failing test proves the extra field is required.

**Step 4: Write or update the failing contract test for the chosen path**

If no new field is needed, add a test that proves the old boxed-border code is
removed.

If a new field is needed, add a source-contract test that proves the field is
projected end-to-end instead of hardcoded in Slint.

**Step 5: Run the focused contract test**

Run:

```bash
cargo test bootstrap_smoke -- --nocapture
cargo test theme_semantic_token_contract_spec -- --nocapture
```

Expected: FAIL until the chosen contract is implemented.

**Step 6: Commit**

```bash
git add src/theme/spec.rs src/app/terminal_theme.rs src/app/bootstrap.rs ui/app-window.slint tests/bootstrap_smoke.rs tests/theme_semantic_token_contract_spec.rs
git commit -m "refactor: clarify ayu row-state theme semantics"
```

### Task 4: Publish the active shell/session colors through the runtime projection chain

**Files:**
- Modify: `src/app/terminal_theme.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/ui_preferences.rs`

**Step 1: Update the projection struct if needed**

If Task 3 added a field, add it to `ProjectedThemePreset` in
`src/app/terminal_theme.rs` and populate it from `app_theme_spec()`.

If Task 3 did not add a field, leave the struct layout unchanged.

**Step 2: Publish the active runtime value from bootstrap**

Extend the shell/session publishing code only as needed to carry the refined
row-state or shell-neighborhood values into `AppWindow`.

**Step 3: Thread the property through Slint**

Add or update `AppWindow` properties so runtime consumers receive the projected
value instead of falling back to detached token usage.

**Step 4: Verify terminal-session consumers still prefer session-scoped runtime properties**

Make sure `ui/shell/workspace-pane.slint` and
`ui/shell/terminal-session-host.slint` keep using runtime session properties for
terminal frame, selection, and scrollbar values.

**Step 5: Run the focused runtime-projection tests**

Run:

```bash
cargo test bootstrap_smoke -- --nocapture
cargo test ui_preferences -- --nocapture
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src/app/terminal_theme.rs src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs ui/app-window.slint ui/shell/workspace-pane.slint ui/shell/terminal-session-host.slint tests/bootstrap_smoke.rs tests/ui_preferences.rs
git commit -m "feat: publish refined ayu runtime shell and session colors"
```

### Task 5: Rewrite asset tree and sidebar active states to subtle fill plus accent rail

**Files:**
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Test: `tests/theme_semantic_token_contract_spec.rs`

**Step 1: Remove the full selected border from asset rows**

In `ui/components/asset-node-row.slint`, replace the current selected/focused
full border logic with:

- selected fill from the projected shell selected color
- a leading 2px accent rail from the projected shell selected accent color
- keyboard focus from `focus_ring` only

**Step 2: Apply the same model to sidebar nav buttons**

In `ui/components/sidebar-nav-button.slint`, remove the active boxed border
look and replace it with the same selected fill plus accent treatment scaled to
that control.

**Step 3: Keep the runtime property plumbing clean**

Only pass properties through `ui/shell/assets-sidebar.slint` and
`ui/shell/sidebar.slint` that the components actually consume.

**Step 4: Run the focused source-contract tests**

Run:

```bash
cargo test theme_semantic_token_contract_spec -- --nocapture
```

Expected: PASS for the selected-row contract checks.

**Step 5: Commit**

```bash
git add ui/components/asset-node-row.slint ui/components/sidebar-nav-button.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint tests/theme_semantic_token_contract_spec.rs
git commit -m "style: refine ayu sidebar and asset row selection states"
```

### Task 6: Align the SFTP right-panel rows with the same selected-row language

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Test: `tests/theme_semantic_token_contract_spec.rs`

**Step 1: Reuse the same active-state semantics in the SFTP list**

Replace the current selected-row treatment so SFTP rows also use:

- subtle selected fill
- a leading accent rail or equivalent edge treatment
- no bright full selected outline

Do not introduce a right-panel-only Ayu selection color.

**Step 2: Keep separators subtle**

While editing the right panel, soften any overly visible row dividers or shell
splits only if they conflict with the refined Ayu ladder.

**Step 3: Run the focused source-contract tests**

Run:

```bash
cargo test theme_semantic_token_contract_spec -- --nocapture
```

Expected: PASS.

**Step 4: Commit**

```bash
git add ui/shell/right-panel.slint tests/theme_semantic_token_contract_spec.rs
git commit -m "style: align ayu sftp row selection treatment"
```

### Task 7: Refresh boot-time parity defaults in `ui/theme/tokens.slint`

**Files:**
- Modify: `ui/theme/tokens.slint`
- Test: `tests/terminal_theme_selection_spec.rs`
- Test: `tests/theme_semantic_token_contract_spec.rs`

**Step 1: Update boot-time parity defaults to match the refined authored values**

Update the Ayu light shell and terminal defaults in `ui/theme/tokens.slint` so
startup parity matches the refined runtime values.

**Step 2: Do not add a second live Ayu system**

Keep the file limited to boot-time defaults. Do not introduce runtime-only
properties or detached selection constants that bypass Rust projection.

**Step 3: Run the parity-focused tests**

Run:

```bash
cargo test terminal_theme -- --nocapture
cargo test theme_semantic_token_contract_spec -- --nocapture
```

Expected: PASS.

**Step 4: Commit**

```bash
git add ui/theme/tokens.slint tests/terminal_theme_selection_spec.rs tests/theme_semantic_token_contract_spec.rs
git commit -m "style: sync ayu boot-time token defaults"
```

### Task 8: Format, run the full requested verification suite, and record results

**Files:**
- Modify: any files touched by formatting or final small fixes
- Verify: `src/theme/spec.rs`
- Verify: `src/app/terminal_theme.rs`
- Verify: `src/app/bootstrap.rs`
- Verify: `src/app/bootstrap/shell_chrome.rs`
- Verify: `ui/theme/tokens.slint`
- Verify: `ui/app-window.slint`
- Verify: `ui/shell/workspace-pane.slint`
- Verify: `ui/shell/terminal-session-host.slint`
- Verify: `ui/components/asset-node-row.slint`
- Verify: `ui/components/sidebar-nav-button.slint`
- Verify: `ui/shell/assets-sidebar.slint`
- Verify: `ui/shell/sidebar.slint`
- Verify: `ui/shell/right-panel.slint`

**Step 1: Run formatting**

Run:

```bash
cargo fmt
```

Expected: PASS with no errors.

**Step 2: Run the required focused commands**

Run:

```bash
cargo test terminal_theme -- --nocapture
cargo test theme_terminal_redesign_spec -- --nocapture
cargo test ui_preferences -- --nocapture
cargo test bootstrap_smoke -- --nocapture
cargo test theme_semantic_token_contract_spec -- --nocapture
```

Expected: PASS.

**Step 3: Run the full test suite**

Run:

```bash
cargo test
```

Expected: PASS.

**Step 4: If any command fails, fix only the directly related issue and rerun**

Do not move on with an unverified claim.

**Step 5: Commit**

```bash
git add src/theme/spec.rs src/app/terminal_theme.rs src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs ui/theme/tokens.slint ui/app-window.slint ui/shell/workspace-pane.slint ui/shell/terminal-session-host.slint ui/components/asset-node-row.slint ui/components/sidebar-nav-button.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/shell/right-panel.slint tests/terminal_theme_selection_spec.rs tests/theme_terminal_redesign_spec.rs tests/bootstrap_smoke.rs tests/theme_semantic_token_contract_spec.rs tests/ui_preferences.rs
git commit -m "feat: refine ayu light and dark shell polish"
```
