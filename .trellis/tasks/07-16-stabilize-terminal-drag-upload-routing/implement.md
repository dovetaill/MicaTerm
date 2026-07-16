# Deterministic terminal drag-upload routing implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use inline execution (recommended) or manual inline execution to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make remote A -> B -> B -> C single-file drops keep selecting dedicated ZMODEM whenever the same SSH server continues to report `rz` successfully.

**Architecture:** Replace the probe collector's EOF shortcut with a small explicit russh message accumulator. Classify absent exit status as an incomplete probe, preserve the existing ZMODEM/SFTP fallback policy, and lock the behavior with pure message-order, live russh, and application-routing regressions.

**Tech Stack:** Rust 2024, Tokio 1.50, russh 0.58, anyhow, tracing, the existing in-process russh test server, and Slint bootstrap smoke fixtures.

## Global Constraints

- The directories A, B, and C are remote shell working directories; every drop contains one local file.
- Preserve the existing route contract: confirmed `rz` selects dedicated ZMODEM, confirmed missing `rz` selects SFTP/Transfer Center, and unavailable cwd retains the safe interactive fallback.
- Do not change ZMODEM framing, upload modal layout, shell wildcard behavior, SFTP transfer mechanics, `rz -q`, or remote path quoting.
- Do not add probe retries, sleeps, new timeouts, dependencies, or session capability caches.
- Never log file contents, credentials, or remote command stdout.
- Follow TDD for the SSH message-order defect. The A/B/B/C fake-runtime test is an end-to-end routing contract and may already pass before the runtime collector changes because that fixture returns a resolved boolean rather than raw russh messages.
- Execute inline in the current worktree. Do not dispatch implementation/check sub-agents or create a separate worktree for this focused change.
- Do not create implementation commits between tasks. Trellis Phase 3.4 will propose one coherent checked commit after implementation and spec synchronization.

---

### Task 1: Reproduce and fix EOF-before-exit-status collection

**Files:**
- Modify: `src/app/ssh/runtime/pump.rs:691-950`
- Test: `src/app/ssh/runtime/pump.rs:1369-1573`
- Test: `tests/ssh_session_manager_spec.rs:1028-1189`
- Test: `tests/ssh_session_manager_spec.rs` near the existing live runtime tests beginning at line 2208

**Interfaces:**
- Consumes: russh `ChannelMsg` values returned by `Channel::wait()`.
- Produces: `RemoteExecOutput::push_message(&mut self, ChannelMsg, &'static str) -> anyhow::Result<bool>` where `true` means collection can finish, and `require_remote_exec_exit_status(Option<u32>, &'static str) -> anyhow::Result<u32>`.

- [x] **Step 1: Add failing pure message-order tests**

Add the accumulator test helper and five tests to `pump.rs`'s existing `tests` module:

```rust
fn collect_remote_exec_test_messages(
    messages: impl IntoIterator<Item = ChannelMsg>,
) -> RemoteExecOutput {
    let mut output = RemoteExecOutput::default();
    for message in messages {
        if output
            .push_message(message, "test remote exec")
            .expect("collect test remote exec message")
        {
            break;
        }
    }
    output
}

#[test]
fn remote_exec_output_completes_when_eof_follows_exit_status() {
    let output = collect_remote_exec_test_messages([
        ChannelMsg::ExitStatus { exit_status: 0 },
        ChannelMsg::Eof,
        ChannelMsg::Close,
    ]);
    assert_eq!(output.exit_status, Some(0));
    assert!(output.saw_eof);
}

#[test]
fn remote_exec_output_waits_for_exit_status_after_eof() {
    let output = collect_remote_exec_test_messages([
        ChannelMsg::Eof,
        ChannelMsg::ExitStatus { exit_status: 0 },
        ChannelMsg::Close,
    ]);
    assert_eq!(output.exit_status, Some(0));
    assert!(output.saw_eof);
}

#[test]
fn remote_exec_output_preserves_data_before_eof_and_late_status() {
    let output = collect_remote_exec_test_messages([
        ChannelMsg::Data {
            data: b"/srv/b\n".as_slice().into(),
        },
        ChannelMsg::Eof,
        ChannelMsg::ExitStatus { exit_status: 0 },
        ChannelMsg::Close,
    ]);
    assert_eq!(output.stdout, b"/srv/b\n");
    assert_eq!(output.exit_status, Some(0));
}

#[test]
fn remote_exec_output_accepts_close_without_eof() {
    let output = collect_remote_exec_test_messages([
        ChannelMsg::ExitStatus { exit_status: 0 },
        ChannelMsg::Close,
    ]);
    assert_eq!(output.exit_status, Some(0));
    assert!(!output.saw_eof);
}

#[test]
fn remote_exec_output_reports_missing_exit_status_as_incomplete() {
    let output = collect_remote_exec_test_messages([ChannelMsg::Eof, ChannelMsg::Close]);
    let error = require_remote_exec_exit_status(output.exit_status, "test remote exec")
        .expect_err("missing exit status must be incomplete");
    assert!(error.to_string().contains("without an exit status"));
}
```

