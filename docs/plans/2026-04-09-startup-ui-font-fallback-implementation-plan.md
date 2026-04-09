# Startup UI Font Fallback Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce real Windows startup private/commit by keeping Slint's startup text path off the eager system-font catalog until a runtime font miss actually requires system fallback.

**Architecture:** Vendor `i-slint-common` and `i-slint-core`, split shared font access into a light primary collection plus a lazy system collection, and make runtime font queries try primary first and system second. Seed the primary collection with bundled `DejaVuSans.ttf` so first-frame Latin UI text can resolve without triggering Windows system font enumeration.

**Tech Stack:** Rust, Slint 1.15.1, fontique, parley, cargo tests, Windows cross-build script

---

### Task 1: Vendor Slint font crates locally

**Files:**
- Modify: `Cargo.toml`
- Create: `vendor/i-slint-common/**`
- Create: `vendor/i-slint-core/**`

**Step 1: Write the failing test**

Use a source-contract test that references the future vendored paths so the repository fails until the crates are locally patched.

```rust
#[test]
fn slint_font_crates_are_vendored_for_startup_memory_work() {
    assert!(std::path::Path::new("vendor/i-slint-common/sharedfontique.rs").exists());
    assert!(std::path::Path::new("vendor/i-slint-core/graphics.rs").exists());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test startup_font_memory_regression slint_font_crates_are_vendored_for_startup_memory_work -- --nocapture`
Expected: FAIL because the vendored crates do not exist yet.

**Step 3: Write minimal implementation**

- Copy `i-slint-common-1.15.1` from cargo registry into `vendor/i-slint-common`
- Copy `i-slint-core-1.15.1` from cargo registry into `vendor/i-slint-core`
- Add `[patch.crates-io]` entries in `Cargo.toml` for both crates

**Step 4: Run test to verify it passes**

Run: `cargo test --test startup_font_memory_regression slint_font_crates_are_vendored_for_startup_memory_work -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml vendor/i-slint-common vendor/i-slint-core tests/startup_font_memory_regression.rs
git commit -m "build: vendor slint font crates for startup tuning"
```

### Task 2: Seed a no-system primary sharedfontique collection

**Files:**
- Modify: `vendor/i-slint-common/sharedfontique.rs`
- Test: `tests/startup_font_memory_regression.rs`

**Step 1: Write the failing test**

Add a source-contract test asserting:

```rust
#[test]
fn startup_primary_font_collection_disables_eager_system_font_scan() {
    let source = std::fs::read_to_string("vendor/i-slint-common/sharedfontique.rs").unwrap();
    assert!(source.contains("system_fonts: false"));
    assert!(source.contains("include_bytes!(\"sharedfontique/DejaVuSans.ttf\")"));
    assert!(source.contains("GenericFamily::SystemUi"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test startup_font_memory_regression startup_primary_font_collection_disables_eager_system_font_scan -- --nocapture`
Expected: FAIL because `sharedfontique` still uses the default system-font-enabled collection.

**Step 3: Write minimal implementation**

In `vendor/i-slint-common/sharedfontique.rs`:

- Build `COLLECTION` with `CollectionOptions { shared: true, system_fonts: false }`
- Always register bundled `DejaVuSans.ttf`
- Attach the bundled family to `SansSerif`, `SystemUi`, and `UiSansSerif`
- Keep `SLINT_DEFAULT_FONT` override support intact

**Step 4: Run test to verify it passes**

Run: `cargo test --test startup_font_memory_regression startup_primary_font_collection_disables_eager_system_font_scan -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add vendor/i-slint-common/sharedfontique.rs tests/startup_font_memory_regression.rs
git commit -m "perf: seed slint startup font collection without system scan"
```

### Task 3: Add lazy system-font fallback entry points

**Files:**
- Modify: `vendor/i-slint-common/sharedfontique.rs`
- Test: `tests/startup_font_memory_regression.rs`

**Step 1: Write the failing test**

Add a contract test asserting:

```rust
#[test]
fn startup_font_source_exposes_lazy_system_collection_helper() {
    let source = std::fs::read_to_string("vendor/i-slint-common/sharedfontique.rs").unwrap();
    assert!(source.contains("pub static SYSTEM_COLLECTION"));
    assert!(source.contains("pub fn get_system_collection() -> Collection"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test startup_font_memory_regression startup_font_source_exposes_lazy_system_collection_helper -- --nocapture`
Expected: FAIL because no lazy system collection helper exists yet.

