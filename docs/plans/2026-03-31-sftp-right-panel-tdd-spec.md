# SFTP 右侧面板 TDD Handoff Spec

日期: 2026-03-31
来源:
- `docs/plans/2026-03-30-sftp-right-panel-design.md`
- `docs/plans/2026-03-30-sftp-right-panel-implementation-plan.md`

状态: SFTP 右侧面板实现已完成 Task 8 验证，可进入下一轮 TDD 补强。

## 1. 已验证范围

- 右侧面板已支持 `appearance` / `sftp` 切换，并持久化 `UiPreferences.right_panel_view`
- `app/sftp` 已提供 session-bound state、runtime abstraction、queue、local ops 与 session binding helper
- SFTP 目录浏览、路径提交、`back / forward / up`、queue summary、context menu state flow 已有聚焦测试
- Follow CWD、manual browse、断线快照保留、retry rebind 已通过 `ssh_terminal_interaction_spec` / `sftp_follow_cwd_spec`
- 本轮最终验证已执行：
  - `cargo test --test ui_preferences --test shell_view_model --test ssh_session_manager_spec --test sftp_panel_state_spec --test sftp_queue_spec --test sftp_runtime_spec --test sftp_right_panel_render_spec --test sftp_transfer_flow_spec --test sftp_follow_cwd_spec -q`
  - `bash tests/ssh_connect_tabs_ui_contract_smoke.sh`
  - `bash tests/assets_context_menu_ui_contract_smoke.sh`
  - `cargo check --workspace`
  - `cargo clippy --workspace -- -D warnings`

## 2. 核心 Rust 结构与职责

### `src/app/sftp/model.rs`

- `SftpPanelMode`
  - `Empty | Connecting | Loading | Ready | Disconnected | Error`
  - 是 Slint 面板状态与操作 enable/disable 的主开关
- `SftpFollowMode`
  - `FollowCwd | ManualBrowse`
  - 约束 cwd 推送是否还能覆盖当前 path
- `SftpDirectoryEntry`
  - 目录项投影，最少包含 `id / name / path / kind / size_bytes`
- `SftpPathHistory`
  - 负责 `push / back / forward`，是 session 层路径历史真源
- `SftpSessionBindingState`
  - 负责 `mode / follow_mode / current_path / history / entries / selected_entry_ids / last_error`
  - 关键不变量：
    - `navigate_manual(...)` 会切到 `ManualBrowse`
    - `follow_terminal_path(...)` 只在 `FollowCwd` 模式生效
    - `mark_disconnected()` 不清空 path/history

### `src/app/sftp/runtime.rs`

- `SftpBackend`
  - 当前 runtime 抽象的 trait 边界
  - 已暴露 `read_dir / mkdir / rename / path_exists / upload_file / download_file / remove_file / remove_dir`
- `SftpRuntimeHandle`
  - 对 `Arc<dyn SftpBackend>` 的 cloneable handle
  - `binding_id()` 用于区分 live SFTP binding 实例

### `src/app/sftp/queue.rs`

- `TransferQueue`
  - 全局传输队列，当前以 `Vec<TransferTask>` 为主存储
- `TransferTask`
  - 关键字段：`session_id / source_path / target_path / direction / action / state / conflict_policy`
- `TransferQueueSummary`
  - 从全局队列派生 `total_count / active_count / failed_count / current_session_count`
- `TransferConflictPolicy`
  - 当前只支持 `Overwrite | Skip`
- `TransferTaskState`
  - `Queued | Running | Paused | Completed | Failed | Cancelled | Conflict`

### `src/app/sftp/session_binding.rs`

- `SftpSessionBinding`
  - session 和 live runtime 的桥接壳
  - `connecting(...)` 与 `disconnected(...)` 区分 live / recoverable snapshot
- `execute_queued_transfers(...)`
  - 当前按 queued task 顺序执行
- `move_entry_between_directories(...)`
  - 负责远端 move 后对当前目录列表和选中态的最小修正
- `delete_entries(...)`
  - 删除前先 `cancel_conflicting_paths(...)`
  - 删除成功后同步移除当前 session 的 entries / selection

### `src/app/ssh/runtime.rs` 与 `src/app/ssh/session_manager.rs`

- `SessionRuntimeEvent::CurrentDirectoryChanged(String)`
  - cwd 变化事件，由 SSH runtime 提取后投递给 manager
- `RusshSftpBackend`
  - 复用当前 SSH runtime 的 SFTP 子通道后端
- `SessionManager`
  - 当前 SFTP session authority
  - 关键接口：
    - `sftp_binding(session_id)`
    - `current_working_directory(session_id)`
    - `sftp_read_dir(...)`
    - `sftp_execute_queued_transfers(...)`
    - `sftp_delete_entries(...)`
    - `sftp_move_entry_between_directories(...)`
    - `retry_session(session_id)`

### `src/shell/view_model.rs`

- `ShellViewModel.sftp_sessions`
  - 每个 SSH session 一份 `SftpSessionBindingState`
- `ShellViewModel.sftp_queue_summary`
  - 当前 active session 的 queue summary 投影入口
