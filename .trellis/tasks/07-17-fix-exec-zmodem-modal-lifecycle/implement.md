# Dedicated exec ZMODEM modal lifecycle implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use inline execution (recommended) or manual inline execution to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make dedicated-exec ZMODEM modal controls reach the transfer that owns them, so terminal states close permanently and a running upload can be cancelled over its own SSH channel.

**Architecture:** Make `SessionManager` authoritative for revision-checked terminal modal projection removal, while retaining identity-safe cleanup of an interactive controller's internal terminal state. Add a generation-scoped command channel in `SshSessionRuntime` so running Cancel reaches the dedicated exec task, writes the ZMODEM abort wire, settles the channel, and publishes a terminal outcome.

**Tech Stack:** Rust 2024, Tokio 1.50, russh 0.58, zmodem2 0.7.1, anyhow, tracing, Slint bootstrap smoke fixtures, and the existing in-process russh integration server.

## Global Constraints

- Completed, Failed, and Cancelled are dismissible terminal phases. AwaitingUploadSelection, AwaitingDownloadDirectory, and Running must use Cancel and must never be hidden by Dismiss.
- Preserve interactive-PTY ZMODEM ownership in the existing main channel pump.
- Preserve cwd probing, `rz` capability detection, `rz -q`, shell quoting, protocol framing, SFTP fallback, Bash wildcard handling, and modal layout/labels.
- Do not keep a completed dedicated exec task alive waiting for Done or close.
- Do not add dependencies, fixed sleeps, retries, capability caches, persisted fields, or public UI transfer ids.
- Do not hold the `SessionManager` registry mutex while invoking `SessionRuntimeControl`.
- A stale projection revision or exec generation must not remove or cancel newer work.
- Lifecycle logs may include session id, generation, phase, command, owner, outcome, path count, byte count, and file count. They must not include file contents, credentials, protocol payloads, or remote command output.
- Execute inline in the current worktree. Do not dispatch implementation/check sub-agents or create a separate worktree for this focused fix.
- Follow TDD: make the real no-`None` dismissal and exec ownership regressions fail before changing their owning production behavior.
- Do not create partial implementation commits between tasks. Trellis Phase 3.4 will create one coherent commit after implementation, checks, and spec synchronization.

---

### Task 1: Make terminal modal dismissal manager-owned and race-safe

**Files:**
- Modify: `src/app/ssh/session_manager.rs:150-185,453-460,940-955,1315-1450`
- Modify: `src/app/ssh/runtime.rs:200-235,472-485,620-632`
- Modify: `src/app/ssh/runtime/pump.rs:330-345`
- Modify: `src/app/ssh/runtime/zmodem.rs:414-425`
- Test: `src/app/ssh/session_manager.rs` in its existing `#[cfg(test)]` module
- Test: `src/app/ssh/runtime/zmodem.rs` near `controller_cancel_queues_abort_wire_and_finishes_locally`
- Test: `tests/bootstrap_smoke.rs:2370-2505,16620-16735`

**Interfaces:**
- Consumes: `SessionRuntimeEvent::ZmodemStateChanged(Some(state))` and terminal-modal callbacks already bound in `src/app/bootstrap.rs`.
- Produces: private `ProjectedZmodemTransfer { revision: u64, state: ZmodemTransferState }`, revision-checked projection removal, `SessionRuntimeControl::dismiss_zmodem_transfer(&self, expected_state: ZmodemTransferState) -> Result<()>`, and `ZmodemController::dismiss_if_matches(&mut self, expected: &ZmodemTransferState) -> bool`.

- [x] **Step 1: Remove the masking behavior from the bootstrap fake and add failing modal regressions**

Extend `ZmodemModalState` with recorded dismiss states, but do not emit
`ZmodemStateChanged(None)` from dismissal:

```rust
#[derive(Clone, Default)]
struct ZmodemModalState {
    event_tx: Arc<Mutex<Option<mpsc::UnboundedSender<SessionRuntimeEvent>>>>,
    dismiss_calls: Arc<Mutex<Vec<ZmodemTransferState>>>,
}

impl ZmodemModalState {
    fn take_dismiss_calls(&self) -> Vec<ZmodemTransferState> {
        std::mem::take(
            &mut *self
                .dismiss_calls
                .lock()
                .expect("lock zmodem dismiss calls"),
        )
    }
}

fn dismiss_zmodem_transfer(&self, expected_state: ZmodemTransferState) -> Result<()> {
    self.state
        .dismiss_calls
        .lock()
        .expect("lock zmodem dismiss calls")
        .push(expected_state);
    Ok(())
}
```

Replace `zmodem_completed_download_modal_exposes_done_open_folder_and_open`
with a name that states the ownership regression and keep its action-label
assertions. After invoking Done, perform at least three
`flush_runtime_projection()` calls before requiring the modal to remain closed.

Add these helpers and focused cases. `open_zmodem_modal_fixture` contains the
existing `bind_with_launcher`, SSH asset activation, and wait for the initial
AwaitingDownloadDirectory modal; it does not create its own test guard:

