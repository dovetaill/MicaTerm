# Terminal Core Seam Slim And Bootstrap Overflow Debug Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the now-pointless single-branch terminal core selection seam and then investigate the bootstrap stack overflow with evidence-first debugging.

**Architecture:** Keep the object-safe `TerminalCoreAdapter` boundary, but collapse all runtime construction paths to the shipped WezTerm implementation so no fake core-selection API remains. Treat the bootstrap stack overflow as a separate debugging track: first lock a minimal failing reproduction, then instrument and trace the `surface clear -> presenter host release` path to identify the recursive cycle before attempting any fix.

**Tech Stack:** Rust, Cargo, source-contract tests, unit tests, bootstrap/session runtime code, targeted `cargo test` runs.

---

### Task 1: Lock the slimmer single-core seam with failing tests

**Files:**
- Modify: `tests/terminal_core_adapter_spec.rs`
- Modify: `tests/terminal_session_spec.rs`

**Step 1: Write the failing tests**
- Add a source-contract assertion that `src/app/terminal_core/types.rs` no longer defines `TerminalCoreKind`.
- Add a source-contract assertion that `src/app/ssh/runtime/terminal.rs` no longer contains `new_with_core_kind` or `new_with_core_kind_and_scrollback`.
- Update `tests/terminal_session_spec.rs` to express the scrollback selection contract directly against `TerminalSession::new_with_scrollback(...)` or `TerminalSession::new(...)`, so the old kind-based helper is no longer needed.

**Step 2: Run tests to verify they fail**
Run: `cargo test --test terminal_core_adapter_spec --test terminal_session_spec -- --nocapture`
Expected: FAIL because `TerminalCoreKind` and the kind-based constructors still exist.

### Task 2: Remove the dead single-branch core-selection API

**Files:**
- Modify: `src/app/terminal_core/mod.rs`
- Modify: `src/app/terminal_core/types.rs`
- Modify: `src/app/ssh/runtime/terminal.rs`
- Modify: `tests/terminal_session_spec.rs`

**Step 1: Remove the unused selector type**
- Delete `TerminalCoreKind` from `src/app/terminal_core/types.rs`.
- Stop re-exporting it from `src/app/terminal_core/mod.rs`.

**Step 2: Collapse construction to the shipped default path**
- Change `create_terminal_core_adapter(...)` to take only `rows`, `cols`, and `scrollback_lines`.
- Remove `new_with_core_kind(...)` and `new_with_core_kind_and_scrollback(...)` from `src/app/ssh/runtime/terminal.rs`.
- Update `TerminalSession::new_with_scrollback(...)` to call the simplified constructor directly.
- Update tests to use the simplified runtime API only.

### Task 3: Verify the slimmer seam

**Files:**
- Verify only: `src/app/terminal_core/mod.rs`
- Verify only: `src/app/terminal_core/types.rs`
- Verify only: `src/app/ssh/runtime/terminal.rs`
- Verify only: `tests/terminal_core_adapter_spec.rs`
- Verify only: `tests/terminal_session_spec.rs`

**Step 1: Run focused verification**
Run: `cargo test --test terminal_core_adapter_spec --test terminal_session_spec -- --nocapture`
Expected: PASS.

### Task 4: Reproduce the bootstrap stack overflow with explicit evidence

**Files:**
- Verify only: `src/app/bootstrap.rs`
- Optionally modify for diagnostics: `src/app/bootstrap.rs`
- Optionally add temporary assertions/logging in local branch only if needed

**Step 1: Confirm the minimal failing reproduction**
Run: `cargo test --lib workspace_session_state_clears_native_terminal_frame_when_surface_clears -- --nocapture`
Expected: stack overflow abort.

**Step 2: Trace the suspected recursive path**
- Inspect `with_bitmap_workspace_presenter_for_test(...)`, `sync_workspace_session_state(...)`, and the surface-clear cleanup path around `WORKSPACE_TERMINAL_RENDERER_HOST` release.
- If the call graph is not obvious, add minimal temporary tracing/assertion instrumentation around:
  - surface state clearing,
  - presenter host release,
  - visible line model publication,
  - any callback or reentrant sync path.

**Step 3: Form a single root-cause hypothesis**
- Write down the specific recursion or reentry edge that explains the overflow.
- Do not implement a fix until the evidence supports that hypothesis.

### Task 5: If the root cause is confirmed, add a failing regression test before fixing

**Files:**
- Modify: `src/app/bootstrap.rs` tests, or a dedicated bootstrap test file if the current one is too coupled

**Step 1: Encode the smallest non-overflowing behavioral contract**
- Add a failing regression test that captures the intended post-clear behavior without triggering an aborting stack overflow in an uncontrolled way, if feasible.
- If a standard failing test cannot be expressed safely because the current failure aborts the process, document that limitation and keep the reproduction command as the evidence gate.

**Step 2: Only then implement the smallest root-cause fix**
- Change one thing.
- Re-run the targeted reproduction.

### Task 6: Final verification and commit boundaries

**Files:**
- Verify only: all files touched in Tasks 1-5

**Step 1: Verify seam-slimming separately**
Run: `cargo test --test terminal_core_adapter_spec --test terminal_session_spec -- --nocapture`
Expected: PASS.

**Step 2: Verify the bootstrap reproduction status**
Run one of:
- `cargo test --lib workspace_session_state_clears_native_terminal_frame_when_surface_clears -- --nocapture`
- or the new targeted regression command if a safe regression test was added.

**Step 3: Keep commit boundaries clean**
- Commit the seam slimming separately from any eventual stack overflow fix.
