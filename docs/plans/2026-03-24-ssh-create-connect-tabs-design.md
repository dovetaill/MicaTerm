# SSH 新建 / 连接 / 标签页 Design

日期: 2026-03-24
执行者: Codex
状态: 方案已确认，等待按需进入 implementation plan

## 背景

本轮任务聚焦三个直接耦合的主题：

- `SSH 新建 / 编辑表单`
- `可实际建立 SSH 连接`
- `连接标签页模型`

用户反馈的问题并不是单点样式瑕疵，而是当前工作区在最近几轮迭代后，同时出现了以下失真：

- `New Folder` / `New SSH Connection` modal 的标题栏、关闭按钮、拖拽区和内容几何关系失稳；
- `New SSH Connection` 的布局被撑坏，标题位置、输入区宽度、底部动作区与滚动区关系异常；
- `SSH tab` 的视觉与关闭行为不符合预期；
- 已有 SSH/runtime/tabs 基线与当前工作区表现之间存在明显偏差，说明近期改动打破了既有契约。

本设计文档只固化本轮确认的架构和交互决策，不直接进入实现。

## 目标

### 本轮目标

- 收敛 modal 的 `chrome ownership`，恢复稳定的标题栏、关闭按钮、拖拽区和内容框架；
- 将 `SSH 新建 / 编辑` 收敛为单页分组表单，避免继续维护一排半成品 tabs；
- 明确密码、私钥、passphrase 与系统凭据存储的语义；
- 保留 `Save / Connect / Test Connection / Save and Connect` 四个动作，并明确其行为边界；
- 明确 `tab / session` 的创建、复用、关闭、失败态与标题规则；
- 将 SSH runtime 的首轮目标提升到“真实可交互 session contract”，不再把“仅能连上并显示 screen_text”视作完成。

### 体验目标

- 交互必须保持 flat / no-radius 方向，与当前项目一致；
- modal 的拖动与关闭行为在所有资产相关弹框上保持一致；
- `SSH` 表单必须让用户一眼读懂每个字段的用途，不依赖 placeholder 传达语义；
- tab 行为要更接近 VS Code / Windows Terminal 的日常使用预期；
- 错误状态应在当前操作上下文内可理解，不制造假闭环。

## 非目标 / 边界

本轮不包含以下内容：

- `russh-sftp` UI、文件传输、远程目录树；
- proxy、tunnel、environment、advanced 的完整业务接通；
- 完整资产持久化重构；
- 多窗口、多 workspace、多 pane；
- 移动端适配；
- renderer 细节的全面重写文档化。

但需要特别说明：

- 本轮虽然不要求把 renderer 完整做完，但目标已经提升为“可交互 session contract”；
- 因此不再接受“只有 runtime probe / connect、UI 只显示 `screen_text` 占位”的停留状态。

## 当前实现现状

## 1. 当前源码与 2026-03-23 旧设计文档存在偏差

2026-03-23 的两份设计文档中，有一部分判断已经被当前源码和 Git 历史证伪。以当前源码为准，现状如下：

