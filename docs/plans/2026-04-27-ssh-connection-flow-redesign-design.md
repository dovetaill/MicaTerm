# SSH Connection Flow Redesign Design

## Goal

Redesign the workspace SSH connection status page into a mature, task-focused connection flow that feels like a premium desktop SSH client instead of a stacked debug/status page.

This redesign is scoped to the existing workspace `connection-progress` page rendered by `ui/shell/terminal-session-host.slint`. It does not change the underlying SSH handshake behavior, host-key trust policy, or retry semantics beyond what is needed to present them more clearly.

## Confirmed Product Direction

The approved direction is a **blocking transition page**:

- the page exists to help the user get connected quickly;
- it is not a long-lived diagnostics workbench;
- diagnostics stay available, but secondary;
- when the flow blocks on a user decision, that decision becomes the page's only real focus;
- when the flow fails, the same page skeleton remains, but the content upgrades into a stronger troubleshooting mode.

## External Research Summary

### VS Code Remote - SSH

VS Code keeps the primary connection UI lightweight and moves detailed logs into the `Remote - SSH` output channel. The main product surface answers only the key task questions while detailed debugging stays out of the way until explicitly requested.

Reference:
- <https://code.visualstudio.com/docs/remote/ssh>

### JetBrains Gateway

JetBrains Gateway uses a wizard-like flow with a single dominant primary action such as `Check Connection and Continue`. Connection setup is treated as a guided product flow, while logs and diagnostics are collected through explicit support / troubleshooting affordances.

References:
- <https://www.jetbrains.com/help/idea/remote-development-a.html>
- <https://www.jetbrains.com/help/idea/remote-development-troubleshooting.html>

### SecureCRT

SecureCRT treats trace and debug logging as explicit secondary capabilities. Logging is important, but it is clearly separated from the user's main session-starting task.

Reference:
- <https://www.vandyke.com/support/tips/configure-trace-options-debug-logging-in-securecrt.html>

### Termius

Termius documentation treats host identity, host chaining, authentication, and troubleshooting stages as first-class product concepts. That suggests the page should present the connection path and blocking stage as a polished product capability, not as raw protocol internals.

References:
- <https://docs.termius.com/organize-and-connect-to-hosts/connecting-to-a-server>
- <https://docs.termius.com/help-center/troubleshooting/i-cant-connect-to-a-host>

## Current Code Map

### Primary UI Surface

- `ui/shell/terminal-session-host.slint`
  - owns the current `connection-progress` branch;
  - currently renders:
    - `header-card`
    - `timeline-card`
    - `current-detail-card`
    - `host-key-card`
    - `diagnostics-card`
    - `footer-row`
  - uses large step cards and green success surfaces for completed steps.

### UI Plumbing

- `ui/shell/workspace-pane.slint`
  - forwards the workspace session connection properties into `TerminalSessionHost`.
- `ui/app-window.slint`
  - owns the app-level properties that back the workspace connection flow.

### State / Projection Layer

- `src/app/ssh/connection_progress.rs`
  - defines `ConnectionHeadlineState`, `ConnectionStepState`, `ConnectionStepStateItem`, `ConnectionHostKeyPrompt`, and `ConnectionAttemptState`.
- `src/app/bootstrap.rs`
  - projects session manager state into UI properties and currently flattens attempt/step data into simple Slint row props.
- `src/shell/tabs.rs`
  - decides which workspace host mode to use for `connecting`, `waiting-user`, `cancelled`, `connected`, or `error` sessions.

### Runtime Sources

- `src/app/ssh/runtime/transport.rs`
  - emits step-level progress such as jump host connect/auth, `direct-tcpip`, target connect, and host-key verification.
- `src/app/ssh/runtime/auth.rs`
  - owns auth and host-key verification helpers.

### Theme / Reusable UI Primitives

- `ui/theme/tokens.slint`
  - provides the current shell color tokens.
- `ui/components/modal-chrome.slint`
  - shows the repo's more mature button and section styling patterns.
