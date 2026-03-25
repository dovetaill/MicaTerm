# SSH 新建 / 连接 / 标签页 Design

日期: 2026-03-25
执行者: Codex
状态: 方案已确认，未进入业务实现

## 背景

本轮仅处理与以下三件事直接相关的边界：

- `SSH 新建 / 编辑表单`
- `可实际建立并维持 SSH 连接`
- `连接标签页 / 会话模型`

用户当前反馈不是单一 UI 抛光问题，而是三个层面同时失真：

- `SSH modal` 的信息架构和密码编辑体验不符合桌面 SSH 客户端预期；
- 已有 SSH runtime 虽能建立最小连接，但 terminal host 仍在显示占位式顶部文案和纯文本行列表；
- 保存后的 secret 在再次连接或重启后不能稳定复用，错误提示仍落到笼统的 `missing SSH password secret`。

## 目标

### 本轮目标

- 去掉 terminal 顶部与终端内容无关的标题 / 副标题 / 状态占位文案；
- 将 terminal host 从纯文本占位宿主升级为真正消费 `wezterm-term` / `termwiz` surface 的单面板终端视图；
- 将 `SSH 新建 / 编辑` 收敛为单页分组表单；
- 在编辑已保存 SSH 时，密码字段必须可直接查看、可直接修改，而不是依赖 `Clear Saved Secret` 之类的反直觉入口；
- modal 动作收敛为仅 `Test` 与 `Save`；
- `Save` 成功后，再次连接与重启程序后的连接都必须能稳定复用已保存 secret；
- 会话断开后，标签仍可重连，资产仍可编辑，不允许出现“断开后两边都卡死”的异常状态。

### 体验目标

- 保持当前 flat / no-radius 方向；
- 交互气质靠近 Windows 11 Fluent 桌面产品，但不引回圆角；
- SSH terminal 默认观感应接近成熟桌面终端客户端的最低合格线：
  - 有 ANSI 颜色
  - 有可见光标
  - 有合理默认字号与行高
  - 没有多余的“terminal ready”式占位提示

## 非目标 / 边界

本轮不包含：

- `russh-sftp` UI 与文件传输；
- 完整资产持久化体系重构；
- proxy / tunnel / environment 的完整业务接通；
- 多 pane、多窗口、多 workspace；
- 全局 terminal renderer 自定义主题系统的完整扩展。

本轮允许触及但只以 SSH 直连为目标：

- `wezterm-term` / `termwiz` surface 投影；
- `russh` keepalive / reconnect 生命周期；
- 凭据存储读取链路的 correctness 修复与错误诊断。

## 当前实现现状

### 1. 当前源码已不是“未接入 SSH”

源码里已经存在最小可运行链路：

- `ConnectionProfile`
- `SessionManager`
- `SshSessionRuntime`
- `WorkspaceTab`

关键位置：

- [profile.rs](/home/wwwroot/mica-term/src/app/ssh/profile.rs)
- [session_manager.rs](/home/wwwroot/mica-term/src/app/ssh/session_manager.rs)
- [runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs)
- [tabs.rs](/home/wwwroot/mica-term/src/shell/tabs.rs)

因此，本轮不是从零设计 SSH，而是纠正“runtime 已有、终端 UI 仍是假宿主、secret 生命周期不稳、modal IA 漂移”的问题。

### 2. Terminal host 仍然是占位宿主

当前 `ui/shell/terminal-session-host.slint` 仍在做这些事：

- 渲染 `session-title`
- 渲染 `session-subtitle`
- 渲染 `Interactive terminal ready.`
- 用 `ListView + Text` 显示 `session-visible-lines`
- 在错误态显示 `Reconnect is available once session lifecycle wiring is completed.`

关键位置：

