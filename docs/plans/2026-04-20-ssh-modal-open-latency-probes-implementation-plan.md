# SSH Modal Open Latency Probes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add precise modal-level latency probes for SSH `Connect` and `SaveAndConnect` so the remaining main-thread hitch can be measured before making deeper async refactors.

**Architecture:** Keep the runtime behavior unchanged and instrument the existing modal callback boundary in `assets_keychain.rs`. Emit new `app.async_latency` events for the synchronous modal path before `SessionManager::open_session()` and after the final sidebar/workspace sync, then document the new flow ids in the existing latency note.

**Tech Stack:** Rust, Slint UI callbacks, `tracing`, existing async latency probe conventions, source-level contract tests.

---

### Task 1: Guard the new SSH modal flows with failing tests

**Files:**
- Modify: `tests/async_latency_contract_spec.rs`

**Step 1: Write the failing test**

Add a source-level contract test that expects:

- `ssh-modal-connect`
- `ssh-modal-save-connect`
- `session-profile-built`
- `modal-confirmed`
- `secrets-persisted`
- `asset-catalog-saved`
- `session-dispatched`
- `ui-return`

The test should read `src/app/bootstrap.rs`, `src/app/bootstrap/assets_keychain.rs`, and `docs/plans/2026-04-20-async-latency-instrumentation.md`.

**Step 2: Run test to verify it fails**

Run: `cargo test --test async_latency_contract_spec`

Expected: FAIL because the new modal flow ids and stage names do not exist yet.

**Step 3: Commit**

```bash
git add tests/async_latency_contract_spec.rs
git commit -m "test: guard ssh modal latency probe contract"
```

### Task 2: Instrument the SSH modal callback without changing behavior

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`

**Step 1: Add the logging helper**

Create a helper alongside `log_ssh_async_latency(...)` that emits:

- target: `app.async_latency`
- flow: `ssh-modal-connect` or `ssh-modal-save-connect`
- stage
- elapsed fields
- session id when available
- asset id when available
- host
- user

**Step 2: Thread the helper through the modal callback**

Instrument `window.on_asset_ssh_modal_action_requested(...)` for:

- `Connect`
  - `session-profile-built`
  - `session-dispatched`
  - `ui-return`
- `SaveAndConnect`
  - `modal-confirmed`
  - `secrets-persisted`
  - `asset-catalog-saved`
  - `session-dispatched`
  - `ui-return`

The `ui-return` stage must be emitted after the callback finishes its synchronous sidebar/workspace sync work so the remaining main-thread blocking window is measurable.

**Step 3: Run the focused test**

Run: `cargo test --test async_latency_contract_spec`

Expected: PASS

**Step 4: Commit**

```bash
git add src/app/bootstrap.rs src/app/bootstrap/assets_keychain.rs tests/async_latency_contract_spec.rs
git commit -m "feat: add ssh modal latency probes"
```

### Task 3: Document how to read the new probes

**Files:**
- Modify: `docs/plans/2026-04-20-async-latency-instrumentation.md`
- Reference: `docs/plans/2026-04-20-ssh-modal-open-latency-probes-implementation-plan.md`

**Step 1: Extend the existing instrumentation note**

Document:

- flow ids `ssh-modal-connect` and `ssh-modal-save-connect`
- each stage name
- why `ui-return` here is different from `ssh-open ui-return`
- how to interpret a high modal `ui-return` versus a low modal `ui-return` with a later slow `ssh-open session-connected`

**Step 2: Run the focused test again**

Run: `cargo test --test async_latency_contract_spec`

Expected: PASS and docs mention the new flow ids/stages.

**Step 3: Final verification**

Run:

```bash
cargo test --test async_latency_contract_spec
git diff --stat
```

Expected:

- contract test passes
- diff only touches the probe implementation and docs

**Step 4: Commit**

```bash
git add docs/plans/2026-04-20-async-latency-instrumentation.md docs/plans/2026-04-20-ssh-modal-open-latency-probes-implementation-plan.md
git commit -m "docs: describe ssh modal latency instrumentation"
```
