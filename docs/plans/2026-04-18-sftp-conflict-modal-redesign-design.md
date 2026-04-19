# SFTP Conflict Modal Redesign Design

## Goal

Refactor the SFTP download conflict modal so it reads like a mature Windows desktop tool:
- one clear decision path for the current download
- no duplicated cancel semantics
- stable header / body / footer layout under long paths and high DPI
- elevated Fluent-like visual treatment with subtle border glow and shadow
- consistent close affordance using the Fluent dismiss icon

## Confirmed Product Decisions

### Primary scenario

This redesign targets the single-item SFTP download conflict flow where a local file already exists.

### Download conflict actions

The download conflict footer keeps exactly three explicit actions:
- `Skip This Download`
- `Auto Rename`
- `Replace Existing`

`Auto Rename` is the recommended action and should receive initial focus.

`Replace Existing` remains available but should read as a destructive choice without turning into a bright danger button.

`Skip This Download` is the secondary, weakest action.

### Close semantics

The modal must not support an ambiguous “close without deciding” path.

Confirmed behavior:
- top-right close affordance equals `Skip This Download`
- `Esc` equals `Skip This Download`
- no separate `Cancel`
- no separate `Cancel Download`

### Close affordance iconography

The top-right affordance must use the Fluent dismiss icon asset rather than a literal `X` glyph:
- `assets/icons/fluent/dismiss-20-regular.svg`

Tooltip and accessibility copy should describe the semantic action, not the ornament, e.g. `Skip this download`.

## Content Design

### Header

- Title stays `Resolve Transfer Conflict`
- Compact subtitle clarifies scope: `This affects the current download only.`
- Drag behavior remains available from the header surface

### Body copy

Use short product copy instead of a dense technical paragraph:
- `A file with the same name already exists locally.`
- `Choose how to continue this download.`

### Information blocks

The body keeps two structured blocks:
- `Remote item`
- `Local target`

Each block should present:
- a quieter label
- a clearer path value
- single-line truncation / ellipsis for long paths
- path text in the existing mono / semi-mono shell typography for readability

### Batch scope

Retain the existing apply-to-batch affordance, but keep it in a distinct section below the path blocks and only for the existing batch-compatible choices.

## Interaction Model

### Keyboard

- initial focus goes to `Auto Rename` for download conflicts
- button-level Enter / Space activation should drive actions
- modal-level Enter should no longer hard-wire `Replace`
- `Esc` triggers the same path as top-right dismiss

### Download resolution mapping

- `Skip This Download` -> existing `skip-requested` flow
- `Auto Rename` -> existing `auto-rename-requested` flow
- `Replace Existing` -> existing `replace-requested` flow

The old `cancel-download-requested` path is removed from the modal surface.

### Transfer-center projection

If a conflict is skipped, the projected transfer-center label should read `Skipped` instead of `Cancelled` where possible, even if the underlying task state remains `Cancelled`.

## Layout Strategy

### Structural fix

Replace the current absolute-positioned body/footer stack with a clear composition:
- elevated shell frame
- header band
- scrollable body region
- independent footer band

This prevents long paths or the batch scope card from colliding with the footer.

### Footer requirements

- divider sits at the top of the footer band only
- footer has its own padding and vertical centering
- action buttons align to the right in a stable row
- no button may visually sit on the divider line

## Visual Direction

### Fluent / Windows 11 treatment

Keep the project’s dark shell, but give this modal a calmer elevated treatment:
- subtle border
- near + far shadow
- restrained halo / border glow
- softer corner radius than legacy square modals

The modal should reference the Transfer Center’s shell language, but remain calmer and more compact.

### Token strategy

Prefer existing shared tokens first. If the utility-panel tokens are too generic, add dedicated conflict-dialog tokens for:
- surface
- header surface
- section surface
- border
- glow
- destructive-outline / emphasis

Light theme slots should be defined even if dark mode remains the main target for this round.

## Engineering Scope

Likely touchpoints:
- `ui/components/sftp-conflict-modal.slint`
- `ui/components/blocking-modal-shell.slint`
- `ui/app-window.slint`
- `src/app/bootstrap/sftp.rs`
- `src/app/bootstrap/shell_chrome.rs`
- `ui/theme/tokens.slint`
- relevant UI / bootstrap tests

## Non-Goals

- introducing WebView or web-style modal patterns
- changing unrelated modal components unless a shared shell extension is needed
- broad SFTP queue state-machine refactors beyond what is needed to project `Skipped` more honestly
