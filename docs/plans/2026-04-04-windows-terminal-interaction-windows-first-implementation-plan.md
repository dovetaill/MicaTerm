# Windows-First Terminal Interaction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve Windows terminal feel in three phases: half-cell hit-testing with wide-character normalization, lower-latency repeated input projection, and a final Windows text polish pass.

**Architecture:** Keep the existing grid-first renderer intact and layer improvements around the interaction model and refresh timing. First unify pointer hit results, then add an input-active fast refresh path, and only then tune Windows text metrics/run splitting once interaction semantics are stable.

**Tech Stack:** Slint, Rust, native Windows terminal presenter, DirectWrite/Direct2D diagnostics, existing terminal runtime/session manager timers.

---

### Task 1: Half-cell hit-testing and wide-char trailing-cell normalization

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/app/ssh/runtime/contracts.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/terminal_glyph_cell_span_spec.rs`

**Step 1: Write the failing tests**

Add tests that prove:
- the host hit-test logic is no longer plain `floor(pointer-x / cell-width)`;
- hit-testing exposes half-cell semantics for column boundaries;
- wide-char trailing cells normalize back to the leading cell before selection state is consumed.

**Step 2: Run the targeted tests to verify they fail**

Run:
```bash
cargo test --test ssh_terminal_interaction_spec half_cell -- --nocapture
cargo test --test bootstrap_smoke wide_char -- --nocapture
```
Expected: failures showing the current host hit-test contract is still left-edge `floor()` based.

**Step 3: Implement the minimal hit-test model**

- add a dedicated local hit-test helper in Slint that computes row/col from cell-edge semantics;
- introduce a normalization hook for wide-character trailing cells;
- route selection anchor/focus and mouse events through the normalized result.

**Step 4: Run targeted tests to verify they pass**

Run:
```bash
cargo test --test ssh_terminal_interaction_spec -- --nocapture
cargo test --test bootstrap_smoke terminal -- --nocapture
```
Expected: selection/copy interaction tests remain green and new half-cell tests pass.

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint src/app/bootstrap.rs src/app/bootstrap/workspace_terminal.rs src/app/ssh/runtime/contracts.rs tests/ssh_terminal_interaction_spec.rs tests/bootstrap_smoke.rs tests/terminal_glyph_cell_span_spec.rs
git commit -m "feat: normalize terminal hit testing on the cell grid"
```

### Task 2: Input-active low-latency projection path

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/runtime/pump.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/terminal_runtime_perf_contract_spec.rs`
- Test: `tests/ssh_session_manager_spec.rs`

**Step 1: Write the failing tests**

Add tests that prove:
- active local input can request a faster refresh cadence than the idle `40ms/50ms` stack;
- repeated input refreshes are coalesced rather than forcing one heavy refresh per keystroke;
- the existing idle-output path still behaves conservatively.

**Step 2: Run the targeted tests to verify they fail**

Run:
```bash
cargo test --test terminal_runtime_perf_contract_spec -- --nocapture
cargo test --test ssh_session_manager_spec dirty -- --nocapture
```
Expected: failures because the current runtime still advertises only the conservative cadence.

**Step 3: Implement the fast path and instrumentation**

- add an input-active refresh mode/gate;
- shorten active dirty/projection latency while preserving coalescing;
- add timing/debug counters for writeback, dirty flush, projection, and present.

**Step 4: Run targeted tests to verify they pass**

Run:
```bash
cargo test --test terminal_runtime_perf_contract_spec -- --nocapture
cargo test --test ssh_session_manager_spec -- --nocapture
cargo test --test bootstrap_smoke terminal_input -- --nocapture
```
Expected: input latency contract tests pass and no regressions in input callback behavior.

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/runtime/pump.rs src/app/ssh/session_manager.rs src/app/bootstrap.rs src/app/bootstrap/workspace_terminal.rs tests/bootstrap_smoke.rs tests/terminal_runtime_perf_contract_spec.rs tests/ssh_session_manager_spec.rs
git commit -m "feat: reduce terminal input projection latency"
```

### Task 3: Windows text polish and run-boundary cleanup

**Files:**
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_layout/shaper.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/windows_frame.rs`
- Test: `tests/windows_terminal_typography_defaults_spec.rs`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`
- Test: `tests/terminal_glyph_origin_snap_spec.rs`
- Test: `tests/terminal_grid_first_renderer_spec.rs`

**Step 1: Write the failing tests**

Add tests that prove:
- Windows text diagnostics expose the baseline/antialias path needed for comparison;
- selection/cursor boundaries can split shaped runs without drifting off the cell grid;
- line-height/baseline defaults stay within the intended Windows-first tuning window.

**Step 2: Run the targeted tests to verify they fail**

Run:
```bash
cargo test --test windows_native_text_renderer_contract_spec -- --nocapture
cargo test --test windows_terminal_typography_defaults_spec -- --nocapture
```
Expected: failures showing diagnostics or run-boundary assumptions are not yet encoded.

**Step 3: Implement the minimal Windows polish**

- tune baseline/line-height where needed for Windows dense-text readability;
- split selection/cursor shaping runs at cell boundaries in the Windows path;
- improve diagnostics output for text renderer path and antialias mode.

**Step 4: Run targeted tests and packaging verification**

Run:
```bash
cargo test --test windows_native_text_renderer_contract_spec -- --nocapture
cargo test --test windows_terminal_typography_defaults_spec -- --nocapture
cargo check --features terminal-native-renderer
./build-win-x64.sh
```
Expected: tests pass and packaged Windows build completes with the expected Skia/native profile.

**Step 5: Commit**

```bash
git add src/app/terminal_presenter.rs src/app/terminal_layout/shaper.rs src/app/terminal_renderer/platform/windows.rs src/app/windows_frame.rs tests/windows_terminal_typography_defaults_spec.rs tests/windows_native_text_renderer_contract_spec.rs tests/terminal_glyph_origin_snap_spec.rs tests/terminal_grid_first_renderer_spec.rs
git commit -m "feat: polish windows terminal text rendering"
```

### Task 4: Final verification and packaged Windows review

**Files:**
- Modify: `docs/plans/2026-04-04-windows-terminal-interaction-windows-first-design.md`
- Modify: `docs/plans/2026-04-04-windows-terminal-interaction-windows-first-implementation-plan.md`

**Step 1: Run the end-to-end verification set**

Run:
```bash
cargo test --test ssh_terminal_interaction_spec -- --nocapture
cargo test --test bootstrap_smoke terminal -- --nocapture
cargo test --test terminal_runtime_perf_contract_spec -- --nocapture
cargo test --test windows_native_text_renderer_contract_spec -- --nocapture
cargo check --features terminal-native-renderer
./build-win-x64.sh
```
Expected: all targeted tests pass and the packaged Windows zip is produced.

**Step 2: Summarize measured results**

Document the final chosen timings, diagnostics fields, and any remaining follow-up items in the two plan docs.

**Step 3: Commit**

```bash
git add docs/plans/2026-04-04-windows-terminal-interaction-windows-first-design.md docs/plans/2026-04-04-windows-terminal-interaction-windows-first-implementation-plan.md
git commit -m "docs: record windows-first terminal interaction rollout"
```
