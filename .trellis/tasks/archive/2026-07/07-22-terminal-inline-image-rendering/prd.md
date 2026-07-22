# Kitty iTerm2 Sixel 终端图片渲染

## Goal

Render static remote terminal images for Kitty Graphics, iTerm2 inline images, and
Sixel by reusing the existing WezTerm terminal core and the current presenter and
renderer architecture.

## Requirements

- Enable Kitty graphics parsing in the existing terminal configuration.
- Support Sixel, iTerm2 `inline=1`, and Kitty direct-data/chunk transfer.
- Reject Kitty file, temporary-file, and shared-memory media for SSH sessions.
- Treat iTerm2 `inline=0` as unsupported and never write local files for it.
- Support static display, placement, deletion, and query replies. Do not add an
  animation scheduler.
- Define one resource/placement projection containing content hash, RGBA/dimensions,
  UV bounds, padding, z-index, image ID, and placement ID.
- Deduplicate resources and avoid copying full image payloads every frame.
- Apply the same model to native and bitmap rendering, including clipping,
  scrolling, resize/reflow, transparency, z-order, damage, and session detach.
- Pass actual viewport pixel width, pixel height, and DPI through terminal-core and
  SSH PTY resize contracts; remove `cols * 8` / `rows * 16` estimates from image
  layout.
- Immediately drain terminal-generated responses after applying remote bytes and
  write them to SSH without waiting for keyboard or mouse input.
- Enforce 25 million pixels, 20 MiB encoded, 100 MiB decoded per image, and 128 MiB
  retained resources per session. Patch the dependency minimally if configuration
  is not exposed.
- Preserve `TERM=xterm-256color` and `TERM_PROGRAM=mica-term`.

## Acceptance Criteria

- [x] Representative static Sixel, iTerm2 inline, and Kitty direct/chunk fixtures
      produce resources and correctly positioned placements.
- [x] Kitty query/delete replies are emitted immediately through the SSH writer.
- [x] Unsupported file-based or shared-memory media cannot access local paths.
- [x] Repeated placements of identical content share a resource allocation.
- [x] Images clip and scroll with the terminal grid and repaint correctly after
      resize, reflow, transparency overlap, and session detach.
- [x] Both native and bitmap rendering paths consume the same projection contract.
- [x] Budget violations fail closed without unbounded allocation or parser stalls.
- [x] Existing text rendering, selection, cursor, scrollback, and terminal input
      tests remain stable.

## Notes

- Kitty graphics use APC `_G`; iTerm2 images use OSC 1337. Protocol detection must
  not conflate the two.
