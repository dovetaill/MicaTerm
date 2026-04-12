# Remove Windows Scene-Image Subsystem Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the Windows `scene-image` terminal subsystem completely so the codebase keeps only the retained-native Windows terminal path.

**Architecture:** Delete the `scene-image` renderer and presenter, collapse runtime profile selection so Windows no longer exposes subsystem switching, and retarget `WindowsSoftwareCompat` to the same retained-native terminal subsystem. Keep generic bitmap fallback infrastructure intact, but remove all Windows-specific code, tests, scripts, and current docs that still treat `scene-image` as a supported subsystem.

**Tech Stack:** Rust, Slint, DirectWrite, retained-native terminal renderer, Cargo integration tests, shell grep audits.

---

### Task 1: Rewrite the source-contract tests to forbid live scene-image support

**Files:**
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`
- Modify: `tests/windows_terminal_native_mode_contract_smoke.sh`
- Reference: `docs/plans/2026-04-11-remove-windows-scene-image-subsystem-design.md`

**Step 1: Write the failing test updates**

Replace `scene-image`-positive assertions with negative assertions and retained-native-only expectations.

```rust
assert!(!content.contains("SceneImage"));
assert!(!content.contains("scene-image"));
assert!(!content.contains("MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM"));
assert!(!content.contains("MICA_TERM_TERMINAL_SUBSYSTEM"));
assert!(content.contains("RetainedNativeSurface"));
assert!(content.contains("WindowsSoftwareCompat"));
```

For the shell smoke test, replace the old grep with checks that the runtime profile no longer contains any `SceneImage` composition arm.

```bash
! grep -F 'SceneImage' src/app/runtime_profile.rs >/dev/null
! grep -F 'scene-image' src/app/runtime_profile.rs >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke -q
bash tests/windows_terminal_native_mode_contract_smoke.sh
```

Expected: FAIL because `src/app/runtime_profile.rs`, `src/app/bootstrap.rs`, and the build/profile scripts still expose `SceneImage` and `scene-image`.

**Step 3: Keep the failing assertions staged while implementing the code cleanup**

Do not weaken the test expectations. These tests should define the desired retained-native-only contract before source changes begin.

**Step 4: Re-run after later tasks**

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke -q
bash tests/windows_terminal_native_mode_contract_smoke.sh
```

Expected: PASS once the runtime profile and build-script cleanup is complete.

**Step 5: Commit**

```bash
git add tests/runtime_profile.rs tests/bootstrap_profile_smoke.rs tests/windows_terminal_native_mode_contract_smoke.sh
git commit -m "test: lock windows terminal to retained-native only"
```

### Task 2: Collapse runtime profile selection to a retained-native-only Windows subsystem

