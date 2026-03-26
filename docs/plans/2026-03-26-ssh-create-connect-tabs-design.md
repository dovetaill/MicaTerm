# SSH 新建 / 连接 / 标签页 Design

日期: 2026-03-26
执行者: Codex
状态: 方案已确认，待按需进入 implementation plan

## 背景

本轮只处理与以下问题直接相关的边界：

- `SSH 新建表单完善`
- `可实际建立 SSH 连接`
- `连接标签页模型`
- `工作区壳层与标签交互 bug`

用户在当前基线上明确指出了三类问题：

- 默认打开主界面时，右侧工作区顶部会先空出一条 `TabBar`，左侧 `Assets` 区又是另一套独立 header，视觉上像两块硬拼出来的壳层；
- 标签关闭按钮 hover 时持续闪烁，且当前标签无法稳定关闭；
- 同一 SSH 资产当前只能维持一个会话，再次双击或右键打开时不会新建第二个连接标签页。

同时，用户要求本轮继续保持当前项目方向：

- 仍以 Rust + Slint + Tokio 为主线；
- 维持当前无圆角方向；
- 不擅自扩展到完整持久化、SFTP UI、proxy/tunnel/business policy 改造；
- 在确认前只做调研与设计，确认后本轮仅产出设计文档，不直接进入实现。

## 目标

### 本轮目标

- 让 `WorkspacePane` 成为统一工作区壳层，不再在空会话态保留一条无意义的顶部标签条；
- 修复标签关闭按钮 hover 闪烁与关闭失败问题；
- 将同一 SSH 资产的打开语义从“默认复用旧会话”改为“每次 `Open` 都新建一个连接标签页”；
- 收敛资产右键菜单中的会话动作，去掉重复或歧义入口；
- 保持当前 `SSH modal` 的总体形态与现有动作集不变，但删除本轮无实际价值的 `Connect Options`、`Proxy Method`、`Session Environment`。

### 体验目标

- 工作区视觉应更接近成熟编辑器 / 终端客户端的“一整块内容区”；
- `TabBar` 与 terminal host 在有会话时属于同一块 surface，而不是上下两张卡片；
- 空工作区应干净，没有多余的 placeholder chrome；
- 同一资产连续打开多个会话时，语义要直观，避免“有时复用、有时新建”的混合规则；
- 标签关闭应稳定、直接，不再依赖 hover 改变命中区几何。

## 非目标 / 边界

本轮不覆盖以下内容：

- `russh-sftp` 的文件传输 UI 与 workflow；
- 完整的 proxy / tunnel / environment 真实接线；
- 多 pane、多窗口、多 workspace；
- 资产目录持久化体系重构；
- 全量 terminal renderer 视觉主题系统扩展；
- 新的 implementation plan 文档，除非后续单独要求。

本轮允许触及但只以 SSH shell 场景为目标：

- `WorkspacePane` / `TabBar` / `ActiveTab` 的壳层职责；
- `SessionManager` 的打开语义；
- 资产右键菜单的 SSH 打开动作；
- `SSH modal` 中与当前 runtime 无关的字段裁剪。

## 当前实现现状

### 1. 当前仓库已经真实接入 SSH runtime，而不是“尚未接入”

当前源码已包含并使用：

- `russh`
- `termwiz`
- `wezterm-term`

关键位置：

