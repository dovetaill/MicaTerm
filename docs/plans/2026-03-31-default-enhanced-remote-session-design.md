# Default Enhanced Remote Session Design

## Goal

Make `mica-term` automatically attempt a richer SSH shell experience by default, without persistently modifying remote user files or breaking ordinary interactive shells.

The feature should make remote sessions feel closer to modern terminals and IDE-integrated terminals by:

1. preserving a normal SSH login path;
2. temporarily enabling shell integration for supported shells in the current session only;
3. improving prompt semantics, command tracking, cwd tracking, and default color richness;
4. falling back safely to a plain terminal when the remote environment is unsupported or risky.

## Current State

- [`src/app/ssh/runtime.rs`](../../src/app/ssh/runtime.rs) opens a regular SSH session channel, requests a PTY, negotiates `xterm-256color`, requests an interactive shell, and pumps data to the terminal surface.
- The runtime already advertises `COLORTERM=truecolor`, so the transport path is not limited to monochrome output.
- The runtime currently parses `OSC 7` current-directory reports from remote output, but does not yet parse or emit a fuller prompt/command shell-integration protocol.
- [`src/app/terminal_theme.rs`](../../src/app/terminal_theme.rs) already owns the terminal palette defaults, so richer ANSI output can be rendered locally once remote programs emit color.
- The terminal renderer is already atlas-based and theme-aware, so this design does not require a renderer rewrite.

## Problem Statement

The “black and white / not like an IDE / not like Termius or HexHub” complaint is not caused by one missing color token.

Three layers are currently underpowered in SSH sessions:

1. remote shells often start in a plain prompt configuration with weak or no ANSI usage;
2. `mica-term` does not yet receive prompt boundaries, command lifecycle marks, or richer shell metadata by default;
3. there is no current mechanism to temporarily enhance bash/zsh/fish sessions without asking users to install dotfile changes on every remote host.

## Non-Goals

- Do not persistently edit remote `~/.bashrc`, `~/.zshrc`, or `config.fish`.
- Do not install software on remote hosts automatically.
- Do not require `sudo`, `tmux`, or root access.
- Do not replace the user’s remote login shell with a custom wrapper shell.
- Do not promise IDE-grade inline syntax highlighting for plain bash when the shell cannot safely support it.

## Constraints

- Default behavior must remain safe for normal SSH access.
- Failure must degrade to a plain terminal session, not a broken shell.
- Existing user prompt themes and shell plugins should be preserved when already present.
- The design must support bash, zsh, and fish first; other shells should simply fall back.
- Bootstrap traffic should avoid polluting remote history whenever possible.

## Industry Guidance

Public terminal documentation points to a consistent pattern:

- VS Code, WezTerm, Ghostty, and kitty all treat semantic shell integration as a protocol problem built around cwd tracking and prompt/command lifecycle markers.
- `OSC 7` and `OSC 133` are the closest thing to a shared compatibility baseline.
- Products with stronger remote experiences use temporary session cooperation rather than requiring permanent remote shell edits for the default case.
- Proprietary protocols still exist for terminal-specific actions, but they are layered on top of standard prompt semantics rather than replacing them entirely.

Key references:

- VS Code shell integration: <https://code.visualstudio.com/docs/terminal/shell-integration>
- WezTerm shell integration: <https://wezterm.org/shell-integration.html>
- Ghostty shell integration: <https://ghostty.org/docs/features/shell-integration>
- kitty shell integration: <https://sw.kovidgoyal.net/kitty/shell-integration/>
- iTerm2 shell integration: <https://iterm2.com/documentation-shell-integration.html>
- Warp SSH / Warpify: <https://docs.warp.dev/terminal/warpify/ssh>
- Windows Terminal shell integration: <https://learn.microsoft.com/en-us/windows/terminal/tutorials/shell-integration>

## Approved Approach

### 1. Default Mode Means “Auto-Try, Then Fall Back”

`mica-term` should enable `Enhanced Remote Session` by default for SSH profiles, but the meaning of “enabled” is:

- connect normally;
- detect whether enhancement is safe and supported;
- attempt one temporary session bootstrap;
- if anything looks wrong, stop and continue as a plain shell.

This keeps the baseline SSH lifecycle unchanged while making enhanced behavior the common case on modern hosts.

### 2. Keep The Existing SSH Startup Path

The SSH runtime should continue to:

1. open a session channel;
2. request a PTY;
3. negotiate environment variables such as `COLORTERM=truecolor`;
4. request the user’s normal interactive shell.

Only after the shell is visibly alive should `mica-term` attempt enhancement. This avoids forcing a custom login command and minimizes incompatibility with account policies, login banners, PAM hooks, and existing shell startup chains.

### 3. Detect The Active Shell Conservatively

Shell detection should use a two-stage strategy:

- A short-lived side channel probes remote shell identity using environment and account metadata such as `$SHELL` and passwd-shell resolution, without contaminating the interactive shell.
- The main interactive session then validates whether the prompt is ready and whether shell integration already appears active.

If the detected shell is `bash`, `zsh`, or `fish`, enhancement may proceed. Otherwise the session should remain plain.

### 4. Use A Layered Protocol Strategy

`mica-term` should separate protocol concerns into two layers.

#### Standard compatibility layer

Prefer standard or widely adopted sequences first:

- `OSC 7` for current working directory
- `OSC 133 A/B/C/D` for prompt start, prompt end, command execution start, and command completion
- parse iTerm2-style `OSC 1337;CurrentDir` and `SetMark`
- optionally parse VS Code `OSC 633` as an additional compatibility path

This allows `mica-term` to benefit from remote environments that are already shell-integration aware and avoids locking the product to a private protocol.

