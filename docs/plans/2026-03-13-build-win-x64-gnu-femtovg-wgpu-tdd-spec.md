# Linux Host Windows GNU FemtoVG WGPU TDD Spec

Date: 2026-03-13
Source Plan: `docs/plans/2026-03-13-build-win-x64-gnu-femtovg-wgpu-implementation-plan.md`
Source Design: `docs/plans/2026-03-13-build-win-x64-gnu-femtovg-wgpu-design.md`

## Goal

Protect the Linux-host packaging route for the pure `femtovg-wgpu-experimental` Windows GNU build without changing the existing Windows MSVC experimental wrapper or the formal release flow.

## Core Contracts

### Wrapper Entry Point

- `build-win-x64-gnu-femtovg-wgpu.sh`

Expected invariants:

- host intent must stay Linux-host only
- target must stay `x86_64-pc-windows-gnu`
- `APP_NAME` must stay `mica-term-femtovg-wgpu-experimental`
- `BIN_NAME` must stay `mica-term`
- `CARGO_NO_DEFAULT_FEATURES` must stay `1`
- `CARGO_FEATURES` must stay `femtovg-wgpu-experimental`
- the wrapper must continue to delegate to `build-desktop.sh`
- help output must keep advertising:
  - `x86_64-pc-windows-gnu`
  - `--no-default-features`
  - `--features femtovg-wgpu-experimental`
  - `dist/mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-gnu-release.zip`

### Documentation Boundary

- `readme.md`

Expected invariants:

- `FemtoVG WGPU Experimental` must list `./build-win-x64-gnu-femtovg-wgpu.sh`
- README must state this is the Linux-host Windows GNU experimental path
- README must keep `./build-win-x64-femtovg-wgpu.sh` as the Windows-host MSVC experimental path
- README must keep `./build-release.sh` as the formal release path

### Existing Rust / Slint Surface

No new Rust `struct`, `trait`, Tokio actor, channel, or Slint callback was introduced by this change.

Expected invariants:

- existing experimental runtime selection remains unchanged
- no renderer fallback may be added through this wrapper
- no UI callback or `slint::invoke_from_event_loop` path is touched by this change set

## Existing Automated Coverage

- `tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh`
- `tests/build_win_x64_femtovg_wgpu_script_smoke.sh`
- `tests/build_win_x64_script_smoke.sh`
- `tests/build_release_script_smoke.sh`

## Recommended Next TDD Additions

1. Add a CI-capable packaging check for Linux hosts with `mingw-w64` installed.
2. Add a regression that verifies the staged archive name remains `mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-gnu-release.zip`.
3. Add a negative check that prevents `build-win-x64-gnu-femtovg-wgpu.sh` from drifting toward formal release semantics.
4. Add a documentation regression that asserts both GNU and MSVC experimental wrappers stay listed with distinct host requirements.

## Edge Cases

- Linux-host packaging depends on the installed Rust target `x86_64-pc-windows-gnu`
- Linux-host packaging also depends on a working `mingw-w64` toolchain and Windows zip packaging path
- the GNU wrapper must stay separate from `build-win-x64-femtovg-wgpu.sh`; merging their semantics would hide host-specific failures
- the wrapper must not introduce a software renderer fallback; this route is pure experimental `femtovg-wgpu-experimental`
- `build-release.sh` must remain formal-only even if more experimental wrappers are added later
- this change set did not add new Tokio channels or shared mutable state, so there is no new channel-blocking or data-race surface beyond the existing build pipeline

## Manual Verification Follow-Up

- on Linux, run `./build-win-x64-gnu-femtovg-wgpu.sh`
- confirm the environment has `rustup target add x86_64-pc-windows-gnu`
- confirm the environment has the required `mingw-w64` linker tools
- confirm the archive output path is `dist/mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-gnu-release.zip`
