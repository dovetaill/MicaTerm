# SFTP 右侧面板 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a fixed right-side SFTP workspace that follows the active SSH tab, supports core remote file operations, and integrates a global transfer queue without changing the main terminal-first product shape.

**Architecture:** Reuse the existing SSH session/runtime as the authority and add an `app/sftp` subsystem for session-bound browsing, transfer queue management, and panel state reducers. Extend the current right panel from a single `Appearance` view into a typed right-panel switcher, and keep Slint as a pure projection/render layer fed by `ShellViewModel`.

**Tech Stack:** Rust 2024, Slint 1.15.1, `russh`, `russh-sftp`, `tokio`, current shell view-model/bootstrap pipeline, `cargo test`, `cargo check`

**Status (2026-03-31):** Implemented in worktree `/.worktree/sftp-right-panel-20260330` and verified by focused tests, smoke tests, `cargo check --workspace`, and `cargo clippy --workspace -- -D warnings`.

---

**Execution Rules:**

- 每个任务先使用 `@superpowers:test-driven-development`，先写失败测试或失败 smoke，再写最小实现，再跑通过。
- 如果任务涉及回调顺序、右键菜单命中、焦点丢失、拖拽事件或会话状态竞态，立即切换 `@superpowers:systematic-debugging`，不要猜。
- 完成所有任务后必须执行 `@superpowers:verification-before-completion`，收集新鲜测试输出后再声称完成。
- 推荐在独立 worktree 中执行；如果当前会话继续实现，先用 `@superpowers:using-git-worktrees`。

### Final Execution Notes (2026-03-31)

- Follow CWD 最终没有新建独立 watcher，而是复用了既有投影链路：
  `SSH runtime event -> SessionManager snapshot -> bootstrap::sync_active_sftp_projection_from_manager(...) -> ShellViewModel -> Slint`
- `SessionRuntimeEvent::CurrentDirectoryChanged(String)` 与 `SessionManager.current_working_directories` 成为 active SFTP path projection 的权威来源。
- retry 按钮最终调用 `SessionManager::retry_session(...)`，而不是由 `ShellViewModel` 自行重建 runtime；disconnect 会保留 path/history snapshot，仅切 panel mode。
- 顶部工具栏最终落地为 `Back / Next / Up / Sync / Re-follow / Path Bar`；`Upload / New Folder` 继续通过 SFTP context menu 进入。
- `TransferQueue` 已具备 `Overwrite / Skip` conflict policy，但 `SftpConflictModal` 组件尚未挂载到 `AppWindow`；后续 TDD 需要覆盖该 UI 缺口。
- TDD 交接文档已写入 `docs/plans/2026-03-31-sftp-right-panel-tdd-spec.md`。

### Task 1: Freeze the right-panel contract for SFTP mode

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/ui_preferences.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/right-panel.slint`
- Test: `tests/ui_preferences.rs`
- Test: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

Add tests asserting:

- `RightPanelView` supports both `appearance` and `sftp`
- `UiPreferences` can persist `right_panel_view = "sftp"`
- default shell state still starts in `appearance`
- right-panel sync path can project `sftp` without panicking

Example assertions:

```rust
assert_eq!(RightPanelView::from_id("sftp"), RightPanelView::Sftp);
assert_eq!(UiPreferences { right_panel_view: "sftp".into(), ..Default::default() }.right_panel_view, "sftp");
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test ui_preferences --test shell_view_model -q
```

Expected: FAIL because `sftp` right-panel mode does not exist yet.

**Step 3: Write the minimal implementation**

- Add `RightPanelView::Sftp`
- Extend `right_panel_view` parsing / serialization
- Extend `sync_right_panel_state(...)`
- Add a placeholder `sftp-panel` branch in `ui/shell/right-panel.slint`
- Add the minimal `AppWindow` properties needed to render an empty SFTP state

Do not add real SFTP behavior yet. This task only freezes the shell contract.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test ui_preferences --test shell_view_model -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/ui_preferences.rs src/app/bootstrap.rs ui/app-window.slint ui/shell/right-panel.slint tests/ui_preferences.rs tests/shell_view_model.rs
git commit -m "feat: add sftp right panel shell contract"
```

### Task 2: Add SFTP domain models, reducers, and queue state

**Files:**
- Create: `src/app/sftp/mod.rs`
- Create: `src/app/sftp/model.rs`
- Create: `src/app/sftp/queue.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/sftp_panel_state_spec.rs`
- Test: `tests/sftp_queue_spec.rs`

