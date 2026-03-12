# Windows Theme Minimal Repro And Skia Removal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove all Windows Skia experimental paths from the main app, keep only the workaround-based formal path, and add a separate minimal Windows repro binary for the offscreen theme-toggle rendering issue.

**Architecture:** The work stays inside an isolated worktree branch. The formal app path is simplified back to a single runtime profile and a single packaging route, while the bug investigation path moves into a separate minimal Slint binary that does not reuse the production bootstrap or shell tree. Every behavior change is driven by tests first, then the minimum code and doc updates needed to make them pass.

**Tech Stack:** Rust, Slint UI, Cargo, shell smoke tests, Markdown docs

---

### Task 1: Remove Skia Feature Flags And Experimental Runtime Path

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `src/app/runtime_profile.rs`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`

**Step 1: Write the failing tests**

Update `tests/runtime_profile.rs` so it only accepts the formal profile:

```rust
use mica_term::app::runtime_profile::{AppBuildFlavor, AppRuntimeProfile, RendererMode};

#[test]
fn formal_profile_uses_single_supported_renderer_path() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Formal);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert!(!profile.requires_backend_lock());
    assert_eq!(profile.forced_backend(), None);
    assert!(profile.uses_theme_redraw_recovery());
}
```

Update `tests/bootstrap_profile_smoke.rs` to remove the experimental assertions and assert that only the formal/software path remains:

```rust
use mica_term::app::runtime_profile::{AppRuntimeProfile, RendererMode};

#[test]
fn formal_profile_is_the_only_bootstrap_profile_path() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert!(!profile.requires_backend_lock());
    assert_eq!(profile.forced_backend(), None);
    assert!(profile.uses_theme_redraw_recovery());
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke -q
```

Expected: FAIL because `SkiaExperimental`, `SkiaSoftware`, or `skia_experimental()` still exist in production code.

**Step 3: Write the minimal implementation**

Make the smallest changes needed:

- In `Cargo.toml`, delete:
  - `slint-renderer-skia = ["slint/renderer-skia"]`
  - `windows-skia-experimental = ["slint-renderer-skia"]`
- In `src/app/runtime_profile.rs`:
  - remove `SkiaExperimental`
  - remove `SkiaSoftware`
  - remove `skia_experimental()`
  - make `requires_backend_lock()` always return `false`
  - make `forced_backend()` always return `None`
  - keep `uses_theme_redraw_recovery()` returning `true`
- In `src/main.rs`:
  - remove `SKIA_SOFTWARE_BACKEND`
  - remove conditional `select_runtime_profile()` branches
  - remove `apply_renderer_lock(...)`
  - always use `AppRuntimeProfile::formal()`

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml src/main.rs src/app/runtime_profile.rs tests/runtime_profile.rs tests/bootstrap_profile_smoke.rs
git commit -m "refactor: remove skia experimental runtime path"
```

### Task 2: Delete Skia Script And Clean Mainline Docs And Tests

**Files:**
- Delete: `build-win-x64-skia.sh`
- Modify: `readme.md`
- Modify: `verification.md`
- Modify: `tests/logging_runtime.rs`
- Modify: `tests/panic_logging.rs`
- Modify: `tests/window_theme_contract_smoke.sh`
- Modify or Delete: `tests/build_win_x64_skia_script_smoke.sh`

**Step 1: Write the failing tests**

Convert the relevant tests to assert that the repository no longer exposes a Skia experimental path.

For `tests/window_theme_contract_smoke.sh`, change the contract to reject the backend lock:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN_FILE="$ROOT_DIR/src/main.rs"

if rg -n "winit-skia-software|windows-skia-experimental" "$MAIN_FILE" >/dev/null; then
  echo "unexpected skia experimental contract remains in src/main.rs"
  exit 1
fi
```

For `tests/logging_runtime.rs`, change the runtime metadata expectation to reject `SkiaExperimental`, `SkiaSoftware`, and `winit-skia-software`.

For `tests/panic_logging.rs`, remove the startup failure message that references `Skia Experimental`.

For `tests/build_win_x64_skia_script_smoke.sh`, either:

- delete the file and remove it from verification flow, or
- repurpose it into a negative smoke test that fails if `build-win-x64-skia.sh` still exists

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test logging_runtime --test panic_logging -q
bash tests/window_theme_contract_smoke.sh
test ! -f build-win-x64-skia.sh
```

Expected: FAIL because the script, docs, and Skia references still exist.

