# Validation and Handoff

Last updated: 2026-08-07

## Scope

This handoff covers task `07-30-clipboard-image-preview-safe-completion` on
`feat/image-dual-channel`.

- Track A adds request-keyed clipboard PNG upload progress, percentage, and
  monotonic-average speed to ordinary Paste.
- Track B adds explicit local terminal-grid display through `Ctrl+Shift+I` and
  `Display Clipboard Image`.
- Ordinary Paste remains upload-and-remote-path insertion.
- Local display has no PTY, SSH, SFTP, remote command, or remote-helper side
  effect.

The implementation was merged before physical Windows acceptance. Keep the
Trellis task active until the manual checklist below is completed.

## Commit Gate Rerun: 2026-08-07

The complete automated gate was rerun immediately before committing:

| Command group | Actual result |
| --- | --- |
| Format, diff, and Trellis validation | All exited 0 |
| Default `cargo check --all-targets` | Exit 0 in 14.9s |
| Strict Clippy | Expected baseline failure: exit 101, 31 library diagnostics / 35 library-test diagnostics |
| Non-strict Clippy | Exit 0 in 38.7s |
| Complete Linux test command | Exit 0 in 63.9s; 1,986 passed, 0 failed, 1 filtered |
| Four renderer feature combinations | All exited 0; each completed in 12.8-14.5s |
| Windows GNU all-targets | Exit 0 in 15.3s |
| Windows MSVC xwin all-targets | Exit 0 in 15.4s |

Warnings were the documented repository/test-helper baseline plus the vendored
Skia `unused_mut` warning. No new failure appeared during the commit gate.

## Automated Validation Commands

Run all commands from the repository or feature-worktree root.

### Structural checks

| Command | Expected/recorded result |
| --- | --- |
| `cargo fmt --all -- --check` | Exit 0 |
| `git diff --check` | Exit 0 |
| `python3 ./.trellis/scripts/task.py validate 07-30-clipboard-image-preview-safe-completion` | Exit 0 |

### Focused suites

| Command | Recorded result |
| --- | --- |
| `cargo test --lib clipboard::tests -- --nocapture` | 14 passed |
| `cargo test --lib clipboard_image_paste::tests -- --nocapture` | 16 passed |
| `cargo test --lib clipboard_inline_image::tests -- --nocapture` | 6 passed |
| `cargo test --test sftp_runtime_spec clipboard_upload -- --nocapture` | 2 passed |
| `cargo test --test terminal_inline_image_spec -- --nocapture` | 14 passed |
| `cargo test --test ssh_terminal_interaction_spec -- --nocapture` | 31 passed |
| `cargo test --test terminal_atlas_renderer_spec -- --nocapture` | 15 passed |
| `cargo test --test native_terminal_surface_contract_spec -- --nocapture` | 44 passed |
| `cargo test --test bootstrap_smoke clipboard_image -- --nocapture` | 3 passed |

Focused total: 145 passed, 0 failed, 0 skipped.

### Complete Linux gate

| Command | Recorded result |
| --- | --- |
| `cargo check --all-targets` | Exit 0 |
| `cargo clippy --all-targets --no-deps -- -D warnings` | Exit 101 from the repository warning baseline; see Remaining Issues |
| `cargo clippy --all-targets --no-deps` | Exit 0; no warning points to a newly added implementation line |
| `cargo test --all-targets --quiet -- --skip bundled_font_assets_cover_terminal_and_shell_contracts` | 1,986 passed, 0 failed, 1 intentionally filtered |

The complete test run may print an `xdg-open` error because the headless Linux
environment has no browser/opener. The command still exits 0; this is unrelated
to clipboard image behavior.

### Renderer feature matrix

All four commands must exit 0:

```bash
cargo check --no-default-features --features slint-renderer-software --all-targets
cargo check --no-default-features --features slint-renderer-skia --all-targets
cargo check --no-default-features --features slint-renderer-software,terminal-native-renderer --all-targets
cargo check --no-default-features --features slint-renderer-skia,terminal-native-renderer --all-targets
```

The bitmap-only variants previously exposed feature-boundary drift in the mock
font import, presenter frame import, Windows presenter stub, and native-only test
gating. Those boundaries were aligned and all four combinations passed.

### Windows cross-compilation

Both commands must exit 0:

```bash
cargo check --target x86_64-pc-windows-gnu --all-targets
cargo xwin check --target x86_64-pc-windows-msvc --all-targets
```

These checks compile Windows clipboard acquisition and Slint bindings, but they
do not replace the physical Windows acceptance checklist.

## Windows Manual Acceptance Still Required

- [ ] At a Bash prompt containing `img=`, capture with `Win+Shift+S`, press
      `Ctrl+Shift+V` once, and confirm preview plus bytes/total/percentage/speed.
      For a fast upload, confirm final measurements remain for about 3.2 seconds.
      Confirm only a quoted path is inserted, with no newline.
- [ ] Run `file "$img"` and
      `stat -c 'file-mode=%a bytes=%s path=%n' "$img"`; verify a valid PNG,
      file mode 0600, and session cache directory mode 0700.
