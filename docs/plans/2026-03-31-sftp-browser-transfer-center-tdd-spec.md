# SFTP Browser Transfer Center TDD Spec

## 范围

本文件对应 `2026-03-31-sftp-browser-transfer-center` 实现批次，覆盖：

- `SftpBrowserController` 驱动的按 session 浏览状态管理
- 紧凑化 `RightPanel` SFTP 浏览器
- titlebar `Transfer` 入口与 `Transfer Center` 首轮 surface
- follow/manual browse、error、disconnected、retry 的最终状态流转

## 核心 Struct

- `SftpBrowserController`
  - 文件: `src/app/sftp/browser_controller.rs`
  - 责任: 按 `session_id` 保存浏览状态，生成 `SftpBrowserLoadRequest`，处理 `open/session_activated/follow_cwd/navigate/refresh/retry`，并用 request token 丢弃旧结果。

- `SftpBrowserLoadRequest`
  - 文件: `src/app/sftp/browser_controller.rs`
  - 字段: `session_id`, `path`, `request_id`
  - 责任: 表示一次目录读取请求，供 `bootstrap` 调用 `SessionManager::sftp_read_dir()`。

- `SftpBrowserSessionState`
  - 文件: `src/app/sftp/browser_state.rs`
  - 字段: `mode`, `follow_mode`, `current_path`, `history`, `entries`, `selected_entry_ids`, `last_error`, `active_request_id`
  - 责任: 表达单个 SSH session 的 SFTP 浏览快照，并封装 `set_connecting/set_loading_follow/set_loading_manual/set_ready/set_error/set_retrying/mark_disconnected`。

- `SftpSessionBindingState`
  - 文件: `src/app/sftp/model.rs`
  - 责任: 作为 `ShellViewModel` 内对 Slint 的投影结构，保留 mode、follow、history、entries 和 error。

- `ShellViewModel`
  - 文件: `src/shell/view_model.rs`
  - 与本次功能直接相关的字段:
    - `sftp_sessions`
    - `sftp_queue_summary`
    - `transfer_center_open`
  - 与本次功能直接相关的方法:
    - `toggle_right_panel()`
    - `toggle_transfer_center()`
    - `retry_sftp_panel()`
    - `reenable_sftp_follow()`

- `TransferQueueSummary`
  - 文件: `src/app/sftp/queue.rs`
  - 字段: `total_count`, `active_count`, `failed_count`, `current_session_count`
  - 责任: 驱动 titlebar queue badge 以及后续 transfer center 汇总态。

## Trait 与接口契约

- `SftpBackend`
  - 文件: `src/app/sftp/queue.rs` 相关测试中被 mock；真实 trait 定义在 SFTP runtime 层
  - 当前实现依赖其 `read_dir/mkdir/rename/path_exists/upload_file/download_file/remove_file/remove_dir`
  - 对本功能最关键的是 `read_dir(path) -> Result<Vec<SftpDirectoryEntry>>`

- `SessionRuntimeLauncher`
  - 文件: `src/app/ssh/session_manager.rs`
  - 责任: 启动 SSH runtime，并通过 `tokio::sync::mpsc::UnboundedSender<SessionRuntimeEvent>` 上报 `Connected/Disconnected/CurrentDirectoryChanged/...`

- `SessionRuntimeControl`
  - 文件: `src/app/ssh/session_manager.rs`
  - 责任: 支持 `disconnect()`、输入事件、resize，以及 `sftp_runtime()`

- `SessionManager`
  - 关键接口:
    - `sftp_read_dir(session_id, path)`
    - `retry_session(session_id)`
    - `current_working_directory(session_id)`
    - `sftp_binding(session_id)`
  - 本次实现中，`bootstrap` 只通过这些接口桥接 UI 与 runtime，不再手写假状态推进。

## Slint Callbacks / Global State / Bindings

本次实现没有引入新的 Slint global singleton；UI 状态全部通过 `AppWindow` 顶层 `in-out property` 投影。

关键 `AppWindow` properties:

- `transfer-center-open`
- `transfer-queue-total`
- `sftp-panel-mode`
- `sftp-panel-path`
- `sftp-panel-follow-mode`
- `sftp-panel-items`
- `sftp-panel-selected-entry-ids`

关键 callback:

- titlebar:
  - `open-transfer-center-requested()`
  - `toggle-right-panel-requested()`
  - `open-sync-modal-requested()`

- SFTP panel:
  - `open-sftp-panel-requested()`
  - `sftp-panel-back-requested()`
  - `sftp-panel-forward-requested()`
  - `sftp-panel-refresh-requested()`
  - `sftp-panel-up-requested()`
  - `sftp-panel-path-submitted(string)`
  - `sftp-panel-retry-requested()`
  - `sftp-panel-reenable-follow-requested()`
  - `sftp-panel-context-menu-requested(string, string, length, length)`

绑定关系:

- `AppWindow.titlebar.transfer-center-open` 绑定到 `root.transfer-center-open`
- `AppWindow.titlebar.transfer-queue-total` 绑定到 `root.transfer-queue-total`
- `AppWindow.right-panel.*` 绑定到 `ShellViewModel` 投影出的 SFTP 浏览状态
- `AppWindow.transfer-center.open` 与 `transfer-center-open` 直连

## Tokio Task / Channel / Actor 交互

- `SessionManager::spawn_session_attempt()` 为每个 session 建立一组 runtime task:
  - 一个事件消费 task，读取 `mpsc::UnboundedReceiver<SessionRuntimeEvent>`
  - 一个 launcher task，调用 `SessionRuntimeLauncher::launch()`

- 事件流:
  - runtime 上报 `Connected`
  - runtime 上报 `CurrentDirectoryChanged(path)`
  - runtime 上报 `Disconnected`
  - `SessionManager` 将这些事件写回内部 registry

