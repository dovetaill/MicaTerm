# Remove Alacritty Terminal Core Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the experimental Alacritty terminal-core path so the repository has a single WezTerm-backed terminal core.

**Architecture:** Keep the existing `TerminalCoreAdapter` seam, but collapse it to the shipped WezTerm implementation only. Remove the experimental adapter, enum branch, constructor helper, dependency wiring, README status text, and tests that only existed to exercise Alacritty parity.

**Tech Stack:** Rust, Cargo, source-contract tests, runtime session tests, README smoke tests.

---

### Task 1: Lock the removal contract with failing tests

**Files:**
- Modify: `tests/terminal_core_adapter_spec.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`

**Step 1: Write the failing tests**
- Add a source-contract assertion in `tests/terminal_core_adapter_spec.rs` that `src/app/terminal_core/mod.rs`, `src/app/terminal_core/types.rs`, `src/app/ssh/runtime/terminal.rs`, and `Cargo.toml` no longer reference `alacritty` or `AlacrittyExperimental`.
- Update `tests/bootstrap_profile_smoke.rs` so README honesty checks expect only the WezTerm-backed core and Rio reference, and explicitly reject the old Alacritty experimental wording.

**Step 2: Run tests to verify they fail**
Run: `cargo test --test terminal_core_adapter_spec --test bootstrap_profile_smoke -- --nocapture`
Expected: FAIL because the repository still exposes the Alacritty adapter and README still documents it.

### Task 2: Remove the experimental Alacritty runtime path

**Files:**
- Modify: `Cargo.toml`
- Delete: `src/app/terminal_core/alacritty_adapter.rs`
- Modify: `src/app/terminal_core/mod.rs`
- Modify: `src/app/terminal_core/types.rs`
- Modify: `src/app/ssh/runtime/terminal.rs`

**Step 1: Remove dependency and feature wiring**
- Delete `terminal-core-alacritty-experimental` from `Cargo.toml`.
- Delete the `alacritty_terminal` dependency entry.

**Step 2: Collapse the terminal core seam to WezTerm only**
- Remove `pub mod alacritty_adapter;` and the `pub use` export from `src/app/terminal_core/mod.rs`.
- Remove the `AlacrittyExperimental` enum variant from `src/app/terminal_core/types.rs`.
- Simplify `create_terminal_core_adapter(...)` to always construct `WeztermTerminalCoreAdapter`.
- Remove `new_with_experimental_alacritty_core(...)` from `src/app/ssh/runtime/terminal.rs`.
- Delete `src/app/terminal_core/alacritty_adapter.rs`.

### Task 3: Remove Alacritty-only tests and README status text

**Files:**
- Modify: `readme.md`
- Delete: `tests/terminal_core_parity_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`

**Step 1: Update README status copy**
- Remove the bullet that describes the Alacritty adapter as experimental.
- Keep the WezTerm-backed core status and Rio reference intact.

**Step 2: Remove tests whose purpose was Alacritty parity**
- Delete the full parity suite in `tests/terminal_core_parity_spec.rs`.
- Delete the Alacritty-specific scrollback, selection, and writeback comparison tests from the remaining suites.
- Keep the WezTerm behavior tests intact.

### Task 4: Verify the repository after removal

**Files:**
- Verify only: `Cargo.toml`
- Verify only: `src/app/terminal_core/mod.rs`
- Verify only: `src/app/terminal_core/types.rs`
- Verify only: `src/app/ssh/runtime/terminal.rs`
- Verify only: `readme.md`
- Verify only: updated tests

**Step 1: Run focused verification**
Run: `cargo test --test terminal_core_adapter_spec --test bootstrap_profile_smoke --test terminal_session_spec --test terminal_scrollback_spec --test ssh_terminal_interaction_spec -- --nocapture`
Expected: PASS.

**Step 2: Run broader compile/test verification**
Run: `cargo test --tests -- --nocapture`
Expected: PASS, or if unrelated failures exist, capture them explicitly before claiming completion.
