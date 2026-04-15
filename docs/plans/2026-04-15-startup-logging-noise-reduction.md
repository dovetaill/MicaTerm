# Startup Logging Noise Reduction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce Windows startup log noise by removing verbose initialization diagnostics while keeping high-value renderer selection and all warning/error diagnostics.

**Architecture:** Tighten the startup logging contract at the source by deleting low-value `info!` emissions from runtime/font/terminal initialization paths, then update the contract tests to assert the quieter behavior and preserve warning-oriented diagnostics. The final behavior should keep one useful renderer-selection summary plus any fallback, drift, or failure warnings/errors.

**Tech Stack:** Rust, tracing, cargo test

---

### Task 1: Lock the quieter logging contract in tests

**Files:**
- Modify: `tests/logging_runtime.rs`
- Modify: `tests/font_diagnostics_contract_spec.rs`
- Modify: `tests/ui_renderer_text_quality_contract_spec.rs`

**Step 1: Write the failing test**

Update the affected tests so they assert these noisy startup messages are absent:
- `initialized runtime profile`
- `resolved app root directories`
- `configured ui shell font fallback policy`
- `ui shell font resolution established`
- `ui text renderer configuration established`
- `terminal font resolution established`
- `initialized workspace terminal presenter`
- `native terminal font chain changed`
- `native terminal text renderer state changed`

Keep assertions that preserve warning/error-oriented diagnostics and the final renderer-selection summary.

**Step 2: Run test to verify it fails**

Run: `cargo test --test logging_runtime --test font_diagnostics_contract_spec --test ui_renderer_text_quality_contract_spec`
Expected: FAIL because the runtime source still emits the old startup `info!` logs and the source-contract tests still reference them.

**Step 3: Write minimal implementation**

Do not add new logging. Only remove or stop calling the noisy startup `info!` paths that the updated tests reject.

**Step 4: Run test to verify it passes**

Run: `cargo test --test logging_runtime --test font_diagnostics_contract_spec --test ui_renderer_text_quality_contract_spec`
Expected: PASS.

### Task 2: Remove noisy startup diagnostics without losing useful signals

**Files:**
- Modify: `src/app/logging/runtime.rs`
- Modify: `src/app/font_diagnostics.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`

**Step 1: Write the failing test**

Use the updated tests from Task 1.

**Step 2: Run test to verify it fails**

Run: `cargo test --test logging_runtime --test font_diagnostics_contract_spec --test ui_renderer_text_quality_contract_spec`
Expected: FAIL.

**Step 3: Write minimal implementation**

- Remove the runtime-profile and app-root metadata emitters or make them no-ops.
- Stop emitting the startup font-diagnostics `info!` records.
- Stop emitting the workspace terminal presenter initialization `info!` record.
- Stop emitting the native terminal font-chain/text-renderer state `info!` records.
- Keep `selected renderer backend` and existing `warn!`/`error!` paths intact.

**Step 4: Run test to verify it passes**

Run: `cargo test --test logging_runtime --test font_diagnostics_contract_spec --test ui_renderer_text_quality_contract_spec`
Expected: PASS.