- [ ] Start another image Paste, type before upload completes, and confirm no
      delayed path enters the modified input. Exercise `Paste path` and
      `Copy path` from the stale card.
- [ ] Invoke both `Ctrl+Shift+I` and `Display Clipboard Image`. Confirm identical
      local placement, no upload preview, no new remote cache file, no remote
      traffic, and a cursor at column zero below the image.
- [ ] Confirm a small image is not enlarged. Near the right edge, test wide and
      tall images against cursor-right width and half-viewport height, with no
      visible aspect distortion beyond cell rounding.
- [ ] Print enough lines to move the local image into scrollback. Verify it in
      native and bitmap presenter modes, then clear/evict history and confirm the
      application remains stable.
- [ ] Enter alternate-screen/application-cursor mode with
      `printf '\033[?1049h\033[?1h'`; verify both inline actions show local
      rejection feedback and send no terminal bytes. Restore with
      `printf '\033[?1l\033[?1049l'` and repeat inside a mouse-reporting TUI.
- [ ] Start display on session A, switch A -> B -> A before preparation finishes,
      and confirm the old result never appears. Start two displays rapidly and
      confirm only the newer pending request appears.
- [ ] Verify ordinary one-line text paste, multiline confirmation, and bracketed
      paste behavior remain unchanged.
- [ ] Re-run the direct Kitty RGBA blue-block fixture. Delete its Kitty ID and
      confirm protocol-owned content is removed without affecting a separately
      displayed local clipboard image.

## Remaining Issues and Follow-up

1. Physical Windows manual acceptance is pending. This is the only unchecked
   implementation-plan step and the reason the Trellis task remains active.
2. Strict Clippy is blocked by the repository baseline: 31 library diagnostics
   (35 when the library test target is included), including existing complexity,
   argument-count, collapsible-control-flow, and test-style warnings. Non-strict
   Clippy exits 0. Do not attribute this baseline to the clipboard image change.
3. The bundled-font asset contract is deliberately filtered from the full Linux
   gate by the implementation plan. Run it separately when the required bundled
   asset environment is available.
4. Headless Linux can emit expected `xdg-open` noise during the full suite even
   though the suite exits 0.
5. The untracked `.superpowers/` directory is unrelated workspace state. It is
   intentionally excluded from commits and prevents automatic worktree removal
   unless it is handled separately.

## Resume Guide

```bash
cd /home/wwwroot/mica-term
git log --graph --oneline --decorate -12
git status --short

cd /home/wwwroot/mica-term/.worktrees/image-dual-channel
sed -n '941,1080p' .trellis/tasks/07-30-clipboard-image-preview-safe-completion/implement.md
```

After the Windows checklist passes, update the remaining checkbox in
`implement.md`, update the PRD manual-acceptance checkbox and Implementation
Status, rerun the structural/full gates, then archive the Trellis task and record
the session journal. Do not archive it before that evidence exists.

## Integration Record

- Source branch: `feat/image-dual-channel`
- Target branch: `master`
- Merge strategy: non-fast-forward merge because both branches diverged from
  `40c71d9`.
- Work commit: `9c074fc` (`feat: add clipboard image progress and local display`).
- Documentation commit: `e03de03` (`docs: record clipboard image validation handoff`).
- Merge commit: `1437d21` (`Merge branch 'feat/image-dual-channel'`).
- The merge completed without conflicts. A post-merge check/test result is
  recorded below before the integration-record follow-up commit.

## Post-Merge Verification

Run from `/home/wwwroot/mica-term` after merge:

```bash
cargo fmt --all -- --check
git diff --check
python3 ./.trellis/scripts/task.py validate 07-30-clipboard-image-preview-safe-completion
cargo check --all-targets
cargo test --all-targets --quiet -- --skip bundled_font_assets_cover_terminal_and_shell_contracts
```

Actual results:

| Check | Result |
| --- | --- |
| Format, diff, and Trellis validation | All exited 0 |
| `cargo check --all-targets` | Exit 0 in 49.4s |
| Final complete Linux test command | Exit 0 in 63.5s; 1,986 passed, 0 failed, 1 filtered |

### Merge-time semantic documentation issue

The first post-merge full test exposed a semantic conflict that Git could not
detect textually. Master commit `1c48973` had modernized `readme.md` while the
feature branch added source-contract tests for current terminal runtime facts.
The new README had removed these still-required facts:

- shipped WezTerm core and Rio reference boundaries;
- retained-native Windows package/presenter and fallback diagnostics;
- packaged memory-baseline playbook and memory diagnostic fields;
- bundled UI/terminal font identities and the Linux/macOS follow-up status.

The failures appeared in `bootstrap_profile_smoke`,
`memory_baseline_contract_spec`, `terminal_memory_diagnostics_contract_spec`,
`startup_font_memory_regression`, and `terminal_scrollback_spec`. The modern
README layout was retained; concise current facts and the existing playbook link
were restored under Development Notes. All five focused targets then passed,
followed by the final 1,986-test all-target run above.