- [x] **Step 2: Make the in-process SSH server emit the real failing order**

In `InteractiveTestServer::exec_request`, insert exit status 0 between EOF and close:

```rust
session.data(
    channel,
    format!("{}\n", self.shell_integration_behavior.shell_path).into_bytes(),
)?;
session.eof(channel)?;
session.exit_status_request(channel, 0)?;
session.close(channel)?;
```

Add `ssh_runtime_accepts_exec_exit_status_sent_after_eof` with the complete
server lifecycle:

```rust
#[test]
fn ssh_runtime_accepts_exec_exit_status_sent_after_eof() {
    let _env_lock = lock_known_hosts_env();
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (server_task, addr, private_key_path, server_public_key, _server_state) =
        runtime.block_on(async {
            spawn_publickey_shell_server(Duration::from_millis(10)).await
        });
    let known_hosts_path = temp_known_hosts_path("exec-eof-before-status");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    known_hosts
        .accept_unknown(
            addr.ip().to_string().as_str(),
            addr.port(),
            &server_public_key,
        )
        .expect("trust test server host key");
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let runtime_handle = runtime.block_on(async {
        SshSessionRuntime::connect(
            sample_publickey_profile(
                "asset-exec-eof-before-status",
                addr.ip().to_string(),
                addr.port(),
                private_key_path.display().to_string(),
            ),
            Uuid::new_v4(),
            Uuid::new_v4(),
            event_tx,
        )
        .await
        .expect("connect ssh runtime")
    });

    assert!(
        runtime_handle
            .remote_command_exists("rz".into())
            .expect("probe remote rz with EOF before exit status")
    );

    runtime_handle.disconnect().expect("disconnect runtime");
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        server_task.await.expect("join test ssh server");
    });
    let _ = fs::remove_file(&private_key_path);
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}
```

- [x] **Step 3: Run the regressions and verify they fail for the expected reason**

Run:

```bash
cargo test -q remote_exec_output_ --lib
cargo test -q ssh_runtime_accepts_exec_exit_status_sent_after_eof --test ssh_session_manager_spec
```

Expected before implementation: the unit target fails to compile because the
accumulator API does not exist, and the live test returns `false` because the
current collector breaks at EOF before reading exit status 0.

- [x] **Step 4: Implement the minimal accumulator**

Extend `RemoteExecOutput` and move the existing loop match into its pure helper:

```rust
#[derive(Debug, Default)]
struct RemoteExecOutput {
    exit_status: Option<u32>,
    stdout: Vec<u8>,
    saw_eof: bool,
    exec_accepted: bool,
}

impl RemoteExecOutput {
    fn push_message(
        &mut self,
        message: ChannelMsg,
        request_label: &'static str,
    ) -> Result<bool> {
        match message {
            ChannelMsg::Success => self.exec_accepted = true,
            ChannelMsg::Failure if !self.exec_accepted => {
                bail!("remote SSH server rejected the {request_label} request");
            }
            ChannelMsg::Data { data } => self.stdout.extend_from_slice(data.as_ref()),
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => {
                self.exit_status = Some(status);
                return Ok(self.saw_eof);
            }
            ChannelMsg::ExitSignal { .. } => {
                self.exit_status = Some(255);
                return Ok(true);
            }
            ChannelMsg::Eof => {
                self.saw_eof = true;
                return Ok(self.exit_status.is_some());
            }
            ChannelMsg::Close => return Ok(true),
            _ => {}
        }
        Ok(false)
    }
}

fn require_remote_exec_exit_status(
    exit_status: Option<u32>,
    request_label: &'static str,
) -> Result<u32> {
    exit_status.ok_or_else(|| anyhow!("{request_label} closed without an exit status"))
}
```

