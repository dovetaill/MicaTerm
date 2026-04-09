# UI and Terminal Typography Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship the approved typography split by giving the Slint shell a dedicated `Sarasa UI SC` UI contract and switching the terminal renderer contract to `JetBrains Mono` + `Sarasa Term SC` with `Medium` as the default terminal weight.

**Architecture:** Keep UI and terminal ownership separate. The Slint tree gets a small shared UI typography layer plus explicit popup coverage, while the Rust terminal renderer updates its bundled assets and shared font constants so bitmap/native/mock paths all describe the same terminal stack.

**Tech Stack:** Rust, Slint 1.15.1, cargo tests, Windows DirectWrite path, bitmap atlas renderer, `./build-win-x64.sh`

---

### Task 1: Lock the approved typography contract in tests

**Files:**
- Create: `tests/ui_typography_defaults_spec.rs`
- Modify: `tests/terminal_font_registration_smoke.rs`
- Modify: `tests/windows_terminal_typography_defaults_spec.rs`
- Modify: `tests/startup_font_memory_regression.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/windows_terminal_diagnostics_spec.rs`

**Step 1: Write the failing test**

Add a new UI source-contract test similar to:

```rust
#[test]
fn app_window_uses_sarasa_ui_sc_as_the_shell_default() {
    let source = std::fs::read_to_string("ui/app-window.slint").unwrap();
    assert!(source.contains("default-font-family: AppTypography.ui-font-family;"));
    assert!(source.contains("default-font-weight: AppTypography.ui-font-weight-regular;"));
}
```

Update existing terminal tests so they expect:

```rust
"JetBrains Mono"
"Sarasa Term SC"
"Medium"
"JetBrainsMono-Medium.ttf"
"SarasaTermSC-Medium.ttf"
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test ui_typography_defaults_spec --test terminal_font_registration_smoke --test windows_terminal_typography_defaults_spec --test startup_font_memory_regression --test terminal_renderer_dwrite_spec --test windows_terminal_diagnostics_spec -- --nocapture
```

Expected: FAIL because the repository still describes `Cascadia Mono` and has no explicit UI typography contract.

**Step 3: Write minimal implementation**

Do not touch implementation code yet beyond what is required to make the new assertions meaningful.

**Step 4: Run test to verify it passes**

Re-run the same command after Tasks 2 and 3 are implemented.

**Step 5: Commit**

```bash
git add tests/ui_typography_defaults_spec.rs tests/terminal_font_registration_smoke.rs tests/windows_terminal_typography_defaults_spec.rs tests/startup_font_memory_regression.rs tests/terminal_renderer_dwrite_spec.rs tests/windows_terminal_diagnostics_spec.rs
git commit -m "test: lock typography refresh contracts"
```

### Task 2: Add the UI typography assets and wire the Slint shell to them

**Files:**
- Create: `assets/fonts/SarasaUiSC/SarasaUiSC-Regular.ttf`
- Create: `assets/fonts/SarasaUiSC/SarasaUiSC-SemiBold.ttf`
- Create: `assets/fonts/SarasaUiSC/LICENSE.txt`
- Create: `ui/theme/typography.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/titlebar-menu.slint`
- Modify: `build.rs`
- Test: `tests/ui_typography_defaults_spec.rs`

**Step 1: Write the failing test**

Extend the UI test so it also asserts:

```rust
assert!(Path::new("assets/fonts/SarasaUiSC/SarasaUiSC-Regular.ttf").exists());
assert!(Path::new("assets/fonts/SarasaUiSC/SarasaUiSC-SemiBold.ttf").exists());
assert!(source.contains("import \"../assets/fonts/SarasaUiSC/SarasaUiSC-Regular.ttf\";"));
assert!(source.contains("import { AppTypography } from \"theme/typography.slint\";"));
```

And in the popup contract:

```rust
let popup = std::fs::read_to_string("ui/components/titlebar-menu.slint").unwrap();
assert!(popup.contains("font-family: AppTypography.ui-font-family;"));
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test ui_typography_defaults_spec app_window_uses_sarasa_ui_sc_as_the_shell_default -- --nocapture
```

Expected: FAIL because the assets/theme file/imports do not exist yet.

**Step 3: Write minimal implementation**

- Add the approved UI assets and license under `assets/fonts/SarasaUiSC/`.
- Create `ui/theme/typography.slint` with constants such as:

```slint
export global AppTypography {
    out property <string> ui-font-family: "Sarasa UI SC";
    out property <int> ui-font-weight-regular: 400;
    out property <int> ui-font-weight-semibold: 600;
}
```

- In `ui/app-window.slint`:
  - import the `SarasaUiSC` font files
  - import `AppTypography`
  - set `default-font-family` and `default-font-weight`
- In `ui/components/titlebar-menu.slint`:
  - import `AppTypography`
  - set menu text to the shared UI family/regular weight
