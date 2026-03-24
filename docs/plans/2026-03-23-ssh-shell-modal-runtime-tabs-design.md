# 2026-03-23 SSH / Shell / Modal / Runtime / Tabs Design

## 背景

当前仓库已经具备以下最小壳层：

- `Assets` 侧边栏、资产树、创建入口、重命名、删除、Tree/Flat 切换已落地
- `New Folder`、`Rename`、`New SSH Connection`、`SSH host key confirm` modal 已存在
- `WorkspaceTabItem`、`ActiveTab`、`workspace-tab-close-requested`、最小 session state dot 已存在
- `Cargo.toml` 已引入 `russh`、`termwiz`、`wezterm-term`、`keyring`
- `src/app/ssh/profile.rs`、`runtime.rs`、`session_manager.rs` 已存在最小 glue code

但当前用户可见行为仍明显失真：

- 启动后 workspace / tab 区域出现白色竖条，且会随 tab 数量被撑开
- asset/SSH 类 modal 点击外部会自动关闭，且均不可拖动
- `Test Connection` / `Connect` / `Save and Connect` 的行为与反馈不真实
- SSH 连接后会出现“有 tab / 像已连接 / 中间仍是占位 host”的假闭环
- tab close 在用户视角下无效
- SSH 资产编辑模型尚未成立

本设计文档用于收敛以上边界，并记录用户已确认的设计决策。

## 目标

- 建立稳定的 `Workspace / Tab / TerminalHost` 布局边界，消除白色竖条与宽度错位
- 建立统一的阻断式 modal contract，覆盖 drag / dismiss / focus / blocking
- 明确定义 `Test / Connect / Save / Save and Connect` 的动作语义与反馈语义
- 为首轮真实 SSH 连接定义清晰的 `russh + termwiz + wezterm-term` 分层
- 明确 tab / session 的创建、复用、关闭、失败态与标题规则
- 为 SSH 资产编辑建立最小可成立的数据契约
- 对当前 Windows 内存占用偏高问题做结构性分析，给出优化边界

## 非目标 / 边界

- 本轮不扩展完整 SFTP、资产目录全量重构、批量编辑、代理链、隧道与高级网络编排
- 本轮不把 `Tunnel / Proxy / Environment / Advanced` 页签补成真实可用能力
- 本轮不把 terminal renderer 的完整绘制实现细节写死到 UI 层
- 本轮不引入圆角回流，所有新方案继续维持 flat/no-radius 方向

## 当前实现现状

### Shell / Workspace

- `AppWindow` 直接在根布局里组合 `Sidebar -> main-workspace -> RightPanel`
- `main-workspace` 使用 `HorizontalLayout` 内部的 `VerticalLayout`
- `TabBar` 已支持 `WorkspaceTabItem`
- `TerminalSessionHost` 仅是状态切换宿主，不是真实 terminal surface

### Modal

- 根层存在统一 `asset-modal-dismiss-layer`
- 当前 outside click 会直接关闭 create/rename/delete modal
- `ssh-host-key` modal 的 outside click 直接走 reject
- modal 具备 `Esc` / `Enter` 的键盘处理，但没有拖动协议

### SSH

- `ConnectionProfile::from_draft()` 已能从 modal draft 归一化 profile
- `SessionManager` 已支持 `ActivateExisting` / `ForceNewTab`
- `SshSessionRuntime::connect()` 当前仅立即发出 `Connected`
- `SessionManager` 能接 runtime event 更新 registry，但 UI 没有持续订阅

### 资产数据

- SSH 资产 payload 目前只持久化：
  - `host`
  - `user`
  - `port`
  - `environment`
  - `proxy_method`
- 当前不能支撑完整 SSH 编辑 / 重连 / secret 管理

## 截图现象与源码映射

### 1. Workspace 白色竖条与宽度被 tab 撑开

现象：

- 无 tab 时，资产列表右侧出现白色窄条
- 打开 tab 后，白色区域宽度随 tab 数量增加而变宽

源码映射：

- `main-workspace` 声明了 `horizontal-stretch: 1`
- 但其内部 `VerticalLayout`、`TabBar`、`content-host` 没有形成稳定的横向填充契约
- `TabBar` 的固有宽度来自 `HorizontalLayout` padding + 固定宽度 tab 项
- 因此 workspace 实际宽度退化为“当前 tab 内容宽度”

结论：

- 这不是简单 border/background 细节，而是 `WorkspacePane` 边界缺失

