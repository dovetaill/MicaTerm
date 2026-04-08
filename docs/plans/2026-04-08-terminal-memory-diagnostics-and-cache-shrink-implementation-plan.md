# Terminal Memory Diagnostics and Cache Shrink Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add opt-in Windows terminal memory diagnostics and shrink retained terminal caches on safe lifecycle boundaries so startup, large-output trim, and post-close memory behavior become explainable and improve measurably.

**Architecture:** Extend the terminal presenter/renderer stack with lightweight cache statistics and cache-clear hooks, gate structured runtime diagnostics behind `MICA_TERM_MEMORY_DIAGNOSTICS=1`, then wire cache shrink into close/no-surface idle paths without changing the existing large-output working-set trim semantics.

**Tech Stack:** Rust, Slint, Windows packaged runtime profile, terminal presenter/renderer stack, structured tracing logs, cargo test

---

### Task 1: Lock the diagnostics toggle contract in tests

**Files:**
- Create: `tests/terminal_memory_diagnostics_contract_spec.rs`
- Modify: `src/app/logging/config.rs`
- Reference: `readme.md`

**Step 1: Write the failing test**

Add a source/contract test asserting that:

- `src/app/logging/config.rs` recognizes `MICA_TERM_MEMORY_DIAGNOSTICS`
- diagnostics stay opt-in instead of always-on
- the repo docs show the reproduction flow with both `MICA_TERM_LOG=debug` and `MICA_TERM_MEMORY_DIAGNOSTICS=1`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test terminal_memory_diagnostics_contract_spec -q
```

Expected: FAIL because the diagnostics env toggle and docs contract do not exist yet.

**Step 3: Write minimal implementation**

Implement:

- a small config helper in `src/app/logging/config.rs` or nearby logging/runtime code that reports whether memory diagnostics are enabled
- doc updates in `readme.md` with the Windows reproduction steps

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test terminal_memory_diagnostics_contract_spec -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add tests/terminal_memory_diagnostics_contract_spec.rs src/app/logging/config.rs readme.md
git commit -m "test: lock terminal memory diagnostics toggle contract"
```

### Task 2: Add cache stats and cache clear hooks with unit tests

**Files:**
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_scene_image.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `tests/terminal_runtime_perf_contract_spec.rs`
- Test: `src/app/terminal_presenter.rs`
- Test: `src/app/terminal_scene_image.rs`
- Test: `tests/terminal_renderer_prepare_cache_spec.rs`

**Step 1: Write the failing tests**

Add tests/source-contract assertions for:

- presenter cache stats being exposed
- scene-image cache stats being exposed
- renderer prepared-row/glyph cache stats being exposed
- explicit cache clear/shrink hooks existing instead of requiring object re-creation

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_runtime_perf_contract_spec --test terminal_renderer_prepare_cache_spec -q
cargo test --lib terminal_presenter -- --nocapture
```

Expected: FAIL because the cache stats/clear hooks are missing.

**Step 3: Write minimal implementation**

Implement:

- presenter-facing diagnostics structs or methods returning bounded cache stats
- `clear_transient_caches()` / similarly named hooks on:
  - presenter
  - scene-image renderer
  - WGPU terminal renderer
- keep the methods cheap and deterministic

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_runtime_perf_contract_spec --test terminal_renderer_prepare_cache_spec -q
cargo test --lib app::terminal_presenter::tests -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_presenter.rs src/app/terminal_scene_image.rs src/app/terminal_renderer/wgpu_renderer.rs tests/terminal_runtime_perf_contract_spec.rs tests/terminal_renderer_prepare_cache_spec.rs
git commit -m "feat: expose terminal cache diagnostics and clear hooks"
```

### Task 3: Emit opt-in runtime memory diagnostics

**Files:**
- Modify: `src/app/memory.rs`
- Modify: `src/app/ssh/runtime/pump.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/logging/runtime.rs`
- Modify: `tests/logging_runtime.rs`
- Test: `tests/terminal_memory_diagnostics_contract_spec.rs`

**Step 1: Write the failing test**

Add a contract/logging test asserting that when diagnostics are enabled:

- trim requests/executions can be logged
- startup or surface refresh memory snapshots can be logged
- default mode stays quiet when diagnostics are disabled

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test terminal_memory_diagnostics_contract_spec --test logging_runtime -q
```

Expected: FAIL because the runtime diagnostics events do not exist yet.

**Step 3: Write minimal implementation**

Implement structured diagnostics events for:

- startup snapshot
- surface refresh
- scroll snapshot
- trim request
- trim execution

Use existing tracing infrastructure and keep output under a dedicated memory-oriented target.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test terminal_memory_diagnostics_contract_spec --test logging_runtime -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/memory.rs src/app/ssh/runtime/pump.rs src/app/bootstrap.rs src/app/logging/runtime.rs tests/logging_runtime.rs tests/terminal_memory_diagnostics_contract_spec.rs
git commit -m "feat: add opt-in terminal memory diagnostics"
```