Then reduce `remote_exec_output` to:

```rust
let mut output = RemoteExecOutput::default();
while let Some(message) = channel.wait().await {
    if output.push_message(message, request_label)? {
        break;
    }
}
Ok(output)
```

This must finish immediately on `EOF + ExitStatus`, regardless of which arrives
first, and still finish on `Close` when EOF is omitted.

- [x] **Step 5: Run the runtime regressions again**

Run:

```bash
cargo test -q remote_exec_output_ --lib
cargo test -q ssh_runtime_accepts_exec_exit_status_sent_after_eof --test ssh_session_manager_spec
```

Expected: all named tests pass without waiting for the three-second probe
timeout.

- [x] **Step 6: Review checkpoint**

Inspect:

```bash
git diff -- src/app/ssh/runtime/pump.rs tests/ssh_session_manager_spec.rs
```

Expected: probe collection no longer breaks on EOF alone; the dedicated ZMODEM
exec loop and interactive terminal pump remain unchanged.

### Task 2: Make incomplete probes explicit and observable

**Files:**
- Modify: `src/app/ssh/runtime/pump.rs:691-735,893-950`
- Test: `src/app/ssh/runtime/pump.rs` in the Task 1 accumulator tests

**Interfaces:**
- Consumes: `RemoteExecOutput.exit_status: Option<u32>`.
- Produces: successful status `0`, confirmed non-zero status, or an anyhow error containing `without an exit status`.

- [x] **Step 1: Classify command-probe status without collapsing `None` into false**

After the existing timeout, log the optional status and require it before the
boolean mapping:

```rust
tracing::debug!(
    target: "app.ssh",
    command_name = command_name.as_str(),
    exit_status = ?status,
    "remote command probe completed"
);
let status = require_remote_exec_exit_status(status, "remote command probe")?;
Ok(status == 0)
```

Keep `Some(nonzero)` as `Ok(false)`; only `None` becomes an error.

- [x] **Step 2: Apply the same incomplete-result rule to cwd probing**

Before parsing stdout, log only status and byte count, then require status:

```rust
tracing::debug!(
    target: "app.ssh",
    exit_status = ?output.exit_status,
    stdout_bytes = output.stdout.len(),
    "remote cwd probe completed"
);
let status =
    require_remote_exec_exit_status(output.exit_status, "remote cwd probe")?;
if status != 0 {
    return Ok(None);
}
```

Do not log `output.stdout`. Preserve the current absolute-path parsing and
`Ok(None)` for a confirmed non-zero status or missing absolute cwd line.

- [x] **Step 3: Run probe and command-construction unit tests**

Run:

```bash
cargo test -q remote_exec_output_ --lib
cargo test -q remote_command_probe_uses_transfer_path_setup --lib
cargo test -q exec_zmodem_upload_uses_quiet_rz --lib
```

Expected: all pass; command quoting, PATH setup, and `rz -q` remain unchanged.

### Task 3: Lock A/B/B/C routing and add decision diagnostics

**Files:**
- Modify: `src/app/bootstrap/sftp.rs:1660-1801`
- Test: `tests/bootstrap_smoke.rs` near `terminal_file_drop_reprobes_cwd_when_previous_cwd_only_came_from_probe`

**Interfaces:**
- Consumes: `SessionManager::resolve_current_working_directory`, `SessionManager::remote_command_exists`, and the existing terminal-drop background scheduler.
- Produces: four dedicated ZMODEM calls targeting `/srv/a`, `/srv/b`, `/srv/b`, and `/srv/c`, plus structured route diagnostics.

