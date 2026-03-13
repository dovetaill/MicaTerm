# APT Dependency Inventory Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a root-level apt dependency inventory file and an interactive apt installer script for the current Windows build chain.

**Architecture:** Keep one human-readable markdown inventory in the repo root and one bash installer beside it. Protect both with a shell smoke test so package names, prompt flow, and cancellation behavior stay stable.

**Tech Stack:** bash, markdown, shell smoke tests

---

### Task 1: Lock The Contract With A Smoke Test

**Files:**
- Create: `tests/apt_packages_contract_smoke.sh`

**Step 1: Write the failing test**

The test should assert:

- `apt-packages.md` exists
- `install-apt-packages.sh` exists
- `bash -n install-apt-packages.sh` passes
- `install-apt-packages.sh --help` mentions the package list
- piping `n` into the script prints the package list and cancels cleanly

**Step 2: Run test to verify it fails**

Run: `bash tests/apt_packages_contract_smoke.sh`

Expected: fail because the new root files do not exist yet.

### Task 2: Add The Root Inventory File

**Files:**
- Create: `apt-packages.md`

**Step 1: Document the apt inventory**

Include:

- the packages actually installed during this Windows build work
- the apt prerequisites for the current build chain
- the direct Cargo dependencies and vendored patches from `Cargo.toml`

### Task 3: Add The Interactive Installer Script

**Files:**
- Create: `install-apt-packages.sh`

**Step 1: Implement the prompt flow**

The script should:

- print the package list first
- require explicit `y`
- run `apt-get update`
- run `apt-get install -y ...`
- print per-package install status
- print key command probes and non-apt follow-up reminders

### Task 4: Verify

**Files:**
- Test: `tests/apt_packages_contract_smoke.sh`

**Step 1: Run the smoke test**

Run: `bash tests/apt_packages_contract_smoke.sh`

Expected: pass.