### 2. Modal click-away 自动关闭

源码映射：

- `ui/app-window.slint` 中 `asset-modal-dismiss-layer.clicked`
- 直接调用 `close-asset-modal-requested()` 或 `ssh-host-key-modal-reject-requested()`

结论：

- 当前是根层统一 dismiss 路径导致 click-away 关闭
- 问题不在单个 modal 内部

### 3. 按钮“有 UI、无真实行为”

源码映射：

- Slint 按钮已有 `TouchArea`
- `bootstrap.rs` 已绑定 `on_asset_ssh_modal_action_requested`
- 但 `Test Connection` 直接写死 `Connection test succeeded.`
- `Connect` / `SaveAndConnect` 最终仍走 stub runtime

结论：

- 当前不是“完全没 wiring”
- 而是 “UI wiring 存在，但 runtime 语义与反馈语义是假的”

### 4. SSH 打开 tab 后仍是占位 host

源码映射：

- `WorkspaceTab.uses_terminal_surface()` 把 `connecting/connected` 视为 terminal mode
- `TerminalSessionHost` 的 `terminal` 分支只显示说明文案
- `SshSessionRuntime::connect()` 立即上报 `Connected`

结论：

- 当前形成了“tab 状态先进入 connected，但 renderer host 仍为空壳”的错位

### 5. Tab close 用户视角无效

源码映射：

- `ActiveTab` 里 close button 有独立 `TouchArea`
- 但组件最后又定义了覆盖整个 tab 的 `TouchArea`

结论：

- close affordance 很可能被整体 hit area 覆盖
- 用户点击 `×` 时实际只触发 `selected()`

## 旧 prompt 中已被源码证伪的表述

- `tabbar` 不是“纯 placeholder”，当前已有最小 tab model 与 close callback
- SSH modal 不是“缺少字段标签”，标准连接页字段和认证切换已存在
- 项目不是“完全没有 SSH runtime glue code”，当前已有最小 `profile/runtime/session_manager`
- `ssh-host-key` modal 已存在，但尚未接入真实连接链路

更准确的表述应为：

- “已有最小壳层，但真实行为未闭环”
- “已有字段和 UI，但交互反馈、runtime、编辑模型、terminal surface 仍未成立”

## 设计要点与最终决策

### 设计要点 1：Workspace / Tab / TerminalHost 布局边界

可选方案：

- A. 只做局部 stretch 修复
- B. 抽出独立 `WorkspacePane`，统一拥有 tab strip、content host、future terminal surface 的宽高契约

最终决策：

- 采用 `1B`

原因：

- 白条问题已证明 workspace 不是单点样式 bug，而是容器边界缺失
- 后续 terminal renderer 接入仍需要稳定 pane contract

落地约束：

- `AppWindow` 只负责组合，不再直接承担 workspace 内部宽度语义
- `WorkspacePane` 必须拥有稳定全宽布局
- `TabBar` 与 `content-host` 都必须显式横向填充

### 设计要点 2：Modal 的 drag / dismiss / blocking / focus 语义

可选方案：

- A. 统一阻断式 modal；禁止 click-away dismiss；标题栏热区拖动 modal 本身
- B. 继续根层 dismiss，modal 标题拖动转为拖动宿主窗口

最终决策：

- 采用 `2A`

原因：

- “拖 modal 却移动整个主窗口”在语义上错误
- 当前项目是自绘壳层，适合建立统一 modal contract

落地约束：

- create / rename / delete / host key / SSH edit 全部进入统一 modal contract
- backdrop 只拦截，不承担关闭逻辑
- `Esc` 仍保留
  - create / rename / SSH edit：关闭
  - host key：reject
- 默认采用标题栏热区拖动
- 关闭时焦点恢复到触发源
- modal 每次重新打开默认居中；拖动位置不跨会话持久化

### 设计要点 3：SSH modal 四个动作的真实语义

可选方案：

- A. 四个动作完全拆开，支持临时连接
- B. 保持 save-first，`Connect` 也先落资产

最终决策：

- 采用 `3A`

最终语义：

- `Test Connection`
  - 做真实网络连通 + SSH 握手 + host key 校验 + 认证校验
  - 不创建 tab
  - 不保存资产
- `Connect`
  - 允许未保存 draft 发起临时 session
  - 打开 tab
  - 不落资产目录
- `Save`
  - 只保存资产
  - 不连接
- `Save and Connect`
  - 先保存资产
  - 再打开 session

反馈策略：