- `ui/components/status-pill.slint`
  - provides a lightweight badge style that can inform the redesigned summary area.

## Current Problems

### Information Architecture

- The page repeats the same story across too many equal-weight surfaces.
- `Verify host key` appears:
  - inside the step list,
  - inside the current detail copy,
  - and again in a separate host-key card.
- Diagnostics sit as a peer surface to the main task instead of a secondary evidence layer.

### Visual Hierarchy

- Completed steps use large green cards, which causes historical success to compete with the active blocker.
- Every major area is a bordered card, so the page reads like a stack of status widgets.
- The current blocking action is not visually singular enough.

### Action Hierarchy

- Footer actions like `Cancel`, `Retry`, `Edit Connection`, `Show Diagnostics`, and `Copy Diagnostics` currently share too much visual weight.
- Task-specific actions and page-level actions are mixed together.

### Product Tone

- The page feels closer to an internal diagnostics page than to a mature SSH client flow.
- The current presentation is too card-heavy, too status-heavy, and not focused enough on the user's immediate task.

### Extensibility

- A flat card list will degrade quickly with:
  - multi-hop chains;
  - more auth prompts;
  - OTP / keyboard-interactive flows;
  - multiple retry attempts.

## Design Principles

1. **One focal task at a time**
   - The page should always make it obvious what the user should do next.

2. **History stays visible but quiet**
   - Completed steps remain readable, but they no longer dominate the screen.

3. **Diagnostics are for depth, not default attention**
   - Logs remain available and copyable, but they do not live on the same layer as the primary task.

4. **Host-key verification is a decision state, not a regular row**
   - When host-key approval is required, the page transitions from progress mode into decision mode.

5. **Failure keeps continuity**
   - The page does not jump into a completely separate error layout.
   - Instead, the same skeleton remains while the focus panel upgrades into a troubleshooting view.

6. **Desktop-native restraint**
   - The final UI should feel like a calm Windows desktop tool: structured, premium, and conservative.

## Final Interaction Model

The redesigned page has one skeleton with three content modes:

- `progressing`
- `decision`
- `troubleshooting`

The layout remains stable across all three. Only the emphasis and content template change.

### 1. Summary Header

This area answers:

- what am I connecting to?
- what is the current overall state?
- where in the path am I blocked right now?

It should include:

- session / target title, such as `Mega`;
- a concise state label, such as `Connecting`, `Waiting for confirmation`, or `Connection failed`;
- the current focus step, such as `Verify host key`;
- a one-line path summary, such as `Local -> Bastion A -> Mega`;
- an optional supporting line, such as `Connected through Bastion A; the target identity needs approval`.

It should not repeat the same headline in three separate text rows.

### 2. Compact Workflow Rail

The current large step cards should become a compact timeline / activity rail.

The rail should:

- show ordered step context;
- optionally group by `hop_label`;
- visually de-emphasize completed steps;
- highlight only the active, blocked, or failed step;
- avoid rendering long diagnostics or fingerprints inline.

#### State Expression

- `completed`
  - small check / dot + subdued text
  - no large green card fill
- `current`
  - slightly stronger text + subtle active surface / accent rail
- `blocked`
  - visually tied to the current decision state
- `failed`
  - highlighted locally without turning the whole page into an alert surface
- `pending`
  - low-contrast placeholder rows

### 3. Current Task Panel

This becomes the only real focal surface on the page.

It replaces the split between `current-detail-card` and `host-key-card`.

#### Progressing Mode

Show:

- current step title;
- short human-readable explanation;
- optional latest summary detail.

Example:

- `Authenticating jump host`
- `Using the configured identity for Bastion A.`

#### Decision Mode

Used for `unknown host key` and future blocking prompts.

For host-key approval it should show:

- title: `Verify host key`
- hop / role context, such as `Target` or `Jump Host 1`
- host
- port
- fingerprint
- brief risk / TOFU explanation
- primary action
- secondary action
- optional advanced details disclosure for the raw OpenSSH public key

