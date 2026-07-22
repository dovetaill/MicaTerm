# Design

## Scenario Boundary

1. Input workflow: inspect the local clipboard, upload an image to the active SSH
   session with SFTP, then route the resulting remote path through the existing
   terminal paste pipeline.
2. Output workflow: let the terminal parser consume remote escape sequences and
   project image resources and placements alongside the text grid.

The workflows share limits and diagnostics conventions, but they do not share a
transport or user action.

## Layer Ownership

- Bootstrap owns paste intent and user-visible errors.
- A platform clipboard adapter owns Windows clipboard decoding and PNG encoding.
- The SSH/SFTP runtime owns canonical remote-home discovery, directory creation,
  exclusive upload, permissions, and cleanup.
- The terminal core owns protocol parsing and extraction of image cell metadata.
- Runtime contracts carry immutable image resources, placements, and viewport
  metrics.
- Presenters and renderers cache decoded resources and composite placements with
  text, selection, and cursor layers.

## Cross-Cutting Contracts

- Encoded input: at most 20 MiB.
- Pixel count: at most 25 million.
- Decoded image: at most 100 MiB.
- Per-session retained image resources: at most 128 MiB.
- Clipboard upload cache: canonical home plus
  `.cache/mica-term/clipboard/<session-id>/`, mode 0700, files mode 0600.
- Protocol v1 is static: animation metadata may be parsed, but no animation
  scheduler is introduced.