**Step 3: Write the minimal implementation**

Make the smallest cleanup needed:

- delete `build-win-x64-skia.sh`
- remove the `Windows Skia Experimental` section from `readme.md`
- update `verification.md` so it no longer claims Skia experimental support
- update the tests listed above so they only describe the single formal/workaround route

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test logging_runtime --test panic_logging -q
bash tests/window_theme_contract_smoke.sh
test ! -f build-win-x64-skia.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add readme.md verification.md tests/logging_runtime.rs tests/panic_logging.rs tests/window_theme_contract_smoke.sh tests/build_win_x64_skia_script_smoke.sh
git rm build-win-x64-skia.sh
git commit -m "chore: remove skia experimental build path"
```

### Task 3: Add The Minimal Windows Theme Repro Binary

**Files:**
- Create: `src/bin/windows_theme_repro.rs`
- Create: `ui/windows-theme-repro.slint`
- Create: `tests/windows_theme_repro_smoke.rs`

**Step 1: Write the failing test**

Create `tests/windows_theme_repro_smoke.rs` with a compile-surface contract that proves the new binary exists and stays decoupled from the production bootstrap.

```rust
use std::fs;
use std::path::Path;

#[test]
fn windows_theme_repro_sources_exist() {
    assert!(Path::new("src/bin/windows_theme_repro.rs").exists());
    assert!(Path::new("ui/windows-theme-repro.slint").exists());
}

#[test]
fn windows_theme_repro_does_not_use_production_bootstrap() {
    let content = fs::read_to_string("src/bin/windows_theme_repro.rs").unwrap();

    assert!(!content.contains("bootstrap::run_with_profile"));
    assert!(!content.contains("window_effects"));
    assert!(content.contains("slint::include_modules!"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test windows_theme_repro_smoke -q
```

Expected: FAIL because the new files do not exist yet.

**Step 3: Write the minimal implementation**

Create `ui/windows-theme-repro.slint` with:

- a standard `Window`
- a `Toggle Theme` button
- a text label showing `Dark` or `Light`
- a large content rectangle whose background flips between solid black and solid white
- a large preferred height to make the offscreen scenario easy to trigger on Windows

Create `src/bin/windows_theme_repro.rs` with:

- `slint::include_modules!();`
- window construction for the repro component
- a small Rust-side toggle handler that flips a `dark_mode` property
- no logging runtime, no runtime profile, no production bootstrap

Suggested shape:

```rust
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let window = WindowsThemeRepro::new()?;
    let weak = window.as_weak();

    window.on_toggle_theme_requested(move || {
        if let Some(window) = weak.upgrade() {
            let next = !window.get_dark_mode();
            window.set_dark_mode(next);
        }
    });

    window.run()
}
```

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test windows_theme_repro_smoke -q
cargo check --bin windows_theme_repro
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/bin/windows_theme_repro.rs ui/windows-theme-repro.slint tests/windows_theme_repro_smoke.rs
git commit -m "feat: add windows theme minimal repro binary"
```

### Task 4: Run Final Verification And Record Manual Windows Repro Steps

**Files:**
- Modify: `verification.md`
- Modify: `docs/plans/2026-03-12-windows-theme-min-repro-and-skia-removal-implementation-plan.md`
- Reference: `docs/plans/2026-03-12-windows-theme-min-repro-and-skia-removal-design.md`

**Step 1: Write the failing documentation delta**

Update `verification.md` by adding a new section stub for this work with unchecked items for:

- mainline Skia removal
- minimal repro compile verification
- Windows manual repro checklist

Keep it intentionally incomplete so the section still reflects work not yet verified.

**Step 2: Run verification commands to confirm the gap**

Run:

```bash
cargo test -q
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: if any command fails, stop and fix the code before writing final verification notes.

**Step 3: Write the final verification notes**

Update `verification.md` with:

- executed commands and pass/fail results
- note that GUI verification is still manual in this Linux environment
- a Windows repro checklist:
  - launch `cargo run --bin windows_theme_repro`
  - drag the window so the bottom exceeds the screen
  - toggle `Dark -> Light -> Dark`
  - observe whether the offscreen area repaints as a whole

**Step 4: Run verification again**

Run:

```bash
cargo test -q
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: PASS

**Step 5: Commit**

```bash
git add verification.md docs/plans/2026-03-12-windows-theme-min-repro-and-skia-removal-implementation-plan.md
git commit -m "docs: record repro verification plan"
```