This panel becomes the page's visual center while the workflow rail steps back.

#### Troubleshooting Mode

Used on failure while staying on the same page skeleton.

Show:

- one-line failure summary;
- failing hop / stage;
- concise remediation suggestion;
- primary recovery action;
- secondary settings action;
- optional diagnostics summary.

Example:

- `Couldn't authenticate to Bastion A`
- `The server rejected the configured identity.`
- `Try again or update the connection settings.`

## Diagnostics Strategy

Diagnostics should move out of the main task stack and into a secondary disclosure section.

### Default State

Collapsed, with a compact header such as:

- `Diagnostics`
- optional count or latest-summary hint

### Expanded State

Show:

- concise diagnostic lines;
- copy action;
- enough technical detail for an advanced user to understand the failing hop / phase.

### Do Not Default To

- full raw error dumps;
- repeated success messages competing with the main task;
- oversized bordered surfaces that look like a second main panel.

## Copy Strategy

### Headline / Supporting Copy

Move explanation into supporting copy, not button labels.

### Button Recommendations

#### Host Key Prompt

- Primary: `Trust key`
- Secondary: `Cancel`
- Supporting copy explains that trusting the key saves it and resumes the connection.

Rationale:

- `Trust and Continue` is understandable but too long and too explanatory for the button itself.
- Splitting it into `Trust key` + `Continue` would create a fake two-step action.
- `Trust key` is shorter, more mature, and more desktop-native.

#### Failure State

- Primary: `Retry`
- Secondary: `Edit settings`

#### Diagnostics

- Use a section label / disclosure such as `Diagnostics`
- Secondary utility action: `Copy details`

### Future Safety Note

`unknown host key` and `changed host key` should not share the same visual or action semantics forever.

This redesign only scopes the current unknown-key flow. If changed-key handling is surfaced later, it should use a more conservative security treatment.

## Visual Direction

### Desired Tone

- mature
- premium
- calm
- clear
- task-focused
- professional
- desktop-native
- restrained Fluent-ish

### Visual Rules

- reduce the number of visible card surfaces;
- keep one primary surface and one secondary disclosure layer;
- reserve strong color for current blockers and destructive states;
- remove large filled success cards;
- use spacing, typography, and containment to create emphasis before using color;
- preserve the existing theme language rather than importing a foreign visual system.

## Engineering Strategy

### Short-Term Approach

Keep the runtime model mostly intact.

The first redesign pass should focus on:

- reorganizing the layout in `ui/shell/terminal-session-host.slint`;
- adding a presentation-oriented derived view model in `src/app/bootstrap.rs`;
- reducing business-logic string branching inside Slint where practical.

### Suggested Derived UI Semantics

The projection layer should derive and pass view-friendly semantics such as:

- page mode: `progressing | decision | troubleshooting`
- current task title
- current task supporting copy
- compact workflow rows
- diagnostics summary visibility / affordance
- page-level primary and secondary action states

This keeps the runtime truth in Rust while reducing presentation inference inside Slint.

### Minimal-Intrusion Data Follow-Ups

If necessary, small future-safe additions may be introduced later, such as:

- a more explicit prompt kind;
- hop grouping metadata stronger than the display label alone;
- richer failure categorization for troubleshooting mode.

These are not required for the first visual redesign pass.

## File Impact

Primary expected changes:

- `ui/shell/terminal-session-host.slint`
- `ui/shell/workspace-pane.slint`
- `ui/app-window.slint`
- `src/app/bootstrap.rs`
- `ui/theme/tokens.slint`
- related UI contract tests

## Verification Goals

After implementation, verify that:

1. the page no longer feels like a stacked debug/status card page;
2. the user can identify the current task instantly;
3. host-key verification clearly becomes the main focus when present;
4. completed steps no longer flood the page with green;
5. diagnostics remain available but secondary;
6. the design still fits the existing shell theme;
7. the structure can handle future multi-hop and multi-auth growth.
