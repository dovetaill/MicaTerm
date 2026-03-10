# Mica Term Icon Logo Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add the approved `M-Frame` logo system to the repository, generate Windows-ready icon assets, and wire the application icon into the Windows build and package flow.

**Architecture:** Store the approved vector sources in-repo under `assets/icons/`, generate deterministic raster assets with a small shell script using `rsvg-convert` and ImageMagick, and commit the generated `png` + `ico` outputs so packaging does not depend on graphics tools at build time. Embed the `.ico` into Windows binaries from `build.rs`, and stage the icon file in the Windows zip package for downstream installer and shortcut use.

**Tech Stack:** SVG, bash, `rsvg-convert`, ImageMagick (`magick`), Rust `build.rs`, `winresource`, Cargo, existing `build-win-x64.sh`

---

**Execution Notes**

- Use `@superpowers:test-driven-development` before each task.
- If a test or export step fails unexpectedly, stop and use `@superpowers:systematic-debugging` instead of guessing.
- Before claiming the feature complete, use `@superpowers:verification-before-completion` and capture exact command output.
- Execute the plan inside the dedicated worktree at `/home/wwwroot/mica-term/.worktrees/feature-icon-logo`.

### Task 1: Add approved vector logo assets

**Files:**
- Create: `assets/icons/mica-term-logo.svg`
- Create: `assets/icons/mica-term-app.svg`
- Create: `assets/icons/mica-term-taskbar.svg`
- Create: `assets/icons/mica-term-mark.svg`
- Test: `tests/icon_svg_assets_smoke.sh`

**Step 1: Write the failing test**

Create `tests/icon_svg_assets_smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

check_file() {
  local path="$1"
  local view_box="$2"

  [[ -f "$path" ]] || {
    echo "missing icon asset: $path" >&2
    exit 1
  }

  grep -F "$view_box" "$path" >/dev/null
}

check_file "$ROOT_DIR/assets/icons/mica-term-logo.svg" 'viewBox="0 0 720 256"'
check_file "$ROOT_DIR/assets/icons/mica-term-app.svg" 'viewBox="0 0 256 256"'
check_file "$ROOT_DIR/assets/icons/mica-term-taskbar.svg" 'viewBox="0 0 256 256"'
check_file "$ROOT_DIR/assets/icons/mica-term-mark.svg" 'viewBox="0 0 256 256"'

grep -F '#4ea1ff' "$ROOT_DIR/assets/icons/mica-term-logo.svg" >/dev/null
grep -F 'id="m-frame"' "$ROOT_DIR/assets/icons/mica-term-app.svg" >/dev/null
grep -F 'id="taskbar-m-frame"' "$ROOT_DIR/assets/icons/mica-term-taskbar.svg" >/dev/null
grep -F 'fill="currentColor"' "$ROOT_DIR/assets/icons/mica-term-mark.svg" >/dev/null
```

**Step 2: Run test to verify it fails**

Run: `bash tests/icon_svg_assets_smoke.sh`

Expected: FAIL with `missing icon asset`

**Step 3: Write minimal implementation**

Create `assets/icons/mica-term-logo.svg`:

```svg
<svg width="720" height="256" viewBox="0 0 720 256" fill="none" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="mica-accent" x1="72" y1="32" x2="184" y2="208" gradientUnits="userSpaceOnUse">
      <stop stop-color="#7CC4FF"/>
      <stop offset="1" stop-color="#4EA1FF"/>
    </linearGradient>
  </defs>
  <rect x="24" y="24" width="208" height="208" rx="48" fill="#131720"/>
  <rect x="24.5" y="24.5" width="207" height="207" rx="47.5" stroke="#FFFFFF" stroke-opacity="0.08"/>
  <path id="m-frame" d="M68 176V80L106 130L144 88L182 176H154L132 123L106 156L82 123V176H68Z" fill="url(#mica-accent)"/>
  <path d="M292 105.5H316.5L358 172L399.5 105.5H424V192H402.5V139L362.5 192H353L313.5 139V192H292V105.5Z" fill="#F5F7FB"/>
  <path d="M445 105.5H512V123.5H466.5V140.5H507.5V158H466.5V192H445V105.5Z" fill="#F5F7FB"/>
  <path d="M528 105.5H549.5V174H588V192H528V105.5Z" fill="#F5F7FB"/>
  <path d="M602 105.5H623.5V192H602V105.5Z" fill="#F5F7FB"/>
  <path d="M644 105.5H666L696 192H673.5L667.5 173.5H640.5L634.5 192H612L644 105.5ZM654 132.5L646 156.5H662L654 132.5Z" fill="#F5F7FB"/>
</svg>
```

Create `assets/icons/mica-term-app.svg`:

