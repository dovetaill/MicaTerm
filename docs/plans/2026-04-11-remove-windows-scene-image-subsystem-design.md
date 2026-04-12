# Remove Windows Scene-Image Subsystem Design

**Status:** Proposed

**Goal**

Delete the Windows `scene-image` terminal subsystem and all live runtime paths that still route to it, so the project keeps only the `retained-native-surface` Windows terminal implementation.

## Decision Summary

The recommended cleanup is a **one-shot runtime cleanup**:

- delete the Windows `scene-image` presenter/renderer implementation,
- delete the runtime/profile selection layer that only exists to switch between `scene-image` and `retained-native-surface`,
- retarget `WindowsSoftwareCompat` to the same retained-native subsystem,
- update tests, build scripts, and current docs so no live surface still describes `scene-image` as a supported Windows mode.

This should be a **deep deletion**, not a soft deprecation. The goal is not to keep dormant code behind an env var; the goal is to remove the subsystem as a supported Windows path.

## Cleanup Depth

Two cleanup depths are possible:

### Level A: Runtime-Clean

Delete all code, tests, build/profile switches, and current user-facing docs that expose `scene-image` as a live Windows subsystem.

This level is required.

### Level B: Grep-Clean Historical Rewrite

Also rewrite or delete old historical design/implementation docs in `docs/plans/` and `docs/wikis/` that mention `scene-image`.

This level is optional and should be handled carefully. Historical plans are implementation records, not live behavior. Rewriting them all creates churn and destroys project history. The recommended approach is:

- clean all live code/tests/docs,
- keep historical docs as archive material,
- only update historical docs if they are still linked as current guidance.

## What Must Be Deleted

### 1. Runtime/profile selection surfaces

These items exist only because Windows currently supports more than one terminal subsystem. If `scene-image` is removed, this layer should be simplified instead of left behind as empty abstraction.

**Delete or collapse:**

- `src/app/runtime_profile.rs`
  - `TerminalCompositionMode::SceneImage`
  - `TerminalSubsystemMode::SceneImage`
  - `runtime_terminal_subsystem_override()`
  - `packaged_terminal_subsystem_override()`
  - `MICA_TERM_TERMINAL_SUBSYSTEM=scene-image` parsing
  - `terminal_subsystem_mode()`
  - `terminal_subsystem_mode_label()`
  - any documentation comments that still describe rollback or verification through `scene-image`

**Retain, but simplify:**

- `AppBuildFlavor::WindowsSoftwareCompat`
  - keep this build flavor because it still describes the host renderer/package family,
  - but change its terminal subsystem from `SceneImage` to retained-native.

**Deep-clean recommendation:**

If only one Windows terminal subsystem remains, remove `TerminalCompositionMode` and `TerminalSubsystemMode` entirely rather than leaving one-value enums in place.

### 2. Bootstrap presenter routing

**Modify:**

- `src/app/bootstrap.rs`

**Delete:**

- `TerminalCompositionMode::SceneImage` branch in `build_workspace_terminal_presenter(...)`
- `build_scene_image_terminal_presenter()`
- the non-feature-gated `"scene-image terminal renderer is unavailable in this build"` error string

**Keep:**

- bitmap fallback only when the app truly does not have a native presenter available
- retained-native presenter as the only Windows native subsystem

### 3. Scene-image presenter/renderer implementation

**Delete:**

- `src/app/terminal_scene_image.rs`
- `src/app/mod.rs` export for `terminal_scene_image`

**Modify:**

- `src/app/terminal_presenter.rs`
  - remove `WindowsSceneImagePresenter`
  - remove `SceneImageTerminalRenderer` imports
  - remove `SceneImageCacheStats`
  - remove all `scene_image_*` cache-stat fields from `TerminalPresenterCacheStats`
  - remove any comments that still describe `scene-image` rollback or fallback behavior

### 4. Scene-image-specific DirectWrite/font tuning path

**Modify:**

- `src/app/terminal_font/windows_dwrite.rs`

**Delete:**

- `load_scene_image_font(...)`
- scene-image-specific bitmap render-profile split
- tests that assert scene-image font-loading behavior

**Keep:**

- the retained-native/native font-loading path
- shared font metric surfaces that are still used by retained-native

## What Should Stay

To avoid over-deleting, the following should remain unless there is a second explicit cleanup pass:

- `BitmapAtlasPresenter` in `src/app/terminal_presenter.rs`
  - this is not the same as the Windows `scene-image` subsystem,
  - it is still the generic bitmap presenter/fallback surface.
- Slint bitmap/image plumbing:
  - `workspace-session-surface-image`
  - `session-surface-image`
  - related image properties in `ui/app-window.slint`, `ui/shell/workspace-pane.slint`, and `ui/shell/terminal-session-host.slint`
  - these still back bitmap fallback behavior and should not be removed as part of the Windows `scene-image` cleanup alone.