**Files:**
- Modify: `src/app/runtime_profile.rs:34-44`
- Modify: `src/app/runtime_profile.rs:105-125`
- Modify: `src/app/runtime_profile.rs:155-158`
- Modify: `src/app/runtime_profile.rs:182-213`
- Modify: `src/app/runtime_profile.rs:262-299`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/terminal_scrollback_spec.rs`

**Step 1: Write the failing runtime/profile contract updates**

Add or update assertions so the profile layer no longer exposes subsystem switching.

```rust
assert!(!content.contains("pub enum TerminalCompositionMode"));
assert!(!content.contains("pub enum TerminalSubsystemMode"));
assert!(!content.contains("MICA_TERM_TERMINAL_SUBSYSTEM"));
assert!(!content.contains("MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM"));
assert!(!content.contains("Some(\"scene-image\")"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test runtime_profile --test terminal_scrollback_spec -q
```

Expected: FAIL because `src/app/runtime_profile.rs` still defines the enums, override parsing, and `WindowsSoftwareCompat => SceneImage` routing.

**Step 3: Implement the runtime profile simplification**

Refactor `src/app/runtime_profile.rs` so Windows no longer exposes a second subsystem.

```rust
// Delete:
pub enum TerminalCompositionMode { ... }
pub enum TerminalSubsystemMode { ... }
fn runtime_terminal_subsystem_override() -> ...
fn packaged_terminal_subsystem_override() -> ...

// Keep packaged/profile selection, but remove subsystem selection entirely.
pub fn prefers_native_terminal_renderer(self) -> bool {
    matches!(self.terminal_render_mode, TerminalRenderMode::Native)
}
```

Implementation requirements:

- remove `SceneImage` and `RetainedNativeSurface` enums entirely,
- remove all parsing for `MICA_TERM_TERMINAL_SUBSYSTEM` and `MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM`,
- keep `WindowsSoftwareCompat` as a build flavor,
- keep `terminal_render_mode` and `native_present_path`,
- keep `WindowsSoftwareCompat` routed to native terminal rendering,
- update comments so they no longer mention rollback through `scene-image`.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test runtime_profile --test terminal_scrollback_spec -q
```

Expected: PASS with no remaining runtime-profile references to `SceneImage` or `scene-image`.

**Step 5: Commit**

```bash
git add src/app/runtime_profile.rs tests/runtime_profile.rs tests/terminal_scrollback_spec.rs
git commit -m "refactor: remove windows terminal subsystem switching"
```

### Task 3: Remove bootstrap routing and presenter plumbing for scene-image

**Files:**
- Modify: `src/app/bootstrap.rs:2045-2097`
- Modify: `src/app/bootstrap.rs:2887-2904`
- Modify: `src/app/terminal_presenter.rs:1-30`
- Modify: `src/app/terminal_presenter.rs:197-210`
- Modify: `src/app/terminal_presenter.rs:564-705`
- Modify: `src/app/mod.rs:23`
- Delete: `src/app/terminal_scene_image.rs`
- Delete: `tests/terminal_scene_image_spec.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/windows_native_text_renderer_contract_spec.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`

**Step 1: Write the failing contract updates**

Remove tests that explicitly require `WindowsSceneImagePresenter` or `terminal_scene_image`, and replace them with retained-native-only assertions.

```rust
assert!(!presenter_source.contains("WindowsSceneImagePresenter"));
assert!(!bootstrap_source.contains("build_scene_image_terminal_presenter"));
assert!(!bootstrap_source.contains("TerminalCompositionMode::SceneImage"));
assert!(!mod_source.contains("pub mod terminal_scene_image"));
```

Delete `tests/terminal_scene_image_spec.rs` entirely rather than leaving a dead test file around.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec --test terminal_color_emoji_spec -q
```

Expected: FAIL because bootstrap and presenter still route to `SceneImage`, and `terminal_scene_image` is still exported.

**Step 3: Delete the implementation and simplify bootstrap**

Refactor bootstrap so Windows native rendering only builds the retained-native presenter.

```rust
fn build_workspace_terminal_presenter(profile: AppRuntimeProfile) -> Result<(Box<dyn TerminalPresenter>, TerminalRenderMode)> {
    if profile.prefers_native_terminal_renderer() {
        return Ok((build_native_terminal_presenter()?, TerminalRenderMode::Native));
    }

    Ok((Box::new(BitmapAtlasPresenter::new()?), TerminalRenderMode::Bitmap))
}
```

Implementation requirements:

- remove `build_scene_image_terminal_presenter()` in both cfg branches,
- remove `WindowsSceneImagePresenter` from `src/app/terminal_presenter.rs`,
- remove scene-image imports and cache-stat fields from `TerminalPresenterCacheStats`,
- delete `src/app/terminal_scene_image.rs`,
- delete `pub mod terminal_scene_image;` from `src/app/mod.rs`,
- keep `BitmapAtlasPresenter` and `PresentedTerminalFrame::Bitmap` intact for generic bitmap fallback.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec --test terminal_color_emoji_spec -q
```

Expected: PASS with retained-native as the only Windows presenter path.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/terminal_presenter.rs src/app/mod.rs tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs tests/terminal_color_emoji_spec.rs
git rm src/app/terminal_scene_image.rs tests/terminal_scene_image_spec.rs
git commit -m "refactor: delete windows scene-image presenter"
```

### Task 4: Remove scene-image-specific DirectWrite and perf/cache contracts

**Files:**
- Modify: `src/app/terminal_font/windows_dwrite.rs:109-115`
- Modify: `src/app/terminal_font/windows_dwrite.rs:806-835`
- Modify: `src/app/terminal_presenter.rs:197-210`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/windows_terminal_typography_defaults_spec.rs`
- Modify: `tests/terminal_runtime_perf_contract_spec.rs`

**Step 1: Write the failing DWrite/perf test updates**

Replace scene-image-specific font/cache assertions with retained-native-only expectations.

```rust
assert!(!source.contains("load_scene_image_font"));
assert!(!source.contains("bitmap render profile"));
assert!(!source.contains("scene_image_mono_glyph_cache_entries"));
assert!(!source.contains("scene_image_working_pixels_bytes"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_renderer_dwrite_spec --test windows_terminal_typography_defaults_spec --test terminal_runtime_perf_contract_spec -q
```

Expected: FAIL because `load_scene_image_font(...)`, scene-image font tests, and scene-image cache stats still exist.

**Step 3: Implement the font/cache cleanup**

Refactor `src/app/terminal_font/windows_dwrite.rs` to keep only the native font-loading entrypoint.

```rust
pub fn load_native_font(&mut self, request: &FontRequest) -> Result<LoadedFont> {
    self.load_font_with_profile(request, FontRenderProfile::windows_native_default())
}
```

Implementation requirements:

- delete `load_scene_image_font(...)`,
- delete test cases that only prove the scene-image-specific bitmap profile,
- update the remaining baseline/metrics test to load through the retained-native path,
- remove `scene_image_*` fields from `TerminalPresenterCacheStats`,
- remove any perf/source-contract test that expects scene-image cache diagnostics to remain exposed.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_renderer_dwrite_spec --test windows_terminal_typography_defaults_spec --test terminal_runtime_perf_contract_spec -q
```

Expected: PASS with no remaining scene-image-specific DWrite or cache contracts.

**Step 5: Commit**

```bash
git add src/app/terminal_font/windows_dwrite.rs src/app/terminal_presenter.rs tests/terminal_renderer_dwrite_spec.rs tests/windows_terminal_typography_defaults_spec.rs tests/terminal_runtime_perf_contract_spec.rs
git commit -m "refactor: remove scene-image font and cache contracts"
```

### Task 5: Clean scripts, current docs, and run the full audit

**Files:**
- Modify: `build-win-x64.sh:1-24`
- Modify: `build-win-x64-software.sh:1-52`
- Modify: `readme.md:68-80`
- Modify: `docs/plans/2026-04-11-default-retained-native-and-log-cleanup-plan.md`
- Modify: `tests/bootstrap_profile_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing script/doc assertions**

Update the tests to reject any current-doc or build-script mention of `scene-image` as a live Windows path.

```rust
assert!(!content.contains("scene-image"));
assert!(!content.contains("MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM"));
assert!(content.contains("retained-native"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_profile_smoke --test bootstrap_smoke -q
rg -n "scene-image|SceneImage|terminal_scene_image|WindowsSceneImagePresenter" src tests readme.md build-win-x64.sh build-win-x64-software.sh
```

Expected: FAIL / produce matches because current docs and scripts still mention `scene-image` and the package subsystem env var.

**Step 3: Clean the scripts and current docs**

Implementation requirements:

- remove `MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM` from `build-win-x64.sh`,
- do not add it to `build-win-x64-software.sh`,
- rewrite `readme.md` so Windows docs describe retained-native as the only live subsystem,
- update the current cleanup plan doc so it no longer describes `scene-image` as a supported override,
- leave clearly historical design/archive docs untouched in this pass.

**Step 4: Run the full verification set**

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke --test bootstrap_smoke --test terminal_scrollback_spec -q
cargo test --test native_terminal_surface_contract_spec --test windows_native_text_renderer_contract_spec --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec --test terminal_runtime_perf_contract_spec -q
bash tests/windows_terminal_native_mode_contract_smoke.sh
rg -n "scene-image|SceneImage|terminal_scene_image|WindowsSceneImagePresenter" src tests readme.md build-win-x64.sh build-win-x64-software.sh
```

Expected:

- all listed tests PASS,
- grep returns no matches in live source/tests/scripts/readme.

**Step 5: Commit**

```bash
git add build-win-x64.sh build-win-x64-software.sh readme.md docs/plans/2026-04-11-default-retained-native-and-log-cleanup-plan.md tests/bootstrap_profile_smoke.rs tests/bootstrap_smoke.rs
git commit -m "docs: remove windows scene-image subsystem references"
```
