# Workspace Terminal Paste CRLF Normalization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ensure workspace clipboard paste normalizes `CRLF`/`CR` to `LF` before modal review and before terminal delivery, without changing logical paste-guard behavior beyond newline normalization.

**Architecture:** Keep the fix local to the workspace clipboard paste pipeline. Add one narrow normalization helper in `src/app/bootstrap/workspace_terminal.rs`, make clipboard-ingress and modal-confirm paths use it as the canonical payload, and lock the behavior with both unit tests and existing bootstrap smoke-test harnesses.

**Tech Stack:** Rust, Slint, existing bootstrap workspace terminal glue, bootstrap smoke tests, `cargo test`, `cargo fmt`

---

### Task 1: Freeze newline normalization behavior with failing tests

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `src/app/bootstrap/workspace_terminal.rs`
- Reference: `docs/plans/2026-05-14-workspace-terminal-paste-crlf-normalization-design.md`

**Step 1: Write the failing helper-focused unit tests**

Add tests in the existing `src/app/bootstrap.rs` test module covering:

```rust
#[test]
fn workspace_paste_text_normalization_preserves_intent() {
    assert_eq!(
        workspace_terminal::normalize_workspace_paste_text(
            "sudo apt update && \\\r\n  sudo apt install -y curl && \\\r\n  echo done\r\n"
        ),
        "sudo apt update && \\\n  sudo apt install -y curl && \\\n  echo done\n"
    );
    assert_eq!(
        workspace_terminal::normalize_workspace_paste_text("one\n\ntwo\n"),
        "one\n\ntwo\n"
    );
}
```

Also assert that:
- `"\\\r\nnext" -> "\\\nnext"`
- normalized result does not contain `"\\\n\n"`
- LF-only input stays identical

**Step 2: Write the failing modal confirm smoke test**

Add/extend a `tests/bootstrap_smoke.rs` case that seeds the clipboard with:

```text
sudo apt update && \
  sudo apt install -y curl && \
  echo done

```

Then assert before confirm:
- warning modal opens
- `app.get_workspace_paste_warning_text()` equals the LF-normalized form
- it does not contain `"\\\n\n"`

**Step 3: Write the failing editor-mode smoke test**

Use a 4-line CRLF payload to trigger editor mode and assert:
- modal opens in editor mode
- modal text is LF-normalized
- confirming sends LF-normalized paste payload

**Step 4: Write the failing no-prompt send-path smoke test**

Use a bracketed-paste-enabled launcher so the warning is skipped, then assert:
- modal does not open
- `take_paste_inputs()` receives LF-normalized text

**Step 5: Run the targeted tests and confirm RED**

Run:

```bash
cargo test workspace_multiline_paste_detection_normalizes_platform_line_endings -- --exact
cargo test workspace_terminal_multiline_paste_warning_normalizes_crlf_line_continuations -- --exact
cargo test workspace_terminal_editor_paste_warning_normalizes_crlf_before_send -- --exact
cargo test workspace_terminal_bracketed_paste_normalizes_crlf_without_warning -- --exact
```

Expected: FAIL because the current code still stores and sends raw clipboard text.

### Task 2: Implement the minimal normalization fix in the workspace paste pipeline

**Files:**
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/app/bootstrap.rs`

**Step 1: Add the normalization helper**

In `src/app/bootstrap/workspace_terminal.rs`, add or rename the helper so the canonical API is:

```rust
pub(super) fn normalize_workspace_paste_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}
```

If useful, keep `normalized_paste_newlines()` as a thin wrapper or replace its call sites directly, but leave only one normalization implementation.

**Step 2: Normalize at clipboard ingress**

In `forward_active_workspace_paste()`:
- read clipboard text
- normalize immediately
- use normalized text for prompt-mode checks, line counts, pending warning text, logging character count, and direct-send path

**Step 3: Normalize editor draft on confirm**

In `window.on_workspace_paste_warning_confirm_requested` within `src/app/bootstrap.rs`, normalize `draft_text` before sending when editor mode is active.

Keep confirm mode unchanged except that `pending.text` is already normalized.

**Step 4: Preserve all existing semantics**

Do not:
- trim text
- collapse blank lines
- remove indentation
- alter bracketed-paste marker behavior
- change threshold constants or threshold decision structure

**Step 5: Run the targeted tests and confirm GREEN**

Run:

```bash
cargo test workspace_multiline_paste_detection_normalizes_platform_line_endings -- --exact
cargo test workspace_terminal_multiline_paste_warning_normalizes_crlf_line_continuations -- --exact
cargo test workspace_terminal_editor_paste_warning_normalizes_crlf_before_send -- --exact
cargo test workspace_terminal_bracketed_paste_normalizes_crlf_without_warning -- --exact
```

Expected: PASS.

### Task 3: Refactor for clarity without changing behavior

**Files:**
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/app/bootstrap.rs`

**Step 1: Remove duplication around newline normalization calls**

If the code now repeats `normalize_workspace_paste_text(...)` in several places, keep the helper usage obvious and central. Prefer clear variable names like `normalized_text`.

**Step 2: Keep comments minimal and only where helpful**

If a comment is added, limit it to one short explanation near the ingress normalization or editor confirm normalization.

**Step 3: Re-run the focused tests**

Run the same targeted tests again and confirm they remain green.

### Task 4: Final verification and formatting

**Files:**
- Modify: only files changed above

**Step 1: Format the code**

Run:

```bash
cargo fmt
```

**Step 2: Run the requested verification suite**

Run:

```bash
cargo test workspace_paste_guard_spec
cargo test workspace_paste_warning_modal_spec
cargo test bootstrap_smoke workspace_terminal -- --nocapture
```

If `workspace_paste_guard_spec` does not exist, run the nearest workspace-paste coverage instead and report that substitution explicitly.

**Step 3: Verify the exact requirements checklist**

Confirm from test evidence and code review that:
- `\r\n` and bare `\r` normalize to `\n`
- `\\\r\n` never becomes `\\\n\n`
- LF-only input remains unchanged
- intentional blank lines remain intact
- modal text and terminal payload match
- no-prompt path also uses normalized payload
- bracketed-paste semantics are otherwise unchanged

**Step 4: Report results**

In the final handoff include:
- root cause
- modified files
- tests added/changed
- before/after payload examples
- exact verification commands run and their results