- `TerminalRenderMode::Bitmap`
  - this is broader than Windows `scene-image` and should remain unless the entire bitmap fallback strategy is being removed.
- `WindowsSoftwareCompat`
  - keep it as a package/profile concept,
  - but reroute it to retained-native.

## Files That Must Be Audited

### Source files

- `src/app/runtime_profile.rs`
- `src/app/bootstrap.rs`
- `src/app/mod.rs`
- `src/app/terminal_presenter.rs`
- `src/app/terminal_scene_image.rs` (delete)
- `src/app/terminal_font/windows_dwrite.rs`

### Build/profile/docs surfaces

- `readme.md`
- `build-win-x64.sh`
- `build-win-x64-software.sh`

### Tests to delete or rewrite

**Delete:**

- `tests/terminal_scene_image_spec.rs`

**Update:**

- `tests/runtime_profile.rs`
- `tests/bootstrap_profile_smoke.rs`
- `tests/bootstrap_smoke.rs`
- `tests/terminal_scrollback_spec.rs`
- `tests/terminal_color_emoji_spec.rs`
- `tests/terminal_renderer_dwrite_spec.rs`
- `tests/windows_terminal_typography_defaults_spec.rs`
- `tests/windows_native_text_renderer_contract_spec.rs`
- `tests/terminal_runtime_perf_contract_spec.rs`
- `tests/native_terminal_surface_contract_spec.rs`
- `tests/windows_terminal_native_mode_contract_smoke.sh`

**Expected change pattern in tests:**

- remove assertions that `SceneImage` exists,
- remove assertions that `scene-image` env/package overrides exist,
- remove assertions that `WindowsSoftwareCompat` routes to `SceneImage`,
- remove assertions that scene-image-specific cache stats/diagnostics remain exposed,
- keep or strengthen assertions that retained-native is the only Windows subsystem.

### Current-doc audit targets

These should be updated if they are still treated as current guidance:

- `readme.md`
- `docs/plans/2026-04-11-default-retained-native-and-log-cleanup-plan.md`

These should usually stay as historical records unless we explicitly choose a grep-clean archive rewrite:

- older `docs/plans/*scene-image*`
- older `docs/plans/*windows-terminal*`
- `docs/wikis/2026-04-08-windows-working-set-trim-findings.md`

## Risks

### Risk 1: Windows software package regresses

`WindowsSoftwareCompat` currently still has code/tests/docs that assume `scene-image` is the software-compatible visible terminal path. Removing `scene-image` means the software package must now prove that retained-native works acceptably under the software host renderer path as well.

**Mitigation:**

- keep the package flavor,
- reroute the subsystem,
- explicitly validate the Windows software package after cleanup.

### Risk 2: Over-delete generic bitmap fallback surfaces

`scene-image` and generic bitmap fallback are related but not identical. Removing Slint image plumbing blindly would likely break the remaining bitmap fallback path.

**Mitigation:**

- delete only the Windows `scene-image` subsystem in this pass,
- keep generic bitmap surfaces unless a separate fallback-removal design is approved.

### Risk 3: Dead abstraction remains after code deletion

If `SceneImage` code is deleted but enums, labels, profile helpers, and diagnostics remain, the project will still carry conceptual dead weight.

**Mitigation:**

- perform deep deletion in `runtime_profile.rs`, not just branch removal in `bootstrap.rs`.

## Verification Standard

After cleanup, the project should meet all of the following:

### 1. Live-reference grep

Run:

```bash
rg -n "scene-image|SceneImage|terminal_scene_image|WindowsSceneImagePresenter" src tests readme.md build-win-x64.sh build-win-x64-software.sh
```

Expected:

- no matches in live source/tests/build scripts/readme,
- any remaining matches should be in clearly historical docs only.

### 2. Runtime/profile verification

Run:

```bash
cargo test --test runtime_profile --test bootstrap_profile_smoke -q
```

Expected:

- tests pass with retained-native as the only Windows subsystem,
- no test still expects `scene-image` env knobs or enum variants.

### 3. Native presenter contract verification

Run:

```bash
cargo test --test native_terminal_surface_contract_spec \
           --test windows_native_text_renderer_contract_spec \
           --test terminal_renderer_dwrite_spec \
           --test terminal_color_emoji_spec -q
```

Expected:

- retained-native-specific contracts pass,
- no scene-image-specific font/cache/contract assertions remain.

### 4. Full test/file audit for cleanup fallout

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_scrollback_spec -q
```

Expected:

- no runtime/profile regression remains from deleting the subsystem,
- no old packaged-default assumptions survive.

## Recommended Execution Rule

Perform this cleanup in one branch and one reviewable change-set. Do not split it into "delete implementation now, update tests/docs later". If the goal is code purity, the implementation, tests, scripts, and current docs must all be cleaned in the same pass.
