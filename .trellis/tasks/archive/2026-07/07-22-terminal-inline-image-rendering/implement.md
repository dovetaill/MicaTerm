# Implementation Plan

1. Add protocol fixtures and failing terminal-core projection tests.
2. Enable Kitty parsing and introduce configurable in-band media and memory limits,
   using a minimal local crate patch if required.
3. Extend terminal core and runtime contracts with viewport metrics, image resources,
   placements, and immediate generated-output draining.
4. Carry real pixel width, height, and DPI from the terminal host through resize and
   SSH PTY updates.
5. Project WezTerm image cells into deduplicated resources and placements.
6. Implement placement composition and damage tracking in native rendering.
7. Implement the same static composition contract in bitmap fallback.
8. Verify protocol queries/deletes, scrolling, resize/reflow, clipping, transparency,
   z-order, limits, and teardown.
9. Run formatting, lint, targeted and full tests, then complete the child and parent
   tasks.
