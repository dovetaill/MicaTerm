# Dead Actions Cleanup Design

Date: 2026-05-20

## Goal

Remove the most misleading no-op actions from the shipped UI so users stop seeing buttons or menu items that look actionable but do nothing useful.

## Scope

This cleanup only removes three entry points:

- `Create New Key` in the keychain identity modal
- `Proxy Chrome via Server` in the SSH asset context menu
- `Upload SSH Public Key (ssh-copy-id)` in the SSH asset context menu

Out of scope:

- SFTP planned actions such as `Open in New SFTP Tab` or `Properties`
- Wiring any replacement business flow
- Changing unrelated copy, layout, or modal behavior

## Root Cause

The current UI still exposes two categories of dead affordances:

1. A true no-op button: `create-ssh-key` is emitted from Slint but has no runtime handler.
2. Planned SSH context actions: two menu items are still rendered even though runtime only returns `... is not wired yet.` feedback.

## Options Considered

### Option A: Keep the entries and shorten the labels

Pros:
- Lowest code churn

Cons:
- Does not fix the product problem because the actions still do nothing
- Still teaches users to click dead surfaces

### Option B: Keep them visible but disabled

Pros:
- Signals roadmap intent

Cons:
- Still adds clutter to core SSH flows
- Still leaves an obviously incomplete shipped experience

### Option C: Remove them from shipped UI for now (chosen)

Pros:
- Smallest behavior change with the clearest user benefit
- Matches current real workflow where SSH key material is configured through edit/keychain flows
- Avoids widening scope into future `ssh-copy-id` or browser-proxy product work

Cons:
- Future planned features lose temporary IA visibility until they are actually implemented

## Decision

Adopt Option C.

The product should only expose actions that either work today or are intentionally disabled for a clear reason. For this pass, the three dead entry points are removed entirely instead of renamed or left as planned placeholders.

## Verification

- Rust contract tests confirm the SSH action tree no longer includes the two dead menu items.
- Smoke tests confirm the old planned-action feedback path is gone for SSH assets.
- Modal/render smoke confirms `Create New Key` is not present in the identity modal.