#### `mica-term` private enhancement layer

Add a private OSC family for terminal-specific actions that standards do not cover cleanly, for example:

- open a file locally
- edit a file locally
- file transfer affordances
- richer command metadata
- capability negotiation for prompt skinning and optional extras

The private layer must only activate when the session explicitly identifies `mica-term`, for example through `TERM_PROGRAM=mica-term` or a dedicated `MICA_TERM_ENHANCED=1` environment flag.

### 5. Bootstrap By Shell, But Preserve User Customization

Enhancement should not blindly repaint every prompt.

The bootstrap scripts should:

- detect whether the active prompt already uses ANSI color;
- detect whether cwd and prompt markers are already being emitted;
- only apply a `mica-term` prompt skin when the prompt is effectively plain;
- otherwise preserve the user’s current prompt and only attach missing lifecycle hooks.

This keeps the feature aligned with mature terminal behavior: prompt semantics and prompt styling are related but should not be coupled by force.

### 6. Bash Needs The Most Conservative Hooking

For bash:

- prefer integrating with an existing `bash-preexec` style environment when present;
- otherwise append the smallest possible wrapper around `PROMPT_COMMAND` and `DEBUG` handling;
- preserve any existing traps and prompt commands by chaining rather than overwriting;
- ensure the bootstrap can disable itself if unsafe shell options or incompatible state are detected.

For zsh:

- use native `precmd` / `preexec` style hooks;
- avoid rewriting an existing themed prompt unless it is plain.

For fish:

- use fish-native prompt/event hooks and rely on fish’s own interactive highlighting/autosuggestion behavior where available.

### 7. Improve Color By Default Without Permanent Remote Config

Within the current session only, the bootstrap may enable conservative color improvements such as:

- richer prompt segments for plain prompts;
- `ls --color=auto`, `grep --color=auto`, `diff --color=auto`, and related safe defaults;
- sensible color-related environment variables where they do not conflict with existing settings.

This should focus on common command readability and terminal richness, not on installing a whole remote shell framework.

### 8. Make Failure Cheap And Visible

Enhancement should have explicit runtime states:

- `Enhanced`
- `Plain`
- `Fallback`

Rules:

- only one automatic bootstrap attempt per SSH session;
- if bootstrap fails, do not retry repeatedly in the same session;
- cache incompatibility locally per host/user/shell fingerprint so repeated failures are avoided;
- allow users to disable automatic enhancement per session or per host.

### 9. History Hygiene Is Best-Effort, Not Reckless

The design should minimize remote history pollution without taking risky steps to hide activity.

Preferred policy:

- perform probing in side channels whenever possible;
- inject bootstrap into the interactive shell only once;
- avoid leaving repeated visible control commands in the session stream;
- use best-effort cleanup of the most recent bootstrap history entry only when the shell supports it safely;
- never perform invasive history rewriting solely to erase evidence of bootstrap.

### 10. Parsing Support Must Match Emission Support

Implementing the remote bootstrap without corresponding parser support in `mica-term` would leave value on the floor.

The terminal runtime and session manager should therefore learn to:

- parse prompt/command markers;
- track prompt boundaries and command lifecycle;
- expose enhancement state to the UI;
- use cwd and command marks for navigation, context, and future features.

## Data Flow

1. SSH session starts normally.
2. A short shell-detection probe runs out-of-band.
3. The interactive shell becomes ready.
4. `mica-term` decides `enhanced` vs `plain`.
5. If enhanced, it injects a shell-specific temporary bootstrap.
6. The shell emits standard markers plus optional `mica-term` private events.
7. The runtime parses those markers and updates session metadata.
8. The UI reflects enhancement state and uses marks/cwd/command info for richer behavior.

## Files Likely In Scope

- Modify [`src/app/ssh/runtime.rs`](../../src/app/ssh/runtime.rs)
- Modify [`src/app/ssh/session_manager.rs`](../../src/app/ssh/session_manager.rs)
- Modify [`ui/shell/terminal-session-host.slint`](../../ui/shell/terminal-session-host.slint)
- Add parser and bootstrap support files under [`src/app/ssh/`](../../src/app/ssh/) as needed
- Add SSH runtime and shell-integration tests under [`tests/`](../../tests/)
- Update user-facing SSH or terminal docs if the new default mode becomes visible in the UI

## Risks

- Bash hook compatibility is the sharpest edge and must be tested against chained `PROMPT_COMMAND` and existing `DEBUG` traps.
- Some hosts may present shells whose `$SHELL` does not match the actual interactive shell.
- Certain prompts or shell frameworks may already emit their own escape sequences; parser logic must tolerate duplicates and partial overlap.
- Aggressive prompt skinning would create user-visible regressions, so prompt replacement must remain opt-in by detection, not assumption.

## Validation Criteria

- A normal SSH session still works when enhancement is disabled, unsupported, or broken.
- Supported bash/zsh/fish sessions can emit cwd and prompt/command lifecycle markers in the current session only.
- Existing colorful prompts are preserved rather than overwritten.
- Plain prompts receive a visibly richer `mica-term` temporary skin.
- Failed enhancement attempts degrade to a plain session without repeated retries.
- Remote dotfiles remain unchanged after disconnect.
- Remote history pollution is minimized to the documented best-effort level.

## Design Outcome

`mica-term` should adopt a default-on, non-persistent SSH enhancement strategy that matches mature terminal practice:

- standard shell-integration sequences first;
- terminal-specific private protocol only for extra actions;
- shell-aware temporary bootstrap;
- one-shot automatic attempts with safe fallback;
- no remote dotfile edits as part of the default experience.
