# Windows Build Script Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a one-command shell script that builds a Windows package and archives it as a zip file.

**Architecture:** Add a repo-root `build-win-x64.sh` script that defaults to `x86_64-pc-windows-gnu`, supports overriding the target via environment variables, performs fail-fast environment checks, builds the Rust binary, stages the packaged files in `dist/`, and zips the result. Add a shell smoke test for syntax and help output. Document prerequisites in `readme.md`.

**Tech Stack:** bash, cargo, rustup, zip, Rust workspace

---

### Task 1: Add build script and smoke test

**Files:**
- Create: `build-win-x64.sh`
- Create: `tests/build_win_x64_script_smoke.sh`
- Modify: `readme.md`

**Step 1: Write the failing test**

Create `tests/build_win_x64_script_smoke.sh` that:
- checks `build-win-x64.sh` exists
- runs `bash -n build-win-x64.sh`
- runs `build-win-x64.sh --help`
- asserts help text contains the default target and output behavior

**Step 2: Run test to verify it fails**

Run: `bash tests/build_win_x64_script_smoke.sh`
Expected: FAIL because `build-win-x64.sh` does not exist yet.

**Step 3: Write minimal implementation**

Implement `build-win-x64.sh` with:
- `--help`
- default `TARGET=x86_64-pc-windows-gnu`
- optional `TARGET=x86_64-pc-windows-msvc`
- environment checks for `cargo`, `rustup`, `zip`, required target, and GNU/MSVC host constraints
- package staging into `dist/<app>-<target>-<profile>/`
- zip output to `dist/<app>-<target>-<profile>.zip`

**Step 4: Run test to verify it passes**

Run: `bash tests/build_win_x64_script_smoke.sh`
Expected: PASS

**Step 5: Verify**

Run:
- `bash -n build-win-x64.sh`
- `bash tests/build_win_x64_script_smoke.sh`
- `cargo test -q`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

Expected: PASS
