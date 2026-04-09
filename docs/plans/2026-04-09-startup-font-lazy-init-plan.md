# Startup Font Lazy Init Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce first-terminal startup private/commit by making Windows terminal font helpers lazy and removing duplicate system font scans.

**Architecture:** Keep `DirectWriteFontSystem::new()` cheap, move expensive font helper creation behind on-demand accessors, and centralize system font database loading so locator and emoji paths share one scan. Preserve font behavior and renderer behavior.

**Tech Stack:** Rust, fontdb, DirectWrite, terminal-native-renderer tests

---

### Task 1: Lock the lazy-init contract with tests

**Files:**
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing tests**
- Add a unit test asserting a new `DirectWriteFontSystem` leaves expensive helpers uninitialized.
- Add a source contract test asserting font scanning is no longer owned separately by locator and emoji modules.

**Step 2: Run tests to verify they fail**
Run: `cargo test --test terminal_renderer_dwrite_spec windows_font_backend_lazy_init_contract -- --nocapture`
Expected: FAIL because the source still eagerly scans fonts / initializes helpers.

**Step 3: Write minimal implementation**
- Introduce lazy helper state inside `DirectWriteFontSystem`.
- Centralize shared system font database creation.

**Step 4: Run tests to verify they pass**
Run the targeted tests again.

**Step 5: Commit**
`git add` the touched files and commit with a focused message.

### Task 2: Keep runtime behavior unchanged

**Files:**
- Modify: `src/app/terminal_font/windows_locator.rs`
- Modify: `src/app/terminal_emoji.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Test: `tests/windows_directwrite_font_chain_spec.rs`
- Test: `tests/terminal_color_emoji_spec.rs`

**Step 1: Write or update failing coverage if needed**
- Add coverage only if the targeted runtime tests do not exercise lazy helper creation paths.

**Step 2: Run tests to verify behavior**
Run: `cargo test --test windows_directwrite_font_chain_spec -- --nocapture`
Run: `cargo test --test terminal_color_emoji_spec dwrite_color_glyph_raster_is_not_a_flat_placeholder_block -- --nocapture`
Expected: existing behavior remains green.

**Step 3: Refine implementation minimally**
- Ensure fallback resolution, face loading, emoji rasterization, and DirectWrite metrics still initialize on demand.

**Step 4: Run focused verification**
- Re-run the targeted tests plus `cargo check`.

**Step 5: Commit**
Create a focused commit for the lazy-init change.
