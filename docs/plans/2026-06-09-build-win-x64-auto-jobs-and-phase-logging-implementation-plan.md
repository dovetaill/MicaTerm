# Build Win x64 Auto Jobs and Phase Logging Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `./build-win-x64.sh` auto-detect a default cargo job count and teach the shared build path to log parallel-compile vs final-link phases clearly.

**Architecture:** Keep the new default behavior wrapper-local in `build-win-x64.sh`, but preserve shared command assembly and cargo execution in `build-desktop.sh`. Detect a wrapper default job count before delegation, surface its origin through shared logging, and stream cargo output through a small watcher that announces the final root-crate phase before staging/archive work begins.

**Tech Stack:** Bash, cargo/cargo-xwin wrapper scripts, shell smoke tests, Markdown docs.

---

### Task 1: Lock the new wrapper default and phase logs with a failing smoke test

**Files:**
- Modify: `tests/build_jobs_script_smoke.sh`
- Reference: `build-win-x64.sh`
- Reference: `build-desktop.sh`

**Step 1: Write the failing test**

Add assertions that:
- `build-desktop.sh` without `BUILD_JOBS` still logs `build jobs: default`
- `build-win-x64.sh` without `BUILD_JOBS` auto-detects a value from fake `nproc`
- wrapper logs include `phase 1/3`, `phase 2/3`, and `phase 3/3`
- fake cargo log contains `--jobs <detected>` for the wrapper-default path

**Step 2: Run test to verify it fails**

Run: `bash tests/build_jobs_script_smoke.sh`
Expected: FAIL because the current wrapper does not auto-detect jobs and the shared script does not emit phase logs.

**Step 3: Write minimal implementation**

No implementation in this task.

**Step 4: Re-run the targeted test after implementation**

Run: `bash tests/build_jobs_script_smoke.sh`
Expected: PASS.

### Task 2: Add wrapper-local auto job detection

**Files:**
- Modify: `build-win-x64.sh`
- Reference: `readme.md`

**Step 1: Implement detection helper**

Add a small helper that, when `BUILD_JOBS` is unset, probes:

```bash
nproc
getconf _NPROCESSORS_ONLN
NUMBER_OF_PROCESSORS
```

and exports both `BUILD_JOBS` and metadata describing the source.

**Step 2: Keep caller overrides authoritative**

If `BUILD_JOBS` is already set, do nothing so explicit caller input still wins.

**Step 3: Run targeted smoke test**

Run: `bash tests/build_jobs_script_smoke.sh`
Expected: still FAIL until shared logging is implemented, but the wrapper job forwarding assertions should move closer to passing.

### Task 3: Add shared phase-aware cargo logging

**Files:**
- Modify: `build-desktop.sh`
- Test: `tests/build_jobs_script_smoke.sh`

**Step 1: Add job-source-aware log formatting**

Teach `resolve_build_jobs` to print either:

```text
BUILD_JOBS=32 -> --jobs 32
```

or:

```text
auto-detected 32 via nproc -> --jobs 32
```

based on wrapper metadata.

**Step 2: Wrap cargo execution with a line watcher**

Before cargo starts, print phase 1. While streaming cargo output, detect root-crate compilation lines and print phase 2 once. After cargo succeeds but before staging, print phase 3.

**Step 3: Run targeted smoke test**

Run: `bash tests/build_jobs_script_smoke.sh`
Expected: PASS.

### Task 4: Update user-facing docs

**Files:**
- Modify: `readme.md`
- Modify: `build-win-x64.sh`
- Modify: `build-desktop.sh`

**Step 1: Update help/readme text**

Document that `./build-win-x64.sh` now auto-detects jobs by default, while `BUILD_JOBS` remains the explicit override.

**Step 2: Mention the new phase logs**

Explain that the build output now announces when it enters the final root-crate compile/link phase where visible parallelism often drops.

**Step 3: Run focused greps**

Run:

```bash
rg -n "BUILD_JOBS|auto-detect|phase 1/3|phase 2/3|phase 3/3" readme.md build-win-x64.sh build-desktop.sh
```

Expected: updated docs and log strings appear in the intended files.

### Task 5: Verify the final behavior

**Files:**
- Verify: `tests/build_jobs_script_smoke.sh`
- Verify: `build-win-x64.sh`
- Verify: `build-desktop.sh`
- Verify: `readme.md`

**Step 1: Run targeted regression coverage**

Run: `bash tests/build_jobs_script_smoke.sh`
Expected: PASS.

**Step 2: Run shared script smoke if needed**

Run: `bash tests/build_desktop_script_smoke.sh`
Expected: PASS.

**Step 3: Inspect diff**

Run: `git diff -- build-win-x64.sh build-desktop.sh tests/build_jobs_script_smoke.sh readme.md docs/plans/2026-06-09-build-win-x64-auto-jobs-and-phase-logging-design.md docs/plans/2026-06-09-build-win-x64-auto-jobs-and-phase-logging-implementation-plan.md`
Expected: changes stay narrowly scoped to wrapper auto-detection, shared logging, tests, and docs.
