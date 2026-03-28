# SSH Proxy Dropdown / HTTP Proxy Design

## Goal

Refine the SSH connection modal so proxy configuration is selectable instead of free-form, add HTTP proxy support alongside SOCKS5, and make upstream SSH proxy selection come from existing saved SSH connections while excluding the asset being edited.

## Confirmed Scope

- Replace `Proxy Type` segmented buttons with a dropdown.
- Add `HTTP` as a first-class proxy type with the same core fields as `SOCKS5`:
  - host
  - port
  - username
  - password
- Replace free-text `Existing SSH Connection` input with a dropdown sourced from saved SSH assets.
- When editing an SSH asset, exclude the current asset from the upstream SSH dropdown to prevent self-proxy loops.

## Data Model Direction

- Keep SSH auth behavior unchanged.
- Extend proxy model/persistence/runtime with an `http` variant instead of overloading `socks5`.
- Store HTTP proxy password in the same saved-secret bundle mechanism used for SSH secrets and SOCKS5 proxy passwords.
- Keep upstream SSH proxy persistence keyed by asset id.

## UI Direction

- Use `ComboBox` from `std-widgets.slint` for:
  - proxy type selection
  - upstream SSH connection selection
- Keep SOCKS5 and HTTP field groups visually parallel so the modal stays predictable.
- If there are no eligible upstream SSH assets, show a disabled/empty selector plus explanatory copy.

## Runtime Direction

- Add HTTP CONNECT transport support to the SSH runtime.
- Keep non-SSH transport proxies outermost in the resolved chain.
- Preserve existing recursive SSH proxy-chain resolution semantics.

## Validation Rules

- `http` proxy requires valid host and port when selected.
- `ssh-asset` proxy requires a selected upstream asset id.
- Editing an asset with a stale self-reference should surface as invalid until the user chooses a different upstream connection.

## Tests To Add

- UI/property round-trip tests for HTTP proxy fields and SSH dropdown properties.
- View-model tests for HTTP draft updates and self-excluding upstream candidate generation.
- Persistence/profile tests for HTTP proxy save/load normalization.
- Runtime session-manager tests covering HTTP CONNECT success and authentication failure paths.
