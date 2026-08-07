# Implementation Plan

## Phase 1: Clipboard Image Upload

1. Define a platform-neutral clipboard payload and keep existing text semantics.
2. Decode Windows bitmap or one image file, validate bounds, and encode PNG.
3. Add an SFTP byte-upload operation with canonical home, exclusive creation,
   permissions, and scoped stale-file cleanup.
4. Route the uploaded POSIX-shell-quoted path through `send_session_paste` for the
   original session.
5. Add unit and integration contract tests, then verify and archive child task 1.

## Phase 2: Terminal Image Rendering

1. Enable and constrain existing WezTerm Kitty/iTerm2/Sixel parsing.
2. Extend terminal/runtime snapshots with deduplicated resources and placements.
3. Drain parser-generated replies immediately into the SSH writer.
4. Carry real viewport pixels and DPI through terminal resize and PTY resize.
5. Composite static image placements in native and bitmap paths with clipping,
   scrolling, transparency, and damage tracking.
6. Add protocol fixtures and renderer contract tests, then verify and archive child
   task 2 and the parent task.
