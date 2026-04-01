# Native-Only Terminal Surface Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the terminal bitmap pipeline with a native-only terminal surface architecture that works on Windows, Linux X11, and Linux Wayland, ships with `Fusion-JetBrainsMapleMono` as the default terminal font, and adds optional semantic highlighting for normal shell text flows.

**Architecture:** Keep the current Slint shell, SSH/session flow, and `wezterm-term` core, but split terminal rendering into shared terminal/text-layout/display-list layers plus platform native surface backends. Implement the platform backends first, switch runtime/build contracts to native-only, then delete the old bitmap/image path after all three backends are wired.

**Tech Stack:** Rust, Slint, Tokio, wezterm-term, termwiz, windows-sys, DirectWrite/Direct2D/DirectComposition, Wayland, X11, raw-window-handle, regex, optional tree-sitter, focused Rust/unit/contract tests

**Status:** Completed on 2026-04-01.

**Final Verification:** `cargo test --workspace -q`, `cargo check --workspace`, and `cargo clippy --workspace -- -D warnings` all pass after the Task 12 handoff docs land.

**TDD Handoff:** `docs/plans/2026-04-01-native-only-terminal-surface-tdd-spec.md`

---

### Task 1: Lock the native-only architecture contract in tests and docs

**Files:**
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/build_win_x64_script_smoke.sh`
- Modify: `tests/build_win_x64_software_script_smoke.sh`
- Modify: `src/app/runtime_profile.rs`
- Modify: `build-win-x64.sh`
- Modify: `build-win-x64-software.sh`

**Step 1: Write the failing test**
- Add assertions that Windows and Linux are moving to native-only terminal surfaces.
- Add assertions that `build-win-x64-software.sh` no longer documents bitmap fallback semantics.
- Add assertions that runtime profile docs no longer describe bitmap as a supported shipping path.

**Step 2: Run test to verify it fails**
Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec --test runtime_profile -q && bash tests/build_win_x64_script_smoke.sh && bash tests/build_win_x64_software_script_smoke.sh`
Expected: FAIL because current runtime/build contracts still mention bitmap fallback semantics.

**Step 3: Write minimal implementation**
- Update contract wording in source/tests.
- Keep the implementation minimal; this task only locks architecture intent.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Commit**
```bash
git add tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs tests/runtime_profile.rs tests/build_win_x64_script_smoke.sh tests/build_win_x64_software_script_smoke.sh src/app/runtime_profile.rs build-win-x64.sh build-win-x64-software.sh
git commit -m "test: lock native-only terminal architecture contract"
```

### Task 2: Introduce a shared platform surface backend abstraction

**Files:**
- Create: `src/app/terminal_renderer/platform/mod.rs`
- Create: `src/app/terminal_renderer/platform/backend.rs`
- Modify: `src/app/terminal_renderer/mod.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing test**
- Add source-level assertions that a backend trait exists for native surface attach/update/present/detach.
- Add assertions that `NativeTerminalSurface` uses a backend object instead of only a retained token.

**Step 2: Run test to verify it fails**
Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: FAIL because no shared backend abstraction exists.

**Step 3: Write minimal implementation**
- Define a platform backend trait and a retained frame/update contract.
- Thread the abstraction through `native_surface.rs` and bootstrap.
- Do not implement platform drawing yet.

**Step 4: Run test to verify it passes**
Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/terminal_renderer/platform/mod.rs src/app/terminal_renderer/platform/backend.rs src/app/terminal_renderer/mod.rs src/app/terminal_renderer/native_surface.rs src/app/bootstrap.rs src/app/terminal_presenter.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "feat: add platform native surface backend abstraction"
```

### Task 3: Complete the Windows native surface backend

**Files:**
- Create: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/platform/mod.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**
- Add source-level assertions for a Windows backend that resolves `HWND`, owns native surface state, and exposes attach/present/detach hooks.
- Add assertions that bootstrap can instantiate the Windows backend.

**Step 2: Run test to verify it fails**
Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because there is no Windows backend implementation.

**Step 3: Write minimal implementation**
- Add `windows.rs` backend scaffolding.
- Resolve the host window handle and keep a native surface state object.
- Wire the backend into the shared surface abstraction.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/platform/mod.rs src/app/terminal_renderer/native_surface.rs src/app/windows_frame.rs src/app/bootstrap.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: add windows native surface backend"
```

### Task 4: Implement Windows native text draw and overlays

**Files:**
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/terminal_renderer/atlas.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_layout/shaper.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`

**Step 1: Write the failing test**
- Add assertions for monochrome glyph draw, color glyph draw, selection overlay, cursor overlay, underline overlay, and IME preview payloads.
- Add tests that distinguish mono atlas resources from color glyph resources.