- `ShellViewModel.sftp_queue_drawer_open`
  - queue drawer UI 开合状态
- `SftpConflictModalState`
  - 当前仅有 shell state：`open / source_path / target_path / can_resume / apply_to_all`

## 3. Slint 属性与回调合同

### `ui/app-window.slint`

已投影属性：

- `sftp-panel-mode`
- `sftp-panel-host-label`
- `sftp-panel-path`
- `sftp-panel-follow-mode`
- `sftp-panel-can-go-back`
- `sftp-panel-can-go-forward`
- `sftp-panel-can-go-up`
- `sftp-panel-actions-enabled`
- `sftp-panel-items`
- `sftp-panel-selected-entry-ids`
- `sftp-panel-queue-active`
- `sftp-panel-queue-failed`
- `sftp-panel-queue-current-session`
- `sftp-queue-drawer-open`

已声明回调：

- `open-sftp-panel-requested()`
- `sftp-panel-back-requested()`
- `sftp-panel-forward-requested()`
- `sftp-panel-refresh-requested()`
- `sftp-panel-up-requested()`
- `sftp-panel-path-submitted(string)`
- `sftp-panel-context-menu-requested(string, string, length, length)`
- `sftp-panel-upload-requested()`
- `sftp-panel-new-folder-requested()`
- `sftp-panel-open-queue-requested()`
- `sftp-panel-retry-requested()`
- `sftp-panel-reenable-follow-requested()`

### 当前已在 `bootstrap` 实际绑定的回调

- `open-sftp-panel-requested`
- `sftp-panel-context-menu-requested`
- `sftp-panel-open-queue-requested`
- `sftp-panel-path-submitted`
- `sftp-panel-back-requested`
- `sftp-panel-forward-requested`
- `sftp-panel-up-requested`
- `sftp-panel-refresh-requested`
- `sftp-panel-retry-requested`
- `sftp-panel-reenable-follow-requested`

### 当前未完成端到端挂载的 UI 合同

- `sftp-panel-upload-requested`
- `sftp-panel-new-folder-requested`
- `ui/components/sftp-conflict-modal.slint` 的全部回调

这些符号已声明或已有 shell state，但尚未形成 `AppWindow -> bootstrap -> SessionManager / queue` 的完整链路，是下一轮 TDD 的重点。

## 4. 当前行为不变量

- active SFTP projection 走单一路径：
  `SSH runtime event -> SessionManager snapshot -> bootstrap::sync_active_sftp_projection_from_manager(...) -> ShellViewModel -> Slint`
- `follow-cwd` 只在 active session 且 follow 模式下推进 path/history
- 用户一旦走 `path submit / back / forward / up`，session 会切到 `manual-browse`
- `reenable_follow()` 会恢复 `follow-cwd` 并立即对齐最近一次 cwd snapshot
- disconnect 时保留 path/history/selection，`mode` 切为 `Disconnected`，文件动作禁用
- retry 时通过 `SessionManager::retry_session(...)` 重新绑定 live runtime，不由 view-model 直接重建 runtime
- queue summary 是全局队列视图，但 `current_session_count` 仍按 active session 过滤

## 5. 边缘情况与风险清单

- Tokio / channel 时序：
  - cwd 事件、disconnect、retry 可能前后脚到达；TDD 需要验证 reconnect 后旧事件不会错误覆盖新 binding 语义
- refresh storm：
  - 当前 Follow CWD 只推进 path/history，不主动做 remote watcher；后续若加自动 refresh，必须验证不会因连续 `cd` 触发刷新风暴
- 选择态一致性：
  - delete / move 后当前目录列表会被最小修正；TDD 需要覆盖目录外 move、删除多选、selection 清理
- queue conflict 流程：
  - queue 层已支持 `Conflict -> resume_conflict(policy)`，但缺少 AppWindow 级弹窗驱动与 apply-to-all 端到端验证
- Slint 合同漂移：
  - `Upload / New Folder` 回调与组件已声明，但工具栏未落地对应按钮；需要决定是删除死接口还是补挂载
- 本地文件系统副作用：
  - `scan_local_sources(...)`、`build_local_download_path(...)` 涉及真实磁盘；TDD 应继续优先使用临时目录隔离测试

## 6. 下一轮 TDD 建议顺序

1. 为 `sftp-panel-upload-requested` 与 `sftp-panel-new-folder-requested` 补红测，确认 `AppWindow` 到 `bootstrap` 的回调链路当前确实未接通。
2. 为 `SftpConflictModal` 增加红测，覆盖 modal 投影、回调分派、`apply-to-all` 与 `resume_conflict(policy)` 交互。
3. 补 session 时序测试：
   - disconnect 后收到迟到 cwd event
   - retry 后旧 binding 事件不会污染新 binding
4. 补 queue 端到端测试：
   - conflict -> choose overwrite/skip -> task 恢复执行
   - delete 与 running task 冲突时的取消提示
5. 补 UI 合同测试：
   - 顶部工具栏若继续保持当前实现，应更新设计快照与 smoke
   - 若决定补回 `Upload / New Folder` 按钮，则先写渲染和点击红测
