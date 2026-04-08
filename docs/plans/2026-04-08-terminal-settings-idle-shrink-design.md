# Terminal Settings + Active Idle Shrink Design

日期: 2026-04-08
执行者: Codex
状态: 已批准，待实现

## 背景

当前仓库已经具备终端渲染、顶部状态栏、偏好持久化、以及成熟的 blocking modal 壳层，但这几个能力还没有在 “terminal memory settings” 这条链路上正确接起来。

与本轮任务直接相关的现状：

- 左上角 `Settings` 入口当前是错误路由：`src/shell/view_model/projection.rs:154` 的 `open_settings_panel()` 仍然调用 `open_sftp_panel()`，导致顶部菜单里的 `Settings` 实际打开的是右侧 `SFTP` 面板，而不是设置入口。
- 右侧面板当前已经是明确的 `SFTP` 工作区语义，不应该再承担通用 settings 职责；`UiPreferences` 里的 `right_panel_view` 也已经默认落到 `sftp`，见 `src/app/ui_preferences.rs:15`。
- 终端 scrollback 默认值仍然写死在 adapter 内部，`wezterm` 与 `alacritty` 两条实现都是 `3500`，见 `src/app/terminal_core/wezterm_adapter.rs:26` 与 `src/app/terminal_core/alacritty_adapter.rs:25`。
- 现有 terminal 内存回收只覆盖 “没有 active surface” 的场景；`update_workspace_terminal_idle_cache_shrink(...)` 在检测到 `has_active_surface` 时会直接返回，见 `src/app/bootstrap.rs:2430`。
- 但是 presenter / renderer / scene-image 的 transient cache 清理能力已经存在，`clear_workspace_terminal_transient_caches(...)` 和 `TerminalPresenter::clear_transient_caches()` 已经是可复用路径，见 `src/app/bootstrap.rs:2379` 与 `src/app/terminal_presenter.rs:552`、`src/app/terminal_presenter.rs:695`。
- Slint shell 已有成熟 modal 挂载方式：`ui/app-window.slint:1101` 的 `OpenSavedSshModal` 与 `ui/app-window.slint:1161` 的 `SyncVaultModal` 都通过 `BlockingModalShell` 接入。

## 本轮目标

### 必须达成

- 把左上角 `Settings` 入口修正为真正的 settings modal，不再打开右侧 `SFTP` 面板；
- 新增一个轻量 `Settings` modal，首版只承载 terminal 相关设置；
- 将 terminal scrollback 默认值从 `3500` 改为 `1500`；
- 将 scrollback 上限做成全局持久化设置，重启后保留，并作用于新建终端会话；
- 新增 “active-window idle cache shrink” 路径：窗口仍显示、terminal surface 仍存在，但 terminal 在一段时间内 idle 时，仅清理 transient caches；
- 将更深层的 Slint / Skia / DXGI purge 方案写入 `todo-0206-0408.md`，本轮不实现。

### 体验目标

- 顶部菜单中的 `Settings` 语义必须正确，用户打开的是 settings，而不是任何业务面板；
- 右侧 `SFTP` 面板保持单一职责，不再承担历史遗留 settings 占位功能；
- terminal memory 的优化优先关注真实可持续回落，不依赖噪声日志或单纯 working-set 漂亮数字；
- active idle shrink 应该尽量“无感”，以回收 transient cache 为主，不影响当前可见 terminal 的连续使用。

## 本轮不覆盖

- 不做 per-session scrollback 配置；
- 不对已存在的 SSH session 强行重配 scrollback 上限；
- 不将 `Sync Settings` 并入普通 settings modal；
- 不在本轮接入 `SkGraphics::PurgeAllCaches()`、`GrDirectContext::performDeferredCleanup()`、`IDXGIDevice3::Trim()`；
- 不在本轮直接修改 Slint / Skia backend 深层生命周期，只记录为后续待办。

## 方案对比

### 方案 1：新增轻量 Settings modal，仅承载 terminal 相关设置

做法：

- 保留左上角菜单里的 `Settings` 入口；
- 把该入口改为打开新的 settings modal；
- modal 首版只提供 `Terminal` 分组；
- 将 `scrollback limit` 与 `active idle shrink` 作为两项正式设置；
- `Sync Settings` 继续独立存在，不混入此 modal。

优点：

- 信息架构正确；
- 改动边界清晰；
- 最符合当前用户诉求；
- 不污染右侧 `SFTP` 面板。

缺点：

- 后续如要扩更多 settings，还需要继续扩这个 modal。

### 方案 2：做一个通用 settings shell modal，首版只有 Terminal 分类

做法：

- 打开 `Settings` 时进入通用 settings shell；
- shell 里做分类导航，但当前只有 `Terminal` 一页。

优点：

- 为将来扩展 Appearance / Editor / Sync 等设置留好壳层。

缺点：

- 首版过重；
- 需要先做分类壳层和导航，而当前没有明确需求。

### 方案 3：保留 `Settings` 文案，但实际打开 “Terminal Memory Settings” 专用 modal

