# Redundant Code Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove source code that has no runtime call path and is only kept alive by tests.

**Architecture:** Keep runtime behavior unchanged by deleting spec-only modules and exports from `src/`, then remove the tests that existed solely to assert those fixtures. Leave startup, windowing, UI binding, and logging paths intact.

**Tech Stack:** Rust, Slint, cargo test

---

### Task 1: Remove dead spec modules and exports

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/shell/mod.rs`
- Modify: `src/theme/mod.rs`
- Modify: `src/theme/spec.rs`
- Modify: `src/shell/assets.rs`
- Modify: `src/shell/view_model.rs`
- Delete: `src/status/mod.rs`
- Delete: `src/status/spec.rs`
- Delete: `src/theme/accessibility.rs`
- Delete: `src/shell/signature.rs`

**Step 1: Delete source items with no runtime consumers**

Remove the `status`, `theme::accessibility`, and `shell::signature` modules, plus `ThemeSpec`, `theme_spec`, `AssetCreateAction`, `WelcomeAction`, and `welcome_actions`.

**Step 2: Rebuild imports and module exports**

Trim the remaining modules so only runtime-used items stay exported.

### Task 2: Remove tests that only asserted deleted fixtures

**Files:**
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Delete: `tests/status_motion.rs`
- Delete: `tests/accessibility_floor.rs`
- Delete: `tests/theme_spec.rs`
- Delete: `tests/signature_surfaces.rs`

**Step 1: Delete pure fixture/spec tests**

Remove integration tests whose only purpose was to exercise deleted dead code.

**Step 2: Keep runtime-facing state coverage**

Preserve the remaining `ShellViewModel` tests that still exercise live state transitions.

### Task 3: Verify the cleanup

**Files:**
- Test: workspace root

**Step 1: Run cargo checks**

Run: `cargo check --all-targets`

**Step 2: Run cargo tests**

Run: `cargo test --all-targets`