- [x] **Step 1: Add the remote A/B/B/C routing contract**

Add `terminal_file_drop_keeps_exec_rz_across_remote_cwd_changes` using
`DelayedCwdRecordingSftpLauncher`, one local `release.env`, and stable
`remote_rz_available = true`:

```rust
for (index, remote_dir) in ["/srv/a", "/srv/b", "/srv/b", "/srv/c"]
    .into_iter()
    .enumerate()
{
    sftp_state.set_remote_cwd(remote_dir);
    app.set_workspace_terminal_external_drop_paths(ModelRc::new(VecModel::from(vec![
        SharedString::from(upload_path.to_string_lossy().to_string()),
    ])));
    app.invoke_workspace_terminal_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        sftp_state
            .zmodem_exec_upload_calls
            .lock()
            .expect("lock zmodem exec upload calls")
            .len()
            == index + 1
    });
}
```

Assert:

```rust
assert_eq!(sftp_state.take_remote_cwd_probe_calls(), 4);
assert_eq!(
    sftp_state.take_remote_command_exists_calls(),
    vec!["rz", "rz", "rz", "rz"]
);
assert_eq!(
    sftp_state.take_zmodem_exec_upload_calls(),
    ["/srv/a", "/srv/b", "/srv/b", "/srv/c"]
        .into_iter()
        .map(|remote_dir| {
            (
                remote_dir.to_string(),
                vec![upload_path.to_string_lossy().to_string()],
            )
        })
        .collect::<Vec<_>>()
);
assert!(sftp_state.take_interactive_zmodem_upload_calls().is_empty());
assert!(sftp_state.take_upload_file_calls().is_empty());
```

- [x] **Step 2: Run the routing contract before diagnostics**

Run:

```bash
cargo test -q terminal_file_drop_keeps_exec_rz_across_remote_cwd_changes --test bootstrap_smoke
```

Expected: pass on the deterministic fake runtime. This is a route/target
contract; Task 1's unit and live russh tests are the regressions that fail on the
original defect.

- [x] **Step 3: Add structured cwd and method decisions**

Add fields to the existing `app.drop` events without logging new payload data:

```text
live tracked cwd: cwd_source="live_tracking"
successful cwd probe: cwd_source="exec_probe"
missing cwd: cwd_source="unavailable", fallback_reason="cwd_unavailable"
cwd probe error: cwd_source="unavailable", fallback_reason="cwd_probe_error"
dedicated upload: upload_method="zmodem_exec"
interactive upload: upload_method="zmodem_interactive", fallback_reason="cwd_unavailable"
confirmed missing rz: upload_method="sftp", fallback_reason="rz_missing"
rz probe error: upload_method="sftp", fallback_reason="rz_probe_error"
```

Keep existing `session_id`, `remote_dir`, `path_count`, and error fields. The
existing generic `method` field may remain for compatibility, but every terminal
drop branch must expose the specific `upload_method` or the cwd decision that
precedes it.

- [x] **Step 4: Run all focused terminal-drop branches**

Run:

```bash
cargo test -q terminal_file_drop_ --test bootstrap_smoke
```

Expected: the new A/B/B/C test and existing dedicated ZMODEM, confirmed-missing
`rz`, cwd-probe-error, cwd-unavailable interactive, and fresh-cwd-probe tests all
pass.

- [x] **Step 5: Review checkpoint**

Inspect:

```bash
git diff -- src/app/bootstrap/sftp.rs tests/bootstrap_smoke.rs
```

Expected: no UI state or transfer implementation changed; only structured
diagnostics and the repeated remote-cwd regression were added.

### Task 4: Full verification and executable contract update

**Files:**
- Modify after verification: `.trellis/spec/backend/quality-guidelines.md`
- Verify: `src/app/ssh/runtime/pump.rs`
- Verify: `src/app/bootstrap/sftp.rs`
- Verify: `tests/ssh_session_manager_spec.rs`
- Verify: `tests/bootstrap_smoke.rs`

