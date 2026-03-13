# Pure FemtoVG WGPU Mainline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the remaining formal/software runtime path and the rejected window recovery experiment so the app ships a single pure `winit + femtovg-wgpu` mainline.

**Architecture:** Collapse runtime profile selection to one GPU-only mainline profile, make startup and packaging treat that route as the default, and delete the app-level recovery/mask experiment entirely. Keep test-only harnesses unless they block the mainline cleanup, so runtime behavior changes stay focused on the shipping path.

**Tech Stack:** Rust, Cargo, Slint 1.15.1, winit, femtovg-wgpu, Bash smoke tests

---

### Task 1: Lock Tests To A Single GPU Mainline

**Files:**
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`
- Modify: `tests/logging_runtime.rs`
- Modify: `tests/panic_logging.rs`

**Step 1: Write the failing test**

Update runtime-profile and logging tests to require a single mainline profile and reject `Formal`, `Software`, and `femtovg-wgpu-experimental` naming.

**Step 2: Run test to verify it fails**

Run: `cargo test --test runtime_profile --test bootstrap_profile_smoke --test logging_runtime --test panic_logging -q`
Expected: FAIL because the code still exposes `formal()` and formal/software metadata.

**Step 3: Write minimal implementation**

Collapse runtime profile code to a single mainline GPU route and update startup failure messaging accordingly.

**Step 4: Run test to verify it passes**

Run: `cargo test --test runtime_profile --test bootstrap_profile_smoke --test logging_runtime --test panic_logging -q`
Expected: PASS

### Task 2: Delete Rejected Recovery Experiment

**Files:**
- Delete: `src/app/window_recovery.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/mod.rs`
- Modify: `ui/app-window.slint`
- Delete: `tests/window_recovery_spec.rs`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `tests/window_theme_contract_smoke.sh`

**Step 1: Write the failing test**

Update UI and contract tests to reject `render-revision`, `experimental-recovery-mask-*`, `window_recovery`, and any recovery/mask hooks in bootstrap.

**Step 2: Run test to verify it fails**

Run: `cargo test --test top_status_bar_smoke -q && bash tests/window_theme_contract_smoke.sh`
Expected: FAIL because the rejected recovery experiment is still wired in.

**Step 3: Write minimal implementation**

Remove the recovery module, mask properties, recovery event handling, and recovery-specific logging while preserving normal theme sync and window placement tracking.

**Step 4: Run test to verify it passes**

Run: `cargo test --test top_status_bar_smoke -q && bash tests/window_theme_contract_smoke.sh`
Expected: PASS

### Task 3: Make Packaging Default To Pure FemtoVG WGPU

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `build-release.sh`
- Modify: `build-win-x64.sh`
- Modify: `build-linux-x64-femtovg-wgpu.sh`
- Modify: `build-win-x64-femtovg-wgpu.sh`
- Modify: `build-win-x64-gnu-femtovg-wgpu.sh`
- Modify: `readme.md`
- Modify: `tests/build_release_script_smoke.sh`
- Modify: `tests/build_win_x64_script_smoke.sh`
- Modify: `tests/build_linux_x64_femtovg_wgpu_script_smoke.sh`
- Modify: `tests/build_win_x64_femtovg_wgpu_script_smoke.sh`
- Modify: `tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh`
- Modify: `tests/femtovg_wgpu_contract_smoke.sh`

**Step 1: Write the failing test**

Update packaging and contract tests so the repo default route is pure `femtovg-wgpu`, without formal/software wording or `--no-default-features --features femtovg-wgpu-experimental`.

**Step 2: Run test to verify it fails**

Run: `cargo test --test runtime_profile --test bootstrap_profile_smoke -q && bash tests/build_release_script_smoke.sh && bash tests/build_win_x64_script_smoke.sh && bash tests/build_linux_x64_femtovg_wgpu_script_smoke.sh && bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh && bash tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh && bash tests/femtovg_wgpu_contract_smoke.sh`
Expected: FAIL because packaging and startup still present the route as formal/experimental split.

**Step 3: Write minimal implementation**

Make `femtovg-wgpu` the default build/runtime shape, keep target wrappers only as host-target conveniences, and rewrite docs/help text to match the single mainline.

**Step 4: Run test to verify it passes**

Run: `cargo test --test runtime_profile --test bootstrap_profile_smoke -q && bash tests/build_release_script_smoke.sh && bash tests/build_win_x64_script_smoke.sh && bash tests/build_linux_x64_femtovg_wgpu_script_smoke.sh && bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh && bash tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh && bash tests/femtovg_wgpu_contract_smoke.sh`
Expected: PASS

### Task 4: Full Verification

**Files:**
- Modify: `verification.md` if needed

**Step 1: Run focused tests**

Run: `cargo test --test runtime_profile --test bootstrap_profile_smoke --test logging_runtime --test panic_logging --test top_status_bar_smoke -q`
Expected: PASS

**Step 2: Run smoke scripts**

Run: `bash tests/window_theme_contract_smoke.sh && bash tests/build_release_script_smoke.sh && bash tests/build_win_x64_script_smoke.sh && bash tests/build_linux_x64_femtovg_wgpu_script_smoke.sh && bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh && bash tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh && bash tests/femtovg_wgpu_contract_smoke.sh`
Expected: PASS

**Step 3: Run repository verification**

Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

Expected: PASS