**Step 3: Write minimal implementation**

Add a secondary lazy system-backed collection in `vendor/i-slint-common/sharedfontique.rs`:

- `SYSTEM_COLLECTION` with `system_fonts: true`
- `get_system_collection()` returning a clone
- Preserve bundled/default font registration behavior only where still necessary

**Step 4: Run test to verify it passes**

Run: `cargo test --test startup_font_memory_regression startup_font_source_exposes_lazy_system_collection_helper -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add vendor/i-slint-common/sharedfontique.rs tests/startup_font_memory_regression.rs
git commit -m "perf: expose lazy slint system font fallback collection"
```

### Task 4: Switch runtime font queries to primary-then-system lookup

**Files:**
- Modify: `vendor/i-slint-core/graphics.rs`
- Test: `tests/startup_font_memory_regression.rs`

**Step 1: Write the failing test**

Add a source-contract test asserting:

```rust
#[test]
fn startup_font_query_uses_primary_then_system_fallback() {
    let source = std::fs::read_to_string("vendor/i-slint-core/graphics.rs").unwrap();
    assert!(source.contains("let mut collection = sharedfontique::get_collection();"));
    assert!(source.contains("let mut system_collection = sharedfontique::get_system_collection();"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test startup_font_memory_regression startup_font_query_uses_primary_then_system_fallback -- --nocapture`
Expected: FAIL because `query_fontique()` only consults the single shared collection.

**Step 3: Write minimal implementation**

Update `vendor/i-slint-core/graphics.rs`:

- Keep the existing query setup in a small helper closure/function
- Query primary collection first
- On `None`, query `get_system_collection()` with the same families/attributes

**Step 4: Run test to verify it passes**

Run: `cargo test --test startup_font_memory_regression startup_font_query_uses_primary_then_system_fallback -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add vendor/i-slint-core/graphics.rs tests/startup_font_memory_regression.rs
git commit -m "perf: query startup slint fonts before system fallback"
```

### Task 5: Keep sharedparley on the lightweight startup context

**Files:**
- Modify: `vendor/i-slint-core/textlayout/sharedparley.rs`
- Test: `tests/startup_font_memory_regression.rs`

**Step 1: Write the failing test**

Add a source-contract test asserting:

```rust
#[test]
fn startup_sharedparley_context_stays_on_primary_collection() {
    let source = std::fs::read_to_string("vendor/i-slint-core/textlayout/sharedparley.rs").unwrap();
    assert!(source.contains("sharedfontique::COLLECTION.inner.clone()"));
    assert!(!source.contains("get_system_collection"));
}
```

**Step 2: Run test to verify it fails (or confirm current expectation before edits)**

Run: `cargo test --test startup_font_memory_regression startup_sharedparley_context_stays_on_primary_collection -- --nocapture`
Expected: PASS now or require minimal adjustment if the exact source strings differ.

**Step 3: Write minimal implementation**

Only change `sharedparley.rs` if needed to make the startup path explicitly depend on the primary collection and not on the system fallback helper.

**Step 4: Run test to verify it passes**

Run: `cargo test --test startup_font_memory_regression startup_sharedparley_context_stays_on_primary_collection -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add vendor/i-slint-core/textlayout/sharedparley.rs tests/startup_font_memory_regression.rs
git commit -m "perf: pin startup parley context to primary font collection"
```

### Task 6: Full verification and Windows build

**Files:**
- Modify: `tests/startup_font_memory_regression.rs`
- Verify: `vendor/i-slint-common/sharedfontique.rs`
- Verify: `vendor/i-slint-core/graphics.rs`
- Verify: `vendor/i-slint-core/textlayout/sharedparley.rs`

**Step 1: Run the focused regression suite**

Run: `cargo test --test startup_font_memory_regression -- --nocapture`
Expected: PASS

**Step 2: Run the existing Slint purge contract suite**

Run: `cargo test --test slint_backend_purge_contract_spec -- --nocapture`
Expected: PASS

**Step 3: Run repository type-check**

Run: `cargo check`
Expected: PASS

**Step 4: Run Windows release packaging**

Run: `./build-win-x64.sh`
Expected: PASS and refresh `dist/mica-term-x86_64-pc-windows-msvc-release-skia.zip`

**Step 5: Commit**

```bash
git add tests/startup_font_memory_regression.rs vendor/i-slint-common vendor/i-slint-core Cargo.toml
git commit -m "perf: defer slint system font fallback after startup"
```