```rust
fn sample_zmodem_transfer_state(phase: ZmodemTransferPhase) -> ZmodemTransferState {
    ZmodemTransferState {
        direction: ZmodemTransferDirection::Upload,
        phase,
        title: "ZMODEM Upload".into(),
        headline: match phase {
            ZmodemTransferPhase::Completed => "Upload complete",
            ZmodemTransferPhase::Failed => "Upload failed",
            ZmodemTransferPhase::Cancelled => "Upload cancelled",
            _ => "Upload in progress",
        }
        .into(),
        status_text: "Transfer lifecycle fixture".into(),
        detail_text: String::new(),
        error_text: String::new(),
        current_file_name: "release.bin".into(),
        files_completed: usize::from(phase == ZmodemTransferPhase::Completed),
        files_total: Some(1),
        bytes_transferred: 3,
        bytes_total: Some(3),
        local_file_path: None,
        local_reveal_path: None,
    }
}

fn open_zmodem_modal_fixture(app: &AppWindow, state: &ZmodemModalState) {
    bind_with_launcher(
        app,
        None,
        Arc::new(ZmodemModalLauncher {
            state: state.clone(),
        }),
    );
    let ssh_id = create_root_ssh(app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_zmodem_transfer_modal_open()
    });
}

fn assert_zmodem_modal_stays_closed(app: &AppWindow) {
    for _ in 0..3 {
        flush_runtime_projection();
        assert!(!app.get_zmodem_transfer_modal_open());
    }
}

#[test]
fn zmodem_completed_modal_close_closes_without_runtime_clear_event() {
    let _guard = init_bootstrap_smoke_test();
    let app = AppWindow::new().unwrap();
    let state = ZmodemModalState::default();
    open_zmodem_modal_fixture(&app, &state);
    state.emit_transfer_state(Some(sample_zmodem_transfer_state(
        ZmodemTransferPhase::Completed,
    )));
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_zmodem_transfer_modal_primary_action_label().as_str() == "Done"
    });

    app.invoke_zmodem_transfer_modal_close_requested();
    assert_zmodem_modal_stays_closed(&app);
    let dismiss_calls = state.take_dismiss_calls();
    assert_eq!(dismiss_calls.len(), 1);
    assert_eq!(
        dismiss_calls[0].phase,
        ZmodemTransferPhase::Completed
    );
}

#[test]
fn zmodem_failed_and_cancelled_modals_close_without_runtime_clear_event() {
    let _guard = init_bootstrap_smoke_test();
    let app = AppWindow::new().unwrap();
    let state = ZmodemModalState::default();
    open_zmodem_modal_fixture(&app, &state);

    for phase in [ZmodemTransferPhase::Failed, ZmodemTransferPhase::Cancelled] {
        state.emit_transfer_state(Some(sample_zmodem_transfer_state(phase)));
        wait_for_condition(Duration::from_secs(2), || {
            flush_runtime_projection();
            app.get_zmodem_transfer_modal_open()
        });
        app.invoke_zmodem_transfer_modal_close_requested();
        assert_zmodem_modal_stays_closed(&app);
    }

    assert_eq!(
        state
            .take_dismiss_calls()
            .into_iter()
            .map(|state| state.phase)
            .collect::<Vec<_>>(),
        vec![ZmodemTransferPhase::Failed, ZmodemTransferPhase::Cancelled]
    );
}
```

For the Done path, rename the existing completed-download test to
`zmodem_completed_modal_done_closes_without_runtime_clear_event` and retain its
temporary file plus Done/Open Folder/Open assertions. Replace only its final
close assertion with `assert_zmodem_modal_stays_closed(&app)`, then require one
recorded dismiss call whose phase is Completed. This preserves the existing
completed-download action coverage while adding the no-runtime-clear regression.

Each case must assert the recorded dismiss call with the displayed terminal
phase. Keep Awaiting/Running fake cancellation separate from Dismiss behavior.

Implementation note: final verification consolidated Done, completed-close,
Failed-close, and Cancelled-close into
`zmodem_completed_and_terminal_modal_actions_close_without_runtime_clear_event`
using one `AppWindow`. This preserves all assertions while preventing multiple
full bootstrap fixtures from accumulating periodic test-backend timers. The
Awaiting Cancel fixture now records routing without emitting `None`, matching
the contract that Cancel must not directly hide live work.

- [x] **Step 2: Run the bootstrap regressions and verify the real failure**

Run:

```bash
cargo test -q zmodem_completed_and_terminal_modal_actions_close_without_runtime_clear_event --test bootstrap_smoke
```

Expected before the manager fix: the modal remains open after Done/close because
the fake runtime no longer manufactures `ZmodemStateChanged(None)`.

- [x] **Step 3: Add failing manager revision and phase-contract unit tests**

Add this module-local builder, then write:

```rust
fn sample_projected_zmodem_state(
    phase: ZmodemTransferPhase,
) -> ZmodemTransferState {
    ZmodemTransferState {
        direction: ZmodemTransferDirection::Upload,
        phase,
        title: "ZMODEM Upload".into(),
        headline: "Lifecycle test".into(),
        status_text: "Transfer lifecycle fixture".into(),
        detail_text: String::new(),
        error_text: String::new(),
        current_file_name: "release.bin".into(),
        files_completed: 0,
        files_total: Some(1),
        bytes_transferred: 0,
        bytes_total: Some(3),
        local_file_path: None,
        local_reveal_path: None,
    }
}
```

