# Terminal/UI Font Unification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the terminal render with a Sarasa-only primary text contract, move all non-terminal shell UI surfaces to bundled MiSans, and remove the retired bundled font families from the repository.

**Architecture:** The Rust terminal renderer keeps full ownership of terminal typography and converges every normal text path on `Sarasa Term SC`, with emoji-only fallback left intact. The Slint shell owns UI chrome typography through a shared `MiSans` contract, while build/package/docs/tests are rewritten so the repo only describes the approved bundled font story.

**Tech Stack:** Rust, Slint, cargo tests, DirectWrite fallback code, atlas renderer (`ab_glyph`/`swash`), shell packaging scripts, source-contract tests

---

### Task 1: Add failing contracts and bundled assets for the MiSans UI shell

**Files:**
- Create: `assets/fonts/MiSans/MiSans-Regular.ttf`
- Create: `assets/fonts/MiSans/MiSans-Semibold.ttf`
- Create: `assets/fonts/MiSans/LICENSE.txt`
- Modify: `ui/theme/typography.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/ui_typography_defaults_spec.rs`
- Modify: `build.rs`

**Step 1: Write the failing test**

Update `tests/ui_typography_defaults_spec.rs` so it asserts the new UI contract instead of the current `Sarasa UI SC` contract.

```rust
assert!(Path::new("assets/fonts/MiSans/MiSans-Regular.ttf").exists());
assert!(Path::new("assets/fonts/MiSans/MiSans-Semibold.ttf").exists());
assert!(Path::new("assets/fonts/MiSans/LICENSE.txt").exists());
assert!(source.contains("ui-font-family: \"MiSans\";"));
assert!(source.contains("import \"../assets/fonts/MiSans/MiSans-Regular.ttf\";"));
assert!(source.contains("import \"../assets/fonts/MiSans/MiSans-Semibold.ttf\";"));
assert!(!source.contains("SarasaUiSC"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test ui_typography_defaults_spec -q`
Expected: FAIL because `MiSans` assets do not exist yet and the Slint typography contract still says `Sarasa UI SC`.

**Step 3: Write minimal implementation**

Add the approved bundled `MiSans` files under `assets/fonts/MiSans/`, then update the shared UI typography contract and app-window imports.

```slint
export global AppTypography {
    out property <string> ui-font-family: "MiSans";
    out property <int> ui-font-weight-regular: 400;
    out property <int> ui-font-weight-semibold: 600;
}
```

```slint
import "../assets/fonts/MiSans/MiSans-Regular.ttf";
import "../assets/fonts/MiSans/MiSans-Semibold.ttf";
default-font-family: AppTypography.ui-font-family;
```

Update `build.rs` to watch the new `MiSans` assets and stop watching `assets/fonts/SarasaUiSC/...`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test ui_typography_defaults_spec -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add assets/fonts/MiSans build.rs ui/theme/typography.slint ui/app-window.slint tests/ui_typography_defaults_spec.rs
git commit -m "feat: add bundled misans ui contract"
```

### Task 2: Switch shared terminal defaults and atlas/mock paths to Sarasa Term SC

**Files:**
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `src/app/terminal_atlas.rs`
- Modify: `src/app/terminal_font/mock.rs`
- Modify: `tests/terminal_font_registration_smoke.rs`
- Modify: `tests/windows_terminal_typography_defaults_spec.rs`
- Modify: `tests/startup_font_memory_regression.rs`

**Step 1: Write the failing test**

Update the terminal contract tests so they describe `Sarasa Term SC` as the primary terminal family and stop expecting bundled `JetBrainsMono` assets.

```rust
assert!(backend_source.contains(
    "pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = \"Sarasa Term SC\";"
));
assert!(!backend_source.contains("JetBrains Mono"));
assert!(atlas_source.contains("assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf"));
assert!(!atlas_source.contains("assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf"));
assert!(Path::new("assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf").exists());
assert!(!Path::new("assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf").exists());
```

Where the existing tests currently expect `JetBrains Mono -> Sarasa Term SC -> Segoe UI Emoji`, replace that with a Sarasa-first contract and update the expected messaging accordingly.

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_font_registration_smoke --test windows_terminal_typography_defaults_spec --test startup_font_memory_regression -q`
Expected: FAIL because the backend, atlas, and mock font system still load JetBrains assets and still advertise the old mixed-family contract.

