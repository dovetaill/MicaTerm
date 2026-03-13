# FemtoVG WGPU Experimental TDD Spec

Date: 2026-03-13
Source Plan: `docs/plans/2026-03-12-femtovg-wgpu-experimental-implementation-plan.md`
Source Design: `docs/plans/2026-03-12-femtovg-wgpu-experimental-design.md`

## Goal

Add regression coverage for the experimental `winit-femtovg-wgpu` build route without changing the formal release semantics.

## Core Contracts

### Runtime Types

- `AppBuildFlavor`
  - `Formal`
  - `FemtoVgWgpuExperimental`
- `RendererMode`
  - `Software`
  - `FemtoVgWgpu`
- `AppRuntimeProfile`
  - `formal()`
  - `femtovg_wgpu_experimental()`
  - `is_experimental()`
  - `forced_backend()`
  - `forced_renderer()`
  - `requires_wgpu_28()`
  - `requires_backend_lock()`

Expected invariants:

- formal profile must stay on `RendererMode::Software`
- experimental profile must force `Some("winit")`
- experimental profile must force `Some("femtovg-wgpu")`
- experimental profile must require `wgpu_28`
- no contract may reintroduce `SkiaExperimental` or `winit-skia-software`

### Entry Point

- `src/main.rs`
  - `select_runtime_profile()` decides build flavor from Cargo feature
  - `apply_renderer_selector(profile)` must run before any `AppWindow::new()`

Expected invariants:

- experimental builds must call `slint::BackendSelector::new()`
- selector must set `backend_name("winit".into())`
- selector must set `renderer_name("femtovg-wgpu".into())`
- selector must call `require_wgpu_28(WGPUConfiguration::default())`
- runtime selection must not use `SLINT_BACKEND`

### Bootstrap / UI Identity

- `src/app/bootstrap.rs`
  - `runtime_window_title(profile)`
  - `startup_failure_message(profile, err)`
  - `run_with_profile(profile)`
- `ui/app-window.slint`
  - `in property <string> window-title`
  - `title: root.window-title;`

Expected invariants:

- experimental title must be `Mica Term [FemtoVG WGPU Experimental]`
- experimental startup failure text must mention `winit-femtovg-wgpu`
- `run_with_profile()` must push the runtime title into the Slint window before `run()`

### Logging

- `src/app/logging/runtime.rs`
  - `emit_runtime_profile_metadata(profile)`

Expected invariants:

- log metadata must include:
  - `build_flavor`
  - `renderer_mode`
  - `forced_backend`
  - `forced_renderer`
  - `wgpu_28_required`

### Build / Packaging

- wrapper scripts:
  - `build-linux-x64-femtovg-wgpu.sh`
  - `build-win-x64-femtovg-wgpu.sh`
- formal aggregator:
  - `build-release.sh`

Expected invariants:

- experimental wrappers must always set:
  - `CARGO_NO_DEFAULT_FEATURES=1`
  - `CARGO_FEATURES=femtovg-wgpu-experimental`
  - `BIN_NAME=mica-term`
- Linux experimental target must be `x86_64-unknown-linux-gnu`
- Windows experimental target must be `x86_64-pc-windows-msvc`
- `build-release.sh` must not mention experimental routes

## Existing Automated Coverage

- `tests/runtime_profile.rs`
- `tests/bootstrap_profile_smoke.rs`
- `tests/femtovg_wgpu_contract_smoke.sh`
- `tests/top_status_bar_smoke.rs`
- `tests/panic_logging.rs`
- `tests/logging_runtime.rs`
- `tests/build_linux_x64_femtovg_wgpu_script_smoke.sh`
- `tests/build_win_x64_femtovg_wgpu_script_smoke.sh`
- `tests/build_release_script_smoke.sh`

## Recommended Next TDD Additions

1. Add a targeted regression test around `select_runtime_profile()`
   - expose or refactor the function if necessary so tests can assert feature-gated selection directly
2. Add a focused test around `apply_renderer_selector(profile)`
   - verify formal builds no-op
   - verify experimental builds fail loudly when selector setup cannot succeed
3. Add a build-script regression around the Windows resource path
   - document the required host tools for `x86_64-pc-windows-msvc`
4. Add a packaging regression that checks stage/archive naming remains `mica-term-femtovg-wgpu-experimental-*`
5. Add a negative test that formal release docs and scripts never mention experimental entrypoints

## Slint / UI Surfaces To Protect

- `AppWindow::set_window_title(...)` is now part of the runtime identity bridge
- `window-title` in `ui/app-window.slint` must remain bound at runtime
- existing callbacks remain part of the window shell surface and must not regress:
  - `drag-requested`
  - `drag-resize-requested`
  - `drag-double-clicked`
  - `minimize-requested`
  - `maximize-toggle-requested`
  - `close-requested`
  - `toggle-theme-mode-requested`
  - `toggle-window-always-on-top-requested`
  - `toggle-assets-sidebar-requested`
  - `sidebar-destination-selected`

## Edge Cases

- selector ordering is critical: any future change that creates a Slint window before `apply_renderer_selector(profile)` breaks the experimental route
- no `software` fallback is allowed once the experimental profile is selected
- failure messages must stay explicit; silent exit or generic `startup failed` text is not acceptable
- runtime identity must remain visible in at least title, stderr/logging, and wrapper output
- `build-release.sh` must remain formal-only even if more experimental wrappers are added later
- Linux host builds for the experimental feature require Wayland development files because Slint's winit stack still resolves `wayland-client.pc`
- Windows MSVC cross-target validation currently depends on:
  - installed Rust target `x86_64-pc-windows-msvc`
  - `llvm-rc`
  - `clang`
  - the local `[patch.crates-io] gpu-allocator` override
- this feature did not add new Tokio actors, channels, or cross-thread UI dispatch paths; there is no new channel-blocking or data-race surface introduced by this change set

## Manual Verification Follow-Up

### Linux

- run `cargo run --no-default-features --features femtovg-wgpu-experimental`
- confirm title, resize, maximize, restore, and theme toggle
- capture stderr and `logs/system-error.log` if startup fails

### Windows

- run `cargo run --target x86_64-pc-windows-msvc --no-default-features --features femtovg-wgpu-experimental`
- run the packaged wrapper from a real Windows MSVC / Git Bash environment
- confirm title, resize, maximize, restore, and theme toggle
- confirm failing startup emits explicit stderr and logging output
