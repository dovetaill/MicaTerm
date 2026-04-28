# Terminal Semantic Boundary Repair Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore terminal semantic highlighting to reliable boundaries by separating shell/input/output/TUI modes, shrinking output rules to conservative detectors, honoring ANSI truth, and preventing Codex/TUI content from being recolored by transcript heuristics.

**Architecture:** Introduce a minimal terminal presentation classification, propagate shell-integration state into the runtime surface/model, tighten prompt/input detection to live shell input only, remove aggressive output/status heuristics, and split presenter retention between raw source frames and styled frames.

**Tech Stack:** Rust, terminal runtime/model/presenter modules, semantic pipeline tests, SSH shell-integration tests, UI preference defaults.

---

### Task 1: Lock the new semantic boundary expectations with failing tests

**Files:**
- Modify: `tests/terminal_semantic_pipeline_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/ssh_shell_integration_spec.rs`
- Modify: `tests/ui_preferences.rs`
- Modify: `src/app/terminal_presenter.rs`
- Reference: `src/app/terminal_semantic/mod.rs`
- Reference: `src/app/terminal_semantic/input_line.rs`
- Reference: `src/app/terminal_semantic/output_rules.rs`
- Reference: `src/app/ssh/runtime/contracts.rs`

**Step 1: Add failing semantic-mode separation tests**
- Add a test that marks a surface as `mouse_grabbed` and asserts transcript-style spans/blocks are suppressed.
- Add a test that marks a surface with application-cursor/app-mode state and asserts input/output transcript heuristics stay off.
- Add a test that keeps alt-screen enabled and asserts semantic analysis stays empty except any explicitly allowed search behavior.

**Step 2: Add failing prompt/input boundary tests**
- Add tests showing a heading or quote-like line ending in `> ` or beginning with `# ` is no longer treated as a prompt.
- Add a test showing prompt fallback only applies to the bottom live row, not arbitrary scrollback rows.

**Step 3: Add failing conservative output tests**
- Add tests showing URLs, file paths, and `file:line:column` remain highlighted.
- Add tests showing natural-language `success`, `done`, `INFO`, `DEBUG`, and prose bullet lines are not highlighted anymore.
- Add tests showing diff/JSON/log rows no longer receive semantic recolor spans.

**Step 4: Add failing runtime/default tests**
- Extend shell-integration tests so runtime state must retain prompt/command flags, not just cwd.
- Update UI-preference tests to expect the tighter defaults (`Focused`, overview markers off).

**Step 5: Add failing presenter-retention tests**
- Add a native presenter unit test that detects raw-source diffing no longer depends on a previously styled frame.

**Step 6: Run focused tests to verify they fail**
Run: `cargo test --test terminal_semantic_pipeline_spec --test terminal_scrollback_spec --test ssh_shell_integration_spec --test ui_preferences -- --nocapture`
Expected: FAIL because the current semantic pipeline still uses broad prompt/output heuristics and the runtime/defaults are not yet tightened.

**Step 7: Commit the failing test scaffold**
```bash
git add tests/terminal_semantic_pipeline_spec.rs tests/terminal_scrollback_spec.rs tests/ssh_shell_integration_spec.rs tests/ui_preferences.rs src/app/terminal_presenter.rs
git commit -m "test: lock terminal semantic boundary repair"
```

### Task 2: Thread presentation mode and shell-integration truth through the runtime/model

**Files:**
- Modify: `src/app/ssh/runtime/contracts.rs`
- Modify: `src/app/terminal_core/types.rs`
- Modify: `src/app/ssh/runtime/pump.rs`
- Modify: `src/app/terminal_model.rs`
- Modify: `src/app/ssh/shell_integration.rs`
- Reference: `tests/ssh_shell_integration_spec.rs`

**Step 1: Add shell-integration surface state**
- Extend the surface/frame contracts with the minimal shell-integration state needed by semantic gating, including prompt/command lifecycle hints.
- Preserve backward-compatible defaults for synthetic test surfaces.

**Step 2: Carry runtime shell integration events into the terminal surface**
- Update the SSH pump/runtime path so prompt start/end, command start, and command finished events are retained alongside cwd.
- Keep sanitizing OSC markers before terminal output is applied.

**Step 3: Add a minimal terminal presentation mode to the model**
- Derive `ShellLive`, `ShellScrollback`, `InlineInteractiveApp`, or `AlternateScreenTui` from surface state.
- Keep the classification thin and local to the terminal presentation/semantic layer.

**Step 4: Run focused shell-integration tests**
Run: `cargo test --test ssh_shell_integration_spec -- --nocapture`
Expected: PASS.

**Step 5: Commit the runtime/model groundwork**
```bash
git add src/app/ssh/runtime/contracts.rs src/app/terminal_core/types.rs src/app/ssh/runtime/pump.rs src/app/terminal_model.rs src/app/ssh/shell_integration.rs tests/ssh_shell_integration_spec.rs
git commit -m "feat: thread terminal shell semantics into the model"
```

### Task 3: Shrink the semantic pipeline to safe input/output boundaries