**Step 3: Write minimal implementation**

Update the terminal backend and local renderer loaders so normal terminal text uses `Sarasa Term SC` everywhere.

```rust
pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = "Sarasa Term SC";
pub const DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY: &str = DEFAULT_TERMINAL_FONT_FAMILY;
pub const WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN: &[&str] = &[
    DEFAULT_TERMINAL_FONT_FAMILY,
    DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY,
];
```

```rust
const TERMINAL_FONT_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf");
```

Make the same swap in `src/app/terminal_font/mock.rs` so test-only shaping/rasterization uses the same bundled family.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_font_registration_smoke --test windows_terminal_typography_defaults_spec --test startup_font_memory_regression -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_font/backend.rs src/app/terminal_atlas.rs src/app/terminal_font/mock.rs tests/terminal_font_registration_smoke.rs tests/windows_terminal_typography_defaults_spec.rs tests/startup_font_memory_regression.rs
git commit -m "feat: switch terminal defaults to sarasa"
```

### Task 3: Align the Windows DirectWrite/fallback path and audit private-use glyph coverage

**Files:**
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_font/windows_fallback.rs`
- Modify: `tests/windows_directwrite_font_chain_spec.rs`
- Modify: `tests/terminal_atlas_renderer_spec.rs`
- Reference: `ui/fonts/SarasaTermSCNerd-Regular.ttf`

**Step 1: Write the failing test**

Update the Windows/native and atlas regression tests so they stop expecting JetBrains/Fusion wording and instead require a Sarasa-owned path, while still guarding emoji/private-use behavior.

```rust
assert!(dwrite_source.contains("assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf"));
assert!(!dwrite_source.contains("assets/fonts/JetBrainsMono/JetBrainsMono-Regular.ttf"));
assert!(fallback_source.contains("DEFAULT_TERMINAL_FONT_FAMILY"));
assert!(!fallback_source.contains("JetBrains Mono"));
assert!(rendered_icon.sprite_kind == ClusterSpriteKind::MonoAlpha);
```

If the private-use glyph test currently encodes a Fusion/Maple-specific expectation, rewrite the assertion so it only requires a Sarasa-family-owned terminal glyph path.

**Step 2: Run test to verify it fails**

Run: `cargo test --test windows_directwrite_font_chain_spec --test terminal_atlas_renderer_spec -q`
Expected: FAIL because the Windows DirectWrite loader still bundles JetBrains Regular and the atlas regression still references the retired Fusion wording.

**Step 3: Write minimal implementation**

Update `src/app/terminal_font/windows_dwrite.rs` to load `SarasaTermSC-Regular.ttf` as the primary bundled face and keep only emoji-specific fallback candidates behind it.

```rust
const BUNDLED_TERMINAL_FONT_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf");
```

In `src/app/terminal_font/windows_fallback.rs`, keep the CJK helper logic only where it is still needed for script classification, but make the resolved family set deduplicate to `[primary, emoji]` for normal text.

Before deleting any Sarasa-family variant, inspect whether `ui/fonts/SarasaTermSCNerd-Regular.ttf` is still the only practical source for private-use terminal glyphs. If it is still needed, keep it as a Sarasa-family exception and update the tests/docs to explain that it is a terminal-only glyph-coverage helper, not a second product font family.

**Step 4: Run test to verify it passes**

Run: `cargo test --test windows_directwrite_font_chain_spec --test terminal_atlas_renderer_spec -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_font/windows_dwrite.rs src/app/terminal_font/windows_fallback.rs tests/windows_directwrite_font_chain_spec.rs tests/terminal_atlas_renderer_spec.rs
git commit -m "feat: align windows font fallback with sarasa"
```

