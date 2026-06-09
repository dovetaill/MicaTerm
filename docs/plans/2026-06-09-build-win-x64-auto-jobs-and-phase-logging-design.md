# Build Win x64 Auto Jobs and Phase Logging Design

Date: 2026-06-09
Author: Codex
Status: approved for implementation

## Goal

Make `./build-win-x64.sh` auto-select a sensible default `--jobs` value when the caller does not set `BUILD_JOBS`, and make build logs explicitly distinguish the broad parallel compile phase from the final root-crate compile/link phase.

## Scope

### In scope

- `build-win-x64.sh` auto-detects a default `BUILD_JOBS` value only when the caller did not provide one.
- `build-desktop.sh` keeps owning the shared `cargo`/`cargo xwin` execution path.
- `build-desktop.sh` prints clearer phase-oriented logs:
  - phase 1/3: parallel dependency compilation
  - phase 2/3: final crate compile + link
  - phase 3/3: package staging + archive
- Shell smoke tests and `readme.md` are updated to lock the behavior.

### Out of scope

- Changing default `BUILD_JOBS` behavior for every other wrapper.
- Changing Cargo profiles, `LTO`, `codegen-units`, `incremental`, or linker flags.
- Replacing `.zip` packaging or parallelizing packaging.
- Changing `build-win-x64-software.sh` semantics.

## Recommended approach

### 1. Keep auto-detection wrapper-local

`build-win-x64.sh` is the user-facing entrypoint that needs the new default. The shared `build-desktop.sh` should not silently change behavior for Linux/macOS or the Windows software wrapper.

When `BUILD_JOBS` is absent, the wrapper should probe in this order:

1. `nproc`
2. `getconf _NPROCESSORS_ONLN`
3. `NUMBER_OF_PROCESSORS`

On success it exports `BUILD_JOBS=<detected>` plus metadata describing the source, then delegates to `build-desktop.sh`.

### 2. Keep argument assembly centralized

`build-desktop.sh` already validates `BUILD_JOBS` and appends `--jobs` to the final cargo command. That remains the single place where the actual cargo argument list is built.

The only shared-layer change for jobs is improved logging so an auto-detected wrapper default can be shown as:

- `auto-detected 32 via nproc -> --jobs 32`

while explicit caller input remains:

- `BUILD_JOBS=16 -> --jobs 16`

### 3. Add phase-aware build logging at the real cargo execution point

The root cause of the user confusion is that the visible concurrency drops near the end of the cargo graph, especially once the root crate becomes the dominant critical path. The shared build script should therefore:

- announce phase 1 before invoking cargo
- stream cargo output through a small line watcher
- emit phase 2 once output indicates the root crate has started compiling or the progress meter has converged on the root crate
- emit phase 3 before staging/archive work begins

The phase-2 message should explicitly explain that visible parallelism may drop there even though the build is not "stuck".

## Testing strategy

- Extend `tests/build_jobs_script_smoke.sh` first.
- Keep the existing `build-desktop.sh` default behavior assertion unchanged.
- Add a new wrapper-default assertion showing `build-win-x64.sh` auto-detects jobs and forwards `--jobs N`.
- Add log assertions for:
  - phase 1/3
  - phase 2/3
  - phase 3/3
  - auto-detected jobs text

## Risks and mitigations

- Output matching can be fragile if cargo wording changes.
  - Mitigation: detect both `Compiling mica-term` style lines and progress-meter lines mentioning `mica-term`.
- Some hosts may not have `nproc`.
  - Mitigation: provide ordered fallbacks and only opt into auto-detection in the wrapper that needs it.
- Wrapper-injected `BUILD_JOBS` could hide whether the value was user-provided.
  - Mitigation: pass explicit source metadata alongside the numeric value.
