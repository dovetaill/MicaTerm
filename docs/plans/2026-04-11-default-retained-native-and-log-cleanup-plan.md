# Default Retained-Native And Log Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep packaged Windows builds on the retained-native path and remove the noisy retained-native diagnostics trace logging while aligning current docs and scripts with the retained-native-only Windows contract.

**Architecture:** Keep the runtime profile on the retained-native path, remove the high-frequency diagnostics trace from bootstrap, and update current scripts/docs/source-level contract tests so they describe retained-native as the only live Windows subsystem.

**Tech Stack:** Rust, cargo tests, shell build script, vendored Slint/winit integration already present in the repo.

---

### Task 1: Lock the intended behavior with failing tests

**Files:**
- Modify: `tests/bootstrap_profile_smoke.rs`
- Modify: `tests/build_win_x64_script_smoke.sh`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/windows_terminal_native_mode_contract_smoke.sh`
- Modify: `tests/windows_terminal_diagnostics_spec.rs`

**Step 1: Write the failing test updates**
- Change script/runtime-profile expectations to retained-native-only Windows behavior.
- Remove the source-level expectation that bootstrap keeps the noisy retained-native diagnostics debug hook.

**Step 2: Run tests to verify they fail**
Run:
- `cargo test --test runtime_profile -- --nocapture`
- `cargo test --test bootstrap_profile_smoke -- --nocapture`
- `cargo test --test terminal_scrollback_spec -- --nocapture`
- `cargo test --test windows_terminal_diagnostics_spec -- --nocapture`
- `bash tests/build_win_x64_script_smoke.sh`
- `bash tests/windows_terminal_native_mode_contract_smoke.sh`

Expected: failures that still reference the removed Windows subsystem wording and the diagnostics hook.

### Task 2: Implement the minimal packaged-default change

**Files:**
- Modify: `build-win-x64.sh`

**Step 1: Change the packaged subsystem default**
- Remove the exported packaged subsystem value so the wrapper no longer advertises a second Windows subsystem selector.

**Step 2: Keep runtime override behavior unchanged**
- Do not add any new Windows subsystem selector; the live contract stays retained-native-only.

**Step 3: Run the targeted tests**
Run:
- `bash tests/build_win_x64_script_smoke.sh`
- `bash tests/windows_terminal_native_mode_contract_smoke.sh`
- `cargo test --test runtime_profile -- --nocapture`

Expected: pass.

### Task 3: Remove the noisy retained-native diagnostics logging

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/windows_terminal_diagnostics_spec.rs`

**Step 1: Remove the per-frame diagnostics trace hook**
- Delete the retained-native diagnostics logging helper and its call site from bootstrap.
- Leave functional diagnostics snapshot data structures intact; only remove the noisy log emission path.

**Step 2: Run the targeted tests**
Run:
- `cargo test --test windows_terminal_diagnostics_spec -- --nocapture`
- `cargo test --test native_terminal_surface_contract_spec -- --nocapture`

Expected: pass.

### Task 4: Run final verification and package build

**Files:**
- Verify only

**Step 1: Run final verification**
Run:
- `cargo check`
- `cargo test --test runtime_profile -- --nocapture`
- `cargo test --test bootstrap_profile_smoke -- --nocapture`
- `cargo test --test terminal_scrollback_spec -- --nocapture`
- `cargo test --test windows_terminal_diagnostics_spec -- --nocapture`
- `cargo test --test native_terminal_surface_contract_spec -- --nocapture`
- `bash tests/build_win_x64_script_smoke.sh`
- `bash tests/windows_terminal_native_mode_contract_smoke.sh`
- `./build-win-x64.sh`

Expected: all pass, and the packaged build stays on retained-native without a Windows subsystem selector.

### Task 5: Commit

**Files:**
- Commit the code and plan doc if verification passes.

**Step 1: Commit**
```bash
git add build-win-x64.sh src/app/bootstrap.rs tests/bootstrap_profile_smoke.rs tests/build_win_x64_script_smoke.sh tests/runtime_profile.rs tests/terminal_scrollback_spec.rs tests/windows_terminal_diagnostics_spec.rs tests/windows_terminal_native_mode_contract_smoke.sh docs/plans/2026-04-11-default-retained-native-and-log-cleanup-plan.md
git commit -m "docs: align retained-native-only windows packaging contract"
```


## Completion Notes

Status: completed on 2026-04-11.

What landed:
- packaged Windows `build-win-x64.sh` now stays on the retained-native child HWND path without exporting a second Windows subsystem selector
- bootstrap removed the per-frame retained-native diagnostics trace hook while keeping the underlying diagnostics data model intact
- runtime/source docs and contract tests were updated so retained-native is described as the only live Windows subsystem

Slint note:
- this follow-up did not change upstream Slint and did not add a new fork branch; it stays on the existing vendored Slint baseline already patched in this repo

Verification run:
- `cargo test --test windows_terminal_diagnostics_spec -- --nocapture`
- `cargo test --test runtime_profile -- --nocapture`
- `cargo test --test native_terminal_surface_contract_spec -- --nocapture`
- `cargo test --test terminal_renderer_dwrite_spec -- --nocapture`
- `cargo test --test windows_native_text_renderer_contract_spec -- --nocapture`
- `bash tests/build_win_x64_script_smoke.sh`
- `bash tests/windows_terminal_native_mode_contract_smoke.sh`
- `cargo check`
- `./build-win-x64.sh`