**Step 1: Write the failing tests**

Add reducer-focused tests covering:

- `SftpPanelMode` transitions: `empty -> connecting -> loading -> ready -> disconnected`
- Follow mode switching: `follow-cwd -> manual-browse -> follow-cwd`
- per-session path history `back / forward / push`
- queue summary aggregation for active / failed / current-session-only tasks

Example test shape:

```rust
#[test]
fn manual_browse_breaks_follow_mode_until_reenabled() {
    let mut state = SftpSessionBindingState::follow("/srv/app");
    state.navigate_manual("/srv/app/releases");
    assert_eq!(state.follow_mode, SftpFollowMode::ManualBrowse);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sftp_panel_state_spec --test sftp_queue_spec -q
```

Expected: FAIL because the SFTP domain module does not exist yet.

**Step 3: Write the minimal implementation**

Create pure Rust domain types such as:

```rust
pub enum SftpPanelMode { Empty, Connecting, Disconnected, Loading, Ready, Error }
pub enum SftpFollowMode { FollowCwd, ManualBrowse }
pub enum TransferTaskState { Queued, Running, Paused, Completed, Failed, Cancelled, Conflict }
```

Add:

- `SftpDirectoryEntry`
- `SftpPathHistory`
- `SftpSessionBindingState`
- `TransferTask`
- `TransferQueueSummary`

Keep this task pure:

- no `Slint`
- no `russh`
- no file dialogs
- no runtime I/O

Expose the new view-model fields from `ShellViewModel`, but only as raw state containers.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sftp_panel_state_spec --test sftp_queue_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/mod.rs src/app/sftp/model.rs src/app/sftp/queue.rs src/app/mod.rs src/shell/view_model.rs tests/sftp_panel_state_spec.rs tests/sftp_queue_spec.rs
git commit -m "feat: add sftp domain state and queue models"
```

### Task 3: Introduce session-bound SFTP runtime scaffolding on top of SSH runtime

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/sftp/runtime.rs`
- Create: `src/app/sftp/session_binding.rs`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Test: `tests/ssh_session_manager_spec.rs`
- Test: `tests/sftp_runtime_spec.rs`

**Step 1: Write the failing tests**

Add tests that require:

- an active SSH session can vend an SFTP capability or binding handle
- retrying or disconnecting a session invalidates the old binding and creates a recoverable SFTP state
- the runtime can represent directory-load requests and operation responses through a fake backend

Example assertions:

```rust
assert!(session_manager.sftp_binding(session_id).is_some());
assert_eq!(binding.mode(), SftpPanelMode::Connecting);
```

For runtime tests, use a fake backend trait rather than a live SSH server.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test ssh_session_manager_spec --test sftp_runtime_spec -q
```

Expected: FAIL because SFTP runtime/binding APIs do not exist yet.

**Step 3: Write the minimal implementation**

- Add `russh-sftp` to `Cargo.toml`
- Create an SFTP runtime abstraction:

```rust
pub trait SftpBackend {
    async fn read_dir(&self, path: &str) -> Result<Vec<SftpDirectoryEntry>>;
    async fn mkdir(&self, path: &str) -> Result<()>;
    async fn rename(&self, from: &str, to: &str) -> Result<()>;
}
```

- Add a real backend wrapper using `russh-sftp`
- Add a session binding layer that maps `session_id -> sftp runtime state`
- Extend `SessionRuntimeControl` or an adjacent runtime contract so the SSH runtime can open an SFTP subsystem using the same authenticated session

Do not implement upload/download streams yet. This task only establishes the channel and state plumbing.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test ssh_session_manager_spec --test sftp_runtime_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml src/app/sftp/runtime.rs src/app/sftp/session_binding.rs src/app/ssh/runtime.rs src/app/ssh/session_manager.rs tests/ssh_session_manager_spec.rs tests/sftp_runtime_spec.rs
git commit -m "feat: add session-bound sftp runtime scaffolding"
```

### Task 4: Project SFTP panel state into Slint and wire the navigation callbacks

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/right-panel.slint`
- Test: `tests/assets_modal_render_spec.rs`
- Test: `tests/sftp_right_panel_render_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add UI contract tests requiring:

- empty state renders when no SSH tab is active
- connected state renders the session line, path bar, and list header
- disconnected state renders retry guidance and disables file actions
- navigation callbacks update the projected path state