**Step 2: Run test to verify it fails**
Run: `cargo test --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec -q`
Expected: FAIL because the current renderer only prepares metadata and does not complete draw contracts.

**Step 3: Write minimal implementation**
- Extend prepared frame data to carry draw-ready display list information.
- Add color glyph cache separation.
- Thread overlay payloads through presenter and renderer contracts.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/terminal_renderer/wgpu_renderer.rs src/app/terminal_renderer/atlas.rs src/app/terminal_presenter.rs src/app/terminal_font/windows_dwrite.rs src/app/terminal_layout/shaper.rs tests/terminal_renderer_dwrite_spec.rs tests/terminal_color_emoji_spec.rs
git commit -m "feat: complete windows native text draw contracts"
```

### Task 5: Add Linux Wayland native surface backend

**Files:**
- Create: `src/app/terminal_renderer/platform/wayland.rs`
- Modify: `src/app/terminal_renderer/platform/mod.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `Cargo.toml`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing test**
- Add source-level assertions that a Wayland backend exists and is selectable when the host runs under Wayland.
- Add assertions that the backend integrates with the shared platform abstraction.

**Step 2: Run test to verify it fails**
Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: FAIL because no Wayland backend exists.

**Step 3: Write minimal implementation**
- Add the Wayland backend scaffold and required dependencies.
- Wire backend selection into the shared platform abstraction.
- Keep the first implementation minimal but structurally complete.

**Step 4: Run test to verify it passes**
Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/terminal_renderer/platform/wayland.rs src/app/terminal_renderer/platform/mod.rs src/app/terminal_renderer/native_surface.rs src/app/bootstrap.rs Cargo.toml tests/native_terminal_surface_contract_spec.rs
git commit -m "feat: add wayland native surface backend"
```

### Task 6: Add Linux X11 native surface backend

**Files:**
- Create: `src/app/terminal_renderer/platform/x11.rs`
- Modify: `src/app/terminal_renderer/platform/mod.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `Cargo.toml`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing test**
- Add source-level assertions that an X11 backend exists and is selectable under X11.
- Add assertions that it integrates with the shared platform abstraction.

**Step 2: Run test to verify it fails**
Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: FAIL because no X11 backend exists.

**Step 3: Write minimal implementation**
- Add the X11 backend scaffold and required dependencies.
- Wire backend selection into the shared platform abstraction.

**Step 4: Run test to verify it passes**
Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/terminal_renderer/platform/x11.rs src/app/terminal_renderer/platform/mod.rs src/app/terminal_renderer/native_surface.rs src/app/bootstrap.rs Cargo.toml tests/native_terminal_surface_contract_spec.rs
git commit -m "feat: add x11 native surface backend"
```

### Task 7: Migrate the default terminal font to Fusion-JetBrainsMapleMono

**Files:**
- Create: `assets/fonts/Fusion-JetBrainsMapleMono/`
- Create: `assets/fonts/Fusion-JetBrainsMapleMono/OFL.txt`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_font/wezterm_font.rs`
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `tests/terminal_font_registration_smoke.rs`
- Modify: `tests/runtime_profile.rs`
- Modify: `build-desktop.sh`

**Step 1: Write the failing test**
- Add tests asserting the new font is bundled, registered, and selected as the default terminal font.
- Add assertions that the old bundled terminal font path is no longer the default.

**Step 2: Run test to verify it fails**
Run: `cargo test --test terminal_font_registration_smoke --test runtime_profile -q`
Expected: FAIL because the existing default font is still active.

**Step 3: Write minimal implementation**
- Add the new bundled font asset layout.
- Switch the default terminal font request to `Fusion-JetBrainsMapleMono`.
- Package the `OFL.txt` license with the distributed assets.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Commit**
```bash
git add assets/fonts/Fusion-JetBrainsMapleMono src/app/terminal_font/windows_dwrite.rs src/app/terminal_font/wezterm_font.rs src/app/terminal_font/backend.rs tests/terminal_font_registration_smoke.rs tests/runtime_profile.rs build-desktop.sh
git commit -m "feat: adopt fusion jetbrains maple mono as default terminal font"
```

### Task 8: Switch runtime/build profiles to native-only terminal mode

**Files:**
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/main.rs`
- Modify: `build-win-x64.sh`
- Modify: `build-win-x64-software.sh`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/build_win_x64_script_smoke.sh`
- Modify: `tests/build_win_x64_software_script_smoke.sh`

**Step 1: Write the failing test**
- Add assertions that packaged runtime profiles select native-only terminal mode.
- Add assertions that both Windows wrapper scripts emit native-only terminal packaging metadata.

