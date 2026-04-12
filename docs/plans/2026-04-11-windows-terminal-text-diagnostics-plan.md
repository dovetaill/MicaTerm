# Windows Terminal Text Diagnostics Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Revert the no-op Windows visual-fit tweak and make the native text path diagnostically observable so we can prove whether spacing work is hitting DirectWrite, grayscale, or bitmap fallback.

**Architecture:** Keep terminal grid metrics untouched. Restore the previous DirectWrite enhanced-contrast behavior, then extend the Windows native text diagnostics snapshot with the missing fallback reason and add low-noise tracing only when the active text path or rendering params materially change.

**Tech Stack:** Rust, DirectWrite/Direct2D, native terminal diagnostics, cargo tests

---

### Task 1: Revert the temporary visual-fit tweak

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing test**
- Update the source-contract test so it expects the original `enhanced_contrast.max(0.65).min(1.0)` behavior again.

**Step 2: Run test to verify it fails**
- Run: `cargo test --test windows_native_text_renderer_contract_spec -q`
- Expected: FAIL because the source still contains the temporary clamp.

**Step 3: Write minimal implementation**
- Restore the original DirectWrite enhanced-contrast tuning in `tuned_directwrite_enhanced_contrast`.

**Step 4: Run test to verify it passes**
- Run: `cargo test --test windows_native_text_renderer_contract_spec -q`
- Expected: PASS.

### Task 2: Surface the real Windows text fallback reason in diagnostics

**Files:**
- Modify: `src/app/terminal_renderer/diagnostics.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/windows_frame.rs`
- Test: `tests/windows_terminal_diagnostics_spec.rs`

**Step 1: Write the failing test**
- Extend diagnostics tests to require a `fallback_reason` field on Windows text diagnostics plus a helper accessor.

**Step 2: Run test to verify it fails**
- Run: `cargo test --test windows_terminal_diagnostics_spec -q`
- Expected: FAIL because the field/helper do not exist yet.

**Step 3: Write minimal implementation**
- Add `fallback_reason` to the diagnostics snapshot, store the last fallback reason in Windows backend state, clear it when `directwrite-d2d` becomes active again, and expose it through a helper in `windows_frame.rs`.

**Step 4: Run test to verify it passes**
- Run: `cargo test --test windows_terminal_diagnostics_spec -q`
- Expected: PASS.

### Task 3: Add low-noise runtime tracing for real text-path evidence

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing test**
- Add a source-contract test that requires a dedicated logging helper and logs on path/params changes rather than noisy per-frame dumps.

**Step 2: Run test to verify it fails**
- Run: `cargo test --test windows_native_text_renderer_contract_spec -q`
- Expected: FAIL because no such helper exists yet.

**Step 3: Write minimal implementation**
- Track the last emitted text-path signature in backend state and `tracing::info!` only when `text_renderer_path`, `fallback_reason`, `pixel_geometry`, `rendering_params_source`, or `enhanced_contrast_per_mille` materially change.

**Step 4: Run tests and compile**
- Run: `cargo test --test windows_native_text_renderer_contract_spec -q`
- Run: `cargo test --test windows_terminal_diagnostics_spec -q`
- Run: `cargo check --workspace`
- Expected: PASS.