Import `ZmodemTransferDirection`, `ZmodemTransferPhase`, and
`ZmodemTransferState` into the existing manager test module, then add:

```rust
#[test]
fn zmodem_projection_revision_prevents_stale_dismissal() {
    let session_id = Uuid::new_v4();
    let mut registry = SessionRegistry::default();
    let completed = sample_projected_zmodem_state(ZmodemTransferPhase::Completed);
    let running = sample_projected_zmodem_state(ZmodemTransferPhase::Running);

    let old_revision = registry.project_zmodem_state(session_id, completed);
    let new_revision = registry.project_zmodem_state(session_id, running.clone());

    assert!(!registry.remove_zmodem_projection_if_revision(session_id, old_revision));
    assert_eq!(
        registry.zmodem_transfers.get(&session_id).map(|entry| &entry.state),
        Some(&running)
    );
    assert!(registry.remove_zmodem_projection_if_revision(session_id, new_revision));
}

#[test]
fn zmodem_dismissible_phase_contract_rejects_live_work() {
    for phase in [
        ZmodemTransferPhase::AwaitingUploadSelection,
        ZmodemTransferPhase::AwaitingDownloadDirectory,
        ZmodemTransferPhase::Running,
    ] {
        assert!(!zmodem_phase_is_dismissible(phase));
    }
    for phase in [
        ZmodemTransferPhase::Completed,
        ZmodemTransferPhase::Failed,
        ZmodemTransferPhase::Cancelled,
    ] {
        assert!(zmodem_phase_is_dismissible(phase));
    }
}
```

Run `cargo test -q zmodem_projection_revision_ --lib` and expect compilation to
fail because the projection record and helpers do not exist yet.

- [x] **Step 4: Implement revisioned projection and authoritative manager dismissal**

Replace the registry map value and add a monotonic counter:

```rust
#[derive(Clone)]
struct ProjectedZmodemTransfer {
    revision: u64,
    state: ZmodemTransferState,
}

struct SessionRegistry {
    zmodem_transfers: HashMap<Uuid, ProjectedZmodemTransfer>,
    next_zmodem_transfer_revision: u64,
}

impl SessionRegistry {
    fn project_zmodem_state(
        &mut self,
        session_id: Uuid,
        state: ZmodemTransferState,
    ) -> u64 {
        self.next_zmodem_transfer_revision = self
            .next_zmodem_transfer_revision
            .checked_add(1)
            .expect("zmodem projection revision overflow");
        let revision = self.next_zmodem_transfer_revision;
        self.zmodem_transfers
            .insert(session_id, ProjectedZmodemTransfer { revision, state });
        revision
    }

    fn remove_zmodem_projection_if_revision(
        &mut self,
        session_id: Uuid,
        expected_revision: u64,
    ) -> bool {
        let matches = self
            .zmodem_transfers
            .get(&session_id)
            .is_some_and(|entry| entry.revision == expected_revision);
        if matches {
            self.zmodem_transfers.remove(&session_id);
        }
        matches
    }
}
```

Update `zmodem_state` to clone `entry.state`, route every `Some(state)` runtime
event through `project_zmodem_state`, and retain existing direct removal for
disconnect/session cleanup.

Change the runtime-control trait dismissal signature to accept the expected
state. Implement manager dismissal with this ordering:

```rust
pub fn dismiss_zmodem_transfer(&self, session_id: Uuid) -> Result<()> {
    let (projected, runtime_control) = {
        let registry = self.registry.lock().expect("lock session registry");
        let Some(projected) = registry.zmodem_transfers.get(&session_id).cloned() else {
            return Ok(());
        };
        if !zmodem_phase_is_dismissible(projected.state.phase) {
            return Err(anyhow!(
                "cannot dismiss active zmodem transfer in phase {:?}",
                projected.state.phase
            ));
        }
        (projected, registry.runtime_controls.get(&session_id).cloned())
    };

    if let Some(runtime_control) = runtime_control {
        if let Err(error) = runtime_control
            .lock()
            .expect("lock session runtime control for zmodem dismiss")
            .dismiss_zmodem_transfer(projected.state.clone())
        {
            tracing::warn!(
                target: "app.zmodem",
                session_id = %session_id,
                lifecycle_command = "dismiss",
                owner = "interactive",
                outcome = "failed",
                error = %error,
                "zmodem runtime cleanup failed; clearing terminal projection"
            );
        }
    }

    let removed = self
        .registry
        .lock()
        .expect("lock session registry")
        .remove_zmodem_projection_if_revision(session_id, projected.revision);
    tracing::info!(
        target: "app.zmodem",
        session_id = %session_id,
        lifecycle_command = "dismiss",
        owner = "projection",
        outcome = if removed { "cleared" } else { "stale" },
        phase = ?projected.state.phase,
        "processed zmodem modal dismissal"
    );
    Ok(())
}
```