- 主反馈位于 modal 内状态区
- 必须存在 `hover / active / disabled / busy / success / error`
- 不使用 toast 代替主反馈

### 设计要点 4：真实 SSH runtime 的首轮边界

可选方案：

- A. `SessionManager -> SshSessionRuntime -> terminal adapter -> renderer host` 分层
- B. 继续把 runtime / terminal state 混进 view model

最终决策：

- 采用 `4A`

分层职责：

- `SessionManager`
  - session 生命周期真源
  - open/activate/force-new/close/disconnect/reconnect
- `SshSessionRuntime`
  - 真实 `russh` transport
  - channel open
  - `request_pty`
  - `request_shell`
  - remote output pump
  - input write-back
  - resize / disconnect / error
- `wezterm-term`
  - 终端仿真状态
- `termwiz`
  - 输入编码、协议辅助类型
- terminal renderer host
  - 只读 terminal snapshot / delta
  - 不持有 transport 逻辑

首轮支持范围：

- password
- private key path
- inline private key content
- host key TOFU
- PTY shell
- resize
- input / output
- disconnect / error

首轮不支持：

- tunnel
- proxy chain
- environment 注入
- advanced 页里的高阶网络能力

### 设计要点 5：SSH tab / session 生命周期

可选方案：

- A. 默认单资产单活跃 session；显式 `Open in New Tab` 才强制新建
- B. 每次打开都新建 tab

最终决策：

- 采用 `5A`

最终规则：

- 默认一个 SSH 资产对应一个活跃 session
- 再次打开同一资产时优先激活已有 tab
- context action `Open in New Tab` 才强制新建第二个 session
- tab 标题显示资产名
- subtitle 显示 `user@host:port`
- close tab = close UI tab + close session
- `Disconnected / Error` tab 保留，用于后续 reconnect
- 关闭当前 tab 后，焦点按“右侧优先，其次左侧，否则 welcome”回退
- SSH 资产交互语义收敛为：
  - 单击：只选中
  - 双击 / Enter / 明确 open action：打开连接

### 设计要点 6：SSH 资产编辑与最小数据契约

可选方案：

- A. 扩展最小 SSH asset profile contract，支撑 edit / reconnect / secret 引用
- B. 只允许编辑非敏感字段，每次重新输入 secret

最终决策：

- 采用 `6A`

最终规则：

- 复用 `AssetsSshConnectionModal`，增加 `create/edit` mode
- edit 时回填非敏感字段
- secret 不写入资产目录
- secret 存 `keyring`
- 资产只保存稳定引用与非敏感 metadata

最小建议扩展字段：

- `auth_method`
- `private_key_source`
- `private_key_path`
- `remark`
- `credential_ref`

密码输入交互：

- 默认脱敏显示
- 末端显示“斜线眼睛”
- 点击后显示真实值，并切换为“无斜线眼睛”

### 设计要点 7：Windows 内存占用 200MB+ 的判断与优化边界

调研结论：

- 当前默认 renderer 是 `femtovg-wgpu + DX12`
- 这一路径天然会带来较高常驻内存，尤其在 Windows 上会包含：
  - swapchain/front-back buffers
  - GPU driver / allocator 常驻
  - 纹理与字体 atlas
  - WGPU device/queue/surface 初始化成本
- Slint 1.15.1 的 WGPU 28 默认已经使用 `MemoryHints::MemoryUsage`，不是最激进的 `Performance`
- 因此 200MB+ 不像是单一泄漏，更像是“GPU backend 基线偏高 + 当前进程还存在自有额外浪费”

当前明确可优化项：

- 进程内当前存在两套 Tokio runtime
  - `src/main.rs` 创建了一套 `AppAsyncRuntime`
  - `bootstrap::run_with_profile()` 接收了 handle，但当前未使用
  - `build_session_bridge()` 又新建了一套 `AppAsyncRuntime`
- 这会额外创建一组 worker threads 与线程栈

内存优化优先级建议：

- P0：合并为单一 Tokio runtime
  - 这是确定的自有浪费
  - 风险低，收益明确
- P1：在 SSH 真连接正式落地前，限制 runtime worker thread 数量
  - 当前后台任务量很小，不需要按 CPU 核数全开
- P2：保留 `WGPUSettings.device_memory_hints = MemoryUsage`
  - 当前已经是这样，不应回退到更激进配置
