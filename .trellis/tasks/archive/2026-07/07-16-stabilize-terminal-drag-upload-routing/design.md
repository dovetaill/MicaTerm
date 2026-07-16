# Deterministic terminal drag-upload routing design

Date: 2026-07-16
Status: approved

## Problem restatement

For one local file dropped into a single SSH terminal session, changing the
remote shell cwd through A -> B -> B -> C must change only the remote upload
target. It must not make the application alternate between dedicated ZMODEM
and SFTP because valid SSH channel messages were delivered in a different
order.

## Confirmed invariants

1. Remote cwd is either supplied by live shell tracking or freshly probed before
   each drop when only probe-derived state exists.
2. A known cwd plus a successful `command -v rz` probe selects dedicated ZMODEM.
3. A confirmed non-zero `rz` probe selects SFTP/Transfer Center.
4. An unavailable cwd retains the existing safe interactive-ZMODEM fallback.
5. SSH `CHANNEL_EOF` ends data in one direction but does not close the channel.
   Exit status is a separate channel request and may arrive after EOF.
6. A probe with no exit status is incomplete. It is not proof that `rz` is
   missing.

The SSH rules in items 5 and 6 follow RFC 4254 sections 5.3, 6.5, and 6.10:
<https://www.rfc-editor.org/rfc/rfc4254.html>.

## Considered approaches

### A. Correct the exec-result state machine (chosen)

Collect stdout, EOF, exit status, and close as independent facts. EOF stops the
stdout direction but does not finish the result until an exit status is known or
the channel actually closes. This fixes both cwd and `rz` probes at their shared
root and adds no persistent state.

### B. Retry an incomplete probe

A retry can reduce the observed frequency but leaves the first collector
incorrect, adds up to another probe timeout to a drop, and can still fail under
the same message ordering. It is rejected as a primary fix.

### C. Cache a positive `rz` result per session

This can stabilize capability selection after the first success, but it does not
fix cwd probing and can become stale when PATH or installed packages change. It
is rejected for this task. A cache can be considered later only with independent
performance evidence.

## Chosen architecture

### Remote exec result collection

Keep `remote_exec_output` as the shared boundary for short-lived cwd and command
probes. Add explicit accumulator state:

```text
RemoteExecOutput
  stdout: Vec<u8>
  exit_status: Option<u32>
  saw_eof: bool
```

Its message transitions are:

| Message | State change | Collection complete |
| --- | --- | --- |
| `Success` | mark exec accepted | no |
| `Data` | append stdout | no |
| `ExitStatus(n)` | store `n` | yes only if EOF was already seen |
| `Eof` | set `saw_eof` | yes only if exit status is already known |
| `Close` or stream end | no additional data | yes |
| `ExitSignal` | preserve current synthetic failure status | yes |
| pre-accept `Failure` | return an error | yes |

This accepts all relevant valid orders:

```text
ExitStatus(0) -> EOF -> Close
EOF -> ExitStatus(0) -> Close
Data -> EOF -> ExitStatus(0) -> Close
ExitStatus(0) -> Close
```

Completing at `EOF + ExitStatus` avoids waiting for a redundant `Close`, so the
existing three-second outer probe timeouts remain a failure bound rather than a
normal delay. A `Close` without exit status still returns an incomplete result
for the caller to classify.

The dedicated ZMODEM data-channel loop is not changed by this collector refactor.
It has protocol-specific completion and transport-close handling; changing that
lifecycle is outside this routing defect.

### Probe result classification

`remote_command_exists` maps results explicitly:

```text
Some(0)       -> Ok(true)
Some(nonzero) -> Ok(false)
None          -> Err(incomplete probe)
```

`resolve_remote_current_working_directory` maps them as follows:

```text
Some(0)       -> parse the absolute cwd from stdout
Some(nonzero) -> Ok(None), preserving the existing unavailable-cwd fallback
None          -> Err(incomplete probe)
```