- Update `build.rs` so typography asset changes trigger rebuilds.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test ui_typography_defaults_spec -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
git add assets/fonts/SarasaUiSC build.rs ui/theme/typography.slint ui/app-window.slint ui/components/titlebar-menu.slint tests/ui_typography_defaults_spec.rs
git commit -m "feat: apply sarasa ui sc to the shell"
```

### Task 3: Switch the terminal renderer contract to JetBrains Mono Medium plus Sarasa Term SC Medium

**Files:**
- Create: `assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf`
- Create: `assets/fonts/JetBrainsMono/OFL.txt`
- Modify: `assets/fonts/SarasaTermSC/` (replace/add `SarasaTermSC-Medium.ttf` and keep license)
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `src/app/terminal_font/mod.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_atlas.rs`
- Modify: `src/app/terminal_font/mock.rs`
- Modify: `src/app/terminal_font/wezterm_font.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/terminal_font_registration_smoke.rs`
- Test: `tests/windows_terminal_typography_defaults_spec.rs`
- Test: `tests/startup_font_memory_regression.rs`
- Test: `tests/terminal_renderer_dwrite_spec.rs`
- Test: `tests/windows_terminal_diagnostics_spec.rs`

**Step 1: Write the failing test**

Update the terminal tests so they assert the new bundled contract:

```rust
assert!(backend_source.contains("pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = \"JetBrains Mono\";"));
assert!(backend_source.contains("pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = \"Medium\";"));
assert!(atlas_source.contains("assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf"));
assert!(dwrite_source.contains("assets/fonts/SarasaTermSC/SarasaTermSC-Medium.ttf"));
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test terminal_font_registration_smoke --test windows_terminal_typography_defaults_spec --test startup_font_memory_regression --test terminal_renderer_dwrite_spec --test windows_terminal_diagnostics_spec -- --nocapture
```

Expected: FAIL because the codebase still points at `Cascadia Mono` and `Regular`.

**Step 3: Write minimal implementation**

- Vendor `JetBrainsMono-Medium.ttf` and `OFL.txt`.
- Vendor `SarasaTermSC-Medium.ttf` and keep the Sarasa license.
- Update the shared terminal constants in `src/app/terminal_font/backend.rs`.
- Update all bundled font byte includes and fallback metadata in:
  - `src/app/terminal_font/windows_dwrite.rs`
  - `src/app/terminal_atlas.rs`
  - `src/app/terminal_font/mock.rs`
- Update stale comments/expectation strings in:
  - `src/app/terminal_font/wezterm_font.rs`
  - `src/app/bootstrap.rs`

Do not retune unrelated rendering behavior unless a test requires it.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test terminal_font_registration_smoke --test windows_terminal_typography_defaults_spec --test startup_font_memory_regression --test terminal_renderer_dwrite_spec --test windows_terminal_diagnostics_spec -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
git add assets/fonts/JetBrainsMono assets/fonts/SarasaTermSC src/app/terminal_font/backend.rs src/app/terminal_font/mod.rs src/app/terminal_font/windows_dwrite.rs src/app/terminal_atlas.rs src/app/terminal_font/mock.rs src/app/terminal_font/wezterm_font.rs src/app/bootstrap.rs tests/terminal_font_registration_smoke.rs tests/windows_terminal_typography_defaults_spec.rs tests/startup_font_memory_regression.rs tests/terminal_renderer_dwrite_spec.rs tests/windows_terminal_diagnostics_spec.rs
git commit -m "feat: refresh terminal typography defaults"
```

### Task 4: Verify the renderer/build still holds after the typography swap

**Files:**
- Modify only if verification exposes a real regression
- Test: `tests/terminal_atlas_renderer_spec.rs`
- Test: `tests/windows_directwrite_font_chain_spec.rs`
- Test: `tests/terminal_layout_harfbuzz_spec.rs`

**Step 1: Write the failing test**

Only add/adjust expectations if the font swap changes real renderer metrics or family-chain behavior.

**Step 2: Run test to verify current status**

Run:

```bash
cargo test --test terminal_atlas_renderer_spec --test windows_directwrite_font_chain_spec --test terminal_layout_harfbuzz_spec -- --nocapture
```

Expected: Either PASS directly or expose a small number of legitimate metric/family-name expectation updates.

**Step 3: Write minimal implementation**

If needed, update only the assertions or metric constants that changed because the approved bundled fonts changed. Do not turn this into a rendering retuning task.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test terminal_atlas_renderer_spec --test windows_directwrite_font_chain_spec --test terminal_layout_harfbuzz_spec -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
git add tests/terminal_atlas_renderer_spec.rs tests/windows_directwrite_font_chain_spec.rs tests/terminal_layout_harfbuzz_spec.rs src/app/terminal_atlas.rs src/app/terminal_font/windows_dwrite.rs
git commit -m "test: align renderer expectations with refreshed fonts"
```

### Task 5: Run final verification and package a Windows build

**Files:**
- Verify only

**Step 1: Run focused test suites**

```bash
cargo test --test ui_typography_defaults_spec --test terminal_font_registration_smoke --test windows_terminal_typography_defaults_spec --test startup_font_memory_regression --test terminal_renderer_dwrite_spec --test windows_terminal_diagnostics_spec --test terminal_atlas_renderer_spec --test windows_directwrite_font_chain_spec --test terminal_layout_harfbuzz_spec -- --nocapture
```

Expected: PASS

**Step 2: Run compile verification**

```bash
cargo check
```

Expected: PASS

**Step 3: Run Windows package verification**

```bash
./build-win-x64.sh
```

Expected: PASS and emit the staged Windows zip package.

**Step 4: Commit**

```bash
git add docs/plans/2026-04-09-ui-terminal-typography-refresh-design.md docs/plans/2026-04-09-ui-terminal-typography-refresh-implementation-plan.md
git commit -m "docs: record typography refresh plan"
```