Example assertions:

```rust
assert!(rendered_pixels_for("SFTP").len() > 0);
assert!(window.get_sftp_panel_mode() == "empty".into());
```

Use a synthetic `AppWindow` setup similar to other render tests in the repo.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sftp_right_panel_render_spec --test bootstrap_smoke -q
```

Expected: FAIL because the SFTP properties, callbacks, and rendering path are incomplete.

**Step 3: Write the minimal implementation**

Extend `AppWindow` with the properties and callbacks needed for:

- panel mode
- active host label
- path text
- list rows
- selected entry ids
- follow mode
- queue summary
- commands: back/forward/refresh/up/path submit/upload/new folder/open queue/retry

Implement `sync_sftp_panel_state(...)` in `src/app/bootstrap.rs` and render the four-layer panel:

- session strip
- browser toolbar
- file list
- transfer summary strip

Keep styling integrated with the current right-panel shell; do not introduce a new visual language.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sftp_right_panel_render_spec --test bootstrap_smoke -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs ui/app-window.slint ui/shell/right-panel.slint tests/sftp_right_panel_render_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: render sftp right panel and navigation shell"
```

### Task 5: Add context menus, create/rename/delete flows, and queue drawer summary behavior

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/right-panel.slint`
- Create: `ui/components/sftp-conflict-modal.slint`
- Test: `tests/assets_context_menu_spec.rs`
- Test: `tests/assets_context_menu_smoke.rs`
- Test: `tests/sftp_context_menu_spec.rs`

**Step 1: Write the failing tests**

Add tests requiring:

- blank-area, file, folder, and multiselect SFTP targets resolve the expected action trees
- disabled states are correct while disconnected or loading
- rename conflict reports inline error instead of silent renaming
- queue summary click opens the queue drawer state

Example assertion:

```rust
assert_eq!(actions[0].id, "upload-files");
assert!(actions.iter().any(|node| node.id == "copy-sftp-url"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_context_menu_spec --test sftp_context_menu_spec -q
```

Expected: FAIL because SFTP context targets and actions do not exist yet.

**Step 3: Write the minimal implementation**

- Add SFTP-specific `ContextTargetKind` branches or an adjacent SFTP context domain
- Add callback wiring for blank-area menu, row menu, and queue summary
- Add conflict-modal shell state
- Implement rename/delete/new-folder action dispatch and state transitions

Do not add upload/download bytes here. This task only connects UI actions and conflict decisions to state.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_context_menu_spec --test sftp_context_menu_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs src/app/bootstrap.rs src/shell/view_model.rs ui/app-window.slint ui/shell/right-panel.slint ui/components/sftp-conflict-modal.slint tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs tests/sftp_context_menu_spec.rs
git commit -m "feat: add sftp context menus and conflict state"
```

### Task 6: Implement upload, download, move, and delete operations with the global queue

**Files:**
- Create: `src/app/sftp/local_ops.rs`
- Modify: `src/app/sftp/runtime.rs`
- Modify: `src/app/sftp/queue.rs`
- Modify: `src/app/sftp/session_binding.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/sftp_queue_spec.rs`
- Test: `tests/sftp_runtime_spec.rs`
- Test: `tests/sftp_transfer_flow_spec.rs`

**Step 1: Write the failing tests**

Add tests covering:

- uploading files creates queued tasks, then running tasks, then completed tasks
- downloading files updates queue summary and current-session filtering
- moving a remote entry into a directory updates the directory listing and clears stale selection
- deletion blocks or cancels conflicting transfer tasks
- conflict tasks can be resumed with a selected policy

Example test shape:

```rust
#[test]
fn upload_to_folder_creates_queue_task_bound_to_session() {
    let queue = enqueue_upload(session_id, "/srv/app", vec![local_file()]);
    assert_eq!(queue.tasks[0].state, TransferTaskState::Queued);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sftp_runtime_spec --test sftp_queue_spec --test sftp_transfer_flow_spec -q
```

Expected: FAIL because transfer execution and local-op handling are incomplete.

**Step 3: Write the minimal implementation**

- Add local source scanning and upload request builders in `local_ops.rs`
- Implement upload/download streaming in `sftp/runtime.rs`
- Implement queue task transitions, cancellation, and retry in `queue.rs`
- Wire bootstrap callbacks for `upload-files`, `upload-folder`, `download`, `download-selected`, `delete`, `move`, and conflict resolution

Prefer the simplest working transfer model:

- per-task async worker
- progress bytes and terminal state updates
- session association on every task

Do not add drag-out to the system file manager in this task.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sftp_runtime_spec --test sftp_queue_spec --test sftp_transfer_flow_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/local_ops.rs src/app/sftp/runtime.rs src/app/sftp/queue.rs src/app/sftp/session_binding.rs src/app/bootstrap.rs tests/sftp_queue_spec.rs tests/sftp_runtime_spec.rs tests/sftp_transfer_flow_spec.rs
git commit -m "feat: implement sftp transfer queue and file operations"
```

### Task 7: Add Follow CWD, retry/disconnect recovery, and final shell verification

**Files:**
- Modify: `src/app/sftp/session_binding.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/ssh/runtime.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`
- Test: `tests/sftp_follow_cwd_spec.rs`
- Test: `tests/sftp_right_panel_render_spec.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add tests requiring:

- shell path updates move the SFTP panel while in `follow-cwd`
- manual navigation switches to `manual-browse`
- explicit re-enable returns to `follow-cwd`
- disconnect keeps the last snapshot but disables actions
- retry rebinds the panel to a new live runtime without losing session history

Example assertion:

```rust
assert_eq!(panel.path, "/srv/app/releases");
assert_eq!(panel.follow_mode, SftpFollowMode::FollowCwd);
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test ssh_terminal_interaction_spec --test sftp_follow_cwd_spec --test sftp_right_panel_render_spec -q
```

Expected: FAIL because Follow CWD and recovery semantics are not wired yet.

**Step 3: Write the minimal implementation**

- Extend the SSH terminal/runtime path observer so current working directory changes can be published
- Feed those updates into `SftpSessionBindingState`
- Add retry/disconnect hooks that preserve snapshot state and selection rules
- Ensure all file actions gate on the current panel mode

Keep the implementation conservative:

- no automatic refresh storming
- debounce repeated cwd updates if needed
- do not add remote filesystem watching

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test ssh_terminal_interaction_spec --test sftp_follow_cwd_spec --test sftp_right_panel_render_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/session_binding.rs src/app/bootstrap.rs src/app/ssh/session_manager.rs src/app/ssh/runtime.rs tests/ssh_terminal_interaction_spec.rs tests/sftp_follow_cwd_spec.rs tests/sftp_right_panel_render_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: add sftp follow cwd and recovery flow"
```

### Task 8: Run full verification and update docs

**Files:**
- Modify: `docs/plans/2026-03-30-sftp-right-panel-design.md`
- Modify: `docs/plans/2026-03-30-sftp-right-panel-implementation-plan.md`
- Reference: `README.md` if a user-facing mention is warranted

**Step 1: Run focused Rust tests**

Run:

```bash
cargo test --test ui_preferences --test shell_view_model --test ssh_session_manager_spec --test sftp_panel_state_spec --test sftp_queue_spec --test sftp_runtime_spec --test sftp_right_panel_render_spec --test sftp_transfer_flow_spec --test sftp_follow_cwd_spec -q
```

Expected: PASS

**Step 2: Run relevant smoke tests**

Run:

```bash
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected: PASS

**Step 3: Run compile verification**

Run:

```bash
cargo check --workspace
```

Expected: PASS

**Step 4: Run lint verification**

Run:

```bash
cargo clippy --workspace -- -D warnings
```

Expected: PASS

**Step 5: Update docs if execution drifted from the plan**

If any task required a different file path, callback name, or reducer split than planned, update:

- `docs/plans/2026-03-30-sftp-right-panel-design.md`
- `docs/plans/2026-03-30-sftp-right-panel-implementation-plan.md`

Keep the docs aligned with shipped behavior.

**Step 6: Generate TDD handoff**

Write a TDD handoff spec covering:

- core structs / traits
- Slint callbacks and properties
- concurrency and retry edge cases
- known UI gaps to cover next

Target path:

```bash
docs/plans/2026-03-31-sftp-right-panel-tdd-spec.md
```

**Step 7: Commit (optional, only if requested)**

```bash
git add docs/plans/2026-03-30-sftp-right-panel-design.md docs/plans/2026-03-30-sftp-right-panel-implementation-plan.md
git commit -m "docs: finalize sftp right panel plan"
```

---

Plan complete and saved to `docs/plans/2026-03-30-sftp-right-panel-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
