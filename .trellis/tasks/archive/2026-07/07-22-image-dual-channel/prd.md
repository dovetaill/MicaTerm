# Mica Term 图片双通道能力

## Goal

Deliver two explicitly separated image workflows: local clipboard images become remote
files whose paths are pasted into the shell, while remote terminal image escape
sequences render inside the terminal viewport.

## Requirements

- Complete the clipboard upload workflow before starting terminal image rendering.
- Preserve the existing text-paste behavior on every platform.
- Treat clipboard upload as an input workflow and terminal protocol rendering as an
  output workflow; do not merge their UI or transport semantics.
- Reuse the existing SSH, SFTP, terminal core, presenter, and renderer ownership
  boundaries.
- Keep `TERM=xterm-256color` and `TERM_PROGRAM=mica-term` for compatibility.
- Enforce bounded image dimensions and memory use without logging image contents.
- Support static images only in the first protocol release.

## Acceptance Criteria

- [x] Windows clipboard bitmap or single-image-file paste uploads one PNG through
      SFTP and pastes the quoted remote absolute path into the originating session.
- [x] Text-only paste and all non-Windows paste behavior remain unchanged.
- [x] Kitty direct-data/chunked, iTerm2 inline, and Sixel output produce terminal
      image placements through one shared resource model.
- [x] Unsupported remote file/shared-memory protocol media are rejected without
      reading or writing local paths.
- [x] Terminal-generated protocol replies are sent to SSH immediately after remote
      bytes are parsed.
- [x] Real viewport pixel dimensions and DPI reach the terminal core and SSH PTY
      resize paths.
- [x] Focused tests, full compilation, formatting, lint, and regression checks are
      recorded before both child tasks are archived.

## Notes

- Xshell documentation confirms text paste and file-transfer workflows, but not
  direct bitmap paste. Warp is the closer product precedent for clipboard images.
- Protocol references: Kitty Graphics Protocol, iTerm2 Inline Images, and DEC Sixel.