```svg
<svg width="256" height="256" viewBox="0 0 256 256" fill="none" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="app-accent" x1="70" y1="42" x2="184" y2="210" gradientUnits="userSpaceOnUse">
      <stop stop-color="#83CCFF"/>
      <stop offset="1" stop-color="#4ea1ff"/>
    </linearGradient>
  </defs>
  <rect x="20" y="20" width="216" height="216" rx="50" fill="#131720"/>
  <rect x="20.5" y="20.5" width="215" height="215" rx="49.5" stroke="#FFFFFF" stroke-opacity="0.08"/>
  <path id="m-frame" d="M66 174V78L104 128L144 86L186 174H156L132 122L104 156L80 122V174H66Z" fill="url(#app-accent)"/>
</svg>
```

Create `assets/icons/mica-term-taskbar.svg`:

```svg
<svg width="256" height="256" viewBox="0 0 256 256" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect x="24" y="24" width="208" height="208" rx="46" fill="#10141C"/>
  <path id="taskbar-m-frame" d="M68 176V80L106 132L146 88L188 176H158L133 124L106 159L81 124V176H68Z" fill="#4ea1ff"/>
</svg>
```

Create `assets/icons/mica-term-mark.svg`:

```svg
<svg width="256" height="256" viewBox="0 0 256 256" fill="none" xmlns="http://www.w3.org/2000/svg">
  <path d="M66 174V78L104 128L144 86L186 174H156L132 122L104 156L80 122V174H66Z" fill="currentColor"/>
</svg>
```

**Step 4: Run test to verify it passes**

Run: `bash tests/icon_svg_assets_smoke.sh`

Expected: PASS with no output

**Step 5: Commit**

```bash
git add assets/icons/mica-term-logo.svg assets/icons/mica-term-app.svg assets/icons/mica-term-taskbar.svg assets/icons/mica-term-mark.svg tests/icon_svg_assets_smoke.sh
git commit -m "feat: add mica term icon source assets"
```

### Task 2: Add deterministic icon export pipeline

**Files:**
- Create: `scripts/export-icons.sh`
- Create: `tests/export_icons_smoke.sh`
- Create: `assets/icons/png/.gitkeep`
- Create: `assets/icons/windows/.gitkeep`
- Create: `assets/icons/png/mica-term-16.png`
- Create: `assets/icons/png/mica-term-20.png`
- Create: `assets/icons/png/mica-term-24.png`
- Create: `assets/icons/png/mica-term-32.png`
- Create: `assets/icons/png/mica-term-40.png`
- Create: `assets/icons/png/mica-term-48.png`
- Create: `assets/icons/png/mica-term-64.png`
- Create: `assets/icons/png/mica-term-128.png`
- Create: `assets/icons/png/mica-term-256.png`
- Create: `assets/icons/windows/mica-term.ico`

**Step 1: Write the failing test**

Create `tests/export_icons_smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/scripts/export-icons.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

[[ -f "$SCRIPT_PATH" ]] || {
  echo "missing export script: $SCRIPT_PATH" >&2
  exit 1
}

bash -n "$SCRIPT_PATH"
OUTPUT_DIR="$TMP_DIR/out" "$SCRIPT_PATH"

for size in 16 20 24 32 40 48 64 128 256; do
  [[ -f "$TMP_DIR/out/png/mica-term-$size.png" ]] || {
    echo "missing png size: $size" >&2
    exit 1
  }
done

[[ -f "$TMP_DIR/out/windows/mica-term.ico" ]] || {
  echo "missing ico output" >&2
  exit 1
}
```

**Step 2: Run test to verify it fails**

Run: `bash tests/export_icons_smoke.sh`

Expected: FAIL with `missing export script`

**Step 3: Write minimal implementation**

Create `scripts/export-icons.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="${SOURCE_DIR:-$ROOT_DIR/assets/icons}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/assets/icons}"

APP_SVG="$SOURCE_DIR/mica-term-app.svg"
TASKBAR_SVG="$SOURCE_DIR/mica-term-taskbar.svg"
PNG_DIR="$OUTPUT_DIR/png"
WINDOWS_DIR="$OUTPUT_DIR/windows"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

require_cmd rsvg-convert
require_cmd magick

[[ -f "$APP_SVG" ]] || { echo "missing app svg: $APP_SVG" >&2; exit 1; }
[[ -f "$TASKBAR_SVG" ]] || { echo "missing taskbar svg: $TASKBAR_SVG" >&2; exit 1; }

mkdir -p "$PNG_DIR" "$WINDOWS_DIR"

for size in 16 20 24 32 40 48 64 128 256; do
  INPUT_SVG="$APP_SVG"
  if [[ "$size" -le 32 ]]; then
    INPUT_SVG="$TASKBAR_SVG"
  fi

  rsvg-convert -w "$size" -h "$size" "$INPUT_SVG" -o "$PNG_DIR/mica-term-$size.png"
done

magick \
  "$PNG_DIR/mica-term-16.png" \
  "$PNG_DIR/mica-term-20.png" \
  "$PNG_DIR/mica-term-24.png" \
  "$PNG_DIR/mica-term-32.png" \
  "$PNG_DIR/mica-term-40.png" \
  "$PNG_DIR/mica-term-48.png" \
  "$PNG_DIR/mica-term-64.png" \
  "$PNG_DIR/mica-term-128.png" \
  "$PNG_DIR/mica-term-256.png" \
  "$WINDOWS_DIR/mica-term.ico"
```

