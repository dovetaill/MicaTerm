# Modal Scroll Wheel-Only Design

## Goal

Remove the ability to scroll shared form-style modals by holding the left mouse button and dragging inside the modal body. These modals should only scroll via the mouse wheel or the scrollbar thumb.

## Approved Direction

- Disable left-button drag panning in the shared `ModalBodyScrollArea`.
- Keep the modal header drag behavior unchanged so users can still drag the dialog itself from the title bar.
- Keep wheel scrolling and scrollbar dragging unchanged.
- Apply the behavior consistently to every modal that uses the shared body scroll chrome instead of special-casing only the SSH connection editor.

## Root Cause

The shared scroll host in `ui/components/modal-chrome.slint` sets `mouse-drag-pan-enabled: true` on its internal `ScrollView`. Because the SSH connection editor reuses this shared body component, pressing the left mouse button inside the modal body and moving vertically pans the scroll view.

## Design Decision

### Change the shared scroll policy, not one modal

The interaction bug appears in the SSH modal, but the behavior is actually defined by shared modal chrome. Turning off drag-panning in the shared component keeps all long-form modals aligned with the same input model:

- wheel scroll for content traversal
- scrollbar drag for precise manual positioning
- header drag for moving the modal window

This avoids an inconsistent state where the SSH modal behaves differently from other shared form dialogs.

## Non-Goals

This change does not alter:

- modal layout or sizing
- keyboard focus behavior
- scrollbar visibility policy
- header drag-to-move behavior
- footer actions or validation messaging
- non-modal scroll containers that do not use `ModalBodyScrollArea`

## Testing Strategy

Lock the new behavior in the existing modal UI contract tests by asserting that shared modal chrome no longer enables mouse drag panning.

Relevant coverage should prove that:

- `ModalBodyScrollArea` still exists as the shared scroll container
- horizontal scrolling remains disabled
- direct drag-panning is no longer enabled
- SSH and other shared modals still point at the same shared body component

## Acceptance Criteria

The implementation is correct when:

- left-button dragging inside the SSH modal body no longer scrolls the content
- mouse wheel scrolling still works
- dragging the scrollbar thumb still works
- dragging from the modal header still moves the modal
- shared modal-body contract tests describe and enforce the new behavior