做法：

- 不建立真正的 settings 语义；
- 只把当前功能塞进一个临时 modal。

优点：

- 改得最快。

缺点：

- `Settings` 入口问题没有真正修正；
- 后续极易形成第二个历史遗留占位口。

## 最终决策

采用 **方案 1：新增轻量 Settings modal，仅承载 terminal 相关设置**。

理由：

- 这是最小的正确修复；
- 可以一次性修正顶部菜单 IA 错误；
- 不需要把普通 settings 与 sync / vault / sftp 三套职责重新混在一起；
- 为后续真正扩展 settings 留下一个正确、可增量扩展的入口。

## 详细设计

### 1. 顶部菜单与信息架构

左上角菜单继续保留三项：

- `Sync Settings`
- `Settings`
- `Close menu`

但行为调整为：

- `Sync Settings`：继续打开当前的 sync modal；
- `Settings`：打开新的 `Settings` modal；
- 右侧面板：继续只代表 `SFTP`，不再承载 settings 占位。

这意味着：

- `src/shell/view_model/projection.rs:154` 的 `open_settings_panel()` 语义必须被移除或改造成真正的 settings modal 打开逻辑；
- `src/app/bootstrap/shell_chrome.rs:63` 的 `window.on_open_settings_panel_requested(...)` 绑定不再调用任何 `open_sftp_panel()` 风格的路径；
- 原有关于 “settings 不应再把 right panel 切到 vault” 的回归测试需要升级为 “settings 打开 modal 且不影响 right panel”。

### 2. Settings modal 的首版结构

首版 modal 只做一个 `Terminal` 分组，不提前引入左侧分类树。

字段：

- `Scrollback limit`
  - 类型：离散选项或下拉
  - 默认：`1500`
  - 候选值：`500 / 1000 / 1500 / 3000 / 5000`
  - 作用范围：全局持久化；只影响新建或重建后的 terminal session
- `Shrink active terminal caches when idle`
  - 类型：布尔开关
  - 默认：开启
  - 作用范围：全局持久化；切换后立即影响 active terminal idle shrink 判定

行为：

- 打开 modal 时展示当前持久化值；
- 改动立即写回 view model 与偏好存储；
- 关闭 modal 只负责收起，不额外提交“表单保存”步骤；
- 不引入 Apply / Cancel 两阶段草稿，首版保持简单直接。

UI 结构沿用现有模式：

- `ui/components/settings-modal.slint`：新增具体 modal 组件；
- `ui/app-window.slint`：新增 `settings-modal-open` 与相关 properties / callbacks，并通过 `BlockingModalShell` 挂载。

### 3. 持久化与状态模型

偏好层继续复用 `UiPreferences`，因为这两项仍然属于“用户侧 UI / terminal 使用偏好”，而不是底层 runtime profile。

`UiPreferences` 计划新增：

- `terminal_scrollback_limit: usize`
- `terminal_active_idle_shrink_enabled: bool`

默认值：

- `terminal_scrollback_limit = 1500`
- `terminal_active_idle_shrink_enabled = true`

状态流：

1. 启动时通过 `load_ui_preferences(...)` 读取；
2. 将 terminal 设置投影到 `ShellViewModel`；
3. `Settings` modal 打开时从 `ShellViewModel` 取当前值；
4. 用户修改时：
   - 更新 `ShellViewModel`
   - 立即保存到 `UiPreferencesStore`
   - 同步更新 terminal runtime 默认设置（供未来新 session 使用）

这里需要一个共享的 terminal runtime defaults 容器，以便 UI 层修改之后，新建 SSH session 能读取到最新值。

建议新增一个轻量共享结构，例如：

- `TerminalRuntimePreferences { scrollback_limit, active_idle_shrink_enabled }`

其职责：

- `scrollback_limit`：供 `LiveSessionRuntimeLauncher` / `SshSessionRuntime` / `TerminalSession` 创建新 terminal core 时读取；
- `active_idle_shrink_enabled`：供 bootstrap 的 active idle shrink 判定直接使用。

### 4. Scrollback 配置注入链路

现有硬编码位置：

- `src/app/terminal_core/wezterm_adapter.rs:26`
- `src/app/terminal_core/alacritty_adapter.rs:25`
- `src/app/terminal_core/mod.rs:11`
- `src/app/ssh/runtime/terminal.rs:155`
- `src/app/ssh/runtime.rs:119`
- `src/app/bootstrap.rs:326` / `src/app/bootstrap.rs:447`

调整方向：

- 不再让 adapter 自己决定固定 scrollback 常量；
- 将 `TerminalSession::new(...)` 扩成能够接收 terminal preferences；
- `create_terminal_core_adapter(...)` 增加 scrollback 参数；
- `LiveSessionRuntimeLauncher` 在发起 session runtime 时携带共享的 terminal defaults；
- `SshSessionRuntime::connect_with_credential_store(...)` 读取 scrollback limit 后创建 terminal session。

语义约束：

