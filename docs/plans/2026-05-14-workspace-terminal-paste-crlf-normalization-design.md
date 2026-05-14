# Workspace Terminal Paste CRLF Normalization Design

**Date:** 2026-05-14

## Goal

Fix the workspace terminal paste pipeline so clipboard text containing Windows-style `CRLF` or legacy `CR` line endings is normalized to Unix `LF` before it is shown in the paste warning modal and before it is sent to the terminal session. This specifically fixes shell line-continuation payloads such as `\\\r\nnext` becoming an unintended blank line or broken continuation when pasted into Linux shells.

## Problem Summary

The repo already has `normalized_paste_newlines(text)` in `src/app/bootstrap/workspace_terminal.rs`, but that helper is currently only used for:

- multiline/logical line counting
- editor threshold character counting
- prompt-mode decisions

The actual clipboard payload still flows through most of the paste pipeline as raw text:

1. `forward_active_workspace_paste()` reads raw clipboard text.
2. `workspace_paste_prompt_mode()` inspects normalized text for guard decisions.
3. `PendingWorkspacePasteWarning.text` stores the original raw clipboard text.
4. `sync_workspace_paste_warning_modal_state()` mirrors that raw text into the modal/editor.
5. `workspace_paste_warning_confirm_requested` sends either the raw pending text or the edited modal draft.
6. `forward_workspace_session_paste()` forwards that text unchanged to `send_session_paste()`.
7. The terminal core only strips bracketed-paste markers; it does not normalize CRLF/CR.

This means the guard UI can count lines correctly while the real payload still contains `\r\n`, which some shells/terminals interpret as separate carriage return plus newline operations. For line continuation sequences such as `\\\r\n`, that can surface as an extra blank line or a broken continuation.

## Root Cause

The workspace paste flow applies newline normalization for analysis but not for the canonical paste payload.

In other words, the code has two effective representations of the same paste:

- a normalized representation used for decisions
- a raw representation used for display and delivery

That split is the bug.

## Constraints

The fix must **not**:

- merge shell continuation lines into one physical line
- trim the paste payload
- remove indentation
- remove user-intended blank lines
- change bracketed-paste behavior
- only fix the modal display while leaving the real `send_session_paste()` payload unchanged
- change prompt/editor threshold semantics beyond the newline normalization itself

## Design Options

### Option A: Normalize at workspace paste ingress and keep one canonical payload

Normalize the clipboard payload immediately after reading it from the system clipboard, then use that canonical text for:

- prompt-mode calculation
- logical line counting
- pending modal state
- modal/editor initial text
- confirm send path
- no-prompt direct send path

Also normalize the editor draft again on confirm as a defensive measure, because edited text comes back from the UI rather than directly from the original pending struct.

**Pros**
- Fixes both UI and actual terminal payload
- Keeps behavior local to workspace clipboard paste
- Preserves snippet paste and other non-clipboard senders unless they explicitly opt in
- Small, understandable change set

**Cons**
- Requires touching both ingress and confirm paths

### Option B: Normalize only in `forward_workspace_session_paste()`

Normalize all text right before `send_session_paste()`.

**Pros**
- One late-stage choke point for delivery

**Cons**
- Modal/editor can still display raw CRLF unless separately fixed
- Would also affect snippet-paste callers and any other non-clipboard caller of `forward_workspace_session_paste()`
- Makes it less obvious that the workspace paste guard is operating on the same canonical text it shows and sends

### Option C: Normalize in terminal core / paste encoding layer

Normalize in `encode_paste()` or deeper.

**Pros**
- Global low-level enforcement

**Cons**
- Too broad for this bug
- Harder to reason about scope and regressions
- Modal/editor still need separate handling
- Couples clipboard cleanup with terminal protocol encoding concerns

## Chosen Design

Choose **Option A**.

Introduce a single helper named `normalize_workspace_paste_text(text: &str) -> String` in `src/app/bootstrap/workspace_terminal.rs`. Its job is intentionally narrow:

- replace `\r\n` with `\n`
- replace bare `\r` with `\n`
- do nothing else

All workspace clipboard paste flows should use the normalized result as their canonical payload.

## Detailed Data Flow After the Fix

1. `forward_active_workspace_paste()` reads system clipboard text.
2. It immediately calls `normalize_workspace_paste_text()`.
3. The normalized text is used for:
   - `workspace_paste_prompt_mode()`
   - `workspace_paste_logical_line_count()`
   - `PendingWorkspacePasteWarning.text`
   - direct no-prompt send
4. `sync_workspace_paste_warning_modal_state()` keeps reflecting `pending.text`, which is now already normalized.
5. On confirm:
   - `Confirm` mode uses normalized `pending.text`
   - `Editor` mode normalizes `draft_text` again before send
6. `forward_workspace_session_paste()` continues forwarding text unchanged, because it now receives canonical LF-normalized text from the workspace clipboard flow.

## Test Strategy

### Unit coverage

Add focused tests near the existing bootstrap unit tests for the new helper:

- `CRLF` becomes `LF`
- bare `CR` becomes `LF`
- `LF`-only input is unchanged
- intentional blank lines are preserved
- `\\\r\n` never becomes `\\\n\n`

### Integration/smoke coverage

Extend the existing workspace paste smoke coverage in `tests/bootstrap_smoke.rs` to assert that:

- confirm-modal text is LF-normalized
- confirm-modal send payload is LF-normalized
- editor-modal text is LF-normalized
- editor-modal send payload is LF-normalized
- no-prompt bracketed-paste path still skips the warning, but now sends LF-normalized payload
- line counts and editor thresholds stay behaviorally the same for the same logical content

## UI Considerations

Current evidence suggests the modal UI is not the root cause. `workspace-paste-warning-modal.slint` binds `TextInput.text` directly to `root.paste-text` and does not appear to inject blank lines on its own.

If tests later show pending text is normalized but the modal still renders extra blank lines, then a follow-up inspection of the Slint `TextInput`/binding behavior is warranted. That is not the first fix path.

## Industry/Reference Notes

A current Ghostty discussion from April 3, 2026 reports the same symptom: pasting `CRLF` into a terminal can make `\r` and `\n` behave like two separate line breaks, while other terminals such as VS Code normalize to a single newline during paste. That aligns with treating ingress normalization as the mature behavior for shell-targeted paste payloads.

Reference:
- https://github.com/ghostty-org/ghostty/discussions/12080

## Files Expected To Change

- `src/app/bootstrap/workspace_terminal.rs`
- `src/app/bootstrap.rs`
- `tests/bootstrap_smoke.rs`
- possibly existing unit tests in `src/app/bootstrap.rs`

No UI file changes are expected unless tests prove the modal layer is still transforming text after ingress normalization.
