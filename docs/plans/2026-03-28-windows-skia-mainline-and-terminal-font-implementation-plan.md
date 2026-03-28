# Windows Skia Mainline and Terminal Font Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `build-win-x64.sh` package a `winit-skia-software` Windows build by default, add `build-win-x64-software.sh` as the software compatibility package, preserve the vendored Slint Winit patch, and keep `SarasaTermSCNerd` embedded in the exe while registering it only when the terminal host is first shown.

**Architecture:** Keep the vendored `i-slint-backend-winit` patch exactly as-is, keep generic development builds on the current low-risk software default, and make Windows packaging wrappers explicitly inject the renderer/build flavor at compile time. Move `SarasaTermSCNerd` out of the `.slint` startup import path and into a one-time runtime registration helper built on Slint's shared Fontique collection.

**Tech Stack:** Rust 2024, Cargo features, Slint 1.15.1 (`renderer-software`, `renderer-skia`, `unstable-fontique-07`, `backend-winit-x11`, `unstable-winit-030`), Bash packaging wrappers, `cargo test`, `cargo check`

---

### Task 1: Introduce explicit packaged renderer/build flavors

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/main.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/runtime_profile.rs`
- Test: `tests/bootstrap_profile_smoke.rs`
- Test: `tests/panic_logging.rs`
- Test: `tests/logging_runtime.rs`

**Step 1: Write the failing tests**

- Update `tests/runtime_profile.rs` so it no longer assumes the only valid packaged renderer is software.
- Make it expect a source-level renderer enum that can describe both `Software` and `SkiaSoftware`.
- Make it expect `forced_renderer()` / startup logging to be derived from build-time packaging inputs instead of a hard-coded `"software"`.
- Update `tests/bootstrap_profile_smoke.rs`, `tests/panic_logging.rs`, and `tests/logging_runtime.rs` to reject hard-coded `winit-software` assumptions in `src/main.rs`, startup failure messages, and runtime metadata.

Example target assertions:

```rust
assert!(content.contains("option_env!(\"MICA_TERM_PACKAGE_RENDERER\")"));
assert!(!content.contains("renderer_name(\"software\".into())"));
assert!(message.contains("winit-skia-software") || message.contains("winit-software"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke --test panic_logging --test logging_runtime -q
```

Expected: FAIL because the current source still hard-codes `software` in `runtime_profile.rs`, `src/main.rs`, and startup/logging text.

**Step 3: Write minimal implementation**

- In `Cargo.toml`, add an explicit `slint-renderer-skia` feature alongside the existing software feature.
- Add `unstable-fontique-07` to the `slint` dependency feature list because Task 3 needs runtime font registration.
- In `src/app/runtime_profile.rs`, introduce build-time selection from compile-time env variables:

```rust
pub enum AppBuildFlavor {
    Development,
    WindowsMainline,
    WindowsSoftwareCompat,
}

pub enum RendererMode {
    Software,
    SkiaSoftware,
}

pub fn packaged() -> Self {
    match (
        option_env!("MICA_TERM_BUILD_FLAVOR"),
        option_env!("MICA_TERM_PACKAGE_RENDERER"),
    ) {
        (Some("windows-mainline"), Some("skia-software")) => { /* ... */ }
        (Some("windows-software-compat"), Some("software")) => { /* ... */ }
        _ => { /* development fallback */ }
    }
}
```

- In `src/main.rs`, stop hard-coding `"software"` and route the selector through the profile:

```rust
BackendSelector::new()
    .backend_name(profile.forced_backend().unwrap().into())
    .renderer_name(profile.forced_renderer().unwrap().into())
    .select()?;
```

- In `src/app/bootstrap.rs`, update startup failure text so it uses the selected renderer label instead of a baked `winit-software` string.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke --test panic_logging --test logging_runtime -q
```

Expected: PASS

### Task 2: Convert Windows packaging to shared Skia/software wrappers

**Files:**
- Modify: `build-desktop.sh`
- Modify: `build-win-x64.sh`
- Create: `build-win-x64-software.sh`
- Modify: `build-release.sh`
- Test: `tests/build_win_x64_script_smoke.sh`
- Create: `tests/build_win_x64_software_script_smoke.sh`
- Modify or Delete: `tests/build_win_x64_skia_script_smoke.sh`
- Test: `tests/build_release_script_smoke.sh`

**Step 1: Write the failing wrapper/smoke tests**

- Rewrite `tests/build_win_x64_script_smoke.sh` so it expects `build-win-x64.sh` to describe itself as the Windows Skia wrapper, not the software wrapper.
- Add `tests/build_win_x64_software_script_smoke.sh` to require a second wrapper named `build-win-x64-software.sh`.
- Replace the negative “Skia must not exist” contract in `tests/build_win_x64_skia_script_smoke.sh`; either delete it or rewrite it to assert the old experimental split no longer exists because Skia is now the mainline Windows wrapper.
- Update `tests/build_release_script_smoke.sh` so it no longer claims the release aggregator is “software-only” if the Windows leg now routes through Skia.

**Step 2: Run the smoke tests to verify they fail**

Run:

```bash
bash tests/build_win_x64_script_smoke.sh
bash tests/build_release_script_smoke.sh
```

Expected: FAIL because the current wrapper still describes itself as the software path and there is no `build-win-x64-software.sh`.

**Step 3: Write minimal implementation**

- Extend `build-desktop.sh` to accept packaging metadata without duplicating build logic:

```bash
PACKAGE_FLAVOR_SUFFIX="${PACKAGE_FLAVOR_SUFFIX:-}"
ARCHIVE_STEM="${APP_NAME}-${TARGET}-${PROFILE}${PACKAGE_FLAVOR_SUFFIX}"
```

- In `build-win-x64.sh`, export the Skia packaging settings before delegating:

```bash
export TARGET="${TARGET:-x86_64-pc-windows-gnu}"
export CARGO_NO_DEFAULT_FEATURES=1
export CARGO_FEATURES="slint-renderer-skia"
export MICA_TERM_BUILD_FLAVOR="windows-mainline"
export MICA_TERM_PACKAGE_RENDERER="skia-software"
export PACKAGE_FLAVOR_SUFFIX="-skia"
exec "$ROOT_DIR/build-desktop.sh" "$@"
```

- Create `build-win-x64-software.sh` with the same wrapper structure, but export:

```bash
export CARGO_NO_DEFAULT_FEATURES=1
export CARGO_FEATURES="slint-renderer-software"
export MICA_TERM_BUILD_FLAVOR="windows-software-compat"
export MICA_TERM_PACKAGE_RENDERER="software"
export PACKAGE_FLAVOR_SUFFIX="-software"
```

- Update `build-release.sh` so the Windows leg explicitly passes the Skia packaging env while Linux stays on the current software/dev path.

**Step 4: Run the smoke tests to verify they pass**

Run:

```bash
bash tests/build_win_x64_script_smoke.sh
bash tests/build_win_x64_software_script_smoke.sh
bash tests/build_release_script_smoke.sh
```

Expected: PASS

### Task 3: Defer Sarasa registration until the terminal host is shown

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/mod.rs`
- Create: `src/app/terminal_font.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/startup_font_memory_regression.rs`
- Create: `tests/terminal_font_registration_smoke.rs`

**Step 1: Write the failing tests**

- Update `tests/startup_font_memory_regression.rs` so it keeps rejecting global Sarasa `.slint` imports but no longer insists that the terminal font family must exclude Sarasa.
- Add a source-contract test `tests/terminal_font_registration_smoke.rs` that expects:
  - no `SarasaTermSCNerd-Regular.ttf` import in `ui/app-window.slint`
  - `terminal-font-family` in `ui/shell/terminal-session-host.slint` starts with `Sarasa Term SC Nerd`
  - `src/app/bootstrap.rs` calls `ensure_terminal_font_registered`
  - `src/app/terminal_font.rs` embeds `SarasaTermSCNerd-Regular.ttf` and uses `slint::fontique_07::shared_collection()`

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test startup_font_memory_regression --test terminal_font_registration_smoke -q
```

Expected: FAIL because there is no runtime font registration helper yet and the terminal font family is still the Iosevka-first fallback stack.

**Step 3: Write minimal implementation**

- Add `pub mod terminal_font;` to `src/app/mod.rs`.
- Create `src/app/terminal_font.rs` with a one-time registration helper:

```rust
use std::sync::{Arc, OnceLock};
use slint::fontique_07::{fontique, shared_collection};

static REGISTER_RESULT: OnceLock<Result<(), String>> = OnceLock::new();
static SARASA_BYTES: &[u8] = include_bytes!("../../ui/fonts/SarasaTermSCNerd-Regular.ttf");

pub fn ensure_terminal_font_registered() -> Result<(), String> {
    REGISTER_RESULT
        .get_or_init(|| {
            let blob = fontique::Blob::new(Arc::new(SARASA_BYTES.to_vec()));
            let mut collection = shared_collection();
            collection.register_fonts(blob, None);
            Ok(())
        })
        .clone()
}
```

- In `ui/shell/terminal-session-host.slint`, change the terminal family string to:

```slint
in property <string> terminal-font-family:
    "Sarasa Term SC Nerd, Iosevka Term, Cascadia Mono, Consolas, monospace";
```

- In `src/app/bootstrap.rs`, hook the lazy registration into `sync_workspace_session_state()` using the existing host-mode contract:

```rust
if state.workspace_session_host_mode() == "terminal" {
    let _ = crate::app::terminal_font::ensure_terminal_font_registered();
}
```

Do not move Sarasa back into any `.slint` import.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test startup_font_memory_regression --test terminal_font_registration_smoke -q
```

Expected: PASS

### Task 4: Update docs and source contracts to the new Windows mainline semantics

**Files:**
- Modify: `readme.md`
- Modify: `verification.md`
- Modify: `tests/window_theme_contract_smoke.sh`
- Reference: `tests/slint_backend_patch_contract_smoke.sh`

**Step 1: Write the failing doc/contract expectations**

- Update source/docs tests so they no longer reject `winit-skia-software` as a forbidden string in mainline packaging code.
- Make README and verification describe:
  - Windows mainline wrapper = Skia
  - Windows software wrapper = compatibility package
  - vendored Slint patch still active
  - Sarasa is embedded and lazy-registered for terminal mode

**Step 2: Run checks to verify they fail**

Run:

```bash
bash tests/window_theme_contract_smoke.sh
rg -n 'mainline software|winit \+ software|software renderer' readme.md verification.md
```

Expected: FAIL / stale matches because README, verification, and the theme contract still assume software is the only current Windows mainline.

**Step 3: Write minimal implementation**

- Update README sections that currently say:
  - shipped build entrypoints always resolve to `winit + software`
  - runtime profile is locked to software
- Update verification to explain the two Windows wrapper outputs and the lazy terminal font registration
- Relax `tests/window_theme_contract_smoke.sh` so it only rejects the removed experimental/recovery leftovers, not the now-legitimate `winit-skia-software` packaging path
- Leave `tests/slint_backend_patch_contract_smoke.sh` unchanged so the vendored patch remains protected

**Step 4: Run checks to verify they pass**

Run:

```bash
bash tests/window_theme_contract_smoke.sh
bash tests/slint_backend_patch_contract_smoke.sh
rg -n 'winit \+ software' readme.md verification.md
```

Expected: theme/patch smoke PASS, and only intentional software-compat mentions remain in docs.

### Task 5: Verification pass for packaging profile wiring and deferred font behavior

**Files:**
- Reference: repo root

**Step 1: Run the targeted verification set**

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke --test panic_logging --test logging_runtime -q
cargo test --test startup_font_memory_regression --test terminal_font_registration_smoke -q
bash tests/build_win_x64_script_smoke.sh
bash tests/build_win_x64_software_script_smoke.sh
bash tests/build_release_script_smoke.sh
bash tests/window_theme_contract_smoke.sh
bash tests/slint_backend_patch_contract_smoke.sh
cargo check -q
```

Expected: PASS

**Step 2: Perform residue scan**

Run:

```bash
rg -n 'build-win-x64-skia\\.sh|windows-skia-experimental|renderer_name\\(\"software\"\\.into\\)|Mica Term failed to initialize winit-software' src tests readme.md verification.md build-*.sh
```

Expected:

- no references to the removed `build-win-x64-skia.sh` experimental wrapper
- no hard-coded `renderer_name("software".into())` in `src/main.rs`
- no stale startup/logging text pinned to `winit-software` when the packaged renderer is selected dynamically

**Step 3: Record residual risks**

- Windows-on-Windows or cross-built Windows runtime memory still needs a real packaged smoke check outside this repo-only verification set
- If `Skia` reproduces the partial-visibility issue that software fixed with `present_existing_buffer()`, that requires a follow-up vendor patch and is intentionally out of scope for this task
