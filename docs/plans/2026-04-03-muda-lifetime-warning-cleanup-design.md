# Muda Lifetime Warning Cleanup Design

## Goal

Remove the noisy `mismatched_lifetime_syntaxes` warnings emitted while building
the vendored `muda` crate through both `build-win-x64.sh` and
`build-win-x64-software.sh`.

## Problem

Both Windows packaging wrappers eventually compile `vendor/muda`, and the crate
currently returns `Ref<MenuChild>` and `RefMut<MenuChild>` from
`vendor/muda/src/platform_impl/mod.rs`. Newer Rust linting warns that these
signatures hide an elided lifetime and should use `Ref<'_, MenuChild>` and
`RefMut<'_, MenuChild>` instead.

The warning is harmless at runtime, but it pollutes Windows build output and
makes it harder to spot real regressions.

## Constraints

- Keep behavior unchanged; this should be a source-level lint cleanup only.
- Fix the warning at the shared vendored source instead of silencing it in
  wrapper scripts.
- Preserve both Windows packaging entry points:
  - `./build-win-x64.sh`
  - `./build-win-x64-software.sh`

## Approaches Considered

### 1. Fix the vendored `muda` signatures

Change the two return types in `vendor/muda/src/platform_impl/mod.rs` to use the
explicit `'_` lifetime recommended by the compiler.

Pros:
- Removes the root cause.
- Keeps all build entry points clean.
- Zero expected runtime behavior change.

Cons:
- Touches vendored code.

### 2. Silence the lint in build scripts

Inject a lint suppression flag from the wrapper scripts or `build-desktop.sh`.

Pros:
- Fast to apply.

Cons:
- Hides the warning instead of fixing it.
- Risks suppressing unrelated warnings from future builds.
- Does not help other `cargo check` or IDE build paths.

### 3. Add `#[allow(mismatched_lifetime_syntaxes)]` inside `muda`

Keep the existing signatures and suppress the lint near the source.

Pros:
- Localized suppression.

Cons:
- Still hides a fixable warning.
- Leaves the source in an outdated form.

## Recommendation

Use approach 1.

This is the smallest real fix: update the two signatures exactly as the compiler
suggests, then verify that a targeted regression check and both Windows wrapper
builds no longer emit the warning.

## Design

### Source change

- Update `MenuItemKind::child` to return `Ref<'_, MenuChild>`.
- Update `MenuItemKind::child_mut` to return `RefMut<'_, MenuChild>`.

### Regression coverage

- Add a shell smoke test that runs targeted `cargo check -p muda` commands for
  Windows targets and fails if `mismatched_lifetime_syntaxes` appears.
- Use that check as the red-green gate before and after the source change.

### Verification

- Run the new smoke test directly.
- Run `./build-win-x64.sh`.
- Run `./build-win-x64-software.sh`.
- Confirm the original warning text no longer appears in fresh output.
