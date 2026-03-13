# Linux Host Windows GNU FemtoVG WGPU Wrapper Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Linux-host wrapper that packages the pure `femtovg-wgpu-experimental` Windows GNU build without changing the existing Windows MSVC experimental entrypoint.

**Architecture:** Keep the current `build-win-x64-femtovg-wgpu.sh` semantics unchanged for Windows-host MSVC packaging. Add a new repo-root wrapper that fixes `TARGET=x86_64-pc-windows-gnu` while reusing the same experimental Cargo feature shape and `build-desktop.sh` packaging pipeline. Lock the new entrypoint with a dedicated shell smoke test and document it in `readme.md`.

**Tech Stack:** bash, cargo, rustup, zip, MinGW-w64, Rust workspace

---

### Task 1: Add Linux-host experimental Windows GNU wrapper

**Files:**
- Create: `build-win-x64-gnu-femtovg-wgpu.sh`
- Create: `tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh`

**Step 1: Write the failing test**

Create `tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh` that:

- checks `build-win-x64-gnu-femtovg-wgpu.sh` exists
- runs `bash -n build-win-x64-gnu-femtovg-wgpu.sh`
- runs `build-win-x64-gnu-femtovg-wgpu.sh --help`
- asserts help text contains:
  - `x86_64-pc-windows-gnu`
  - `mica-term-femtovg-wgpu-experimental`
  - `--no-default-features`
  - `.zip`
- asserts script text contains `femtovg-wgpu-experimental`

**Step 2: Run test to verify it fails**

Run:

```bash
bash tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh
```

Expected: FAIL because the new wrapper does not exist yet.

**Step 3: Write minimal implementation**

Create `build-win-x64-gnu-femtovg-wgpu.sh` with:

- `#!/usr/bin/env bash`
- `set -euo pipefail`
- `--help`
- fixed `TARGET="${TARGET:-x86_64-pc-windows-gnu}"`
- fixed `APP_NAME="${APP_NAME:-mica-term-femtovg-wgpu-experimental}"`
- fixed `BIN_NAME="${BIN_NAME:-mica-term}"`
- fixed `CARGO_NO_DEFAULT_FEATURES="${CARGO_NO_DEFAULT_FEATURES:-1}"`
- fixed `CARGO_FEATURES="${CARGO_FEATURES:-femtovg-wgpu-experimental}"`
- final `exec "$ROOT_DIR/build-desktop.sh" "$@"`

Help text must explicitly say:

- this is the Linux-host Windows GNU experimental path
- target is `x86_64-pc-windows-gnu`
- output is `dist/mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-gnu-release.zip`

**Step 4: Run test to verify it passes**

Run:

```bash
bash tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh
```

Expected: PASS

**Step 5: Verify related wrapper contracts**

Run:

```bash
bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh
bash tests/build_win_x64_script_smoke.sh
bash tests/build_release_script_smoke.sh
```

Expected: PASS, proving the new GNU wrapper did not blur the MSVC experimental path or the formal release path.

**Step 6: Commit**

```bash
git add build-win-x64-gnu-femtovg-wgpu.sh \
  tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh
git commit -m "build: add linux windows gnu femtovg wrapper"
```

### Task 2: Document the new wrapper without changing formal release semantics

**Files:**
- Modify: `readme.md`

**Step 1: Write the failing documentation expectation**

Run:

```bash
rg -n 'build-win-x64-gnu-femtovg-wgpu.sh|x86_64-pc-windows-gnu|mica-term-femtovg-wgpu-experimental' readme.md
```

Expected: FAIL or incomplete output because the new GNU experimental wrapper is not documented yet.

**Step 2: Write minimal documentation**

Update `readme.md` in the `FemtoVG WGPU Experimental` section to add:

- `./build-win-x64-gnu-femtovg-wgpu.sh`
- Linux-host Windows GNU experimental target:
  - `x86_64-pc-windows-gnu`
- same renderer shape:
  - `--no-default-features`
  - `--features femtovg-wgpu-experimental`
- output archive:
  - `dist/mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-gnu-release.zip`
- clear separation from:
  - `build-win-x64-femtovg-wgpu.sh` for Windows-host MSVC packaging
  - `build-release.sh` for the formal release flow

**Step 3: Run documentation and smoke verification**

Run:

```bash
rg -n 'build-win-x64-gnu-femtovg-wgpu.sh|x86_64-pc-windows-gnu|mica-term-femtovg-wgpu-experimental' readme.md
bash tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh
bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh
bash tests/build_release_script_smoke.sh
```

Expected: PASS

**Step 4: Run workspace verification**

Run:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: PASS

**Step 5: Commit**

```bash
git add readme.md
git commit -m "docs: add linux windows gnu femtovg wrapper"
```