The final implementation may extract the logging block into a local helper, but
must preserve lock release before the runtime-control call and conditional
revision removal afterward.

- [x] **Step 5: Make interactive controller cleanup identity-safe and non-projecting**

Add:

```rust
pub(super) fn dismiss_if_matches(
    &mut self,
    expected: &ZmodemTransferState,
) -> bool {
    if self.current_state() != Some(expected) {
        return false;
    }
    self.dismiss()
}
```

Change the runtime command to:

```rust
DismissZmodem {
    expected_state: ZmodemTransferState,
},
```

The main pump handler must clear only the matching terminal controller state
and consume its local dirty change without calling `emit_zmodem_state_changes`:

```rust
Some(RuntimeCommand::DismissZmodem { expected_state }) => {
    let dismissed = zmodem.dismiss_if_matches(&expected_state);
    if dismissed {
        assert_eq!(zmodem.take_modal_state_change(), Some(None));
    }
    tracing::debug!(
        target: "app.zmodem",
        session_id = %session_id,
        lifecycle_command = "dismiss",
        owner = "interactive",
        outcome = if dismissed { "cleared" } else { "ignored" },
        "processed interactive zmodem controller cleanup"
    );
}
```

Add `controller_dismiss_if_matches_never_clears_different_state` beside the
existing controller dismissal test. It must assert a mismatched expected state
returns false and leaves the actual terminal state intact.

- [x] **Step 6: Run manager, controller, and bootstrap dismissal coverage**

Run:

```bash
cargo test -q zmodem_projection_revision_ --lib
cargo test -q zmodem_dismissible_phase_ --lib
cargo test -q controller_dismiss_if_matches_ --lib
cargo test -q zmodem_completed_and_terminal_modal_actions_close_without_runtime_clear_event --test bootstrap_smoke
```

Expected: all pass. The bootstrap fixture's dismiss call records the expected
terminal state but emits no clear event, and repeated projection flushes do not
reopen the modal.

- [x] **Step 7: Lock the existing Escape wiring contract**

Add `zmodem_modal_escape_routes_to_close_contract` to `tests/bootstrap_smoke.rs`.
Read `ui/app-window.slint`, isolate the `if root.zmodem-transfer-modal-open`
block, and require both:

```rust
assert!(zmodem_block.contains(
    "escape-requested => {\n            root.zmodem-transfer-modal-close-requested();"
));
assert!(zmodem_block.contains(
    "close-action-requested => {\n                root.zmodem-transfer-modal-close-requested();"
));
```

Run `cargo test -q zmodem_modal_escape_routes_to_close_contract --test bootstrap_smoke`.
Expected: pass without changing Slint.

### Task 2: Register and route the active dedicated exec lifecycle

**Files:**
- Modify: `src/app/ssh/runtime.rs:35-55,186-225,360-485,665-750`
- Modify: `src/app/ssh/runtime/pump.rs:752-805`
- Test: `src/app/ssh/runtime.rs` in its existing `#[cfg(test)]` module

**Interfaces:**
- Consumes: synchronous calls to `SshSessionRuntime::start_zmodem_upload_to_remote_dir` and `cancel_zmodem_transfer`.
- Produces: `ExecZmodemCommand::Cancel`, `ExecZmodemTransferSlot`, `ExecZmodemTransferRegistration`, `ExecZmodemCancelRoute`, and a command receiver passed to `run_zmodem_exec_upload`.

- [x] **Step 1: Add failing exec-slot generation tests**

Add these tests in `runtime.rs`:

```rust
#[test]
fn exec_zmodem_cancel_routes_to_active_generation() {
    let slot = Arc::new(Mutex::new(ExecZmodemTransferSlot::default()));
    let (command_tx, mut command_rx) = mpsc::unbounded_channel();
    let generation = slot
        .lock()
        .expect("lock exec zmodem slot")
        .register(command_tx)
        .expect("register exec zmodem transfer");

    assert_eq!(
        route_exec_zmodem_cancel(&slot),
        ExecZmodemCancelRoute::Routed(generation)
    );
    assert!(matches!(command_rx.try_recv(), Ok(ExecZmodemCommand::Cancel)));
}

#[test]
fn stale_exec_zmodem_registration_cannot_clear_newer_generation() {
    let slot = Arc::new(Mutex::new(ExecZmodemTransferSlot::default()));
    let (first_tx, first_rx) = mpsc::unbounded_channel();
    let first = slot.lock().unwrap().register(first_tx).unwrap();
    drop(first_rx);
    assert_eq!(route_exec_zmodem_cancel(&slot), ExecZmodemCancelRoute::NotActive);

    let (second_tx, _second_rx) = mpsc::unbounded_channel();
    let second = slot.lock().unwrap().register(second_tx).unwrap();
    assert_ne!(first, second);
    assert!(!slot.lock().unwrap().clear_if_generation(first));
    assert_eq!(slot.lock().unwrap().active_generation(), Some(second));
}

#[test]
fn exec_zmodem_slot_rejects_overlapping_live_registration() {
    let mut slot = ExecZmodemTransferSlot::default();
    let (first_tx, _first_rx) = mpsc::unbounded_channel();
    slot.register(first_tx).expect("register first transfer");
    let (second_tx, _second_rx) = mpsc::unbounded_channel();
    assert!(slot.register(second_tx).is_err());
}
```

