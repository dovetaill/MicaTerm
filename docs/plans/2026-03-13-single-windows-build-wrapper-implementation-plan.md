# Single Windows Build Wrapper Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Consolidate all Windows packaging entrypoints into a single `build-win-x64.sh` wrapper that defaults to GNU, supports MSVC through `TARGET=...`, and removes redundant wrapper scripts.

**Architecture:** Keep `build-desktop.sh` as the shared implementation and make `build-win-x64.sh` the only Windows-specific shell entrypoint. The wrapper will expose both supported Windows x64 targets in `--help`, print the resolved target before delegating, and the repository contract will be enforced by one smoke test instead of three separate wrapper tests.

**Tech Stack:** Bash, ripgrep-based shell smoke tests, repository README documentation.

---

### Task 1: Update the Windows wrapper contract

**Files:**
- Modify: `build-win-x64.sh`
- Test: `tests/build_win_x64_script_smoke.sh`

**Step 1: Write the failing test**

Extend `tests/build_win_x64_script_smoke.sh` to require:
- `build-win-x64.sh --help` documents both GNU and MSVC usage.
- the wrapper source contains a `Windows wrapper target:` log line.
- legacy wrapper paths no longer exist.

**Step 2: Run test to verify it fails**

Run: `bash tests/build_win_x64_script_smoke.sh`
Expected: FAIL because the old wrappers still exist and the current wrapper does not yet expose the new help/log contract.

**Step 3: Write minimal implementation**

Update `build-win-x64.sh` to:
- support `--help`
- document default GNU usage and MSVC override usage
- print the resolved Windows target before exec'ing `build-desktop.sh`

**Step 4: Run test to verify it passes**

Run: `bash tests/build_win_x64_script_smoke.sh`
Expected: PASS

### Task 2: Remove redundant wrappers and obsolete tests

**Files:**
- Delete: `build-win-x64-femtovg-wgpu.sh`
- Delete: `build-win-x64-gnu-femtovg-wgpu.sh`
- Delete: `tests/build_win_x64_femtovg_wgpu_script_smoke.sh`
- Delete: `tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh`

**Step 1: Remove obsolete files**

Delete the redundant wrapper scripts and their dedicated smoke tests.

**Step 2: Verify repository references**

Run: `rg -n "build-win-x64-femtovg-wgpu\\.sh|build-win-x64-gnu-femtovg-wgpu\\.sh" readme.md tests *.sh`
Expected: no remaining live references outside historical docs/plans.

### Task 3: Update README and verify the new contract

**Files:**
- Modify: `readme.md`
- Reference: `build-win-x64.sh`

**Step 1: Update docs**

Adjust the Windows build section so it documents:
- `./build-win-x64.sh` as the single Windows wrapper
- default GNU behavior
- MSVC override via `TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh`

**Step 2: Run verification**

Run:
- `cargo fmt --check`
- `bash tests/build_win_x64_script_smoke.sh`
- `bash tests/build_release_script_smoke.sh`
- `bash tests/windows_icon_integration_smoke.sh`

Expected: all pass