- [Cargo.toml](/home/wwwroot/mica-term/Cargo.toml#L19)
- [runtime.rs](/home/wwwroot/mica-term/src/app/ssh/runtime.rs#L122)
- [session_manager.rs](/home/wwwroot/mica-term/src/app/ssh/session_manager.rs#L79)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L979)

这与更早的设计上下文存在偏差。当前问题不是“还没接入 SSH”，而是“接入后 UI 语义和近期重构不一致”。

### 2. 当前空工作区会固定渲染一条独立 `TabBar`

`WorkspacePane` 当前无论是否有 session，都固定渲染：

- `TabBar`
- `TerminalSessionHost`

关键位置：

- [workspace-pane.slint](/home/wwwroot/mica-term/ui/shell/workspace-pane.slint#L28)
- [workspace-pane.slint](/home/wwwroot/mica-term/ui/shell/workspace-pane.slint#L33)

而 `TabBar` 自己是独立 36px 顶条：

- [tabbar.slint](/home/wwwroot/mica-term/ui/shell/tabbar.slint#L20)

这正对应用户看到的“右侧顶部先空出来一条”的问题。

### 3. 左侧 `Assets` 区与右侧工作区当前不是同一套 vertical rhythm

左侧 `AssetsSidebar` 自己有：

- 44px header
- 独立边框
- 独立 surface

关键位置：

- [assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint#L68)
- [assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint#L78)

右侧则是另一套固定 `TabBar + content host`。两边并没有共享同一块内容区壳层，所以截图里会产生明显“拼接感”。

### 4. 标签关闭按钮闪烁的根因是 hover 与命中区几何互相反馈

当前 `ActiveTab` 的关键逻辑是：

- `close-visible = root.active || root.has-hover`
- `has-hover = content-hit-target.has-hover || close-hit-target.has-hover`
- `content-hit-target.width = root.close-visible ? close-button.x : parent.width`

关键位置：

- [active-tab.slint](/home/wwwroot/mica-term/ui/components/active-tab.slint#L16)
- [active-tab.slint](/home/wwwroot/mica-term/ui/components/active-tab.slint#L17)
- [active-tab.slint](/home/wwwroot/mica-term/ui/components/active-tab.slint#L107)

这意味着 hover 状态会改变 hit target 几何，而几何变化又会影响 hover 判定，本质上就是一个自反馈抖动源。最近提交 `3c598c1 fix: restyle ssh workspace tabs like vscode` 引入了这套行为。

### 5. 当前“同一 SSH 资产再次打开”是明确复用旧 session，不是偶发 bug

当前双击 / `Open` 的路径最终走 `OpenSessionMode::ActivateExisting`：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L1245)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2827)

而 `SessionManager::open_session()` 在 `ActivateExisting` 模式下，会先按 `asset_id` 查已有 session 并直接复用：

- [session_manager.rs](/home/wwwroot/mica-term/src/app/ssh/session_manager.rs#L89)

只有 `Open in New Tab` 会显式走 `ForceNewTab`：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2844)

所以“只能开一个 SSH 连接”是当前既定产品语义，不是随机故障。

### 6. 当前测试已经把“复用旧 tab”锁定为既有契约

现有测试明确断言：

- 双击同一 SSH 资产会复用已有 tab；
- context menu `Open` 会复用已有 tab；
- 只有 `Open in New Tab` 才会产生第二个 session。

关键位置：

- [assets_explorer_smoke.rs](/home/wwwroot/mica-term/tests/assets_explorer_smoke.rs#L220)
- [assets_explorer_smoke.rs](/home/wwwroot/mica-term/tests/assets_explorer_smoke.rs#L246)
- [assets_explorer_smoke.rs](/home/wwwroot/mica-term/tests/assets_explorer_smoke.rs#L276)
- [workspace_tabs_spec.rs](/home/wwwroot/mica-term/tests/workspace_tabs_spec.rs#L521)

本次调研中已实际运行：

- `cargo test --test workspace_tabs_spec --test assets_explorer_smoke`
- `cargo test --test bootstrap_smoke connect_action`
- `cargo test --test bootstrap_smoke save_and_connect`

上述测试当前全部通过，说明“单资产默认复用旧 session”仍是现行主线，不是未验证状态。

### 7. 当前 SSH modal 已有字段 label，但动作契约与 UI 又发生了漂移

当前 modal 实际已经是带 label 的单页分组表单：

- [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L348)
- [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L479)

但当前 footer 只剩：

- `Test`
- `Save`

关键位置：

- [assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L690)

而 Rust 侧仍保留：

- `Connect`
- `SaveAndConnect`

关键位置：

- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L98)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L2409)

这说明当前 modal 的核心问题已经不是“没有标签”，而是“字段边界和动作集经历了多轮裁剪后没有完全收口”。

### 8. `Connect Options`、`Proxy Method`、`Session Environment` 当前没有进入真实连接链路

当前 `environment` 与 `proxy_method` 会存入资产 spec：

- [view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L1622)

但不会进入 `ConnectionProfile` 的真实连接参数：

- [profile.rs](/home/wwwroot/mica-term/src/app/ssh/profile.rs#L19)

因此这两类字段在当前阶段只是 metadata，不是 runtime contract。用户已明确要求把这部分从 modal 中删掉。

## 设计要点拆分

### 设计点 1：工作区壳层与空标签态

### 方案 A：`WorkspacePane` 拥有统一 surface，无会话时隐藏 `TabBar`

内容：

- `WorkspacePane` 继续作为工作区 owner；
- 无 tab / 无 session 时不渲染顶部标签条；
- welcome 与 terminal host 共用同一块工作区背景；
- 有 session 后再在同一块 surface 内分出标签区与 terminal 区。

实现复杂度：

- 中

与当前架构契合度：

- 高，符合当前 `WorkspacePane` owner 方向

交互一致性：

- 高，更接近 VS Code / 现代桌面终端的内容区体验

可维护性：

- 高，后续 tab/header/terminal 的职责边界更清晰

潜在风险：

- 需要同步调整现有 workspace 壳层测试与几何契约；
- 空工作区与有会话工作区的顶部预算将不再完全一致。

### 方案 B：保留始终可见的独立 `TabBar`，仅做视觉收敛

内容：

- 继续在空工作区保留一条顶部 `TabBar`；
- 只调整背景、边框、padding、颜色，让它看起来更像一体。

实现复杂度：

- 低到中

与当前架构契合度：

- 中

交互一致性：

- 中，视觉上会比现在好，但空标签条仍然没有业务价值

可维护性：

- 中

潜在风险：

- 只能缓解“丑”，不能根治空工作区的结构性冗余；
- 未来仍然可能反复出现“为什么没有 tab 也要占一条顶栏”的争议。

### 最终决策

采用方案 A。

补充约束：

- 空工作区不显示 `TabBar`；
- 有 session 时，`TabBar` 与 terminal host 必须处于同一块工作区 surface 内；
- 不允许再把左侧 `Assets` header 的高度节奏硬对齐到一个没有会话语义的空标签条上。

### 设计点 2：标签关闭按钮命中区与 hover 行为

### 方案 A：关闭区固定占位，hover 只改视觉，不改几何

内容：

- close hit target 的几何固定；
- 文本区宽度不再依赖 `close-visible` 动态变化；
- hover 只影响 icon 可见度与背景态；
- 关闭动作与选中动作始终使用稳定分区。

实现复杂度：

- 低

与当前架构契合度：

- 高

交互一致性：

- 高

可维护性：

- 高

潜在风险：

- 会占用固定的标题宽度预算；
- 极窄标签下可显示文本宽度会略少。

### 方案 B：继续沿用 hover 才出现关闭按钮，且动态改变命中区

内容：

- 保持当前“鼠标移上去才出现 close affordance”；
- 继续让 hover 影响布局几何。

实现复杂度：

- 中

与当前架构契合度：

- 中

交互一致性：

- 低

可维护性：

- 低

潜在风险：

- 当前闪烁问题会持续；
- 关闭按钮与标签选中之间的命中边界难以稳定。

### 最终决策

采用方案 A。

补充约束：

- 关闭按钮必须稳定可点；
- 不允许再通过 hover 改变 `TouchArea` 宽度；
- `ActiveTab` 内的 hover 状态不再参与命中区几何计算。

### 设计点 3：同一 SSH 资产再次打开时的会话语义

### 方案 A：维持当前语义，默认复用已有 live session

内容：

- 双击 / `Open` 默认复用已有 session；
- 只有显式 `Open in New Tab` 时才新开第二个 session。

实现复杂度：

- 低

与当前架构契合度：

- 最高，完全符合当前 `SessionManager` 与测试契约

交互一致性：

- 中

可维护性：

- 高

潜在风险：

- 与用户当前明确预期不一致；
- 用户会持续感知为“为什么开不出第二个连接”。

### 方案 B：把 SSH 资产视为启动模板，每次 `Open` 都新建一个连接标签页

内容：

- 同一 SSH 资产允许并行多个 live session；
- 双击或 context menu `Open` 每次都新建一个 tab；
- 不再保留“同资产默认复用旧 session”的规则。

实现复杂度：

- 中到高

与当前架构契合度：

- 中，需要调整 `SessionManager` 与 session projection 语义

交互一致性：

- 高，更符合终端 / SSH 客户端的多会话心智

可维护性：

- 中到高

潜在风险：

- 与当前测试契约正面冲突；
- 资产级 live-session 标记与 context menu 状态逻辑都要重算。

### 最终决策

采用方案 B。

补充约束：

- 同一 SSH 资产可同时存在多个 session tab；
- 新开会话的默认入口就是 `Open`；
- 不再把“资产 id 唯一映射一个活跃 session”视为产品前提。

### 设计点 4：资产右键菜单中的 SSH 会话动作

### 方案 A：只保留一个 `Open`，删除 `Open in New Tab` 与资产级 `Close`

内容：

- `Open` 的唯一语义是“新建一个连接标签页”；
- 删除 `Open in New Tab`，避免重复入口；
- 删除资产级 `Close Connection`，session 关闭只通过标签关闭按钮触发。

实现复杂度：

- 中

与当前架构契合度：

- 中到高

交互一致性：

- 高

可维护性：

- 高

潜在风险：

- 需要同步收敛 context menu 测试、enabled state 和反馈文案；
- 用户将不再能从资产右键直接断开所有会话。

### 方案 B：保留 `Open`、`Open in New Tab`、`Close`

内容：

- 继续保留多个会话入口；
- 通过文案或状态细分区分它们。

实现复杂度：

- 中

与当前架构契合度：

- 中

交互一致性：

- 低到中，容易语义重复

可维护性：

- 中低

潜在风险：

- 用户会困惑 `Open` 与 `Open in New Tab` 到底差在哪；
- 在多会话前提下，资产级 `Close` 的目标也会变得模糊。

### 最终决策

采用方案 A。

补充约束：

- SSH 资产的右键菜单只保留一个打开入口；
- 该入口命名继续使用 `Open`；
- `Open in New Tab` 从菜单与状态逻辑中彻底删除；
- 资产级 `Close Connection` 从菜单与状态逻辑中彻底删除。

### 设计点 5：SSH modal 中无效连接选项的裁剪

### 方案 A：保留 `Connect Options`、`Proxy Method`、`Session Environment`，但标记为未接线

内容：

- 继续保留这组字段；
- 通过说明文案强调其当前不参与 runtime。

实现复杂度：

- 低

与当前架构契合度：

- 高

交互一致性：

- 中低，保留了无效输入

可维护性：

- 中

潜在风险：

- 用户继续输入当前不会生效的信息；
- 表单认知负担增大。

### 方案 B：保持 modal 现有整体形态与动作集，但删除 `Connect Options`、`Proxy Method`、`Session Environment`

内容：

- 不回退到旧版 tabbed modal；
- 不新增新的动作按钮；
- 只删除本轮无实际价值的连接选项分组与两个字段；
- 其他已生效字段和现有交互保持不变。

实现复杂度：

- 低到中

与当前架构契合度：

- 高

交互一致性：

- 高

可维护性：

- 高

潜在风险：

- 如果后续真要接入 proxy / environment，需要重新设计归位位置。

### 最终决策

采用方案 B。

补充约束：

- `Connect Options` 分组整体删除；
- `Proxy Method` 删除；
- `Session Environment` 删除；
- 当前 modal 的整体页面形态、认证分组、`Test` / `Save` 动作保持不变。

## 方案对比摘要

| 设计点 | 方案 A | 方案 B | 最终选择 |
| --- | --- | --- | --- |
| 工作区壳层 | 统一 surface，空态隐藏 `TabBar` | 空态保留独立 `TabBar` | A |
| 标签关闭交互 | 固定命中区，hover 不改几何 | hover 驱动动态几何 | A |
| 同资产多会话 | 默认复用已有 session | 每次 `Open` 新建 session tab | B |
| 右键菜单打开/关闭 | 仅保留一个 `Open`，去掉 `Open in New Tab` 与资产级 `Close` | 保留多个重叠入口 | A |
| Modal 无效选项 | 保留但标未接线 | 直接删除无效分组与字段 | B |

## 最终决策

本轮确认后的最终决策如下：

- `WorkspacePane` 作为统一工作区壳层 owner；
- 空工作区隐藏 `TabBar`；
- `TabBar` 仅在存在 session tab 时出现；
- `ActiveTab` 的关闭命中区固定，不再让 hover 改变几何；
- 同一 SSH 资产每次 `Open` 都新建一个连接标签页；
- SSH 资产右键菜单仅保留一个 `Open`；
- `Open in New Tab` 删除；
- 资产级 `Close Connection` 删除；
- `SSH modal` 保持当前整体形态与现有动作集不变；
- `Connect Options`、`Proxy Method`、`Session Environment` 删除。

## 实施步骤

本节只记录高层实施顺序，不展开为 implementation plan。

1. 先收敛 `WorkspacePane` 壳层职责，改为“空态无标签、有会话再显示标签”。
2. 修正 `ActiveTab` 命中区与 hover 关系，消除关闭按钮闪烁与无法关闭问题。
3. 改写 `SessionManager` / `bootstrap` 中的 SSH 资产打开语义，使 `Open` 每次都走新建 session tab 路径。
4. 收敛 SSH 资产右键菜单，移除 `Open in New Tab` 与资产级 `Close Connection`，同步更新 enabled state 逻辑。
5. 裁剪 `SSH modal` 中的 `Connect Options`、`Proxy Method`、`Session Environment`，保持其余现有表单与动作不变。
6. 更新相关测试契约，使其从“默认复用旧 tab”转为“每次 `Open` 新建 tab”。

## 风险与回滚策略

### 主要风险

- 当前大量测试默认绑定“同资产默认复用旧 session”，改为多会话后需要成组更新；
- 删除资产级 `Close Connection` 后，context menu 相关状态逻辑和测试会有连锁变化；
- 空态隐藏 `TabBar` 后，已有工作区几何断言与截图基线会变化；
- `ActiveTab` 命中区调整如果处理不当，可能引入新的文本裁剪或 hover 视觉回归。

### 回滚策略

- 若多会话语义改动引发超预期回归，可单独回退 `SessionManager` 的打开语义与对应测试，不必回退 UI 壳层优化；
- 若空态隐藏 `TabBar` 影响过大，可只回退 `WorkspacePane` 的空态展示策略，而保留 `ActiveTab` 命中区修复；
- 若 modal 字段裁剪引发兼容问题，可仅恢复表单展示层，不恢复其进入 runtime 的错误暗示。

## 验证清单

- [ ] 空工作区默认不显示顶部 `TabBar`
- [ ] 左侧 `Assets` 区与右侧工作区不再出现明显“硬拼接”观感
- [ ] 有 session 时，标签区与 terminal host 处于同一块工作区 surface 内
- [ ] 标签关闭按钮 hover 不再闪烁
- [ ] 标签关闭按钮可稳定关闭当前 session
- [ ] 同一 SSH 资产连续执行两次 `Open` 后会产生两个不同 `session_id`
- [ ] SSH 资产右键菜单中不再出现 `Open in New Tab`
- [ ] SSH 资产右键菜单中不再出现资产级 `Close Connection`
- [ ] `Open` 是 SSH 资产右键菜单中唯一的连接入口
- [ ] `SSH modal` 中不再出现 `Connect Options`
- [ ] `SSH modal` 中不再出现 `Proxy Method`
- [ ] `SSH modal` 中不再出现 `Session Environment`
- [ ] 现有 `Test` / `Save` 动作保持可用
- [ ] 相关测试从“默认复用旧 session”收敛到“每次 `Open` 新建 session tab”
