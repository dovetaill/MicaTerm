# Terminal Color Emoji Rendering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add true color emoji rendering to terminal body cells on Windows and Linux while preserving the current Sarasa-based atlas path for normal terminal text.

**Architecture:** Keep [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs) as the single terminal surface compositor, but split cluster rendering into a mono Sarasa path and a color emoji fallback path. The emoji path will use a new [`src/app/terminal_emoji.rs`](../../src/app/terminal_emoji.rs) module backed by `fontdb`, `swash`, and `unicode-properties`, and the sprite cache will be extended from alpha-only masks to dual mono/RGBA sprite kinds.

**Tech Stack:** Rust, Slint, `ab_glyph`, `fontdb`, `swash`, `unicode-properties`, existing terminal atlas tests

---

### Task 1: Lock The New Emoji Rendering Contract In Tests

**Files:**
- Create: `tests/terminal_color_emoji_spec.rs`
- Modify: `tests/terminal_atlas_renderer_spec.rs`
- Reference: `src/app/ssh/runtime.rs`
- Reference: `src/app/terminal_atlas.rs`

**Step 1: Write the failing test**

Add focused tests that lock these behaviors:
- emoji-presenting terminal cells must no longer disappear into fully blank pixels;
- Nerd Font private-use glyphs must stay on the mono path;
- repeated emoji cells must be cacheable instead of re-rasterized every frame.

Suggested coverage:

```rust
#[test]
fn emoji_clusters_are_not_treated_as_blank_terminal_cells() { /* ... */ }

#[test]
fn nerd_font_private_use_cells_do_not_route_to_emoji_rendering() { /* ... */ }
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_color_emoji_spec --test terminal_atlas_renderer_spec -q`

Expected: FAIL because the current atlas renderer still renders missing emoji clusters as transparent sprites.

**Step 3: Write minimal implementation**

Introduce only the minimum public seams needed for testing:
- a dedicated emoji renderer module target in the crate graph;
- sprite-kind observability or fake-renderer injection sufficient for tests to assert behavior.

Do not implement real system emoji rendering yet; only add enough structure so the failing tests express the desired contract.

**Step 4: Run test to verify it passes or fails for the right next reason**

Run: `cargo test --test terminal_color_emoji_spec --test terminal_atlas_renderer_spec -q`

Expected: tests now fail on the missing real emoji path rather than on missing test seams.

**Step 5: Commit**

```bash
git add tests/terminal_color_emoji_spec.rs tests/terminal_atlas_renderer_spec.rs src/app/terminal_atlas.rs
git commit -m "test: lock terminal color emoji renderer contract"
```

### Task 2: Add Emoji Classification And Resolver Infrastructure

**Files:**
- Create: `src/app/terminal_emoji.rs`
- Modify: `src/app/mod.rs`
- Modify: `Cargo.toml`
- Test: `tests/terminal_color_emoji_spec.rs`

**Step 1: Write the failing test**

Add unit/integration coverage for:
- `🦀`, `📦`, `🌐`, and VS16/ZWJ sequences being classified as emoji-rendered clusters;
- `` and normal ASCII text remaining mono-rendered clusters;
- missing preferred emoji fonts resolving to an explicit visible-fallback decision instead of a silent blank.

Suggested API shape:

```rust
pub enum ClusterRenderKind {
    Mono,
    Emoji,
}

pub fn classify_cluster_render_kind(text: &str) -> ClusterRenderKind { /* ... */ }
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_color_emoji_spec classify_cluster_render_kind -q`

Expected: FAIL because there is no classifier or resolver module yet.

**Step 3: Write minimal implementation**

Implement:
- `ClusterRenderKind`
- emoji property detection based on `unicode-properties`
- preferred-family selection helpers for Windows and Linux
- an explicit fallback outcome for “no color emoji font available”

Add dependencies in [`Cargo.toml`](../../Cargo.toml):

