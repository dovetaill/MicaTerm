# Desktop Build Matrix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a unified desktop packaging script that supports Linux x64/arm64, macOS, and Windows ARM64 while keeping the existing Windows x64 script as a compatibility wrapper.

**Architecture:** Introduce a repo-root `build-desktop.sh` as the single implementation for desktop packaging. Route `build-win-x64.sh` through it, expand shell smoke coverage for the new target matrix, and update `readme.md` to document host constraints and archive formats without adding installer or signing workflows.

**Tech Stack:** bash, cargo, rustup, tar, gzip, zip, Rust workspace

---

### Task 1: Add failing smoke coverage for the unified desktop entrypoint

**Files:**
- Create: `tests/build_desktop_script_smoke.sh`
- Modify: `tests/build_win_x64_script_smoke.sh`

**Step 1: Write the failing test**

Create `tests/build_desktop_script_smoke.sh` that:
- checks `build-desktop.sh` exists
- runs `bash -n build-desktop.sh`
- runs `build-desktop.sh --help`
- asserts help text contains:
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
  - `aarch64-pc-windows-msvc`
  - both `zip` and `tar.gz`

Update `tests/build_win_x64_script_smoke.sh` so it continues to assert the compatibility wrapper exists and that `--help` still exposes Windows x64 targets.

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/build_desktop_script_smoke.sh
bash tests/build_win_x64_script_smoke.sh
```

Expected:

- `build_desktop_script_smoke.sh` fails because `build-desktop.sh` does not exist yet
- `build_win_x64_script_smoke.sh` still passes against the current wrapper-free implementation

**Step 3: Write the minimal implementation needed for test progress**

Create a placeholder `build-desktop.sh` with:

```bash
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" ]]; then
  cat <<'EOF'
  supported targets...
EOF
  exit 0
fi
```

Keep the placeholder intentionally minimal so the first test can move from "missing file" to validating help output.

**Step 4: Re-run tests**

Run:

```bash
bash tests/build_desktop_script_smoke.sh
```

Expected: fail only if the help text is still incomplete or malformed.

**Step 5: Commit**

```bash
git add tests/build_desktop_script_smoke.sh tests/build_win_x64_script_smoke.sh build-desktop.sh
git commit -m "test: add desktop build smoke coverage"
```

### Task 2: Implement the unified desktop packaging script and wrapper routing

**Files:**
- Modify: `build-desktop.sh`
- Modify: `build-win-x64.sh`

**Step 1: Write the failing behavior check**

Use the smoke tests from Task 1 as the failing specification, then add one more assertion to `tests/build_desktop_script_smoke.sh` for output naming:

```bash
grep -F "dist/<app>-<target>-<profile>.tar.gz" <<<"$HELP_OUTPUT" >/dev/null
grep -F "dist/<app>-<target>-<profile>.zip" <<<"$HELP_OUTPUT" >/dev/null
```

**Step 2: Run tests to verify the new assertions fail**

Run:

```bash
bash tests/build_desktop_script_smoke.sh
```

Expected: FAIL until the final help text and routing logic are implemented.

**Step 3: Write minimal implementation**

Implement `build-desktop.sh` with these behaviors:

- shared environment parsing:

```bash
TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
PROFILE="${PROFILE:-release}"
APP_NAME=...
BIN_NAME=...
DIST_DIR=...
```

- common validation:

```bash
require_cmd cargo
require_cmd rustup
require_cmd tar
require_cmd gzip
```

- target-specific routing:

```bash
case "$TARGET" in
  x86_64-unknown-linux-gnu)
    ARCHIVE_EXT="tar.gz"
    BIN_SUFFIX=""
    ;;
  aarch64-unknown-linux-gnu)
    require linker env or known linker binary
    ARCHIVE_EXT="tar.gz"
    ;;
  x86_64-apple-darwin|aarch64-apple-darwin)
    enforce Darwin host
    ARCHIVE_EXT="tar.gz"
    ;;
  x86_64-pc-windows-gnu)
    require_cmd zip
    require MinGW linker
    BIN_SUFFIX=".exe"
    ARCHIVE_EXT="zip"
    ;;
  x86_64-pc-windows-msvc|aarch64-pc-windows-msvc)
    require_cmd zip
    enforce Windows/MSVC host
    BIN_SUFFIX=".exe"
    ARCHIVE_EXT="zip"
    ;;
  *)
    fail "unsupported TARGET ..."
    ;;
esac
```

- build + stage + archive:

```bash
cargo build "${PROFILE_ARGS[@]}" --target "$TARGET" --locked
mkdir -p "$STAGE_DIR"
cp "$BIN_PATH" "$STAGE_DIR/"
cp readme.md "$STAGE_DIR/README.md"
```

For archive creation:

```bash
tar -C "$DIST_DIR" -czf "$ARCHIVE_PATH" "$(basename "$STAGE_DIR")"
```

or:

```bash
zip -rq "$(basename "$ARCHIVE_PATH")" "$(basename "$STAGE_DIR")"
```

Convert `build-win-x64.sh` into a thin wrapper:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=...
TARGET="${TARGET:-x86_64-pc-windows-gnu}"
export TARGET
exec "$ROOT_DIR/build-desktop.sh" "$@"
```

**Step 4: Run targeted verification**

Run:

```bash
bash -n build-desktop.sh
bash -n build-win-x64.sh
bash tests/build_desktop_script_smoke.sh
bash tests/build_win_x64_script_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add build-desktop.sh build-win-x64.sh tests/build_desktop_script_smoke.sh tests/build_win_x64_script_smoke.sh
git commit -m "build: add unified desktop packaging script"
```

### Task 3: Document the build matrix and run repository verification

**Files:**
- Modify: `readme.md`

**Step 1: Write the failing documentation expectation**

Add documentation assertions to `tests/build_desktop_script_smoke.sh` only if they can be kept lightweight; otherwise use README changes as the deliverable for this task and rely on manual verification.

The README must state:

- `./build-desktop.sh` is the primary entrypoint
- `build-win-x64.sh` is a compatibility wrapper
- Linux/macOS use `.tar.gz`
- Windows uses `.zip`
- macOS and Windows ARM64 require appropriate hosts

**Step 2: Run a focused diff review before editing**

Run:

```bash
git diff -- readme.md
```

Expected: no unintended pending README edits in this worktree.

**Step 3: Write minimal implementation**

Update `readme.md` to replace the Windows-only build section with a desktop build section that includes:

- primary command examples:

```bash
./build-desktop.sh
TARGET=aarch64-unknown-linux-gnu ./build-desktop.sh
TARGET=x86_64-apple-darwin ./build-desktop.sh
TARGET=aarch64-pc-windows-msvc ./build-desktop.sh
```

- compatibility note:

```bash
./build-win-x64.sh
TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh
```

- prerequisite notes for:
  - Linux ARM64 linker
  - macOS host requirement
  - Windows GNU linker
  - Windows MSVC host requirement

**Step 4: Run full verification**

Run:

```bash
bash tests/build_desktop_script_smoke.sh
bash tests/build_win_x64_script_smoke.sh
cargo test -q
cargo fmt --check
cargo check --workspace
```

Expected: PASS

**Step 5: Commit**

```bash
git add readme.md
git commit -m "docs: describe desktop build matrix"
```