- UI 侧没有新增独立 actor；仍然使用 `bootstrap` 中的 `session_projection_timer` 周期性从 `SessionManager` 拉取投影

- 本次与 SFTP 浏览直接相关的 timer 驱动逻辑:
  - `sync_active_sftp_browser_follow_request()`
  - `sync_active_sftp_browser_pending_request()`

## 状态流转

### 打开 SFTP 浏览器

1. `AppWindow.open_sftp_panel_requested()`
2. `bootstrap::open_active_sftp_browser_for_current_session()`
3. `SftpBrowserController.open()` 或 `session_activated()`
4. `SessionManager::sftp_read_dir()`
5. `SftpBrowserController.apply_loaded_directory()`
6. `project_sftp_browser_state_into_view_model()`
7. `sync_right_panel_state()` 将结果写入 Slint

### FollowCwd 自动跟随

1. runtime 发出 `CurrentDirectoryChanged`
2. `SessionManager` 更新 `current_working_directories`
3. `session_projection_timer` 触发
4. `sync_active_sftp_browser_follow_request()` 只在 `follow_mode == FollowCwd` 时发起新请求
5. 手动浏览期间不会被新的 cwd 覆盖

### ManualBrowse

1. 用户通过路径栏提交路径或导航目录
2. `SftpBrowserController.navigate()` 将 session 切换为 `ManualBrowse`
3. 后续 `CurrentDirectoryChanged` 事件不会覆盖当前浏览路径
4. 只有 `sftp-panel-reenable-follow-requested()` 才恢复到 `FollowCwd`

### Disconnected / Retry

1. runtime 发出 `Disconnected`
2. `SessionManager` 将 session 标记为 disconnected
3. `SftpBrowserController.mark_disconnected()` 保留 `current_path/history/follow_mode`
4. 用户点击 `Retry`
5. `SessionManager::retry_session()` 重建 SSH runtime
6. `SftpBrowserController.retry()` 创建 pending request，并保持 last path 与 follow/manual 语义
7. 当 runtime 恢复后，`sync_active_sftp_browser_pending_request()` 消费 pending request，重新读取目录并回到 `Ready`

## 关键错误处理策略

- 旧请求保护:
  - 通过 `active_request_id` 和 `request_id` 丢弃过期目录读取结果

- runtime 已断开但目录读取失败:
  - `execute_sftp_browser_request()` 优先检查 `SessionManager::sftp_binding(...).mode() == Disconnected`
  - 此时走 `mark_disconnected()`，而不是错误态

- 普通目录读取失败:
  - `apply_load_error()` 保留当前路径并写入 `last_error`
  - UI 使用 `status-row` 呈现错误，不覆盖整个浏览区

- retry 恢复:
  - 即使 runtime 尚未重新连上，也先把 request 保存在 controller 中
  - 等待 projection timer 检测到新的 binding 后再执行真实读取

## Edge Cases

- Tokio channel 阻塞或消息堆积
  - 当前 session runtime 事件通道仍然是 `UnboundedSender`
  - surface 相关事件在 `SessionManager` 内已有 coalesce，但 cwd/connection 事件理论上仍可能堆积
  - 后续可考虑把高频事件进一步压缩，或把部分路径改成 bounded channel

- UI 线程更新时机不正确
  - 当前实现依赖 `session_projection_timer` 周期拉取，不是事件直推
  - 优点是保持 UI 线程单入口
  - 风险是 reconnect 后存在一个 timer tick 的恢复延迟

- 数据竞争或共享状态不一致
  - `SessionManager` registry 仍由 `Mutex` 保护
  - `ShellViewModel` 与 Slint 状态通过单线程投影同步，避免直接跨线程改 UI
  - 风险点主要在 runtime registry 与 UI 快照之间的短暂滞后

- 资源释放时序问题
  - `retry_session()` / `disconnect_session()` 会先移除旧 runtime control，再触发 `disconnect()`
  - 若关闭顺序被未来改坏，可能导致旧 runtime 事件回写已失效 session

- 异步任务取消或界面关闭后的悬挂回调
  - `session_projection_timer` 通过 `handle.upgrade()` 保护窗口已销毁场景
  - 仍需关注 future 任务完成时是否继续往已关闭窗口路径推进

- Slint model 更新与实际数据源不同步
  - `sync_vec_model()` 负责把 `ShellViewModel` 中的 `entries/selected ids` 投影到 `AppWindow`
  - 若后续为 transfer center 增加独立 rows model，需要保持与 `TransferQueue` 数据源同步策略一致

## 后续测试建议

- 单元测试
  - 为 `SftpBrowserSessionState::set_retrying()` 增加显式测试，确保它不会重置 `follow_mode/history`
  - 为 `SftpBrowserController::pending_request()` 增加测试，覆盖 disconnected -> retry -> reconnect 的中间态

- 集成测试
  - 增加 `Retry` 后 runtime 延迟恢复多个 tick 的场景，验证 pending request 只消费一次
  - 增加 transfer center 打开后 queue total 变化时 badge 与 surface 文案同步的测试

- UI 交互测试
  - 为 `status-row` 增加 render spec，覆盖 `connecting/loading/error/disconnected`
  - 为 transfer center 增加 render spec，覆盖 tab strip、empty state、后续 rows table
  - 为右侧表格新增真实 `Modified` 元数据后，补充多列对齐与截断测试

- 回归测试
  - 增加 tab 切换 + manual browse + disconnect + retry 的组合路径，验证不会意外回到 `FollowCwd`
  - 增加关闭窗口后 timer tick 与 runtime reconnect 并发场景，确认不会出现悬挂 UI 更新
