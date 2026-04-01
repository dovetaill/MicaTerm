# WezTerm Font Adoption Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce a first-phase local WezTerm font backend adapter scaffold into the terminal rendering stack so the repo stops treating the current bundled-font atlas path as the long-term text stack.

**Architecture:** Keep the existing session/runtime/UI flow intact, but add a new local `WeztermFontSystem` module that publishes the migration stage, upstream source targets, and the current `harfbuzz` linking blocker. Phase 1 is structural and testable; it does not attempt to port `wezterm-gui`, and it does not wire the direct cargo dependency until the current shaping dependency is replaced.

**Tech Stack:** Rust, Cargo git dependencies, WezTerm `wezterm-font`, existing terminal font/layout seams, focused Rust tests

---

### Task 1: Add the phase-1 adapter module and contract test

**Files:**
- Modify: `src/app/terminal_font/mod.rs`
- Create: `src/app/terminal_font/wezterm_font.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

Add a test to `tests/terminal_renderer_dwrite_spec.rs` that asserts:

- a new terminal font adapter module exists in the source tree
- the adapter is wired into `src/app/terminal_font/mod.rs`
- the adapter is re-exported from the terminal font module

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_dwrite_spec wezterm_font_backend_source_is_wired_into_the_terminal_font_stack -q`

Expected: FAIL because the module and export do not exist yet.

**Step 3: Write minimal implementation**

- Add a source module declaration in `src/app/terminal_font/mod.rs`
- Re-export the adapter from `src/app/terminal_font/mod.rs`
- Create a new source file placeholder for the adapter module

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_renderer_dwrite_spec wezterm_font_backend_source_is_wired_into_the_terminal_font_stack -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_font/mod.rs src/app/terminal_font/wezterm_font.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: add wezterm font backend scaffold"
```

### Task 2: Add a concrete WezTerm font adapter contract

**Files:**
- Create: `src/app/terminal_font/wezterm_font.rs`
- Modify: `src/app/terminal_font/mod.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

Add a test that asserts the new adapter source defines:

- `pub struct WeztermFontSystem`
- `pub enum WeztermFontIntegrationStage`
- a constructor
- a stage accessor
- an upstream source accessor
- a current blocker accessor

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_dwrite_spec wezterm_font_backend_source_exposes_metrics_and_glyph_contracts -q`

Expected: FAIL because the placeholder module is still empty or incomplete.

**Step 3: Write minimal implementation**

Implement `src/app/terminal_font/wezterm_font.rs` with:

- a small wrapper struct for phase-1 migration tracking
- a constructor
- an explicit integration stage enum
- the exact upstream WezTerm source files being tracked
- the current blocker string documenting the `harfbuzz` links conflict

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_renderer_dwrite_spec wezterm_font_backend_source_exposes_metrics_and_glyph_contracts -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_font/wezterm_font.rs src/app/terminal_font/mod.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: add wezterm font adapter contracts"
```

### Task 3: Verify the scaffold compiles and does not regress the current stack

**Files:**
- Verify only: `src/app/terminal_font/wezterm_font.rs`
- Verify only: `src/app/terminal_font/mod.rs`
- Verify only: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Run focused tests**

Run: `cargo test --test terminal_renderer_dwrite_spec -q`

Expected: PASS

**Step 2: Run compile verification**

Run: `cargo check -q`

Expected: PASS

**Step 3: Commit**

```bash
git add src/app/terminal_font/wezterm_font.rs src/app/terminal_font/mod.rs tests/terminal_renderer_dwrite_spec.rs docs/plans/2026-03-31-wezterm-font-adoption-design.md docs/plans/2026-03-31-wezterm-font-adoption-implementation-plan.md
git commit -m "docs: plan wezterm font adoption"
```

Plan complete and saved to `docs/plans/2026-03-31-wezterm-font-adoption-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Proceeding in this session because the user already asked to continue immediately.