- `Cargo.toml` 已包含 `keyring`、`russh`、`termwiz`、`tokio`、`wezterm-term`：
  - [Cargo.toml](/home/wwwroot/mica-term/Cargo.toml#L13)
- `ConnectionProfile`、`SshSessionRuntime`、`SessionManager` 已存在：
  - [profile.rs](/home/wwwroot/mica-term/src/app/ssh/profile.rs#L9)
  - [runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs#L31)
  - [session_manager.rs](/home/wwwroot/mica-term/src/app/ssh/session_manager.rs#L15)
- `TabBar` 与 `ActiveTab` 已经是 real model + close callback，而不是 placeholder：
  - [tabbar.slint](/home/wwwroot/mica-term/ui/shell/tabbar.slint#L14)
  - [active-tab.slint](/home/wwwroot/mica-term/ui/components/active-tab.slint#L5)
- `SSH modal` 已有字段 label、认证切换、四个动作按钮：
  - [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L80)
  - [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L382)

更准确的描述不是“完全没有 SSH/runtime/tabs”，而是：

- 已有一套最小可运行 SSH/runtime/tabs 基线；
- 当前工作区在 modal 与部分 UI 结构上继续演化后，把既有契约打坏了；
- terminal host 仍停留在占位宿主，尚未形成真实交互式 terminal surface。

## 2. modal 壳层正在发生 ownership 迁移，且这是当前回归的核心来源

`HEAD` 版本的 `BlockingModalShell` 自己拥有：

- 标题文本
- close button
- drag 区
- modal frame

而当前工作区版本已经把这些从 shell 中移除，改为每个 modal 自己绘制 header：

- 当前 shell 只剩定位、裁剪和 frame：
  - [blocking-modal-shell.slint](/home/wwwroot/mica-term/ui/components/blocking-modal-shell.slint#L3)
- `AssetsFolderCreateModal` 自己画 header / close / drag：
  - [assets-folder-create-modal.slint](/home/wwwroot/mica-term/ui/components/assets-folder-create-modal.slint#L39)
- `AssetsSshConnectionModal` 自己画 header / close / drag：
  - [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L189)

这条重构线与当前截图中的问题高度吻合：

- 标题跑位；
- 关闭按钮消失或不在预期位置；
- modal body 与 footer 的几何关系失稳；
- 输入区和滚动区越界；
- drag 区和 frame 命中关系变得脆弱。

## 3. `SSH 新建 / 编辑` 已有最小功能，但信息架构不适合继续扩

当前 `AssetsSshConnectionModal` 已包含：

- `name / host / user / port`
- `auth_method / private_key_source`
- `password / private_key_content / private_key_path / passphrase`
- `remark / environment / proxy_method`
- `Save / Connect / Test Connection / Save and Connect`

对应代码：

- [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L189)
- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L83)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2042)

但当前 UI 结构仍沿用多 tab 形态：

- `Standard`
- `Proxy`
- `Environment`
- `Advanced`

同时 `ShellViewModel` 的 enum 里仍保留 `Tunnel`：

- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L51)

这意味着当前信息架构已经出现漂移：

- state 层和 UI 层对 tab 集不完全一致；
- 首轮连接所需字段已经全部集中在 `Standard`；
- 继续保留一排“部分有效 / 部分未接通”的 tab，只会扩大维护面。

## 4. `Save / Connect / Test / Save and Connect` 已有 wiring，但语义仍需正式固化

当前动作已经绑定到 `bootstrap`：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2042)

并且已有以下最小语义：

- `Save` 保存资产与 secret
- `Test Connection` probe
- `Connect` 走临时 asset id 打开会话
- `Save and Connect` 先保存再开会话

相关代码：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2055)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2111)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2140)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L556)

这说明动作模型不是从零开始设计，而是需要收敛和正式定稿。

## 5. `tab / session` 已有最小模型，但 terminal host 仍是过渡态

当前存在：

- `WorkspaceTab`
- `SessionManager`
- `OpenSessionMode::ActivateExisting / ForceNewTab`
- tab 关闭、复用、error tab 保留

对应代码：