Run `cargo test -q exec_zmodem_ --lib`. Expected before implementation:
compilation fails because these lifecycle types do not exist.

- [x] **Step 2: Implement the generation-scoped slot and drop guard**

Add private runtime types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecZmodemCommand {
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecZmodemCancelRoute {
    Routed(u64),
    NotActive,
}

struct ActiveExecZmodemTransfer {
    generation: u64,
    command_tx: mpsc::UnboundedSender<ExecZmodemCommand>,
}

#[derive(Default)]
struct ExecZmodemTransferSlot {
    next_generation: u64,
    active: Option<ActiveExecZmodemTransfer>,
}
```

`register` must reject an open active sender, discard only a closed sender,
allocate a checked monotonically increasing generation, and store the new
sender. Implement the complete slot contract as:

```rust
impl ExecZmodemTransferSlot {
    fn register(
        &mut self,
        command_tx: mpsc::UnboundedSender<ExecZmodemCommand>,
    ) -> Result<u64> {
        if self
            .active
            .as_ref()
            .is_some_and(|active| !active.command_tx.is_closed())
        {
            return Err(anyhow!("a dedicated exec zmodem upload is already active"));
        }
        self.active = None;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("exec zmodem generation overflow"))?;
        let generation = self.next_generation;
        self.active = Some(ActiveExecZmodemTransfer {
            generation,
            command_tx,
        });
        Ok(generation)
    }

    fn clear_if_generation(&mut self, expected_generation: u64) -> bool {
        let matches = self
            .active
            .as_ref()
            .is_some_and(|active| active.generation == expected_generation);
        if matches {
            self.active = None;
        }
        matches
    }

    #[cfg(test)]
    fn active_generation(&self) -> Option<u64> {
        self.active.as_ref().map(|active| active.generation)
    }
}

fn route_exec_zmodem_cancel(
    slot: &Arc<Mutex<ExecZmodemTransferSlot>>,
) -> ExecZmodemCancelRoute {
    let active = slot
        .lock()
        .expect("lock exec zmodem lifecycle slot")
        .active
        .as_ref()
        .map(|active| (active.generation, active.command_tx.clone()));
    let Some((generation, command_tx)) = active else {
        return ExecZmodemCancelRoute::NotActive;
    };
    if command_tx.send(ExecZmodemCommand::Cancel).is_ok() {
        return ExecZmodemCancelRoute::Routed(generation);
    }
    slot.lock()
        .expect("lock stale exec zmodem lifecycle slot")
        .clear_if_generation(generation);
    ExecZmodemCancelRoute::NotActive
}
```

Add a drop guard which holds only a weak slot reference:

```rust
pub(super) struct ExecZmodemTransferRegistration {
    slot: std::sync::Weak<Mutex<ExecZmodemTransferSlot>>,
    generation: u64,
}

impl Drop for ExecZmodemTransferRegistration {
    fn drop(&mut self) {
        let Some(slot) = self.slot.upgrade() else {
            return;
        };
        slot.lock()
            .expect("lock exec zmodem slot for task cleanup")
            .clear_if_generation(self.generation);
    }
}
```

Implement `route_exec_zmodem_cancel` by cloning `(generation, sender)` while
locked, releasing the lock before `send`, and conditionally clearing the same
generation on send failure.

- [x] **Step 3: Register the slot when starting an exec upload**

Add `exec_zmodem_transfer: Arc<Mutex<ExecZmodemTransferSlot>>` to
`SshSessionRuntime` and initialize it in `connect_with_credential_store`.

Change `start_zmodem_upload_to_remote_dir` to create/register the command
channel before spawning:

```rust
let (exec_command_tx, exec_command_rx) = mpsc::unbounded_channel();
let generation = self
    .exec_zmodem_transfer
    .lock()
    .map_err(|_| anyhow!("failed to lock exec zmodem lifecycle slot"))?
    .register(exec_command_tx)?;
