# Build Win X64 Repro Wrapper Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a dedicated Windows x64 packaging wrapper for the minimal repro binary without changing the existing formal build entry.

**Architecture:** The new script remains a thin wrapper over `build-desktop.sh`, just like the existing Windows formal wrapper. Validation is shell-first: a new smoke test locks the expected environment variables and help passthrough behavior, then the wrapper and README are updated to match.

**Tech Stack:** Bash, Cargo packaging script, shell smoke tests, Markdown docs

---

### Task 1: Add The Dedicated Repro Wrapper

**Files:**
- Create: `build-win-x64-repro.sh`
- Create: `tests/build_win_x64_repro_script_smoke.sh`
- Modify: `readme.md`

**Step 1: Write the failing test**

Create `tests/build_win_x64_repro_script_smoke.sh` with a contract like:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-win-x64-repro.sh"

[[ -f "$SCRIPT_PATH" ]] || {
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
}
```

Then assert:

- `bash -n "$SCRIPT_PATH"` passes
- script contains:
  - `TARGET="${TARGET:-x86_64-pc-windows-gnu}"`
  - `APP_NAME="${APP_NAME:-windows-theme-repro}"`
  - `BIN_NAME="${BIN_NAME:-windows_theme_repro}"`
- `"$SCRIPT_PATH" --help` includes:
  - `x86_64-pc-windows-gnu`
  - `.zip`

**Step 2: Run test to verify it fails**

Run:

```bash
bash tests/build_win_x64_repro_script_smoke.sh
```

Expected: FAIL because the wrapper does not exist yet.

**Step 3: Write minimal implementation**

- Add `build-win-x64-repro.sh` as a thin wrapper over `build-desktop.sh`
- Update `readme.md` with one short subsection showing:
  - `./build-win-x64-repro.sh`
  - the resulting archive name

**Step 4: Run test to verify it passes**

Run:

```bash
bash tests/build_win_x64_repro_script_smoke.sh
```

Expected: PASS

**Step 5: Run project verification**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: PASS

**Step 6: Commit**

```bash
git add build-win-x64-repro.sh tests/build_win_x64_repro_script_smoke.sh readme.md
git commit -m "feat: add windows repro build wrapper"
```