Create placeholder keep files:

```bash
mkdir -p assets/icons/png assets/icons/windows
touch assets/icons/png/.gitkeep assets/icons/windows/.gitkeep
```

Run the exporter to create committed outputs:

```bash
bash scripts/export-icons.sh
```

**Step 4: Run test to verify it passes**

Run: `bash tests/export_icons_smoke.sh`

Expected: PASS with no output

**Step 5: Commit**

```bash
git add scripts/export-icons.sh tests/export_icons_smoke.sh assets/icons/png assets/icons/windows
git commit -m "feat: add icon export pipeline"
```

### Task 3: Wire the Windows icon into the build and package flow

**Files:**
- Modify: `Cargo.toml`
- Modify: `build.rs`
- Modify: `build-win-x64.sh`
- Modify: `readme.md`
- Create: `tests/windows_icon_integration_smoke.sh`

**Step 1: Write the failing test**

Create `tests/windows_icon_integration_smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

grep -F 'winresource' "$ROOT_DIR/Cargo.toml" >/dev/null
grep -F 'assets/icons/windows/mica-term.ico' "$ROOT_DIR/build.rs" >/dev/null
grep -F 'assets/icons/windows/mica-term.ico' "$ROOT_DIR/build-win-x64.sh" >/dev/null
grep -F 'scripts/export-icons.sh' "$ROOT_DIR/readme.md" >/dev/null

[[ -f "$ROOT_DIR/assets/icons/windows/mica-term.ico" ]] || {
  echo "missing committed windows icon" >&2
  exit 1
}
```

**Step 2: Run test to verify it fails**

Run: `bash tests/windows_icon_integration_smoke.sh`

Expected: FAIL because `winresource` and icon references are missing

**Step 3: Write minimal implementation**

Modify `Cargo.toml` build dependencies:

```toml
[build-dependencies]
slint-build = "1.15.1"
winresource = "0.1.19"
```

Modify `build.rs`:

```rust
fn main() {
    slint_build::compile("ui/app-window.slint").expect("failed to compile Slint UI");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/windows/mica-term.ico");
        res.compile().expect("failed to compile Windows icon resources");
    }
}
```

Modify `build-win-x64.sh` to stage the icon:

```bash
ICON_PATH="$ROOT_DIR/assets/icons/windows/mica-term.ico"
```

Insert after the executable copy:

```bash
if [[ -f "$ICON_PATH" ]]; then
  cp "$ICON_PATH" "$STAGE_DIR/"
fi
```

Modify `readme.md`:

```md
## Icon Assets

- Source vectors: `assets/icons/`
- Export script: `scripts/export-icons.sh`
- Windows icon: `assets/icons/windows/mica-term.ico`
```

**Step 4: Run tests to verify they pass**

Run: `bash tests/windows_icon_integration_smoke.sh`

Expected: PASS with no output

Run: `cargo test`

Expected: PASS with existing test suite still green

**Step 5: Commit**

```bash
git add Cargo.toml build.rs build-win-x64.sh readme.md tests/windows_icon_integration_smoke.sh
git commit -m "feat: wire windows icon into build packaging"
```

### Task 4: Verify icon outputs and packaging end-to-end

**Files:**
- Verify only: `assets/icons/`
- Verify only: `build-win-x64.sh`

**Step 1: Run focused verification**

Run:

```bash
bash tests/icon_svg_assets_smoke.sh
bash tests/export_icons_smoke.sh
bash tests/windows_icon_integration_smoke.sh
cargo test
```

Expected: PASS

**Step 2: Rebuild exported assets from source**

Run:

```bash
rm -f assets/icons/png/mica-term-*.png assets/icons/windows/mica-term.ico
bash scripts/export-icons.sh
git diff -- assets/icons
```

Expected: regenerated assets match the committed outputs or only show intended binary diffs after the first generation

**Step 3: Verify Windows package staging**

Run:

```bash
./build-win-x64.sh --help
```

Expected: help output still succeeds

If the Windows GNU target and linker are installed, also run:

```bash
TARGET=x86_64-pc-windows-gnu PROFILE=release ./build-win-x64.sh
unzip -l dist/mica-term-x86_64-pc-windows-gnu-release.zip | grep -F 'mica-term.ico'
```

Expected: archive contains `mica-term.ico`

**Step 4: Commit final verification updates if needed**

```bash
git add assets/icons scripts/export-icons.sh tests
git commit -m "chore: verify icon packaging outputs"
```