let registration = ExecZmodemTransferRegistration {
    slot: Arc::downgrade(&self.exec_zmodem_transfer),
    generation,
};
self.async_runtime.spawn(run_zmodem_exec_upload(
    self.session_id,
    Arc::clone(&self.handle),
    self.event_tx.clone(),
    local_paths,
    remote_dir,
    generation,
    exec_command_rx,
    registration,
));
```

The registration argument is intentionally retained by the task even when no
method reads it; its Drop is the all-path cleanup.

- [x] **Step 4: Route Cancel to exec first and preserve interactive fallback**

Implement:

```rust
pub fn cancel_zmodem_transfer(&self) -> Result<()> {
    match route_exec_zmodem_cancel(&self.exec_zmodem_transfer) {
        ExecZmodemCancelRoute::Routed(generation) => {
            tracing::info!(
                target: "app.zmodem",
                session_id = %self.session_id,
                transfer_generation = generation,
                lifecycle_command = "cancel",
                owner = "exec",
                outcome = "routed",
                "routed zmodem cancellation"
            );
            Ok(())
        }
        ExecZmodemCancelRoute::NotActive => {
            self.command_tx
                .send(RuntimeCommand::CancelZmodem)
                .map_err(|_| anyhow!("ssh runtime zmodem cancel channel is closed"))
        }
    }
}
```

Log interactive fallback with `owner="interactive"`; do not change its main
pump handler.

- [x] **Step 5: Run slot tests and current ZMODEM controller coverage**

Run:

```bash
cargo test -q exec_zmodem_ --lib
cargo test -q controller_cancel_queues_abort_wire_and_finishes_locally --lib
```

Expected: pass. Review `git diff -- src/app/ssh/runtime.rs` and verify the task
guard holds `Weak`, overlapping registration is rejected, and stale cleanup is
generation-conditional.

### Task 3: Cancel the dedicated exec task and prove wire ownership live

**Files:**
- Modify: `src/app/ssh/runtime/pump.rs:752-915,1010-1065`
- Modify: `src/app/ssh/runtime/zmodem.rs:14`
- Test: `tests/ssh_session_manager_spec.rs:1-45,1029-1220,1260-1400,2250-2360`

**Interfaces:**
- Consumes: `mpsc::UnboundedReceiver<ExecZmodemCommand>` registered in Task 2 and russh channel messages.
- Produces: `ExecZmodemUploadOutcome::{Completed, Cancelled, RuntimeClosed}`, cancel-aware channel selection, a terminal Cancelled state, abort-wire delivery, and deterministic channel EOF/close.

- [x] **Step 1: Extend the live russh fixture for a dedicated `rz -q` exec**

Import `zmodem2::{Action, Receiver}`. Add binary channel input plus EOF/close
capture fields to `InteractiveServerState`:

```rust
channel_inputs: Arc<Mutex<Vec<(ChannelId, Vec<u8>)>>>,
channel_eofs: Arc<Mutex<Vec<ChannelId>>>,
channel_closes: Arc<Mutex<Vec<ChannelId>>>,

impl InteractiveServerState {
    fn received_wire_contains(&self, expected: &[u8]) -> bool {
        self.channel_inputs
            .lock()
            .expect("lock channel inputs")
            .iter()
            .any(|(_, bytes)| bytes.windows(expected.len()).any(|window| window == expected))
    }

    fn abort_channel_is_settled(&self, abort_wire: &[u8]) -> bool {
        let abort_channel = self
            .channel_inputs
            .lock()
            .expect("lock channel inputs")
            .iter()
            .find(|(_, bytes)| {
                bytes
                    .windows(abort_wire.len())
                    .any(|window| window == abort_wire)
            })
            .map(|(channel, _)| *channel);
        let Some(abort_channel) = abort_channel else {
            return false;
        };
        self.channel_eofs
            .lock()
            .expect("lock channel eofs")
            .contains(&abort_channel)
            && self
                .channel_closes
                .lock()
                .expect("lock channel closes")
                .contains(&abort_channel)
    }
}
```

Add this behavior field to `InteractiveTestServer` and thread it through the
spawn helper; existing wrappers pass the default:

```rust
#[derive(Clone, Copy, Default)]
enum ZmodemExecServerBehavior {
    #[default]
    ProbeOnly,
    EmitZrinit,
    WithholdZrinit,
}

zmodem_exec_behavior: ZmodemExecServerBehavior,
```

For commands ending in `&& rz -q`, `EmitZrinit` sends the first 18 bytes from a
real receiver initialization action and leaves the channel open:

```rust
let mut receiver = Receiver::new().expect("create test zmodem receiver");
let zrinit = match receiver.poll() {
    Action::WriteWire(bytes) => bytes[..18].to_vec(),
    other => panic!("unexpected receiver initialization action: {other:?}"),
};
session.data(channel, zrinit)?;
```

`WithholdZrinit` acknowledges exec but sends no data. `ProbeOnly` preserves the
existing shell-path/EOF/status/close response. In `Handler::data`, record
`(channel, data.to_vec())` before preserving the existing shell-input/bootstrap
behavior. Implement `channel_eof` and `channel_close` to push the channel id into
the corresponding vectors.

- [x] **Step 2: Add a failing live cancellation regression**

Define the existing abort contract locally in the integration test:

```rust
const ZMODEM_ABORT_WIRE: &[u8] = b"**\x18B070000000067d4\r\n\x11";
```

Add `ssh_runtime_cancels_dedicated_exec_zmodem_with_abort_wire`:

1. Start the server with `EmitZrinit` and connect `SshSessionRuntime` using the
   existing known-host setup.
2. Create a small temporary local file and call
   `start_zmodem_upload_to_remote_dir(vec![path], "/root/1".into())`.
3. Read runtime events with a one-second timeout until phase Running appears.
4. Call `runtime_handle.cancel_zmodem_transfer()`.
5. Read until phase Cancelled appears.
6. Wait until `server_state.received_wire_contains(ZMODEM_ABORT_WIRE)` and
   `server_state.abort_channel_is_settled(ZMODEM_ABORT_WIRE)` are both true.
7. Disconnect and remove the key, known-host, and upload temp files.

Run:

```bash
cargo test -q ssh_runtime_cancels_dedicated_exec_zmodem_with_abort_wire --test ssh_session_manager_spec
```

Expected before the pump fix: Cancel is accepted by the runtime slot but the
exec task never reads it, so the test times out without Cancelled/abort evidence.

- [x] **Step 3: Add a pre-handshake cancellation regression**

Add `ssh_runtime_cancels_dedicated_exec_zmodem_before_handshake_timeout` using
`WithholdZrinit`. Start the upload, immediately call Cancel, and require a
Cancelled state plus exec channel settlement within one second, which is below
`ZMODEM_EXEC_UPLOAD_HANDSHAKE_TIMEOUT`. Assert no subsequent Failed state is
received for that generation.

Run the named test and expect it to fail by waiting for the existing handshake
timeout or publishing Failed.

- [x] **Step 4: Introduce explicit exec outcomes and command-aware waiting**

Make the abort constant visible to the sibling pump module:

```rust
pub(super) const ZMODEM_ABORT_WIRE: &[u8] = b"**\x18B070000000067d4\r\n\x11";
```

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecZmodemUploadOutcome {
    Completed,
    Cancelled,
    RuntimeClosed,
}

enum ExecZmodemInput {
    Channel(Option<ChannelMsg>),
    Command(Option<ExecZmodemCommand>),
}
```

