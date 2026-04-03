# Muda Lifetime Warning Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the `mismatched_lifetime_syntaxes` warning from the vendored `muda` crate so both Windows build wrappers produce cleaner output.

**Architecture:** Add a targeted regression smoke test that fails when the warning appears during `cargo check -p muda` for Windows targets, then make the minimal lifetime-signature change in `vendor/muda/src/platform_impl/mod.rs`. Finish by rerunning the smoke test and both Windows wrapper builds.

**Tech Stack:** Bash smoke tests, Cargo, Rust, vendored `muda`, Windows cross-target packaging scripts

---

### Task 1: Pin the warning regression with a shell smoke test

**Files:**
- Create: `tests/muda_lifetime_warning_smoke.sh`
- Reference: `vendor/muda/src/platform_impl/mod.rs`

**Step 1: Write the failing test**

Create a smoke test that:
- runs `cargo check -p muda --target x86_64-pc-windows-msvc --quiet`
- runs `cargo check -p muda --target x86_64-pc-windows-gnu --quiet`
- fails when stderr contains `mismatched_lifetime_syntaxes`

**Step 2: Run test to verify it fails**

Run:

```bash
bash tests/muda_lifetime_warning_smoke.sh
```

Expected: FAIL because the vendored signatures still emit the lifetime warning.

**Step 3: Write minimal implementation**

Do not change production code in this task. Adjust only the test until it fails
for the intended warning text.

**Step 4: Run test to verify it fails correctly**

Run:

```bash
bash tests/muda_lifetime_warning_smoke.sh
```

Expected: FAIL for the warning match, not because the script is malformed.

**Step 5: Commit**

```bash
git add tests/muda_lifetime_warning_smoke.sh
git commit -m "test: pin muda lifetime warning regression"
```

### Task 2: Apply the minimal vendored fix

**Files:**
- Modify: `vendor/muda/src/platform_impl/mod.rs`
- Test: `tests/muda_lifetime_warning_smoke.sh`

**Step 1: Use the failing smoke test**

Keep `tests/muda_lifetime_warning_smoke.sh` as the red state.

**Step 2: Run test to verify it fails**

Run:

```bash
bash tests/muda_lifetime_warning_smoke.sh
```

Expected: FAIL before the source fix.

**Step 3: Write minimal implementation**

Change only:
- `Ref<MenuChild>` -> `Ref<'_, MenuChild>`
- `RefMut<MenuChild>` -> `RefMut<'_, MenuChild>`

No other vendored refactors or lint suppression.

**Step 4: Run test to verify it passes**

Run:

```bash
bash tests/muda_lifetime_warning_smoke.sh
```

Expected: PASS with no matched warning output.

**Step 5: Commit**

```bash
git add vendor/muda/src/platform_impl/mod.rs tests/muda_lifetime_warning_smoke.sh
git commit -m "fix: remove vendored muda lifetime warnings"
```

### Task 3: Verify both Windows wrapper builds stay clean

**Files:**
- Reference: `build-win-x64.sh`
- Reference: `build-win-x64-software.sh`

**Step 1: Run the mainline Windows build**

Run:

```bash
./build-win-x64.sh
```

Expected: PASS without the original `mismatched_lifetime_syntaxes` warning.

**Step 2: Run the software compatibility Windows build**

Run:

```bash
./build-win-x64-software.sh
```

Expected: PASS without the original `mismatched_lifetime_syntaxes` warning.

**Step 3: Review the diff**

Run:

```bash
git status --short
git diff -- vendor/muda/src/platform_impl/mod.rs tests/muda_lifetime_warning_smoke.sh docs/plans/2026-04-03-muda-lifetime-warning-cleanup-design.md docs/plans/2026-04-03-muda-lifetime-warning-cleanup-implementation-plan.md
```

Expected: Only the planned vendored signature fix, regression test, and design
docs remain.

**Step 4: Commit**

Run:

```bash
git add vendor/muda/src/platform_impl/mod.rs tests/muda_lifetime_warning_smoke.sh docs/plans/2026-04-03-muda-lifetime-warning-cleanup-design.md docs/plans/2026-04-03-muda-lifetime-warning-cleanup-implementation-plan.md
git commit -m "fix: clean vendored muda build warnings"
```