**Files:**
- Modify: `src/app/terminal_semantic/mod.rs`
- Modify: `src/app/terminal_semantic/input_line.rs`
- Modify: `src/app/terminal_semantic/output_rules.rs`
- Modify: `src/app/terminal_semantic/output_blocks.rs`
- Modify: `src/app/terminal_semantic/command_blocks.rs`
- Modify: `tests/terminal_semantic_pipeline_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Reference: `docs/plans/2026-04-28-terminal-semantic-boundaries-design.md`

**Step 1: Gate semantic analysis by presentation mode**
- Teach the semantic entry point to disable transcript heuristics in inline-app and alt-screen modes.
- Keep shell scrollback distinct from the live bottom-row shell mode.

**Step 2: Tighten input highlighting**
- Remove risky generic prompt markers (`# ` and `> `).
- Limit prompt fallback to the live bottom row.
- Prefer shell-integration prompt/input truth when present.

**Step 3: Reduce output rules to conservative detectors**
- Remove generic success/failure/severity phrase recoloring and broad diff/JSON/log semantic spans.
- Keep URL/path/file-location/network endpoint/search detection.
- Ensure command status inference no longer depends on generic lexical failure signals.

**Step 4: Disable transcript-style block overlays in non-shell modes**
- Prevent JSON/XML/log block overlays from appearing in app/TUI modes.
- Keep the remaining overlays explainable and minimal.

**Step 5: Run focused semantic tests**
Run: `cargo test --test terminal_semantic_pipeline_spec --test terminal_scrollback_spec -- --nocapture`
Expected: PASS.

**Step 6: Commit the semantic pipeline repair**
```bash
git add src/app/terminal_semantic/mod.rs src/app/terminal_semantic/input_line.rs src/app/terminal_semantic/output_rules.rs src/app/terminal_semantic/output_blocks.rs src/app/terminal_semantic/command_blocks.rs tests/terminal_semantic_pipeline_spec.rs tests/terminal_scrollback_spec.rs
 git commit -m "fix: tighten terminal semantic highlighting boundaries"
```

### Task 4: Split presenter retention between raw and styled frames

**Files:**
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_model.rs`
- Modify: `tests/terminal_semantic_pipeline_spec.rs`
- Reference: `src/app/terminal_semantic/mod.rs`

**Step 1: Store separate previous source/styled frames**
- Keep raw-frame diffing based on the unstyled previous frame.
- Keep renderer/style reuse based on the styled previous frame.

**Step 2: Preserve row-dirty correctness after style projection**
- Ensure semantic style projection invalidates only the rows whose rendered styling actually changed.
- Add any minimal row-memory helper needed to avoid re-highlighting whole frames.

**Step 3: Run presenter/semantic tests**
Run: `cargo test --test terminal_semantic_pipeline_spec -- --nocapture`
Expected: PASS.

**Step 4: Commit the presenter fix**
```bash
git add src/app/terminal_presenter.rs src/app/terminal_model.rs tests/terminal_semantic_pipeline_spec.rs
git commit -m "fix: separate raw and styled terminal presenter frames"
```

### Task 5: Tighten defaults and run full verification

**Files:**
- Modify: `src/app/ui_preferences.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `ui/components/settings-modal.slint`
- Modify: `tests/ui_preferences.rs`
- Reference: all files touched in Tasks 1-4

**Step 1: Set conservative defaults**
- Default output rule profile to `Focused`.
- Default overview markers off.
- Keep input highlighting enabled.

**Step 2: Update settings copy if needed**
- Keep the UI copy aligned with the new conservative behavior without adding new controls.

**Step 3: Run focused regression tests**
Run: `cargo test --test terminal_semantic_pipeline_spec --test terminal_scrollback_spec --test ssh_shell_integration_spec --test ui_preferences -- --nocapture`
Expected: PASS.

**Step 4: Run adjacent terminal regressions**
Run: `cargo test --test terminal_model_spec --test ssh_terminal_interaction_spec --test terminal_core_parity_spec -- --nocapture`
Expected: PASS.

**Step 5: Refresh docs if implementation details shifted**
- Update the design doc if naming or boundaries changed during implementation.
- Update this plan with any extra verification steps uncovered during execution.

**Step 6: Commit the completed repair**
```bash
git add docs/plans/2026-04-28-terminal-semantic-boundaries-design.md docs/plans/2026-04-28-terminal-semantic-boundaries-implementation-plan.md src/app/ui_preferences.rs src/shell/view_model.rs src/shell/view_model/projection.rs ui/components/settings-modal.slint tests/ui_preferences.rs src/app/ssh/runtime/contracts.rs src/app/terminal_core/types.rs src/app/ssh/runtime/pump.rs src/app/terminal_model.rs src/app/ssh/shell_integration.rs src/app/terminal_semantic/mod.rs src/app/terminal_semantic/input_line.rs src/app/terminal_semantic/output_rules.rs src/app/terminal_semantic/output_blocks.rs src/app/terminal_semantic/command_blocks.rs src/app/terminal_presenter.rs tests/terminal_semantic_pipeline_spec.rs tests/terminal_scrollback_spec.rs tests/ssh_shell_integration_spec.rs
 git commit -m "fix: restore terminal semantic highlighting boundaries"
```