Change `run_zmodem_exec_upload_inner` to return
`Result<ExecZmodemUploadOutcome>` and select the next input in both phases:

```rust
let input = if upload_started {
    tokio::select! {
        command = exec_command_rx.recv() => ExecZmodemInput::Command(command),
        message = channel.wait() => ExecZmodemInput::Channel(message),
    }
} else {
    let remaining = ZMODEM_EXEC_UPLOAD_HANDSHAKE_TIMEOUT
        .checked_sub(handshake_started.elapsed())
        .unwrap_or(Duration::ZERO);
    if remaining.is_zero() {
        bail!("remote rz did not emit a ZMODEM upload handshake");
    }
    tokio::select! {
        command = exec_command_rx.recv() => ExecZmodemInput::Command(command),
        result = timeout(remaining, channel.wait()) => {
            ExecZmodemInput::Channel(
                result.context("remote rz did not emit a ZMODEM upload handshake")?
            )
        }
    }
};
```

`Command(None)` returns RuntimeClosed after best-effort EOF/close and does not
publish a new modal. `Channel(message)` preserves the current message handling.

- [x] **Step 5: Implement Cancelled without falling through to Failed**

For `Command(Some(ExecZmodemCommand::Cancel))`:

```rust
if zmodem.current_state().is_some() {
    zmodem.cancel().context("cancel dedicated exec zmodem upload")?;
    emit_zmodem_state_changes(&mut zmodem, &event_tx);
    if drive_zmodem(&handle, channel.id(), &event_tx, &mut zmodem)
        .await
        .is_none()
    {
        bail!("failed to write ZMODEM abort bytes to SSH exec channel");
    }
} else {
    let _ = handle.data(channel.id(), ZMODEM_ABORT_WIRE.to_vec()).await;
    let _ = event_tx.send(SessionRuntimeEvent::ZmodemStateChanged(Some(
        cancelled_zmodem_upload_state(),
    )));
}
let _ = channel.eof().await;
let _ = channel.close().await;
return Ok(ExecZmodemUploadOutcome::Cancelled);
```

Add `cancelled_zmodem_upload_state()` beside
`failed_zmodem_upload_state()`, with Upload direction, Cancelled phase, title
`ZMODEM Upload`, headline `Upload cancelled`, and status text matching the
controller's existing cancelled upload wording.

Match the inner outcome in the wrapper. Completed keeps the existing completion
log, Cancelled emits an info lifecycle log, RuntimeClosed emits debug, and only
`Err(error)` publishes `failed_zmodem_upload_state`. Include
`transfer_generation` in all dedicated exec lifecycle logs.

- [x] **Step 6: Run live cancel and runtime unit coverage**

Run:

```bash
cargo test -q ssh_runtime_cancels_dedicated_exec_zmodem_with_abort_wire --test ssh_session_manager_spec
cargo test -q ssh_runtime_cancels_dedicated_exec_zmodem_before_handshake_timeout --test ssh_session_manager_spec
cargo test -q exec_zmodem_ --lib
cargo test -q controller_cancel_queues_abort_wire_and_finishes_locally --lib
```

Expected: all pass. The live server observes the exact abort sequence and the
runtime event stream ends in Cancelled rather than a handshake-timeout Failed
state.

- [x] **Step 7: Review the task lifecycle for all exits**

Inspect:

```bash
git diff -- src/app/ssh/runtime.rs src/app/ssh/runtime/pump.rs src/app/ssh/runtime/zmodem.rs tests/ssh_session_manager_spec.rs
```

Require these facts in the diff: one registration per start, weak guarded
cleanup, command selection before/during handshake, best-effort channel close,
no completed-task wait for UI dismissal, and no changes to the interactive pump
Cancel behavior.

### Task 4: Full verification and executable contract update