- 首版只保证“新 session 使用新值”；
- 已经存在的 session 不做就地迁移，避免不同 terminal backend 的 buffer 语义混乱。

### 5. Active-window idle cache shrink

现有路径仅处理 “no active surface”：

- `src/app/bootstrap.rs:2430` 在 `has_active_surface == true` 时直接 return；
- `src/app/bootstrap.rs:2466` 才会在 no-surface idle 到期后清 cache 并 release host。

本轮新增一条独立的 “active but idle” 分支：

- terminal surface 仍存在；
- 窗口仍活跃显示；
- 但 terminal 在一定时间内没有新输出、没有 viewport 变化；
- 此时只执行 `clear_workspace_terminal_transient_caches(...)`；
- 明确不调用 `release_workspace_terminal_renderer_resources()`。

建议判定信号：

- active workspace terminal surface 存在；
- `surface.seqno` 在一段时间内保持不变；
- `surface.viewport_offset_lines` 在同一时间段内保持不变；
- 设置开关允许该行为；
- 到达独立常量，例如 `WORKSPACE_TERMINAL_ACTIVE_IDLE_CACHE_SHRINK_MS = 2_000`。

这样做的原因：

- `clear_transient_caches()` 已经是成熟路径；
- 它只会清 presenter / renderer / scene-image 的再生缓存；
- 不会动 terminal core 内部 scrollback 内容；
- 风险主要是恢复后的首帧更重，而不是内容丢失或 host 重建闪烁。

本轮明确不做：

- 不 drop host；
- 不 drop native surface；
- 不释放 terminal core / session manager 持有的 scrollback；
- 不做 DirectWrite / Skia 深层 purge。

### 6. Deferred backend purge 待办

本轮将新增 `todo-0206-0408.md`，只记录后续待办，不在当前实现内落地。

待办分层：

1. `SkiaRenderer` 生命周期对 `layer_cache` 的补充清理；
2. `WinitSkiaRenderer` 在 minimized / occluded / hidden 场景下的 suspend 或轻量 cleanup；
3. `SkGraphics::PurgeAllCaches()` 的应用级背景回收策略；
4. `GrDirectContext::performDeferredCleanup()` / `purgeUnlockedResources()` / `freeGpuResources()` 的 surface teardown 策略；
5. `IDXGIDevice3::Trim()` 的 DXGI 层回收路径。

这些项统一视为后续更深层的 Slint / Skia backend 级 purge，不与当前 terminal-facing 修复混合。

## 风险与取舍

### 1. Scrollback 改小后的兼容性

风险：

- 极长命令输出场景下，用户可能更早失去较旧历史。

取舍：

- `1500` 作为默认值换来更低的长期 commit 占用；
- 同时保留全局持久化设置，让重度用户可自行调高。

### 2. Active idle shrink 的恢复成本

风险：

- 清掉 shaped-row / glyph / scene-image cache 后，恢复交互时首帧成本会上升。

取舍：

- 这条路径只清 transient cache，不释放 host；
- 以 2 秒级 idle 阈值降低抖动风险；
- 先换真实常驻回落，再观察是否需要进一步节流。

### 3. 作用域边界

风险：

- 用户可能期望 settings 改完后立即改变当前已存在 session 的 scrollback。

取舍：

- 首版明确只作用于新 session；
- 这样不会引入对 `wezterm` / `alacritty` 不同 runtime buffer 语义的热重配复杂度；
- 需要即时生效的只有 active idle shrink 开关，这一项会立即生效。

## 验收标准

- 左上角 `Settings` 入口打开的是 settings modal，而不是右侧 `SFTP` 面板；
- 右侧面板的 `right_panel_view` 在点击 `Settings` 后不被改写；
- `UiPreferences` 能正确持久化：
  - `terminal_scrollback_limit = 1500` 默认值
  - `terminal_active_idle_shrink_enabled = true` 默认值
- 新建 SSH session 使用用户设置的 scrollback 值；
- active terminal 在 idle 达到阈值后会清理 transient caches，但不会释放 host；
- `Sync Settings` modal 行为保持不变；
- 新增 `todo-0206-0408.md`，记录更深层 backend purge 项目。

## 涉及文件

预计直接相关文件：

- `src/app/ui_preferences.rs`
- `src/shell/view_model.rs`
- `src/shell/view_model/projection.rs`
- `src/app/bootstrap.rs`
- `src/app/bootstrap/shell_chrome.rs`
- `src/app/ssh/runtime.rs`
- `src/app/ssh/runtime/terminal.rs`
- `src/app/terminal_core/mod.rs`
- `src/app/terminal_core/wezterm_adapter.rs`
- `src/app/terminal_core/alacritty_adapter.rs`
- `ui/app-window.slint`
- `ui/components/titlebar-menu.slint`
- `ui/components/settings-modal.slint`
- `tests/ui_preferences.rs`
- `tests/top_status_bar_smoke.rs`
- `tests/top_status_bar_ui_contract_smoke.sh`
- `tests/vault_settings_smoke.rs`
- `todo-0206-0408.md`
