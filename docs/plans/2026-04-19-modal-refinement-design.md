# Modal Refinement Design

## Goal

Repair the visual and layout regressions introduced by the modal shell unification so form dialogs regain a lighter Fluent/Windows desktop rhythm while preserving the shared modal stack, ESC dismissal, and token-driven consistency.

## Problem Summary

The current shared modal chrome solved behavior consistency, but it also introduced three regressions across form dialogs:

1. The body/footer primitives now add too much framing, so form modals read as nested cards instead of one elevated shell with organized content.
2. Form layouts still rely on ad-hoc fixed-height wrappers and local width guesses, which makes two-column rows feel compressed and inconsistent.
3. Shadow/glow tuning is too heavy in light theme, so both modal shells and transfer utility surfaces render as stacked gray edges rather than one clean elevated layer.

## Root Causes

### Shared chrome over-framing

`BlockingModalShell` already provides the elevated outer surface, but `ModalBodyScrollArea` and `ModalFooterBar` add another visible frame/action rail. This creates shell -> body panel -> section card -> field nesting in dialogs that should only need shell -> content -> controls.

### Form sections are using cards for structure rather than emphasis

The refactor moved most form groups into `DialogSectionCard`, even when the group only contains plain fields. That is appropriate for summaries, warnings, or decision blocks, but too heavy for standard input sections.

### Layout rules are implicit instead of systematic

Form rows were migrated with existing `HorizontalLayout` wrappers and fixed heights. They stretch after multiple layers of padding and card insets, so fields no longer feel optically aligned or intentionally proportioned.

### Elevation tokens are too broad and too opaque

The shell and utility panel shadows/glow use offsets and opacities that are readable as several stacked rectangles. The form dialogs need a cleaner, tighter elevation profile.

## Approved Repair Direction

### 1. Keep one strong shell, lighten everything inside

- Preserve `BlockingModalShell` as the only true elevated shell.
- Reduce shell shadow/glow to one clean far shadow + one lighter contact shadow + a restrained halo.
- Apply the same elevation cleanup to Transfer Center / Transfer Conflict so they stay in the same family.

### 2. Change body/footer primitives from "extra shells" to "layout scaffolding"

- `ModalBodyScrollArea` should default to an unframed content flow for form dialogs.
- `ModalFooterBar` should keep divider + padding + button alignment, but drop the heavy inner rail.
- Use cards only when content needs visual emphasis, not as the default structure.

### 3. Introduce a lighter form rhythm

- Standard form modal body structure:
  - Header
  - Optional intro copy
  - Direct form rows / grouped sections
  - Footer
- Use section titles and spacing for structure first.
- Reserve `DialogSectionCard` for summaries, warnings, auth detail blocks, or special sub-surfaces.

### 4. Normalize form row behavior

- Use consistent row gaps and vertical rhythm.
- Use explicit primary/secondary column proportions for split rows.
- Avoid fixed row heights; row containers should size from child field heights so helper/error text never crushes the grid.
- Keep short fields like port narrow but stable.

### 5. Replace slogan subtitles with product copy

- Form modal headers should use short natural titles.
- Subtitles become optional one-line helper copy and should often be omitted.
- Header height should feel compact and desktop-native, not promo-like.

## Scope

### Shared primitives

- `ui/components/blocking-modal-shell.slint`
- `ui/components/modal-chrome.slint`
- `ui/theme/tokens.slint`
- `ui/shell/transfer-center.slint`

### Form modals to rebalance

- `ui/components/assets-ssh-connection-modal.slint`
- `ui/components/assets-folder-create-modal.slint`
- `ui/components/assets-snippet-modal.slint`
- `ui/components/assets-keychain-ssh-key-modal.slint`
- `ui/components/assets-keychain-identity-modal.slint`
- `ui/components/settings-modal.slint`
- `ui/components/sync-vault-modal.slint`

### Verification

- `tests/assets_modal_smoke.rs`
- `tests/assets_modal_render_spec.rs`

## Non-Goals

- Do not change modal stack ownership or ESC semantics.
- Do not alter form business rules or backend behavior.
- Do not force form modals to visually match Transfer Center one-to-one.

## Success Criteria

- Form dialogs look like a single elevated shell with clear content, not nested cards.
- Two-column rows feel balanced and consistent, especially in SSH forms.
- Header/footer become lighter and more product-like.
- Shell and transfer surfaces keep a restrained halo and cleaner shadows in both themes.
- Existing modal dismissal/focus behavior remains intact.