```toml
fontdb = "..."
swash = "..."
unicode-properties = "..."
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_color_emoji_spec classify_cluster_render_kind -q`

Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml src/app/mod.rs src/app/terminal_emoji.rs tests/terminal_color_emoji_spec.rs
git commit -m "feat: add terminal emoji classification infrastructure"
```

### Task 3: Implement System Emoji Rasterization

**Files:**
- Modify: `src/app/terminal_emoji.rs`
- Test: `tests/terminal_color_emoji_spec.rs`
- Reference: `src/app/ssh/runtime.rs`

**Step 1: Write the failing test**

Add coverage that proves the emoji renderer can return an RGBA sprite result and that failure cases return a visible fallback instead of transparency.

Suggested API shape:

```rust
pub struct EmojiSprite {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn rasterize_cluster(&self, text: &str, span: u32, cell_width: u32, cell_height: u32)
    -> EmojiRenderOutcome { /* ... */ }
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_color_emoji_spec emoji_rasterizer -q`

Expected: FAIL because the resolver exists but no real color sprite rendering exists yet.

**Step 3: Write minimal implementation**

In [`src/app/terminal_emoji.rs`](../../src/app/terminal_emoji.rs):
- load system font metadata with `fontdb`;
- pick the preferred emoji family for the current platform;
- use `swash` to shape and rasterize color glyph content into RGBA pixels;
- center the rendered emoji inside the terminal cell span supplied by the caller;
- emit a replacement fallback outcome on rasterization failure.

Keep the output API terminal-focused: it should return ready-to-blit RGBA sprite data, not expose general text layout abstractions.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_color_emoji_spec emoji_rasterizer -q`

Expected: PASS for deterministic fake/injected coverage and PASS for non-font-dependent resolver behavior

**Step 5: Commit**

```bash
git add src/app/terminal_emoji.rs tests/terminal_color_emoji_spec.rs
git commit -m "feat: add system color emoji rasterizer"
```

### Task 4: Integrate Emoji Sprites Into The Atlas Renderer

**Files:**
- Modify: `src/app/terminal_atlas.rs`
- Modify: `tests/terminal_atlas_renderer_spec.rs`
- Reference: `src/app/terminal_emoji.rs`

**Step 1: Write the failing test**

Extend atlas renderer coverage so it proves:
- emoji clusters are composited as RGBA sprites into the terminal surface;
- selection/background repainting does not erase emoji pixels;
- repeated emoji clusters reuse cached sprite entries across frames.

Suggested assertion shape:

```rust
assert!(
    frame.image.to_rgba8().unwrap().as_slice() != before.as_slice(),
    "emoji sprite composition should visibly change the terminal surface"
);
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_atlas_renderer_spec -q`

Expected: FAIL because the atlas cache and blitter are still alpha-only.

**Step 3: Write minimal implementation**

Refactor [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs):
- replace the current cached sprite struct with a dual-kind enum such as:

```rust
enum CachedClusterSprite {
    MonoAlpha { width: u32, height: u32, alpha: Vec<u8> },
    ColorRgba { width: u32, height: u32, rgba: Vec<Rgba8Pixel> },
}
```

- route emoji-presenting clusters to the new emoji rasterizer;
- keep all other clusters on the current Sarasa `ab_glyph` path;
- add an RGBA blit path alongside the existing alpha-mask blitter;
- keep row hashing, selection fills, and cursor logic unchanged apart from sprite-kind support.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_atlas_renderer_spec -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_atlas.rs tests/terminal_atlas_renderer_spec.rs
git commit -m "feat: composite color emoji into terminal atlas"
```

### Task 5: Document Runtime Expectations And Verify

**Files:**
- Modify: `readme.md`
- Reference: `src/app/terminal_emoji.rs`
- Reference: `src/app/terminal_atlas.rs`

**Step 1: Write the failing doc/test expectation**

Add or update documentation to state:
- terminal body now supports color emoji through system emoji fonts;
- Windows expects `Segoe UI Emoji`;
- Linux expects a system color emoji font such as `Noto Color Emoji`;
- missing-font environments fall back to a visible replacement glyph plus diagnostic log.

If there is an existing renderer contract test that references terminal font behavior only, update it so documentation and tests agree.

**Step 2: Run focused checks**

Run: `cargo test --test terminal_color_emoji_spec --test terminal_atlas_renderer_spec -q`

Expected: PASS

**Step 3: Run compile verification**

Run: `cargo check`

Expected: PASS

**Step 4: Review diff**

Run: `git diff -- Cargo.toml src/app/mod.rs src/app/terminal_emoji.rs src/app/terminal_atlas.rs tests/terminal_color_emoji_spec.rs tests/terminal_atlas_renderer_spec.rs readme.md`

Expected: only terminal emoji renderer changes relevant to this feature

**Step 5: Commit**

```bash
git add Cargo.toml src/app/mod.rs src/app/terminal_emoji.rs src/app/terminal_atlas.rs tests/terminal_color_emoji_spec.rs tests/terminal_atlas_renderer_spec.rs readme.md
git commit -m "feat: add terminal color emoji rendering"
```

### Task 6: Manual Platform Smoke Verification

**Files:**
- Reference only

**Step 1: Windows smoke**

Launch the app on Windows and verify terminal output for:
- `echo 📦`
- `echo 🦀`
- `echo 🌐`
- one ZWJ sequence such as `👨‍💻`

Expected: true color emoji render inside the terminal grid with stable alignment.

**Step 2: Linux smoke**

Launch the app on Linux with `Noto Color Emoji` installed and verify the same sequence.

Expected: true color emoji render inside the terminal grid with stable alignment.

**Step 3: Linux missing-font smoke**

Temporarily run on a Linux environment without a color emoji font.

Expected: visible replacement glyph plus diagnostic log, not a blank cell.
