# Windows Console Assets Context Menu Unimplemented Actions

Date: 2026-03-17
Scope: `Window Console` assets context menu

## Planned But Not Wired Yet

| Action ID | Scene | UI Label | Current Runtime Behavior | Expected Final Behavior | Dependency / Notes | Priority |
| --- | --- | --- | --- | --- | --- | --- |
| `proxy-chrome-via-server` | `ssh-connection` | `Proxy Chrome via Server` | Click keeps the menu open and shows `StatusPill` feedback | Launch a browser profile or proxy bridge through the selected SSH connection | Depends on real SSH session lifecycle and browser/proxy product decision | P1 |
| `upload-ssh-public-key` | `ssh-connection` | `Upload SSH Public Key (ssh-copy-id)` | Present in IA as `Planned`; keyboard / click path should stay available for future wiring | Run `ssh-copy-id` style public-key upload with success / failure feedback | Depends on SSH auth flow, local key discovery, and host capability checks | P1 |

## Notes

- This file tracks actions that are intentionally exposed in the menu IA but are not wired to real business execution yet.
- Actions that are merely shell placeholders and currently close the menu without business work are not treated as `Planned` in this file; this list stays focused on the explicit planned-action pathway.
- The first shipped blank-area and item create IA is now limited to `New Folder` and `New SSH Connection`.
- Legacy protocol entries such as `Local Terminal`, `Serial`, `Telnet`, and `SSH Tunnel` are backlog references only; they are not part of the current shipped context menu.