**Step 2: Run test to verify it fails**
Run: `cargo test --test runtime_profile -q && bash tests/build_win_x64_script_smoke.sh && bash tests/build_win_x64_software_script_smoke.sh`
Expected: FAIL because the runtime/build contracts still carry mixed semantics.

**Step 3: Write minimal implementation**
- Remove bitmap shipping semantics from runtime profiles.
- Make both Windows wrapper scripts emit native-only terminal mode metadata.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/runtime_profile.rs src/main.rs build-win-x64.sh build-win-x64-software.sh tests/runtime_profile.rs tests/build_win_x64_script_smoke.sh tests/build_win_x64_software_script_smoke.sh
git commit -m "feat: switch packaged terminal runtime to native-only"
```

### Task 9: Add semantic overlay detection for normal shell output

**Files:**
- Create: `src/app/terminal_semantic/mod.rs`
- Create: `src/app/terminal_semantic/output_blocks.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`

**Step 1: Write the failing test**
- Add tests for JSON/XML/log block detection over normal shell output.
- Add assertions that semantic overlays are display-list overlays, not ANSI rewrites.

**Step 2: Run test to verify it fails**
Run: `cargo test --test terminal_scrollback_spec --test terminal_color_emoji_spec -q`
Expected: FAIL because no semantic overlay layer exists.

**Step 3: Write minimal implementation**
- Add output block detection and semantic overlay descriptors.
- Thread overlay descriptors into the presenter/display-list path.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/terminal_semantic/mod.rs src/app/terminal_semantic/output_blocks.rs src/app/terminal_presenter.rs src/app/bootstrap.rs tests/terminal_color_emoji_spec.rs tests/terminal_scrollback_spec.rs
git commit -m "feat: add semantic overlays for shell output blocks"
```

### Task 10: Add input-line highlighting with alternate-screen guardrails

**Files:**
- Create: `src/app/terminal_semantic/input_line.rs`
- Modify: `src/app/terminal_semantic/mod.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`

**Step 1: Write the failing test**
- Add tests for shell input line highlighting.
- Add tests that semantic highlighting is disabled in alternate-screen / TUI mode.

**Step 2: Run test to verify it fails**
Run: `cargo test --test ssh_terminal_interaction_spec --test terminal_scrollback_spec -q`
Expected: FAIL because input-line highlighting and alternate-screen guards do not exist.

**Step 3: Write minimal implementation**
- Add regex/bash-aware input-line highlighting.
- Add explicit alternate-screen disable logic.
- Thread the overlay through the display-list path only when safe.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/terminal_semantic/input_line.rs src/app/terminal_semantic/mod.rs src/app/bootstrap.rs src/app/terminal_presenter.rs tests/ssh_terminal_interaction_spec.rs tests/terminal_scrollback_spec.rs
git commit -m "feat: add input-line semantic highlighting"
```

### Task 11: Remove the bitmap/image terminal pipeline

**Files:**
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_renderer/mod.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**
- Add assertions that `session-surface-image`, bitmap renderer mode, and bitmap presenter contracts are gone.
- Add assertions that the UI only threads native frame/native surface state.

**Step 2: Run test to verify it fails**
Run: `cargo test --test native_terminal_surface_contract_spec --test runtime_profile --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because the bitmap pipeline is still present.

**Step 3: Write minimal implementation**
- Remove bitmap runtime mode and presenter code.
- Remove Slint image surface properties and bindings.
- Keep only native surface contracts.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/runtime_profile.rs src/app/bootstrap.rs src/app/terminal_presenter.rs src/app/terminal_renderer/mod.rs ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint tests/native_terminal_surface_contract_spec.rs tests/runtime_profile.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "refactor: remove bitmap terminal pipeline"
```

### Task 12: Run full verification and write the TDD handoff spec

**Files:**
- Create: `docs/plans/2026-04-01-native-only-terminal-surface-tdd-spec.md`
- Modify: `docs/plans/2026-04-01-native-only-terminal-surface-design.md`
- Modify: `docs/plans/2026-04-01-native-only-terminal-surface-implementation-plan.md`

**Step 1: Run targeted verification**
Run: `cargo test --workspace -q`
Expected: PASS

**Step 2: Run workspace compile verification**
Run: `cargo check --workspace`
Expected: PASS

**Step 3: Run lint verification**
Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 4: Write the TDD handoff spec**
- Document the final struct/trait/callback/channel/state-flow contracts.
- Record edge cases for redraw timing, backend teardown, channel pressure, stale callbacks, and overlay desync.

**Step 5: Commit**
```bash
git add docs/plans/2026-04-01-native-only-terminal-surface-tdd-spec.md docs/plans/2026-04-01-native-only-terminal-surface-design.md docs/plans/2026-04-01-native-only-terminal-surface-implementation-plan.md
git commit -m "docs: add native-only terminal tdd handoff"
```
