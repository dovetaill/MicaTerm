# Terminal Font Tightening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Tighten the bundled terminal typography so the software renderer reads less loose and less thin without introducing a new font asset.

**Architecture:** Keep the current bundled `SarasaTermSCNerd-Regular.ttf` asset, but retune the renderer-side typography contract. The implementation should first lock down tighter metrics expectations in tests, then adjust atlas/native font metrics and glyph coverage mapping with the smallest possible code change.

**Tech Stack:** Rust, Slint, `ab_glyph`, terminal atlas renderer, source-level contract tests

---

### Task 1: Lock down tighter bundled font contracts

**Files:**
- Modify: `tests/terminal_atlas_renderer_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

- Tighten the atlas metrics expectations so the bundled Sarasa renderer must expose denser width/height values than the current loose contract.
- Add a source-level contract assertion that the native font backend no longer uses the current oversized default `17.5` px sizing.

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_loads_sarasa_metrics_and_emits_a_surface_image -q`
Expected: FAIL because the current metrics are still tuned for the looser contract.

Run: `cargo test --test terminal_renderer_dwrite_spec windows_dwrite_font_backend_source_exposes_rasterization_contract -q`
Expected: FAIL once the new source assertion is added and the backend still references the old sizing contract.

**Step 3: Write minimal implementation**

- Retune the atlas renderer constants to reduce loose spacing and slightly increase glyph darkness.
- Retune the shared native font request size and backend metrics so native and software paths stay aligned.

**Step 4: Run targeted tests to verify they pass**

Run: `cargo test --test terminal_atlas_renderer_spec --test terminal_renderer_dwrite_spec -q`
Expected: PASS

**Step 5: Run workspace verification**

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS
