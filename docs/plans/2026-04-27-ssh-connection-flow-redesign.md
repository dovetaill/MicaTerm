# SSH Connection Flow Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the workspace SSH `connection-progress` page into a mature, task-focused flow with a compact workflow rail, a single current-task panel, and progressive diagnostics disclosure.

**Architecture:** Keep the existing SSH runtime and `ConnectionAttemptState` as the source of truth, but add a richer presentation layer in `src/app/bootstrap.rs` so Slint receives page-oriented semantics instead of only flattened rows. Then replace the current stacked-card `connection-progress` layout in `ui/shell/terminal-session-host.slint` with one stable skeleton that supports progressing, decision, and troubleshooting modes.

**Tech Stack:** Rust, Slint, existing SSH runtime/session manager pipeline, shell theme tokens, cargo UI contract tests, existing bootstrap smoke tests

---

### Task 1: Lock The New Copy And Structure Contract In Tests

**Files:**
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`
- Reference: `ui/shell/terminal-session-host.slint`
- Reference: `ui/shell/workspace-pane.slint`
- Reference: `ui/app-window.slint`

**Step 1: Write the failing test**

Replace the old string-contract expectations that hard-code the stacked-card UX.

Add / update assertions so the tests expect:

- `Trust key` instead of `Trust and Continue`
- `Copy details` instead of `Copy Diagnostics`
- a `Diagnostics` disclosure label instead of `Show Diagnostics`
- continued support for inline `trust-host-key` and `reject-host-key` callback routing
- continued rendering of the `connection-progress` branch

Suggested assertions:

```rust
assert!(terminal_host.contains("Trust key"));
assert!(terminal_host.contains("Copy details"));
assert!(terminal_host.contains("Diagnostics"));
assert!(!terminal_host.contains("Trust and Continue"));
assert!(!terminal_host.contains("Copy Diagnostics"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract -- --nocapture`

Expected: FAIL because the current Slint file still contains the old button labels and stacked-card layout text.

**Step 3: Write minimal implementation**

Update only the test files so they clearly describe the new UX contract. Do not change production code yet.

**Step 4: Run test to verify it fails for the right reason**

Run: `cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract -- --nocapture`

Expected: FAIL only because the UI still exposes the old labels and structure.

**Step 5: Commit**

```bash
git add tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "test: update ssh connection flow ui contract"
```

### Task 2: Add Presentation Semantics For The Redesigned Page

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Reference: `src/app/ssh/connection_progress.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing test**

Add tests that lock the new derived page semantics the UI will need, such as:

- a page mode like `progressing`, `decision`, or `troubleshooting`
- a current-task title string
- a current-task body / summary string
- a diagnostics summary string or visibility affordance

Suggested shape:

```rust
assert_eq!(app.get_workspace_session_connection_page_mode().as_str(), "decision");
assert_eq!(app.get_workspace_session_connection_task_title().as_str(), "Verify host key");
```

Expected app-window properties to add first:

```slint
in-out property <string> workspace-session-connection-page-mode: "";
in-out property <string> workspace-session-connection-task-title: "";
in-out property <string> workspace-session-connection-task-detail: "";
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke workspace_host_key -- --nocapture`

Expected: FAIL because these derived presentation properties do not exist yet.

**Step 3: Write minimal implementation**

In `src/app/bootstrap.rs`:

- derive `page_mode` from the active attempt state:
  - `waiting-user` + prompt => `decision`
  - `error` / failed active step => `troubleshooting`
  - otherwise => `progressing`
- derive `task_title` from the active step or prompt
- derive `task_detail` from the current step detail or failure summary

In `ui/app-window.slint` and `ui/shell/workspace-pane.slint`:

- add and forward the new page-oriented properties into `TerminalSessionHost`

**Step 4: Run test to verify it passes**

Run: `cargo test --test bootstrap_smoke workspace_host_key -- --nocapture`

Expected: PASS, with the host-key flow exposing the new decision-mode semantics.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs ui/app-window.slint ui/shell/workspace-pane.slint tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "feat: derive ssh connection page presentation state"
```

### Task 3: Replace The Stacked Cards With A Single-Skeleton Connection Sheet

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Reference: `ui/theme/tokens.slint`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing test**

Add or tighten contract checks that prove the old layout is gone and the new skeleton exists.

Look for signals such as:

- one summary/header region
- one current-task panel
- diagnostics disclosure text
- no `Trust and Continue`
- no `Show Diagnostics`
- no `Copy Diagnostics`

Suggested shell assertions:

```bash
! grep -F 'Trust and Continue' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Show Diagnostics' "$WORKSPACE_HOST" >/dev/null
grep -F 'Trust key' "$WORKSPACE_HOST" >/dev/null
grep -F 'Diagnostics' "$WORKSPACE_HOST" >/dev/null
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract_exposes_inline_host_key_actions -- --nocapture`

Expected: FAIL because `TerminalSessionHost` still renders the old stacked cards and labels.

**Step 3: Write minimal implementation**

In `ui/shell/terminal-session-host.slint`:

- keep the `if root.mode == "connection-progress"` branch
- replace the current sequence of:
  - `header-card`
  - `timeline-card`
  - `current-detail-card`
  - `host-key-card`
  - `diagnostics-card`
  - `footer-row`
- with a stable skeleton containing:
  - summary header
  - compact workflow rail
  - unified current-task panel
  - diagnostics disclosure section
  - reduced page-level action bar

Do not over-abstract yet; keep new helper components local to this file unless duplication forces extraction.

**Step 4: Run test to verify it passes**

Run: `cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: rebuild ssh connection progress surface"
```

### Task 4: Turn The Step List Into A Quiet Workflow Rail

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/theme/tokens.slint`
- Test: `tests/workspace_tabs_spec.rs`
- Optional Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Add source-level checks that lock the visual intent:

- the step list still iterates over `connection-progress-steps`
- completed steps are no longer painted with `ThemeTokens.status-success-surface`
- diagnostics and long action labels are no longer used as timeline-row UI

Suggested assertions:

```rust
assert!(!terminal_host.contains("ThemeTokens.status-success-surface"));
assert!(terminal_host.contains("for step in root.connection-progress-steps"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract -- --nocapture`

Expected: FAIL because completed steps still use green success surfaces in the current implementation.

**Step 3: Write minimal implementation**

Rework the rail row visuals in `ui/shell/terminal-session-host.slint` so that:

- `done` rows use quiet text + small status glyph / dot
- `running` and `blocked` rows receive subtle emphasis
- `failed` rows get local accent, not a full-page error treatment
- rows become compact, not 56px full cards

If needed, add small dedicated tokens in `ui/theme/tokens.slint`, for example:

```slint
out property <brush> connection-step-active-surface: dark-mode ? #202b37 : #edf3fb;
out property <brush> connection-step-blocked-surface: dark-mode ? #222c38 : #f3f7fb;
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint ui/theme/tokens.slint tests/workspace_tabs_spec.rs
git commit -m "style: quiet completed ssh workflow steps"
```

### Task 5: Merge Current Detail And Host-Key Prompt Into A Unified Task Panel

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing test**

Add tests that lock the behavioral expectations of the unified task panel:

- host-key prompts still route through `trust-host-key` and `reject-host-key`
- the page mode becomes `decision` while waiting for host-key approval
- the primary host-key button label is `Trust key`
- the page-level footer no longer duplicates task-level host-key actions

Suggested assertions:

```rust
assert_eq!(app.get_workspace_session_connection_page_mode().as_str(), "decision");
assert!(terminal_host.contains("Trust key"));
assert!(!terminal_host.contains("Trust and Continue"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke workspace_host_key -- --nocapture`

Expected: FAIL because the current layout still treats current detail and host-key prompt as separate peer cards.

**Step 3: Write minimal implementation**

In `ui/shell/terminal-session-host.slint`:

- remove the separate `current-detail-card`
- remove the separate `host-key-card`
- render one task panel that switches template based on page mode and prompt presence
- keep `trust-host-key` and `reject-host-key` callback actions unchanged

In `src/app/bootstrap.rs`:

- make sure the derived task title / detail stay meaningful for running, decision, and failure cases

**Step 4: Run test to verify it passes**

Run: `cargo test --test bootstrap_smoke workspace_host_key -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "feat: unify ssh connection task panel"
```

### Task 6: Move Diagnostics Into Progressive Disclosure And Rebalance Actions

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Add tests that lock the new action hierarchy:

- page-level actions are limited to flow actions such as `Cancel`, `Retry`, and `Edit settings`
- diagnostics show up under a `Diagnostics` disclosure instead of two equal-weight footer buttons
- the copy utility is named `Copy details`

Suggested assertions:

```rust
assert!(terminal_host.contains("Diagnostics"));
assert!(terminal_host.contains("Copy details"));
assert!(terminal_host.contains("Edit settings"));
assert!(!terminal_host.contains("Copy Diagnostics"));
assert!(!terminal_host.contains("Edit Connection"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract -- --nocapture`

Expected: FAIL because the current footer still exposes `Show Diagnostics`, `Copy Diagnostics`, and `Edit Connection`.

**Step 3: Write minimal implementation**

In `ui/shell/terminal-session-host.slint`:

- collapse diagnostics into a disclosure section labeled `Diagnostics`
- rename the utility action to `Copy details`
- rename the recovery action to `Edit settings`
- keep `Cancel` and `Retry` as page-level flow actions
- keep diagnostics utilities visually secondary to flow actions

In `src/app/bootstrap.rs`:

- if needed, expose a diagnostics summary string or visibility state that lets the disclosure show a helpful collapsed summary

**Step 4: Run test to verify it passes**

Run: `cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint src/app/bootstrap.rs tests/workspace_tabs_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: rebalance ssh diagnostics and flow actions"
```

### Task 7: Verify Failure-Mode Continuity And Final Regressions

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Reference: `ui/shell/terminal-session-host.slint`
- Reference: `src/app/bootstrap.rs`

**Step 1: Write the failing test**

Add final coverage that proves:

- host-key decision mode still keeps the native terminal rect collapsed until the real terminal resumes
- rejecting the host key keeps the same tab on the connection sheet
- failure keeps the same page skeleton instead of dropping into a generic error surface
- retry-capable failures still expose `Retry`

Suggested checks can build on the existing host-key geometry tests in `tests/bootstrap_smoke.rs`.

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke workspace_host_key -- --nocapture`

Expected: FAIL if any of the connection-sheet continuity assumptions were broken while restructuring the UI.

**Step 3: Write minimal implementation**

Fix only the continuity issues revealed by the tests. Avoid introducing new layout patterns or unrelated refactors.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test workspace_tabs_spec --test bootstrap_smoke --test ssh_connection_timeline_spec -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
git add tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs ui/shell/terminal-session-host.slint src/app/bootstrap.rs
git commit -m "test: lock redesigned ssh connection flow regressions"
```

### Task 8: Run End-To-End Verification For The Redesign

**Files:**
- Reference: `ui/shell/terminal-session-host.slint`
- Reference: `src/app/bootstrap.rs`
- Reference: `ui/theme/tokens.slint`
- Reference: `tests/workspace_tabs_spec.rs`
- Reference: `tests/bootstrap_smoke.rs`
- Reference: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Run focused UI/source contract checks**

Run:

```bash
cargo test --test workspace_tabs_spec connection_progress_workspace_host_contract -- --nocapture
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 2: Run focused bootstrap / host-key flow checks**

Run:

```bash
cargo test --test bootstrap_smoke workspace_host_key -- --nocapture
```

Expected: PASS

**Step 3: Run broader SSH timeline regression coverage**

Run:

```bash
cargo test --test ssh_connection_timeline_spec -- --nocapture
```

Expected: PASS

**Step 4: Sanity-check copy and structure**

Run:

```bash
rg -n "Trust and Continue|Show Diagnostics|Copy Diagnostics|Edit Connection" ui/shell/terminal-session-host.slint tests
```

Expected: no matches in the redesigned connection-progress surface or its updated contracts.

**Step 5: Commit**

```bash
git add docs/plans/2026-04-27-ssh-connection-flow-redesign-design.md docs/plans/2026-04-27-ssh-connection-flow-redesign.md
git commit -m "docs: capture ssh connection flow redesign plan"
```