- P3：如后续仍超预期，再评估是否通过 `WGPUConfiguration::Automatic` 明确设置更保守的 memory hint / backend 参数
- P4：若产品允许牺牲 GPU 视觉路径，可单独评估 Skia/software experimental build 作为低内存路线
  - 但这不应默认回流 mainline

结论：

- “200MB+ 是否完全异常”：不是完全异常
- “能否优化”：可以
- “最先该动哪里”：不是 SSH 壳层，而是 runtime duplication 与线程策略，其次才是 renderer strategy

## 实施步骤

1. 抽出独立 `WorkspacePane`，修正 workspace 全宽布局契约
2. 建立统一 modal contract，移除 click-away dismiss
3. 修正 `ActiveTab` 命中模型，确保 close affordance 不被整块 touch area 覆盖
4. 建立 SSH modal action state machine
5. 将 `Connect/Test/Save/SaveAndConnect` 收敛为真实语义
6. 引入真实 `SessionManager -> SshSessionRuntime -> terminal adapter` 流水线
7. 把 runtime event 持续同步回 UI，而不是只在 open/close 时同步一次
8. 扩展 SSH 资产最小 profile contract，支撑 edit/reconnect
9. 实现 SSH edit mode 与 secret reveal 交互
10. 合并重复 Tokio runtime，并评估 worker thread 策略

## 风险与回滚策略

主要风险：

- workspace/layout 改造可能影响 right panel 和 sidebar 几何
- modal contract 改造会改变现有 overlay 层级与焦点转移
- 真实 SSH runtime 接入后，host key / auth / resize / disconnect 路径会暴露更多边界问题
- 扩展 SSH asset payload 会影响现有 catalog schema 与映射

回滚策略：

- `WorkspacePane` 与 modal contract 分步提交，确保 layout 回归可单独回滚
- SSH runtime 在 feature slice 上拆分：
  - transport/channel
  - terminal adapter
  - UI bridge
- SSH asset schema 扩展与 runtime 接入解耦，避免一次性绑定
- renderer strategy 不在本轮主线内切换，避免将内存问题与 SSH 架构问题混为一谈

## 验证清单

- [ ] 启动后 workspace 不再出现白色竖条
- [ ] tab 数量变化不会改变 workspace 宿主宽度
- [ ] 所有 asset/SSH modal 默认阻断，click-away 不再关闭
- [ ] modal 支持标题栏热区拖动，且拖动的是 modal 本身
- [ ] `Esc` 关闭规则符合已确认语义
- [ ] `Test Connection` 不再伪成功，真实反映握手/认证结果
- [ ] `Connect` 能在未保存 draft 下发起临时会话
- [ ] `Save` 只保存，不打开 tab
- [ ] `Save and Connect` 先保存后连接
- [ ] tab close 在用户视角下真实可用
- [ ] 同一 SSH 资产重复打开时默认激活已有 tab
- [ ] `Open in New Tab` 可显式强制新建 session
- [ ] SSH edit mode 能回填已有 profile
- [ ] password secret 采用 keyring 引用，不落资产目录
- [ ] 进程内只保留一套 Tokio runtime
- [ ] 在相同窗口尺寸下，常驻内存相比当前基线可观察下降

## 参考

- Slint `TouchArea` 文档
- Slint frameless drag 讨论
- `russh::client::Session`
- `russh::Channel`
- `termwiz` 文档
- `wgpu` DX12 memory hints 与 Slint WGPU 28 配置默认值

## Implementation Status

日期: 2026-03-24
状态: 计划内 Task 1 到 Task 10 已完成；自动化回归通过

实现结果：

- `WorkspacePane`、tab strip、terminal host 的宽高契约已落地
- blocking modal contract、drag callback、focus restore hook 已接线
- `Save / Connect / TestConnection / SaveAndConnect` 已按设计分流
- `SessionManager -> SshSessionRuntime -> TerminalSurfaceState -> ShellViewModel -> Slint` 链路已闭环
- `known_hosts` TOFU、真实 SSH transport、PTY shell、surface snapshot 已落地
- 默认同资产复用 session，`Open in New Tab` 才强制新建第二个 session

文档产物：

- 验证记录：`docs/plans/2026-03-23-ssh-shell-modal-runtime-tabs-verification.md`
- TDD 交接：`docs/plans/2026-03-24-ssh-shell-modal-runtime-tabs-tdd-spec.md`

说明：

- 自动化验证已完成，包含聚焦测试集、3 个 UI contract smoke、全量 `cargo test`
- Win11 真机视觉/交互与内存基线仍需人工补充确认，详见 verification 文档