- [terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint#L136)
- [terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint#L155)
- [terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint#L179)
- [terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint#L229)

这正对应用户看到的“终端上面那几行不应该有”“没有颜色”“没有光标”“像假的终端”。

### 3. SSH runtime 已接通，但 keepalive 配置明显不对

当前 `src/app/ssh/runtime.rs` 在 client config 上设置了：

- `inactivity_timeout: Some(Duration::from_secs(30))`

关键位置：

- [runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs#L164)

结合 `russh` 官方语义，这更像是连接空闲回收阈值，不是用户期待的保活策略。它与“连接一会就断”的现象高度相关。

### 4. 密码编辑语义仍然错误

当前 SSH modal 仍保留：

- `secret_retention_message`
- `can_clear_saved_secret`
- `clear_saved_secret_requested`
- `Clear Saved Secret`

关键位置：

- [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L208)
- [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L533)
- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L66)

这与用户确认的目标冲突。用户要求的是：

- 编辑时可以直接看到当前密码；
- 可以直接改密码；
- 不要再用 `Clear Saved Secret` 这种绕路方式。

### 5. Modal footer 已被 recent commit 收敛为 `Test` + `Save`

当前 `HEAD` 的 modal footer 只显示：

- `Test`
- `Save`

关键位置：

- [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L749)

但 `bootstrap` 和 `view_model` 里仍残留 `Connect` / `SaveAndConnect` 合约：

- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L107)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2265)

因此当前状态不是“设计已统一”，而是 UI 与动作 contract 漂移。

### 6. Secret 保存链路存在 correctness 风险

当前 secret 持久化依赖：

- `ssh/saved-secrets/{asset_id}` 作为 keyring key
- `AssetSshConnectionSpec.credential_ref`
- `sync_saved_ssh_secrets()`
- `load_secret_bundle()`

关键位置：

- [credentials.rs](/home/wwwroot/mica-term/src/app/ssh/credentials.rs#L18)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L646)
- [profile.rs](/home/wwwroot/mica-term/src/app/ssh/profile.rs#L138)
- [runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs#L300)

当前失败表现是：

- 保存后，当前进程内 `Test` 可能成功；
- 但再次连接或重启后，认证阶段仍可能落到：
  - `missing SSH password secret for '{name}'`

关键位置：

- [runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs#L317)

这说明现在的问题不是“用户不会用”，而是 secret 恢复链路没有形成稳定、可诊断的 contract。

## 设计要点拆分

## 设计要点 1：Terminal 渲染契约

### 方案 A：继续沿用 text-only host，只做样式修补

内容：

- 保留 `visible_lines: Vec<String>` 投影；
- 仅删除顶部文案、调大字体、伪造一个光标；
- 不做 cell-level 颜色与属性渲染。

实现复杂度：

- 低

与当前架构契合度：

- 表面契合，实质绕开了 `wezterm-term` 已有能力

交互一致性：

- 低，仍然不像真实终端

可维护性：

- 低，后续补颜色 / cursor / selection 还得重做

潜在风险：

- 很快再次撞上“像假的终端”同类问题；
- 会把占位 UI 继续误当成 terminal renderer。

### 方案 B：改为真实单面板 terminal surface

内容：

- `TerminalSessionHost` 不再展示 title/subtitle/status 占位信息；
- UI 直接消费 `wezterm-term` / `termwiz` surface 的 cell、attribute、cursor 信息；
- 首轮只做单 pane、单 surface，不扩展 split/pane；
- 默认提供真实 ANSI 颜色、反色选中、可见 cursor、合理字号和行高。

实现复杂度：

- 中高

与当前架构契合度：

- 高，和现有 `SshSessionRuntime -> TerminalSession -> TerminalSurfaceState` 链路一致

交互一致性：

- 高，能和成熟终端产品的核心感知对齐

可维护性：

- 高，后续 selection、IME、链接识别都能在同一 surface contract 上扩展

潜在风险：

- 需要重新定义 Slint 侧 terminal surface 的投影结构；
- 若 cell diff 设计不当，可能带来重绘成本。

### 最终决策

采用方案 B。

补充约束：

- 顶部 `Sharon / root@host / Interactive terminal ready.` 一类非终端内容全部移除；
- 错误与重连提示不再侵入 terminal canvas 顶部，而是归入 tab 状态或专用错误视图；
- 默认 terminal 视觉基线按 Windows 桌面终端使用习惯设定，首轮优先保证“像一个真终端”而不是做装饰性卡片布局。

## 设计要点 2：Session 生命周期、保活与标签页复用

### 方案 A：按资产复用标签，断开后可重连，并改为真实 keepalive 模型

内容：

- 已保存 SSH 资产默认采用 `ActivateExisting` 语义；
- 同一资产默认复用同一 tab；
- 手动断开或远端断开后，tab 进入 `Disconnected` / `Reconnectable` 状态；
- reconnect 成功后复用原 tab，不新建重复 tab；
- 用 `keepalive_interval` / `keepalive_max` 取代 30 秒 `inactivity_timeout` 方案；
- 编辑资产不会被断线状态锁死。

实现复杂度：

- 中

与当前架构契合度：

- 高，和 `SessionManager::OpenSessionMode::ActivateExisting` 一致

交互一致性：

- 高，更接近 VS Code / Windows Terminal / Termius 的资产会话预期

可维护性：

- 高，asset_id -> active session 的关系清晰

潜在风险：

- reconnect 时要小心旧 runtime control、surface seqno、tab 激活状态的清理顺序；
- 若错误态与断开态不区分，UI 仍会混乱。

### 方案 B：每次连接都新建 tab，不做资产级复用

内容：

- 连接动作始终新建 tab；
- 断线后用户重新开一个新 tab；
- 老 tab 仅作为历史记录留存或直接关闭。

实现复杂度：

- 低

与当前架构契合度：

- 中，和现有 `ActivateExisting` 方向相反

交互一致性：

- 中低，和资产型 SSH 管理体验不一致

可维护性：

- 中，短期简单，长期 tab 污染严重

潜在风险：

- 用户很容易堆出多个相同资产 tab；
- “再次连接”与“重新连接”语义混淆。

### 最终决策

采用方案 A。

补充约束：

- “连接一会就断”被定义为 bug，不再接受 30 秒 inactivity 回收；
- 断线后必须同时满足两件事：
  - 资产仍可编辑；
  - 原 tab 仍可重连；
- 关闭 tab 只关闭 session，不删除资产；
- 资产发起连接时，若已有同资产断线 tab，优先激活并触发 reconnect。

## 设计要点 3：SSH 表单信息架构与密码编辑语义

### 方案 A：保留多 tab 表单和 secret retention 语义

内容：

- 继续使用 `Standard / Proxy / Environment / Advanced` tabs；
- 编辑时密码保持空白；
- 用 `Leave blank to keep saved secret` 和 `Clear Saved Secret` 控制 secret。

实现复杂度：

- 低

与当前架构契合度：

- 中，只是延续现状

交互一致性：

- 低，不符合桌面 SSH 客户端的编辑预期

可维护性：

- 低，多个 tab 中只有一个 tab 真正决定首轮连接成功率

潜在风险：

- 用户继续不知道字段重点；
- 密码编辑继续反直觉。

### 方案 B：单页分组表单，但已保存密码只显示“已保存”占位

内容：

- 去掉多 tab，改成单页分组；
- 编辑时密码默认不回填实际 secret，只显示 masked placeholder；
- 支持 `Show` 只是切换占位态，不直接读取真实密码。

实现复杂度：

- 中

与当前架构契合度：

- 高

交互一致性：

- 中，较现状好，但仍不符合用户明确要求

可维护性：

- 中高

潜在风险：

- 用户依然无法确认当前保存的密码到底是什么；
- “修改密码”仍然要靠覆盖输入，心智不稳。

### 方案 C：单页分组表单，编辑时直接加载真实 secret，默认脱敏，可 Show / Hide / 修改

内容：

- 去掉 `Standard / Proxy / Environment / Advanced` 顶部 tabs，改为单页分组：
  - `Basic`
  - `Authentication`
  - `Notes`
- `Proxy`、`Environment` 本轮保留为非主路径字段，不再占据一级 tab；
- 打开编辑 modal 时，若存在 saved secret，立即从 credential store 读取并注入 draft；
- 密码字段默认 masked；
- `Show` 直接显示真实密码；
- 用户可直接在同一输入框修改密码；
- `Save` 时覆盖保存为新的 secret；
- 去掉 `Clear Saved Secret`；
- 若凭据读取失败，不显示假装“已保存”的占位，而是显示明确的 inline 错误信息。

实现复杂度：

- 中高

与当前架构契合度：

- 高，和现有 draft / credential store / edit modal 结构兼容

交互一致性：

- 高，符合用户明确要求，也更接近成熟桌面 SSH 客户端

可维护性：

- 高，secret 的显示、编辑、覆盖写入都走同一条语义链

潜在风险：

- 需要处理“编辑 modal 打开时读取 keyring 失败”的状态分支；
- 需要保证 draft 生命周期内不会把 secret 意外泄露到错误日志或无关状态复制。

### 最终决策

采用方案 C。

补充约束：

- 用户进入编辑态时看到的是“当前真实密码的脱敏态”，不是空白保留语义；
- `Show` / `Hide` 只影响显示，不影响 secret 是否存在；
- `Save` 永远以当前字段内容为准；
- 不再暴露 `Clear Saved Secret`；
- 如果将来需要删除 secret，只能放到明显的次级危险操作里，不占主编辑流。

## 设计要点 4：Modal 动作矩阵

### 方案 A：modal 仅保留 `Test` 与 `Save`

内容：

- `Test`：测试当前 draft 是否能建立认证与 shell 前置链路；
- `Save`：保存资产与 secret，但不自动打开 tab；
- 真正的连接入口放在资产列表：
  - 双击资产
  - 回车
  - context menu `Connect`

实现复杂度：

- 低

与当前架构契合度：

- 中高，当前 UI 已被 recent commit 收敛到这个方向

交互一致性：

- 高，适合“资产先建好，再连接”的资源管理流

可维护性：

- 高，modal 只承担编辑 / 校验，不承担会话跳转分支

潜在风险：

- 若资产列表入口不够顺手，会让用户觉得少一步；
- 需要把连接入口在资产列表里做得更明确。

### 方案 B：恢复 `Connect / Save / Test / Save and Connect`

内容：

- modal 底部恢复四动作；
- 创建和连接一步完成。

实现复杂度：

- 中

与当前架构契合度：

- 中，逻辑残留还在，但 UI 已经不再这么呈现

交互一致性：

- 中，适合试验型流程，但会重新放大 modal 的职责

可维护性：

- 中低，状态分叉更多

潜在风险：

- 保存失败、测试失败、连接失败三种路径更容易相互污染；
- 与本轮“只修 SSH 新建 / 编辑和真实连接边界”不够收敛。

### 最终决策

采用方案 A。

补充约束：

- modal footer 只保留 `Test` 与 `Save`；
- `Save` 成功后应明确告诉用户资产已可连接；
- 资产列表必须承担“真正连接入口”的职责；
- `Test` 成功不自动生成 tab。

## 设计要点 5：Secret 持久化正确性与错误诊断

### 方案 A：维持当前“读不到就报 missing secret”的弱契约

内容：

- 继续把所有读取失败都归并成 `missing SSH password secret`；
- 不区分是：
  - `credential_ref` 未持久化
  - keyring 无条目
  - keyring 读取报错
  - bundle 为空

实现复杂度：

- 低

与当前架构契合度：

- 表面契合

交互一致性：

- 低，用户无法知道系统到底坏在哪里

可维护性：

- 低，后续排查只能靠猜

潜在风险：

- “保存成功但重启后失效”会长期反复出现且难以诊断。

### 方案 B：将 secret 恢复定义为强契约，并提供分层错误诊断

内容：

- `Save` 成功的定义中必须包含：
  - 资产 spec 已持久化
  - `credential_ref` 已稳定写入
  - 对应 secret 已成功写入 system credential store
- 后续 `Connect` 必须先按 `credential_ref` 回读 secret；
- 错误要分层：
  - saved asset 缺少 `credential_ref`
  - system credential store 中无对应条目
  - system credential store 读取失败
  - 已读到 bundle 但 password 字段为空
- UI 文案不能再只给出 `missing SSH password secret`，必须带出具体失败层级；
- 编辑 modal 打开时也走同一读取链路，避免“连接失败”和“编辑看不到密码”成为两套状态机。

实现复杂度：

- 中

与当前架构契合度：

- 高，直接强化现有 `CredentialStore / credential_ref / load_secret_bundle` 合约

交互一致性：

- 高，保存、编辑、连接、重启后重连都遵循同一 secret contract

可维护性：

- 高，问题定位会直接落到具体边界

潜在风险：

- 需要补齐 keyring 失败分支的 UI 表达；
- 需要保证日志里不泄露 secret 内容。

### 最终决策

采用方案 B。

补充约束：

- 这是 correctness bug 修复目标，不是“可选增强项”；
- “保存后当前进程内能用，但重启后不能用”被视为未完成；
- 若系统 keyring 不可用，必须给出明确错误，不允许伪装成用户没填密码；
- 首轮实现仍坚持 `OS keyring only`，不引入本地加密 fallback store。

## Git 历史结论

最近相关提交显示当前工作区确实发生了方向漂移：

- `84bb178 fix: reuse cached ssh secrets after save`
- `41de381 fix(ui): unify modal footers with integrated layout`
- `d8c00cb refactor(ui): simplify ssh modal footer actions`
- `80afb79 fix(ui): restore modal shell geometry and visibility`
- `7927735 fix: reuse saved ssh secrets in edit modal`
- `75fcb17 feat: finish ssh create connect tabs flow`
- `0c6ea98 feat: complete ssh shell modal runtime tabs flow`

结论：

- modal footer 已经向 `Test + Save` 收敛；
- 但 secret 编辑语义、terminal surface 契约、重启后 secret correctness 仍未收口；
- 本文档用于覆盖这些仍未定稿的边界，并作为 2026-03-24 方案在本轮范围内的更新版。

## 最终决策

本轮最终确认如下：

- `1B`：terminal 改为真实 single-surface renderer，移除顶部占位文案
- `2A`：按资产复用 tab，支持 reconnect，替换错误的 30 秒 inactivity 方案
- `3C`：SSH 编辑时直接加载真实 secret，默认脱敏，可 Show / Hide / 修改
- `4A`：modal 只保留 `Test` 与 `Save`
- `5B`：把“保存后重连 / 重启后重连必须可用”写成强 correctness contract，并提供分层错误诊断

## 实施步骤

以下为设计落地顺序，不等同于实现 patch：

1. 收敛 SSH modal 的单页信息架构与字段分组，删除多 tab 导航与 `Clear Saved Secret` 语义。
2. 定义“编辑态 secret hydration” contract：modal 打开时读取 secret，失败时给出明确 inline 诊断。
3. 重定义 terminal host 投影协议，使 Slint 端消费 cell / attribute / cursor，而不是 `visible_lines: Vec<String>`。
4. 调整 session lifecycle：断线态、错误态、reconnectable 态、tab 复用与 active tab 激活顺序。
5. 替换 `inactivity_timeout` 策略，引入真正的 keepalive 配置。
6. 强化 secret persistence correctness：保存、编辑、连接、重启后的读取链路统一走 `credential_ref + CredentialStore` 合约。
7. 补齐错误文案与验证用例，确保不再出现空洞的 `missing SSH password secret` 主导 UX。

## 风险与回滚策略

### 风险

- terminal surface 改造会触及 `TerminalSurfaceState` 的结构，若 diff 粒度设计错误，可能引发重绘压力；
- 编辑 modal 读取真实 secret 后，draft 生命周期必须避免日志泄露与无关状态复制；
- reconnect 复用旧 tab 时，如果 runtime control 与 UI projection 清理顺序错误，容易出现“旧画面残留 / 新 runtime 不接管”的问题；
- system keyring 在不同 Windows 环境上的行为差异，可能暴露此前未覆盖的异常路径。

### 回滚策略

- terminal surface 改造按“新投影结构 + 旧 host 删除前并行验证”的方式推进，必要时可先回退到旧 host，但不保留顶部占位文案；
- secret correctness 修复若发现 keyring 读写兼容性问题，可保留现有存储后端，但不回退到模糊错误文案；
- modal IA 若需要分阶段迁移，可先完成单页表单与密码编辑语义，再移除残余旧 enum / dead action contract。

## 验证清单

- 保存 password 型 SSH 后，不关闭程序直接连接，必须成功。
- 保存 password 型 SSH 后，关闭程序再打开，连接必须成功。
- 编辑已保存 SSH 时，密码字段默认脱敏显示真实值，点击 `Show` 后可看到真实密码。
- 编辑已保存 SSH 时，直接修改密码后 `Save`，再次连接必须使用新密码。
- terminal 连接成功后，不再显示 `Interactive terminal ready.`、`root@host` 这类非终端内容作为顶部装饰。
- terminal 至少具备 ANSI 颜色、可见 cursor、合理字号与行高。
- 空闲超过 30 秒后，会话不能因为错误的 inactivity 配置自动断开。
- 断线后原 tab 仍可重连，且资产仍可编辑。
- 若 secret 读取失败，UI 必须给出明确失败层级，不得仅显示 `missing SSH password secret`。