**Interfaces:**
- Consumes: Tasks 1-3 and PRD requirements R1-R6.
- Produces: a verified SSH EOF/exit-status contract and final acceptance mapping.

- [x] **Step 1: Run focused and owning test targets**

Run:

```bash
cargo test -q remote_exec_output_ --lib
cargo test -q ssh_runtime_accepts_exec_exit_status_sent_after_eof --test ssh_session_manager_spec
cargo test -q terminal_file_drop_ --test bootstrap_smoke
cargo test -q --lib
cargo test -q --test ssh_session_manager_spec
cargo test -q --test bootstrap_smoke
```

Expected: all pass. The live russh regression must complete normally rather than
passing only after the three-second timeout.

- [x] **Step 2: Run formatting, build, and lint checks**

Run:

```bash
cargo fmt --all -- --check
cargo check -q
cargo clippy --all-targets --message-format short
git diff --check
```

Expected: formatting, check, and whitespace validation pass. Compare clippy
output with the repository baseline and introduce no new warning in changed
code; record any pre-existing repository warnings rather than hiding them.

- [x] **Step 3: Audit the exact semantic boundary**

Run:

```bash
git grep -n "ChannelMsg::Close | ChannelMsg::Eof\|ChannelMsg::Eof | ChannelMsg::Close\|require_remote_exec_exit_status\|upload_method\|fallback_reason" -- src/app/ssh tests
```

Expected: the short-lived exec probe collector no longer terminates on EOF
alone. Other interactive-terminal or ZMODEM loops may retain protocol-specific
EOF handling and must not be mechanically rewritten.

- [x] **Step 4: Capture the prevention contract**

Load `trellis-update-spec` and add this focused rule to
`.trellis/spec/backend/quality-guidelines.md` without rewriting unrelated
sections:

```markdown
- Short-lived SSH exec collectors must treat EOF, exit status, and close as
  independent messages. Do not classify a probe from EOF alone; preserve valid
  EOF-before-exit-status ordering and surface a closed channel with no status as
  incomplete.
- Required regressions: status-before-EOF, EOF-before-status, data plus
  EOF-before-status, close without EOF, missing status, and one live russh
  EOF-before-status test.
```

- [x] **Step 5: Run the Trellis quality gate and map acceptance**

Load `trellis-check`, run its required spec/code/test checks, then map:

```text
AC1 -> pure accumulator order tests
AC2 -> live russh test plus missing-status unit test
AC3/AC4 -> A/B/B/C bootstrap smoke regression
AC5 -> existing confirmed-missing-rz SFTP regression
AC6 -> existing cwd-unavailable interactive regression
AC7 -> full commands from Tasks 4.1 and 4.2
```

Expected: no unexplained failure and no unrelated source change.

Verification evidence recorded on 2026-07-16:

```text
cargo test -q remote_exec_output_ --lib                         5 passed
cargo test -q ssh_runtime_accepts_exec_exit_status_sent_after_eof
  --test ssh_session_manager_spec                               1 passed
cargo test -q terminal_file_drop_ --test bootstrap_smoke       13 passed
cargo test -q --lib                                            169 passed
cargo test -q --test ssh_session_manager_spec                  45 passed
cargo test -q --test bootstrap_smoke                           288 passed
cargo test -q --test ssh_terminal_interaction_spec             30 passed
cargo fmt --all -- --check                                     passed
cargo check -q                                                 passed
git diff --check                                                passed
```

`cargo clippy --all-targets --message-format short` reaches the repository's
pre-existing `clippy::never_loop` hard error in `pump.rs` outside this task's
changed hunks. Re-running the entire command with only
`-A clippy::never_loop` completes successfully and reports no warning in the
new accumulator, probe-classification, logging, or regression-test lines.

- [x] **Step 6: Prepare the implementation commit for user confirmation**

Follow Trellis Phase 3.4: show one coherent commit proposal containing the
runtime fix, routing diagnostics, tests, spec update, and task artifacts. Commit
only after the user confirms that final batch; do not amend or push.
