# SSH Status Reference Alignment Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the SSH connection status workspace so `verifying_host_key_with_jump_host` closely matches `dist/tmp/1.png`, while also supporting five additional stable preview states inside the MCA TERM shell.

**Architecture:** Keep the existing SSH runtime and workspace shell intact, but replace the `connection-progress` surface with a reference-aligned single-column product page. Add preview-state fixtures and hop-driven view data so the UI can render direct, jump-host, warning, and failed flows from the same layout.

**Tech Stack:** Rust, Slint, SVG assets, existing SSH connection progress projection.

---

### Task 1: Document and scaffold the reference-aligned workstream

**Files:**
- Create: `docs/plans/2026-04-29-ssh-status-reference-alignment-design.md`
- Create: `docs/plans/2026-04-29-ssh-status-reference-alignment-implementation-plan.md`

**Step 1: Save the approved design constraints**
- Record that `dist/tmp/1.png` is the single visual source of truth.
- Record the required six preview states.
- Record the forbidden patterns: left narrow rail, fixed-bottom details, failed-only layout, mono UI typography.

**Step 2: Verify the plan file exists**
Run: `test -f docs/plans/2026-04-29-ssh-status-reference-alignment-implementation-plan.md`
Expected: exit code 0.

**Step 3: Commit later with implementation**
- Do not create a docs-only commit; include documentation in the feature commit.

### Task 2: Add stable preview/demo states without breaking runtime logic

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/ssh/connection_progress.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/terminal-session-host.slint`

**Step 1: Write the failing search/assertion step**
Run: `rg -n "verifying_host_key_direct|verifying_host_key_with_jump_host|connecting_trusted_direct|connecting_trusted_with_jump_host|host_key_changed_warning|failed_jump_host" src ui`
Expected: no matches before implementation.

**Step 2: Add preview-state fixture model**
- Introduce a lightweight preview selector/read path that can be driven by env var, dev flag, or demo profile.
- Ensure the six required states can be selected deterministically.

**Step 3: Wire preview data into existing connection progress projection**
- Populate session title, badge/headline, hop data, host key info, details, and diagnostics from fixtures.
- Ensure real SSH runtime is untouched when preview is not enabled.

**Step 4: Verify the new fixtures are discoverable**
Run: `rg -n "verifying_host_key_direct|verifying_host_key_with_jump_host|connecting_trusted_direct|connecting_trusted_with_jump_host|host_key_changed_warning|failed_jump_host" src ui`
Expected: six states found in the new preview scaffolding.

### Task 3: Replace the current connection-progress layout with the reference page structure

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/theme/typography.slint`

**Step 1: Remove the left-rail layout from the connection-progress surface**
- Replace `summary-header + workflow-rail + current-task-panel + diagnostics-only section` with a single-column page.

**Step 2: Build the page header**
- Title, status badge, and no enclosing summary card.

**Step 3: Build the horizontal hop chain**
- Render hop items from data.
- Support direct, single jump host, and multi-hop expansion.
- Add clearer `current` / `failed` visual hierarchy.

**Step 4: Build the state-driven main card**
- `Verify host key`
- `Connecting`
- `Host key changed`
- `Connection failed`
- Keep one shared shell and swap only body/content/actions.

**Step 5: Build a real collapsible details section**
- Keep it below the main card.
- Show structured details, not diagnostics only.

**Step 6: Move actions into one bottom action bar**
- Left tertiary cancel.
- Right state-specific primary actions.

### Task 4: Replace icons and tighten the visual system around the reference

**Files:**
- Create: `assets/icons/ssh-flow/ssh-local-device.svg`
- Create: `assets/icons/ssh-flow/ssh-jump-host.svg`
- Create: `assets/icons/ssh-flow/ssh-target-server.svg`
- Create: `assets/icons/ssh-flow/ssh-verify-shield-key.svg`
- Create: `assets/icons/ssh-flow/ssh-connection-details-gear.svg`
- Create: `assets/icons/ssh-flow/ssh-copy.svg`
- Create: `assets/icons/ssh-flow/ssh-chevron-right.svg`
- Create: `assets/icons/ssh-flow/ssh-status-waiting.svg`
- Create: `assets/icons/ssh-flow/ssh-status-warning.svg`
- Create: `assets/icons/ssh-flow/ssh-status-failed.svg`
- Modify: `ui/shell/terminal-session-host.slint`

**Step 1: Create a consistent SVG family**
- Use 24x24 or 32x32 viewBox, rounded joins, consistent stroke weight.

**Step 2: Replace old connection-progress icon usage**
- Stop relying on old or generic fluent icons in this page.

**Step 3: Verify the assets are in place**
Run: `find assets/icons/ssh-flow -maxdepth 1 -type f | sort`
Expected: all 10 SVG files listed.

### Task 5: Verify preview states, capture output, and prepare the final commit

**Files:**
- Modify: any touched files above as needed for fixes
- Output: `dist/tmp/ssh-status-preview-*.png` if capture tooling is available

**Step 1: Run formatting/build checks**
Run: `cargo fmt --all --manifest-path Cargo.toml`
Expected: formatting completes.

**Step 2: Run targeted compile/tests**
Run: `cargo test -q --manifest-path Cargo.toml --no-run`
Expected: compile succeeds.

**Step 3: Verify all six preview states manually or with screenshots**
- `verifying_host_key_direct`
- `verifying_host_key_with_jump_host`
- `connecting_trusted_direct`
- `connecting_trusted_with_jump_host`
- `host_key_changed_warning`
- `failed_jump_host`

**Step 4: Commit the feature**
```bash
git add docs/plans/2026-04-29-ssh-status-reference-alignment-design.md \
        docs/plans/2026-04-29-ssh-status-reference-alignment-implementation-plan.md \
        assets/icons/ssh-flow \
        src/app/bootstrap.rs \
        src/app/ssh/connection_progress.rs \
        ui/app-window.slint \
        ui/shell/workspace-pane.slint \
        ui/shell/terminal-session-host.slint \
        ui/theme/tokens.slint \
        ui/theme/typography.slint

git commit -m "feat: align ssh status page with reference workspace"
```
