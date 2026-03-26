# SSH Tabbar VS Code Bugfix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make SSH workspace tabs render as compact VS Code-like tabs instead of stretching across the terminal surface, while preserving a real SSH disconnect on close.

**Architecture:** Keep the Rust-side session lifecycle intact and confine the bugfix to Slint tab chrome. Tighten UI contract tests first, then rebuild `ActiveTab` and `TabBar` so tabs size from content, expose a real Fluent close icon button, and keep selection and close hit targets independent.

**Tech Stack:** Rust, Slint, shell smoke tests, Cargo integration tests

---

### Task 1: Lock the broken tab chrome with tests

**Files:**
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing test**

- Require `ActiveTab` to use the Fluent dismiss icon instead of a text `×`.
- Require the tab row to give each tab `horizontal-stretch: 0`.
- Require a trailing spacer so empty width stays to the right of the tabs.

**Step 2: Run test to verify it fails**

Run: `cargo test --test workspace_tabs_spec -- --nocapture`
Expected: FAIL on the new tab chrome assertions.

Run: `bash tests/ssh_connect_tabs_ui_contract_smoke.sh`
Expected: FAIL because the current Slint files still use the simplified chip implementation.

**Step 3: Commit**

```bash
git add tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "test: lock ssh tabbar chrome contract"
```

### Task 2: Rebuild the SSH tab chrome

**Files:**
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/shell/tabbar.slint`

**Step 1: Write minimal implementation**

- Add a private Fluent dismiss icon image to `ActiveTab`.
- Replace the text close affordance with a proper icon button shell.
- Keep title-only chrome with ellipsis and independent content/close touch areas.
- Make the tab row left-aligned, disable per-tab stretch, and add a trailing spacer that absorbs remaining width.
- Tune active, hover, and pressed surfaces toward a VS Code-like tab strip.

**Step 2: Run test to verify it passes**

Run: `cargo test --test workspace_tabs_spec -- --nocapture`
Expected: PASS

Run: `bash tests/ssh_connect_tabs_ui_contract_smoke.sh`
Expected: PASS

**Step 3: Commit**

```bash
git add ui/components/active-tab.slint ui/shell/tabbar.slint
git commit -m "fix: restyle ssh workspace tabs"
```

### Task 3: Verify close still tears down the SSH runtime

**Files:**
- Reference: `src/app/bootstrap.rs`
- Reference: `src/app/ssh/session_manager.rs`
- Reference: `src/app/ssh/runtime.rs`

**Step 1: Verify the close path**

- Confirm the UI still emits `workspace-tab-close-requested`.
- Confirm Rust still calls `SessionManager::close_session`.
- Confirm runtime teardown still sends `Disconnect::ByApplication`.

**Step 2: Run focused verification**

Run: `cargo test --test workspace_tabs_spec close_affordance_is_modeled_separately_from_select_action -- --nocapture`
Expected: PASS

**Step 3: Commit**

```bash
git add docs/plans/2026-03-25-ssh-tabbar-vscode-bugfix-implementation-plan.md
git commit -m "docs: record ssh tabbar bugfix plan"
```