- [tabs.rs](/home/wwwroot/mica-term/src/shell/tabs.rs#L5)
- [session_manager.rs](/home/wwwroot/mica-term/src/app/ssh/session_manager.rs#L15)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L840)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2499)

但当前 `TerminalSessionHost` 仍只是：

- welcome / terminal / session-error 三态切换
- 展示 `screen_text`
- 用 `session-surface-seqno` 显示 “terminal-surface-ready”

对应代码：

- [terminal-session-host.slint](/home/wwwroot/mica-term/ui/shell/terminal-session-host.slint#L6)
- [runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs#L516)

所以当前 terminal 链路的真实状态应定义为：

- 真实 SSH transport 已存在；
- `wezterm-term` 维护 terminal state 已存在；
- 但 UI 仍没有形成真实输入、resize、renderer surface 的闭环。

## 6. 当前测试能通过，但没有覆盖用户截图暴露的几何问题

本次调研中，以下测试通过：

- `cargo test --test assets_modal_smoke --test workspace_tabs_spec --test ssh_session_manager_spec`
- `bash tests/assets_modal_ui_contract_smoke.sh`
- `bash tests/ssh_connect_tabs_ui_contract_smoke.sh`

说明当前测试主要锁定：

- 结构契约
- model 契约
- 最小 SSH/session 行为

但并没有覆盖：

- modal header ownership
- overlay 层级
- content/footer 几何约束
- close hit target 是否被覆盖
- title / drag / scroll 区之间的真实视觉和命中关系

这也是为什么同类问题会反复回归。

## 设计要点拆分

## 设计要点 1：modal 的 `chrome ownership`

### 方案 A：由 `BlockingModalShell` 统一拥有 chrome

内容：

- `BlockingModalShell` 统一负责 title / close / drag / frame / scrim；
- 子 modal 只负责 body、footer、局部 secondary navigation；
- `New Folder`、`Edit SSH`、`Delete Asset`、`SSH host key confirm` 共享同一套外层几何契约。

优点：

- 实现复杂度最低；
- 与当前壳层结构最契合；
- 所有 modal 的交互一致性最高；
- 后续维护成本最低；
- 能直接切断当前截图中由 ownership 漂移引发的一串回归。

缺点：

- 子 modal 内部会失去一部分自主排版自由；
- 如果某个 modal 未来确实需要复杂自定义 header，需要额外扩展 shell slot。

潜在风险：

- shell 组件会重新成为 modal 体系的单一关键点；
- 需要把近期工作区里“header 下沉”的改动整体回收到更稳定的边界上。

### 方案 B：由每个 modal 自己拥有完整 chrome

内容：

- shell 只负责定位、裁剪和遮罩；
- 每个 modal 自己处理 title、close、drag、header-divider、focus 语义。

优点：

- modal 自主性更高；
- 特例样式可以在单组件内部完成。

缺点：

- 实现复杂度更高；
- 与当前项目的共享壳层方向不一致；
- 交互一致性最容易飘；
- 每次改一个 modal 都可能重新打坏 title / close / drag 的契约。

潜在风险：

- 当前截图问题基本就是此方案在半收敛状态下的风险落地；
- 后续所有资产 modal 都会持续承受重复回归成本。

### 最终决策

选择方案 A。

### 决策说明

- 本轮不再继续“shell 只定位、子 modal 自己画整套 header”的方向；
- `BlockingModalShell` 必须重新成为统一 chrome owner；
- 子 modal 可以保留局部二级 tabs 或内容分组，但不再自行拥有顶层 window-like header。

## 设计要点 2：`SSH 新建 / 编辑表单` 的信息架构

### 方案 A：保留多 tab，`Standard` 做完整，其余保持可见

优点：

- 变更范围小；
- 与当前 UI 结构最贴近。

缺点：

- 当前 `SSH` 首轮必需字段已经完全集中在 `Standard`；
- 继续保留多 tab 会把“未接通能力”持续暴露给用户；
- state 与 UI 的 tab 集已经有漂移，继续沿用会扩大维护面。

### 方案 B：收敛为单页分组表单

内容：

- 移除当前顶层 `Standard / Proxy / Environment / Advanced` tab 结构；
- 收敛为单页滚动表单；
- 采用明确分组：
  - `Connection`
  - `Authentication`
  - `Metadata`
  - `More Settings` 或同级的可折叠非首轮字段区

优点：

- 与当前任务目标最匹配；
- 信息路径最短；
- 不再需要维护一排半成品 tabs；
- 更适合桌面 modal 的有限空间；
- 对 `New SSH` 与 `Edit SSH` 都更稳定。

缺点：

- 需要一次性重排现有 modal 内容结构；
- 某些后续字段需要通过折叠区而不是 tab 继续扩展。

潜在风险：

- 与当前 UI 形态相比是明显的 IA 调整；
- 若未来高级项暴涨，可能还需要再次拆层。

### 最终决策

选择方案 B。

### 决策说明

- 本轮不再保留“顶层多 tab modal”；
- `SSH 新建 / 编辑` 统一采用单页分组表单；
- 非首轮能力不再通过并列 tab 预占位，而通过折叠区、说明区或明确的后续扩展位承载；
- `Tunnel` 不进入本轮 IA。

### 单页分组建议

#### Connection

- `Name`
- `Host`
- `User`
- `Port`

#### Authentication

- `Authentication Type`
- `Password`
- `Private Key Source`
- `Private Key Content`
- `Private Key Path`
- `Passphrase`

#### Metadata

- `Remark`

#### More Settings

- `Environment`
- `Proxy Method`

这里的 `More Settings` 本轮只保留数据入口，不承诺完整联通。

## 设计要点 3：密码 / 认证信息的输入与保存语义

### 方案 A：`Save` 默认保存 secret 到系统凭据存储

内容：

- `Save` 与 `Save and Connect` 默认保存可持久化 secret；
- `Connect` 只建立临时会话，不保存资产、不落 secret；
- 编辑已保存资产时，secret 字段留空表示“保留已有 secret”；
- 只有显式清除动作才删除已保存 secret。

优点：

- 与当前代码最契合；
- 实现复杂度最低；
- 行为一致，便于维护；
- 不会把本轮扩大成完整 secret 管理设计。

缺点：

- 对部分用户来说，“默认保存”不够显式；
- 需要 UI 文案明确解释留空与覆盖语义。

潜在风险：

- 如果文案不清楚，用户容易误判编辑时的 secret 更新结果。

### 方案 B：显式增加 “Remember secret” 与 “Keep / Replace / Clear”

优点：

- 语义最完整；
- 用户对 secret 生命周期的控制最清晰。

缺点：

- 本轮 modal 会明显变复杂；
- 需要更多状态分支和校验；
- 已经超出当前“只分析与 SSH 新建、连接、标签页直接相关边界”的节奏。

### 最终决策

选择方案 A。

### 决策说明

- 本轮沿用“保存资产即保存 secret”的默认语义；
- 但必须在 edit 模式显式说明：
  - 留空不代表清空；
  - 留空表示保留当前系统凭据存储中的 secret；
  - 只有显式清除动作才删除 secret。

## 设计要点 4：动作模型与 tab / session 语义

### 方案 A：保留四动作，并允许临时连接

内容：

- `Save`：保存资产与 secret，不打开会话；
- `Test Connection`：仅 probe，不创建 tab；
- `Connect`：创建临时会话 tab，不保存资产；
- `Save and Connect`：先保存资产与 secret，再打开会话；
- 同一已保存 SSH 资产再次打开时默认激活已有 tab；
- 只有显式 `Open in New Tab` 才创建第二个 session；
- 失败连接保留 error tab，供用户查看错误并手动关闭。

优点：

- 与当前已有 wiring 最贴近；
- 兼顾正式资产和临时连接；
- 贴近桌面 SSH 客户端和编辑器式 tab 使用心智。

缺点：

- 动作矩阵较多；
- 需要把失败态、复用态和临时态语义写得足够清楚。

潜在风险：

- 如果按钮层级和文案不收敛，用户会混淆 `Connect` 与 `Save and Connect`。

### 方案 B：save-first，不再提供纯临时连接

优点：

- 模型更简单；
- 会话与资产天然绑定。

缺点：

- 丢失快速临时连接能力；
- 与当前已有 `Connect` wiring 不一致。

### 最终决策

选择方案 A。

### 决策说明

- 本轮保留四动作；
- `Connect` 继续合法存在，且明确是“临时连接，不保存资产”；
- 已保存资产默认复用已有 tab；
- context menu 的 `Open in New Tab` 作为强制新 session 入口继续保留；
- tab 标题优先 `Name`，为空时回退到 host 推导值：
  - [tabs.rs](/home/wwwroot/mica-term/src/shell/tabs.rs#L54)

## 设计要点 5：`wezterm-term + termwiz + russh` 的首轮接入边界

### 方案 A：停留在当前 read-only terminal host

内容：

- 保留已有真实 transport；
- UI 继续只显示 `screen_text`；
- 输入、resize、renderer surface 留到后续。

优点：

- 范围最小；
- 对现有代码改动最少。

缺点：

- “可实际建立 SSH 连接”在用户视角下仍然不成立；
- 仍会出现“连上了，但不像真正终端”的产品落差；
- 会继续制造假闭环。

### 方案 B：本轮提升到真实可交互 session contract

内容：

- 保留当前 `SessionManager -> SshSessionRuntime -> wezterm-term` 分层；
- 补齐 UI 到 runtime 的输入、resize、render-ready surface 契约；
- `TerminalSessionHost` 不再以纯 `screen_text` 占位结束本轮；
- 首轮只做单 pane、单 surface，不扩成完整 renderer 重写。

优点：

- 与项目长期方向最一致；
- 对“真实 SSH 终端客户端”的目标更诚实；
- 一旦 contract 成立，后续 renderer 演进有稳定落点。

缺点：

- 范围和复杂度明显高于仅做 connect/probe；
- 需要把 UI 事件、runtime 写入、surface 投影边界一起收拢。

潜在风险：

- 如果 renderer 接口抽象过度，会引入不必要复杂度；
- 如果只补一半，又会出现新的假闭环。

### 最终决策

选择方案 B。

### 决策说明

- 本轮“可实际建立 SSH 连接”按更高标准定义：
  - 不只是 transport connected；
  - 而是至少形成可交互 session contract。
- 本轮仍不要求完整 renderer 设计全部定稿，但不能再把 `TerminalSessionHost` 纯文本占位视作完成。

## 方案对比总结

| 设计点 | 方案 A | 方案 B | 最终选择 |
| --- | --- | --- | --- |
| modal chrome ownership | shell 统一拥有 | 各 modal 自己拥有 | `1A` |
| SSH form IA | 保留多 tab | 单页分组表单 | `2B` |
| secret 保存语义 | 默认保存 | 显式 remember / replace / clear | `3A` |
| 动作模型 | 保留四动作与临时连接 | save-first | `4A` |
| runtime 接入边界 | read-only host | 可交互 session contract | `5B` |

## 最终决策

本轮确认的最终方案如下：

1. `BlockingModalShell` 重新成为统一 modal chrome owner；
2. `SSH 新建 / 编辑` 收敛为单页分组表单；
3. `Save` / `Save and Connect` 默认持久化 secret 到系统凭据存储，edit 模式下留空表示保留已有 secret；
4. 保留 `Save / Connect / Test Connection / Save and Connect` 四动作，并保留临时连接；
5. runtime 边界提升到真实可交互 session contract，不再接受只显示 `screen_text` 的占位终态。

## 实施步骤

这里只定义高层实施顺序，不展开 implementation plan。

### Phase 1：收敛 modal ownership

- 先恢复 `BlockingModalShell` 统一拥有 title / close / drag / frame；
- 统一 `New Folder`、`Edit SSH`、`Delete Asset`、`SSH host key confirm` 的 modal 外层契约；
- 为 modal 几何、overlay 层级和点击命中补足测试。

### Phase 2：重做 SSH 单页表单 IA

- 拆除当前顶层多 tab 结构；
- 收敛为单页分组表单；
- 明确 edit 模式下的 secret 保留文案与清除入口；
- 重新整理 footer 动作布局和滚动区边界。

### Phase 3：收敛动作模型与资产 / session 投影

- 用正式状态机收敛 `Save / Connect / Test / Save and Connect`；
- 明确临时连接与已保存资产连接的差异；
- 收紧 tab 复用、失败态、关闭后的回退选择规则。

### Phase 4：补齐可交互 session contract

- 在不扩成完整 renderer 重写的前提下，补齐 terminal input / resize / surface-ready 边界；
- 让 `TerminalSessionHost` 从纯文本占位过渡到真实可交互宿主；
- 保持单 window、多 tab、单 pane 的首轮范围。

## 风险与回滚策略

### 风险

- `BlockingModalShell` 回收 ownership 时，可能与当前工作区里已经下沉到子 modal 的逻辑产生冲突；
- 单页表单重排会触发一轮 UI 自动化和截图基线更新；
- secret 语义如果文案不清楚，edit 模式会持续制造误解；
- 可交互 session contract 若只完成一半，用户仍会感知为假终端；
- 当前工作区是脏的，且与最近 SSH/runtime/tabs 提交重叠，实施时需要先厘清哪些改动保留、哪些回滚。

### 回滚策略

- modal ownership 改造按组件边界分阶段回退，不一次性揉在 SSH/runtime 改造里；
- 若单页表单方案造成过大 UI 风险，可先保留字段分组与 footer 收敛，再继续收掉剩余 tab 壳层；
- 若可交互 session contract 超出本轮承受范围，允许分解为单独 implementation plan，但不能把 read-only `screen_text` 占位重新包装成“真实 terminal”。

## 验证清单

- [ ] `New Folder` modal 恢复稳定 header / close / drag / footer 几何
- [ ] `New SSH / Edit SSH` modal 不再出现标题错位、输入框越界、footer 不可见
- [ ] modal outside click、blocking、focus restore 语义与设计一致
- [ ] `SSH` 表单收敛为单页分组 IA，字段标签、说明和必填标识清晰
- [ ] edit 模式下 secret 留空保留旧值的语义有明确 UI 文案
- [ ] `Save / Connect / Test / Save and Connect` 行为与反馈语义一致
- [ ] 同资产默认复用现有 tab，`Open in New Tab` 才创建第二个 session
- [ ] failed connection 会保留 error tab，且用户可关闭
- [ ] tab close 命中与 select 命中不冲突
- [ ] terminal host 不再停留在纯 `screen_text` 占位终态
- [ ] 未引入圆角回流
- [ ] 未擅自扩散到完整 SFTP / proxy / tunnel / 多 pane
