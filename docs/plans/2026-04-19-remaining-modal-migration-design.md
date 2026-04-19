# Remaining Modal Migration Design

**Context**

The first modal refactor wave unified the form-heavy asset and settings dialogs around the shared modal infrastructure (`BlockingModalShell`, `ModalHeaderBar`, `ModalBodyScrollArea`, `ModalFooterBar`, and shared modal tokens). Four older dialogs still keep bespoke internal chrome and should be migrated into the same design system:

- `assets-rename-modal`
- `assets-delete-confirm-modal`
- `ssh-host-key-confirm-modal`
- `sftp-remote-file-modal`

The target visual language remains the same as the previous wave: closer to Transfer Center / Resolve Transfer Conflict, desktop-native, compact, elevated, and calm across light and dark themes.

## Goals

- Move the remaining old dialogs onto the shared modal shell/chrome system.
- Keep close behavior and keyboard behavior fully unified with the topmost blocking modal flow.
- Preserve semantic differences between simple confirm dialogs and the editor-style remote file dialog.
- Add render coverage for both light and dark theme presentations of the migrated dialogs.

## Recommended Approach

Use a two-tier migration strategy on top of the existing shared primitives rather than introducing new shell abstractions in this wave.

### Tier 1: Confirm / lightweight form dialogs

Applies to:
- `assets-rename-modal`
- `assets-delete-confirm-modal`
- `ssh-host-key-confirm-modal`

These dialogs should adopt:
- `ModalHeaderBar`
- `ModalFooterBar`
- shared section surfaces via `DialogSectionCard`
- shared text input / banner components where needed

The goal is not to turn them into large multi-section forms, but to bring them into the same surface, spacing, radius, footer, and button hierarchy as the newer dialogs.

### Tier 2: Editor-style dialog

Applies to:
- `sftp-remote-file-modal`

This dialog should keep its editor-centric layout, but still migrate to:
- the same shared modal shell
- the same header/footer chrome
- shared token-backed status / error / action styling

Its body should remain more tool-like than form-like: path context, editing surface, status messaging, and save/cancel flow.

## Why not introduce `ConfirmDialogShell` / `EditorDialogShell` yet?

That abstraction may still be worthwhile later, but it would expand the scope of this wave. The current shared primitives are now expressive enough to migrate the remaining dialogs directly while avoiding another layer of infrastructure churn.

## Interaction Rules

- `ESC` remains handled by `BlockingModalShell`; migrated dialogs should not reintroduce divergent modal-level close handling.
- `X`, `ESC`, and Cancel/Reject actions should continue to route through the same close path for each dialog.
- Focus behavior should be dialog-appropriate:
  - rename -> text input
  - delete confirm -> safest actionable button row with explicit destructive hierarchy
  - host key confirm -> action row with clear trust/reject semantics
  - remote file modal -> editor input / content surface
- Focus restore remains delegated to the shared workspace restore path already added in the first wave.

## Styling Direction

### Confirm dialogs

- Use one primary section card rather than raw body copy on a flat background.
- Keep copy concise, but give the message surface enough structure to feel like a modern desktop dialog instead of a legacy admin prompt.
- Destructive actions should use the shared danger hierarchy rather than legacy accent-only emphasis.

### Remote file dialog

- Use the same header/footer shell, but treat the editor body as a calm elevated work surface.
- Status and error areas should use shared inline banners or equivalent token-backed surfaces.
- Light and dark themes should both clearly show the editor plane, path context, and footer action bar.

## Test Strategy

### Contract / structure tests

Extend existing modal smoke tests to assert that the remaining dialogs now rely on:
- `ModalHeaderBar`
- `ModalFooterBar`
- shared cards / fields / banners where applicable
- shared shell escape routing already wired through `AppWindow`

### Render tests

Extend `tests/assets_modal_render_spec.rs` with:
- visible footer/action checks for rename / delete / host-key / remote-file dialogs
- light/dark theme comparisons to prove the migrated shells remain legible in both modes
- targeted region checks for confirm-dialog section surfaces and remote-file editor body separation

The render tests should follow the current software-renderer pixel-region pattern instead of adding binary golden files to the repository.

## Expected Outcome

After this wave, all current blocking dialogs in scope should share the same design language, close behavior, spacing rhythm, footer structure, and theme behavior. Any future modal work can continue from the shared primitives instead of creating new one-off shells.
