# Windows Startup Memory Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce startup memory for both Windows packaging paths by lazy-initializing the workspace terminal presenter instead of constructing it during bootstrap.

**Architecture:** Keep the workspace presenter in an optional slot, publish lightweight fallback cell metrics during welcome-mode startup, and initialize the presenter only when an active terminal surface actually needs it. Preserve the native retained-frame path for mainline packages and the bitmap compatibility path for software packages.

**Tech Stack:** Rust, Slint, Windows runtime profiles, retained native terminal frame pipeline, cargo test, Windows packaging shell scripts

---

### Task 1: Lock the lazy-init contract in tests

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Test: `tests/runtime_profile.rs`

**Step 1: Write the failing test**

Add source-contract assertions that:
- `src/app/bootstrap.rs` no longer contains `install_workspace_terminal_presenter(window, profile);`
- `src/app/bootstrap.rs` contains a lazy-init helper such as `ensure_workspace_terminal_presenter`
- `src/app/bootstrap.rs` stores the workspace presenter as `Option<Box<dyn TerminalPresenter>>`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_renderer_dwrite_spec -q
```

Expected: FAIL because bootstrap still eagerly installs the presenter and the presenter slot is not optional yet.

**Step 3: Write minimal implementation**

No production changes in this task. Only adjust tests until they express the lazy-init behavior clearly.

**Step 4: Run test to verify it fails correctly**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_renderer_dwrite_spec -q
```

Expected: FAIL for the intended contract mismatch, not for syntax or unrelated test issues.

**Step 5: Commit**

```bash
git add tests/bootstrap_smoke.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "test: pin lazy terminal presenter startup contract"
```

### Task 2: Implement lazy presenter initialization

**Files:**
- Modify: `src/app/bootstrap.rs`
- Reference: `src/app/runtime_profile.rs`
- Reference: `src/app/terminal_presenter.rs`

**Step 1: Write the failing test**

Use the contract tests from Task 1 as the failing red state.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_renderer_dwrite_spec -q
```

Expected: FAIL before implementation.

**Step 3: Write minimal implementation**

Implement:
- optional presenter storage in `WORKSPACE_TERMINAL_PRESENTER`
- fallback workspace cell-size constants for pre-terminal startup
- helper to lazily create the presenter only when `profile.prefers_native_terminal_renderer()` is true
- render-time callsites that use the helper before requesting cell metrics or presenting native frames
- no eager presenter install in bootstrap

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_renderer_dwrite_spec --test runtime_profile -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/terminal_renderer_dwrite_spec.rs tests/runtime_profile.rs
git commit -m "refactor: lazily initialize windows terminal presenter"
```

### Task 3: Verify both Windows packaging paths

**Files:**
- Reference: `build-win-x64.sh`
- Reference: `build-win-x64-software.sh`

**Step 1: Run targeted Rust verification**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_renderer_dwrite_spec --test runtime_profile --test native_terminal_surface_contract_spec -q
```

Expected: PASS.

**Step 2: Run the software Windows package build**

Run:

```bash
./build-win-x64-software.sh
```

Expected: PASS and produce the software Windows package.

**Step 3: Run the mainline Windows package build**

Run:

```bash
./build-win-x64.sh
```

Expected: PASS and produce the native/skia Windows package.

**Step 4: Review the diff**

Run:

```bash
git status --short
git diff -- src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/terminal_renderer_dwrite_spec.rs tests/runtime_profile.rs
```

Expected: Only the intended lazy-init and test updates remain.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/terminal_renderer_dwrite_spec.rs tests/runtime_profile.rs
git commit -m "fix: reduce windows startup memory"
```