**Files:**
- Modify after behavior is verified: `.trellis/spec/backend/quality-guidelines.md`
- Verify: `src/app/ssh/session_manager.rs`
- Verify: `src/app/ssh/runtime.rs`
- Verify: `src/app/ssh/runtime/pump.rs`
- Verify: `src/app/ssh/runtime/zmodem.rs`
- Verify: `tests/bootstrap_smoke.rs`
- Verify: `tests/ssh_session_manager_spec.rs`

**Interfaces:**
- Consumes: all implementation tasks and PRD requirements R1-R8.
- Produces: passing focused/owning checks, a durable ZMODEM ownership contract, and evidence for AC1-AC7.

- [x] **Step 1: Run formatting and focused regressions**

Run:

```bash
cargo fmt --check
cargo test -q zmodem_projection_revision_ --lib
cargo test -q zmodem_dismissible_phase_ --lib
cargo test -q controller_dismiss_if_matches_ --lib
cargo test -q exec_zmodem_ --lib
cargo test -q zmodem_completed_and_terminal_modal_actions_close_without_runtime_clear_event --test bootstrap_smoke
cargo test -q zmodem_modal_escape_routes_to_close_contract --test bootstrap_smoke
cargo test -q ssh_runtime_cancels_dedicated_exec_zmodem_ --test ssh_session_manager_spec
```

Expected: all pass without ignored tests or timeout retries.

- [x] **Step 2: Run owning regression targets**

Run:

```bash
cargo test -q --lib
cargo test -q --test ssh_session_manager_spec
cargo test -q --test bootstrap_smoke
cargo test -q --test ssh_terminal_interaction_spec
```

Expected: all pass. In particular, existing interactive ZMODEM detection,
final-wire, ordinary-star, remote cwd probe, A/B/B/C routing, and SFTP fallback
tests remain green.

- [x] **Step 3: Run build, lint, and diff checks**

Run:

```bash
cargo check -q
cargo clippy --all-targets -- -D warnings
git diff --check
```

Expected: no compiler/lint/whitespace errors and no dependency or generated-file
churn.

Verification note: `cargo check -q`, `cargo fmt --check`, and
`git diff --check` pass. Repo-wide Clippy still reports 35 pre-existing
`-D warnings` failures in unchanged code. The two findings initially introduced
by this task were fixed, and the remaining diagnostics do not point to changed
lines.

- [x] **Step 4: Update the backend ZMODEM lifecycle contract**

In `.trellis/spec/backend/quality-guidelines.md`, add exact contracts stating:

- `SessionManager` owns terminal modal projection dismissal and removes only the
  snapshotted revision after releasing the registry lock for runtime cleanup.
- Runtime dismissal cleanup may clear only a matching inactive interactive
  controller and must not emit a delayed unconditional `None` projection.
- A dedicated exec upload registers one generation-scoped Cancel sender; the
  task guard clears only its own generation on every exit.
- Running dedicated exec Cancel reaches its task, writes the standard abort wire
  when possible, settles the exec channel, and publishes Cancelled or Failed.
- Bootstrap dismissal tests must not fake success by emitting `None` from the
  runtime control; live russh coverage must prove the dedicated exec abort wire.

Preserve all existing stream-gate, final-wire, routing, cwd, and SFTP contracts.

- [x] **Step 5: Run Trellis quality verification**

Load `trellis-check` and execute its required spec compliance, reuse,
cross-layer flow, lint, type/build, and test checks. Record exact commands and
any environment-only limitation. Do not claim Windows manual behavior was
observed from a non-Windows test environment.

- [ ] **Step 6: Perform the Windows manual verification matrix when available**

On the Windows build used for the original report:

1. Complete a dedicated exec upload, click Done, and wait through several UI
   refresh intervals; the modal must stay closed.
2. Repeat with title-bar X and Escape.
3. Cancel a sufficiently large running upload; the modal must become Cancelled,
   remote `rz` must exit, and the terminal session must remain usable.
4. Start another upload after dismissal/cancellation; an older control command
   must not close or cancel the new transfer.

If this environment is unavailable, report the live russh evidence and list the
Windows interaction matrix as residual manual verification rather than marking
it executed.

- [x] **Step 7: Final review and acceptance mapping**

Review `git status --short`, `git diff --stat`, and the complete diff. Confirm:

- AC1: Done closes and repeated projection flushes stay closed.
- AC2: close callback works and Escape retains the same callback wiring.
- AC3: Failed and Cancelled terminal projections dismiss without runtime `None`.
- AC4: live dedicated exec Cancel produces Cancelled plus exact abort wire.
- AC5: projection revision and exec generation tests reject stale actions.
- AC6: interactive ZMODEM, drag routing, terminal interaction, and SFTP tests pass.
- AC7: formatting, build, lint, owning tests, and Trellis checks pass.

Only after this mapping is evidenced should Phase 3.3 spec synchronization and
Phase 3.4 commit proceed.

Acceptance mapping was completed with passing unit and owning integration
targets. The Windows interaction matrix in Step 6 remains an explicit manual
follow-up because this workspace is Linux; the in-process russh tests provide
transport-level evidence for running and pre-handshake cancellation.