### Task 4: Shrink caches when the active terminal surface disappears

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_renderer/host.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Test: `tests/terminal_memory_diagnostics_contract_spec.rs`

**Step 1: Write the failing test**

Add a contract/behavior test asserting that:

- when no active workspace terminal surface remains, terminal renderer host caches are explicitly cleared or shrunk
- closing a workspace tab/session reaches that shrink path

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test bootstrap_smoke --test ssh_session_manager_spec --test terminal_memory_diagnostics_contract_spec -q
```

Expected: FAIL because close/no-surface currently clears UI/native-frame state but not presenter caches.

**Step 3: Write minimal implementation**

Implement:

- a renderer-host level `clear_transient_caches()` entry point
- bootstrap wiring that calls it when the active workspace surface becomes `None`
- diagnostics around before/after cache stats when enabled

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test bootstrap_smoke --test ssh_session_manager_spec --test terminal_memory_diagnostics_contract_spec -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/terminal_renderer/host.rs tests/bootstrap_smoke.rs tests/ssh_session_manager_spec.rs tests/terminal_memory_diagnostics_contract_spec.rs
git commit -m "fix: shrink terminal caches when workspace surface clears"
```

### Task 5: Add idle shrink only when no active surface exists

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Test: `tests/terminal_memory_diagnostics_contract_spec.rs`

**Step 1: Write the failing test**

Add a contract test asserting that:

- idle shrink is scheduled only when there is no active workspace terminal surface
- active typing/scrolling paths do not immediately clear caches

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_memory_diagnostics_contract_spec -q
```

Expected: FAIL because no idle-shrink scheduling exists yet.

**Step 3: Write minimal implementation**

Implement:

- a small idle timer/gate in bootstrap
- scheduling only after the workspace surface becomes absent
- a cache clear/shrink call plus diagnostics log on timer fire

Keep the delay conservative and avoid touching the existing large-output working-set trim path.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_memory_diagnostics_contract_spec -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/bootstrap/workspace_terminal.rs tests/bootstrap_smoke.rs tests/terminal_memory_diagnostics_contract_spec.rs
git commit -m "feat: idle-shrink terminal caches without active surfaces"
```

### Task 6: Verify the full memory-diagnostics path end to end

**Files:**
- Reference: `readme.md`
- Reference: `build-win-x64.sh`
- Reference: `src/app/memory.rs`
- Reference: `src/app/bootstrap.rs`

**Step 1: Run focused Rust verification**

Run:

```bash
cargo test --test terminal_memory_diagnostics_contract_spec --test terminal_runtime_perf_contract_spec --test terminal_renderer_prepare_cache_spec --test bootstrap_smoke --test ssh_session_manager_spec --test logging_runtime -q
```

Expected: PASS.

**Step 2: Re-run existing memory-related regressions**

Run:

```bash
cargo test --test startup_font_memory_regression -q
cargo test working_set_trim_scheduler --lib -q
cargo test terminal_session_preserves_deeper_scrollback_history_for_large_bursts --test terminal_session_spec -q
```

Expected: PASS.

**Step 3: Review diff**

Run:

```bash
git status --short
git diff -- src/app/bootstrap.rs src/app/bootstrap/workspace_terminal.rs src/app/memory.rs src/app/terminal_presenter.rs src/app/terminal_scene_image.rs src/app/terminal_renderer/host.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/logging/config.rs src/app/logging/runtime.rs readme.md tests/terminal_memory_diagnostics_contract_spec.rs tests/bootstrap_smoke.rs tests/ssh_session_manager_spec.rs tests/logging_runtime.rs tests/terminal_runtime_perf_contract_spec.rs tests/terminal_renderer_prepare_cache_spec.rs
```

Expected: only intended diagnostics and shrink-path changes remain.

**Step 4: Manual Windows reproduction**

Run on Windows packaged build:

```powershell
cd .\dist\mica-term-x86_64-pc-windows-msvc-release-skia
ni .mica-term-portable -ItemType File -Force
$env:MICA_TERM_LOG = "debug"
$env:MICA_TERM_MEMORY_DIAGNOSTICS = "1"
.\mica-term.exe
```

Then collect logs for:

- startup idle for 30-60 seconds
- 5 sessions with `history` + heavy scrolling + close
- one session with large `cat` output and 5-10 seconds idle

Expected: logs explicitly show startup snapshot, close-shrink / idle-shrink behavior, and large-output trim behavior.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/bootstrap/workspace_terminal.rs src/app/memory.rs src/app/terminal_presenter.rs src/app/terminal_scene_image.rs src/app/terminal_renderer/host.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/logging/config.rs src/app/logging/runtime.rs readme.md tests/terminal_memory_diagnostics_contract_spec.rs tests/bootstrap_smoke.rs tests/ssh_session_manager_spec.rs tests/logging_runtime.rs tests/terminal_runtime_perf_contract_spec.rs tests/terminal_renderer_prepare_cache_spec.rs
git commit -m "fix: add terminal memory diagnostics and cache shrink paths"
```