The drop scheduler preserves its current operational fallback on an actual probe
error: cwd errors can use the existing safe interactive route, while an `rz`
probe error can fall back to SFTP. The difference after this fix is that a valid
EOF-before-status response is no longer mislabeled as either case.

### Routing and diagnostics

The routing order stays unchanged:

```text
single local file
  -> active ZMODEM receiver exists: reuse it
  -> obtain remote cwd
     -> unavailable and safe: interactive ZMODEM
     -> available: probe rz
        -> confirmed present: dedicated ZMODEM modal
        -> confirmed absent: SFTP / Transfer Center
        -> probe error: logged SFTP fallback
```

Structured diagnostics will make each decision auditable:

- `cwd_source`: `live_tracking`, `exec_probe`, or `unavailable`
- `exit_status`: the raw optional probe status at the runtime boundary
- `upload_method`: `zmodem_exec`, `zmodem_interactive`, or `sftp`
- `fallback_reason`: `cwd_unavailable`, `rz_missing`, or `rz_probe_error`

Existing safe identifiers such as session id, remote directory, command name,
and local path count may remain. File contents, credentials, and command output
must not be logged.

## Test design

### Runtime unit tests

Extract the accumulator transition into a small synchronous helper so message
ordering can be tested without a live SSH server. Cover:

1. `ExitStatus(0) -> EOF -> Close` retains status 0.
2. `EOF -> ExitStatus(0) -> Close` retains status 0.
3. `Data -> EOF -> ExitStatus(0) -> Close` retains both stdout and status 0.
4. `ExitStatus(0) -> Close` completes without requiring EOF.
5. `EOF -> Close` produces `exit_status == None`.
6. Missing status is classified as an incomplete command/cwd probe rather than
   command absence.

### Live russh regression

Extend the existing in-process SSH server fixture in
`tests/ssh_session_manager_spec.rs` so an exec request sends data, EOF, exit
status 0, then close. Call `SshSessionRuntime::remote_command_exists("rz")`
against that server and require `true`. This test exercises the actual russh
channel queue and fails with the current EOF break, complementing the pure
message-order tests.

### Terminal-drop integration regression

Using one SSH session, one local file, stable `rz_available = true`, and no live
cwd tracking:

1. Set the remote cwd fixture to A and drop the file.
2. Set it to B and drop the same file.
3. Keep it at B and drop the same file again.
4. Set it to C and drop the same file.

Assert four cwd probes, four `rz` probes, dedicated ZMODEM targets A/B/B/C in
order, no interactive ZMODEM calls, and no SFTP upload calls. Existing tests for
confirmed missing `rz` and unavailable-cwd interactive fallback remain the
contract tests for the other branches.

## Compatibility and failure behavior

- No persisted data, public API, protocol framing, or UI layout changes.
- No new retry, cache, thread, or timeout.
- A standards-compliant server response becomes deterministic.
- A genuinely incomplete or timed-out probe still takes the existing safe
  fallback and emits a reasoned warning.
- Remote cwd quoting and `rz -q` command construction are unchanged.

## Scope and rollback

Expected production edits are limited to
`src/app/ssh/runtime/pump.rs` and targeted routing diagnostics in
`src/app/bootstrap/sftp.rs`; regression coverage belongs in the runtime unit
tests, `tests/ssh_session_manager_spec.rs`, and `tests/bootstrap_smoke.rs`.

The accumulator/classification change can be reverted independently of routing
logs and integration tests. No migration or cleanup step is required.

## Acceptance mapping

- PRD AC1 and AC2: accumulator ordering and incomplete-result unit tests plus
  the in-process russh EOF-before-exit-status regression.
- PRD AC3 and AC4: A -> B -> B -> C terminal-drop integration regression.
- PRD AC5 and AC6: existing missing-`rz` and cwd-unavailable tests plus focused
  reruns after the change.
- PRD AC7: formatting, focused tests, applicable full checks, and Trellis quality
  verification before completion.
