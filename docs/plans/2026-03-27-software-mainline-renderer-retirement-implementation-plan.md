# Software Mainline Renderer Retirement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore the shipped mainline to `winit + software renderer` and remove all compile-time mainline bindings to `wgpu-28`, `DX12`, and `femtovg`.

**Architecture:** Keep the existing startup chain `main -> runtime_profile -> bootstrap`, but collapse renderer selection back to a single software-only mainline path. Remove the GPU-specific Cargo feature, selector logic, wrappers, and smoke contracts, while preserving historical renderer documents as archive-only references.

**Tech Stack:** Rust 2024, Cargo features, Slint 1.15.1 `backend-winit` + `renderer-software`, Bash smoke checks, `cargo test`, `cargo check`

---

### Task 1: Re-establish software as the only mainline renderer contract

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/runtime_profile.rs`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`

**Step 1: Write the failing tests**

- Change `tests/runtime_profile.rs` so it expects:
  - `AppRuntimeProfile::mainline().renderer_mode == RendererMode::Software`
  - `forced_backend() == Some("winit")`
  - `forced_renderer() == Some("software")`
  - `requires_wgpu_28() == false`
- Change source assertions so they reject `FemtoVgWgpu` mainline assumptions and require a software feature declaration in `Cargo.toml`.
- Change `tests/bootstrap_profile_smoke.rs` to assert the software-only runtime lock.

**Step 2: Run tests to verify they fail**

Run: `cargo test --test runtime_profile --test bootstrap_profile_smoke -q`

Expected: FAIL because current source still locks mainline to `femtovg-wgpu`.

**Step 3: Write minimal implementation**

- In `Cargo.toml`, restore `default = ["slint-renderer-software"]`
- Define `slint-renderer-software = ["slint/renderer-software"]`
- Remove `slint-renderer-femtovg-wgpu`
- Remove GPU-only `[patch.crates-io]` entries if nothing else requires them
- In `src/app/runtime_profile.rs`, change `RendererMode` to `Software` and make mainline report software-only properties

**Step 4: Run tests to verify they pass**

Run: `cargo test --test runtime_profile --test bootstrap_profile_smoke -q`

Expected: PASS

### Task 2: Remove GPU selector code from the startup path

**Files:**
- Modify: `src/main.rs`

**Step 1: Write the failing test**

- The updated `tests/bootstrap_profile_smoke.rs` from Task 1 acts as the red test.
- Add source assertions if needed to require `renderer_name("software".into())` and reject `wgpu_28`, `DX12`, and `femtovg-wgpu` in `src/main.rs`.

**Step 2: Run the test to verify it fails**

Run: `cargo test --test bootstrap_profile_smoke -q`

Expected: FAIL because `src/main.rs` still uses `wgpu_28` and DX12-specific selector code.

**Step 3: Write minimal implementation**

- Collapse `apply_renderer_selector()` to a single `BackendSelector::new()`
- Keep `backend_name("winit")`
- Force `renderer_name("software")`
- Remove all `#[cfg(feature = "slint-renderer-femtovg-wgpu")]` branches
- Remove all `wgpu_28`, `WGPUConfiguration`, and Windows DX12 logic

**Step 4: Run tests to verify they pass**

Run: `cargo test --test bootstrap_profile_smoke -q`

Expected: PASS

### Task 3: Remove live GPU wrappers, smoke contracts, and stale current-state docs

**Files:**
- Delete: `build-linux-x64-femtovg-wgpu.sh`
- Delete: `tests/build_linux_x64_femtovg_wgpu_script_smoke.sh`
- Delete: `tests/femtovg_wgpu_contract_smoke.sh`
- Modify: `readme.md`
- Modify: `verification.md`
- Modify: `apt-packages.md`

**Step 1: Write the failing smoke/doc expectations**

- Add or update tests/docs assertions so README and verification no longer describe GPU mainline.
- Use grep-based checks locally to confirm old references still exist before removal.

**Step 2: Run checks to verify they fail**

Run:
- `rg -n 'femtovg-wgpu|wgpu-28|DX12' readme.md verification.md apt-packages.md`
- `test -f build-linux-x64-femtovg-wgpu.sh`
- `test -f tests/femtovg_wgpu_contract_smoke.sh`

Expected: matches/files still exist.

**Step 3: Write minimal implementation**

- Delete the live wrapper and smoke files
- Rewrite README mainline sections to describe software-only mainline
- Rewrite verification sections that still claim GPU experimental/current behavior
- Update `apt-packages.md` so vendored femtovg patches are no longer described as current runtime requirements

**Step 4: Run checks to verify the live references are gone**

Run: `rg -n 'femtovg-wgpu|wgpu-28|DX12' readme.md verification.md apt-packages.md tests build-linux-x64-femtovg-wgpu.sh`

Expected: no live hits outside archive docs or deleted paths

### Task 4: Refresh lockfile/build health and verify terminal regressions did not reopen

**Files:**
- Modify: `Cargo.lock`
- Reference: `src/app/bootstrap.rs`
- Reference: `src/app/ssh/runtime.rs`
- Reference: `src/app/ssh/session_manager.rs`
- Reference: `tests/bootstrap_smoke.rs`
- Reference: `tests/ssh_session_manager_spec.rs`

**Step 1: Use existing regression tests as the red/green safety net**

- Keep the SSH memory-related tests already added in this workspace
- Re-run the exact terminal projection and session-manager tests after the renderer removal

**Step 2: Refresh dependencies**

Run: `cargo check`

Expected: lockfile or dependency resolution changes may be required after removing GPU features/patches.

**Step 3: Apply minimal follow-up changes**

- If `Cargo.lock` changes, keep only the dependency updates needed by software-only mainline
- Do not touch unrelated dirty files

**Step 4: Run verification**

Run:
- `cargo test --test runtime_profile --test bootstrap_profile_smoke -q`
- `cargo test --test ssh_session_manager_spec runtime_surface_snapshot_tracks_visible_rows_instead_of_single_placeholder_copy -- --exact`
- `cargo test --test bootstrap_smoke runtime_events_refresh_workspace_terminal_projection_after_opening_saved_asset -- --exact`
- `cargo test --test bootstrap_smoke workspace_terminal_input_callback_updates_active_session_surface -- --exact`
- `cargo test --test bootstrap_smoke bootstrap_projects_terminal_scrollback_state_into_window_properties -- --exact`
- `cargo check -q`

Expected: PASS

### Task 5: Perform final repository-level residue scan

**Files:**
- Reference: repo root

**Step 1: Run residue scan**

Run:
- `rg -n 'wgpu_28|wgpu-28|renderer-femtovg-wgpu|femtovg-wgpu|DX12' src tests readme.md verification.md apt-packages.md Cargo.toml`

**Step 2: Interpret results**

- Live product files should no longer contain these terms
- Archive docs under `docs/plans/` may still contain them
- `Cargo.lock` may retain historical package names until refreshed; if they remain after `cargo check`, inspect whether they are still referenced

**Step 3: Record any residual risk**

- If some references remain intentionally in archive docs, call that out explicitly in the final report
