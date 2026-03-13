# APT Packages

This file records the apt-level system packages that were either actually installed during the recent Windows build work or are currently relevant to the build chain used by this repository.

## APT Packages Installed During This Windows Build Work

These packages are directly evidenced by the current repo notes and verification artifacts for the March 2026 Windows build work:

- `gcc-mingw-w64-x86-64-posix`
  - Installed so Linux hosts can provide `x86_64-w64-mingw32-gcc` for the `x86_64-pc-windows-gnu` build path.
- `llvm-19`
  - Installed for the Windows MSVC cross-target validation flow that needed `llvm-rc`.
- `clang-19`
  - Installed for the same MSVC cross-target validation flow that needed `clang`.

## Current APT Prerequisites For The Build Chain

These are the apt-managed packages that are currently useful for the active local build paths and their supporting tooling:

- `gcc-mingw-w64-x86-64-posix`
  - Required for `./build-win-x64.sh` when targeting `x86_64-pc-windows-gnu`.
- `zip`
  - Required by `build-desktop.sh` for Windows archive packaging.
- `libwayland-dev`
  - Required on Linux hosts so Slint's winit stack can resolve `wayland-client.pc`.
- `pkg-config`
  - Recommended host-side helper for resolving `wayland-client.pc` and similar native dependency probes on Debian/Ubuntu systems.
- `llvm-19`
  - Needed when validating the `x86_64-pc-windows-msvc` cross-target path from Linux.
- `clang-19`
  - Needed together with `llvm-19` for the same MSVC validation path.

## Non-APT Prerequisites

These are part of the current build flow, but they are not installed by `apt-get install` in this repository:

- `rustup target add x86_64-pc-windows-gnu`
- `rustup target add x86_64-pc-windows-msvc`
- `cargo`
- `rustup`

## Current Cargo-Managed Project Dependencies

These dependencies come from `Cargo.toml` and are managed by Cargo rather than apt:

- Core UI/runtime crates in the current tree include `slint` and the vendored patch `i-slint-renderer-femtovg`.

### Runtime Dependencies

- `anyhow = "1.0.102"`
- `directories = "5"`
- `serde = "1.0.228"` with `derive`
- `serde_json = "1"`
- `slint = "1.15.1"` with:
  - `std`
  - `backend-winit-x11`
  - `compat-1-2`
  - `unstable-winit-030`
- `tokio = "1.50.0"`
- `tracing = "0.1"`
- `tracing-appender = "0.2"`
- `tracing-subscriber = "0.3"` with `fmt`, `env-filter`
- `window-vibrancy = "0.7.1"`
- `windows-sys = "0.59"`

### Build Dependencies

- `slint-build = "1.15.1"`
- `winresource = "0.1.19"`

### Dev Dependencies

- `filetime = "0.2"`
- `i-slint-backend-testing = "1.15.1"`

### Vendored Patches

- `gpu-allocator`
  - Pinned through `[patch.crates-io]` to `vendor/gpu-allocator`
- `i-slint-renderer-femtovg`
  - Pinned through `[patch.crates-io]` to `vendor/i-slint-renderer-femtovg`

## Scope Notes

- This file is intentionally limited to the current repository and the Windows build work completed in March 2026.
- It is a documentation snapshot, not a lockfile.
- If the build chain changes, update this file and `install-apt-packages.sh` together.
