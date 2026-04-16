# Duplicate SSH Tab Titles And Transfer Error Details Design

## Context

The session manager already owns duplicate SSH tab numbering and gap reuse. The remaining work is to verify the UI path keeps rendering those resolved titles, and to make transfer failures/conflicts legible without turning the transfer center into a heavy workspace.

## Decisions

1. Session titles stay single-sourced from `SessionHandle.title`; do not add a second numbering layer in workspace projection.
2. Add UI-level regression coverage for duplicate SSH tabs so `name`, `name(2)`, `name(3)` and gap reuse are proven end-to-end.
3. Extend `TransferCenterItem` with lightweight error presentation fields and show them only for `Failed` / `Conflict` rows.
4. Keep transfer-center interaction lightweight: one inline truncated error line plus hover tooltip for the full message.

## Data Flow

- `SessionManager::open_session(...)` keeps resolving display titles.
- Workspace/bootstrap continues projecting `tab.title` directly into `WorkspaceTabItem`.
- `TransferTask.error_message` is projected in `shell_chrome` into `error_summary` and `error_tooltip`.
- `TransferCenter` owns a small hover tooltip state and `AppWindow` renders a dedicated overlay, reusing the existing tooltip component.

## UX Rules

- No click-to-expand rows, drawers, or modals for transfer errors.
- Only rows with actionable problems surface error text/tooltip.
- Running/completed rows stay visually compact.
- Cancelled rows remain muted unless later requirements say otherwise.

## Verification

- Bootstrap/UI regression for duplicate SSH tab labels and suffix reuse.
- Transfer-center smoke/bootstrap tests for inline failed/conflict summaries and tooltip wiring.
- Existing transfer/SFTP suites must remain green.