### Task 4: Remove retired bundled font families and rewrite build/package/docs references

**Files:**
- Delete: `assets/fonts/JetBrainsMono`
- Delete: `assets/fonts/CascadiaMono`
- Delete: `assets/fonts/SarasaUiSC`
- Delete: `assets/fonts/Fusion-JetBrainsMapleMono`
- Modify: `build.rs`
- Modify: `build-desktop.sh`
- Modify: `readme.md`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/terminal_font_registration_smoke.rs`
- Modify: `tests/startup_font_memory_regression.rs`

**Step 1: Write the failing test**

Update packaging/docs contract tests so they require the new bundled-license story and stop expecting the removed font directories.

```rust
assert!(content.contains("assets/fonts/MiSans/LICENSE.txt"));
assert!(content.contains("assets/fonts/SarasaTermSC/LICENSE.txt"));
assert!(!content.contains("assets/fonts/JetBrainsMono/OFL.txt"));
assert!(!content.contains("assets/fonts/SarasaUiSC/LICENSE.txt"));
assert!(!Path::new("assets/fonts/JetBrainsMono").exists());
assert!(!Path::new("assets/fonts/CascadiaMono").exists());
assert!(!Path::new("assets/fonts/SarasaUiSC").exists());
assert!(!Path::new("assets/fonts/Fusion-JetBrainsMapleMono").exists());
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test runtime_profile --test terminal_font_registration_smoke --test startup_font_memory_regression -q`
Expected: FAIL because build/package/docs still mention the retired font families and the old directories still exist.

**Step 3: Write minimal implementation**

Remove the retired asset directories and rewrite the scripts/docs accordingly.

```bash
rm -rf assets/fonts/JetBrainsMono assets/fonts/CascadiaMono assets/fonts/SarasaUiSC assets/fonts/Fusion-JetBrainsMapleMono
```

Then update `build-desktop.sh` so license staging creates only the approved font directories, for example:

```bash
mkdir -p "$license_root/MiSans" "$license_root/SarasaTermSC"
cp "$ROOT_DIR/assets/fonts/MiSans/LICENSE.txt" "$license_root/MiSans/LICENSE.txt"
cp "$ROOT_DIR/assets/fonts/SarasaTermSC/LICENSE.txt" "$license_root/SarasaTermSC/LICENSE.txt"
```

Rewrite `readme.md` so it no longer claims the atlas or startup shell use the retired families.

**Step 4: Run test to verify it passes**

Run: `cargo test --test runtime_profile --test terminal_font_registration_smoke --test startup_font_memory_regression -q`
Expected: PASS.

**Step 5: Commit**

```bash
git add build.rs build-desktop.sh readme.md tests/runtime_profile.rs tests/terminal_font_registration_smoke.rs tests/startup_font_memory_regression.rs
git add -A assets/fonts
git commit -m "refactor: retire legacy bundled font families"
```

### Task 5: Run end-to-end verification and package validation

**Files:**
- Reference: `docs/plans/2026-04-13-terminal-ui-font-unification-design.md`
- Reference: `docs/plans/2026-04-13-terminal-ui-font-unification-implementation-plan.md`

**Step 1: Run the focused typography/font suite**

Run:

```bash
cargo test --test ui_typography_defaults_spec --test terminal_font_registration_smoke --test windows_terminal_typography_defaults_spec --test startup_font_memory_regression --test windows_directwrite_font_chain_spec --test terminal_atlas_renderer_spec --test runtime_profile -q
```

Expected: PASS.

**Step 2: Run the full compile verification**

Run: `cargo check`
Expected: PASS.

**Step 3: Run Windows packaging verification**

Run: `./build-win-x64.sh`
Expected: PASS and produce the packaged Windows artifact without references to retired font-license bundles.

**Step 4: Inspect the repo state before the final commit**

Run:

```bash
git status --short
git log --oneline -5
```

Expected: only intentional font-unification changes are present, and the worktree branch has the task commits listed in order.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: unify terminal and ui font bundles"
```
