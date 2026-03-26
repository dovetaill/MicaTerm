# SSH Workspace / Terminal Flow Design

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

---

## 追加设计任务：SSH 输入层级与终端显示修正

日期: 2026-03-26
执行者: Codex
状态: 方案已确认，仅记录 design，不展开 implementation plan

### 背景

在 `SSH create/connect tabs` 主线落地后，当前 terminal host 仍存在四类关键问题：

- 某些远端登录提示没有产品价值，尤其是
  `Activate the web console with: systemctl enable --now cockpit.socket`
  会直接污染首屏；
- 输入链路仍停留在“普通文本 + 少量 named key + 部分鼠标事件”的半成品状态，导致
  `Ctrl+C`、滚轮、`F` 键、应用模式下的方向键、bracketed paste 等行为不完整；
- 亮色模式下，terminal viewport 的背景 ownership 与 palette strategy 错位，出现“只有渲染到字符的区域才是黑底，其余仍是浅色壳层”的断裂感；
- 当前 renderer 仍以逐 cell 的 Slint `Rectangle/Text` 绘制为主，缺少更高一级的 presenter，
  难以达到接近 VS Code editor 区域的整体观感。

本轮目标不是直接实现，而是把最终确认后的架构边界、视觉边界和输入边界固化为设计结论。

### 目标

本轮设计目标：

- 全局精确隐藏远端输出中 exact match 的 cockpit 提示行；
- 将 terminal host 从分裂的输入转发逻辑升级为统一输入事件层；
- 补齐以下输入类别的架构边界：
  普通字符输入、`Ctrl` / `Alt` / `Shift` 组合键、`Enter` / `Backspace` / `Tab` / `Esc`、
  箭头键、`Home` / `End` / `PageUp` / `PageDown` / `Delete` / `F` 键、
  鼠标点击 / 拖拽 / 滚轮、bracketed paste、窗口 resize；
- 让 terminal viewport 的整块背景、默认前景、cursor、selection 与 app theme 协同，而不是继续混用静态 Slint 背景与默认黑色 terminal palette；
- 将 renderer 提升到 presenter 级，而不是继续沿用纯逐 cell 拼装，以支撑接近 VS Code editor 的 light-mode IDE 质感；
- 行级视觉增强仅允许使用 contextual tint，不允许做全局强制 zebra striping。

### 非目标 / 边界

本轮设计不覆盖：

- SSH 连接配置、tab 语义、资产菜单等上一轮已确认范围；
- `russh-sftp` UI；
- 多 pane / 多窗口 / 多 workspace；
- 终端语义高亮、shell parser、命令语义分析；
- implementation plan 文档，除非后续单独要求。

本轮允许调整但必须保持边界清晰的内容：

- `TerminalSessionHost` 的 focus / input capture 结构；
- `bootstrap` 中 terminal callback 到 runtime 的事件归一化；
- `runtime` 中 `wezterm-term` 的 palette、key、mouse、paste、resize 接口使用方式；
- 基于 Slint 的 terminal presenter / viewport rendering 策略。

### 当前实现现状

#### 1. 当前 named key 编码没有复用 live terminal state

当前 `encode_named_key_input(...)` 会临时 new 一个 `TerminalSession` 来编码按键，而不是让当前
活跃 terminal 实例自己编码。这会绕过运行时里的 `application_cursor_keys`、
`modify_other_keys`、当前 keyboard encoding 等状态。

这也是 `vim`、`htop`、`tmux` 一类场景里“能进但用不顺”的根因之一。

#### 2. 当前 paste 不是 bracketed paste

当前 paste 路径是读取系统剪贴板，再把文本当普通输入直接发给 SSH channel。

这意味着即使远端已经启用 bracketed paste mode，当前 UI 仍不会走 terminal-native paste 语义。

#### 3. 当前鼠标只覆盖 press / move / release 的一部分

当前 UI 只转发了 left/right 的 `down/up` 与 `move`，没有滚轮事件，也没有更高层的统一输入抽象。

#### 4. 当前 terminal palette 默认就是 dark terminal

当前 `SessionTerminalConfig::color_palette()` 直接返回 `ColorPalette::default()`。
上游默认 palette 的 `background` 是黑色，因此 light mode 下如果不显式接管 palette，
默认背景就天然偏黑。

#### 5. 当前 viewport 背景和 terminal 默认背景不是同一个 owner

当前 Slint 侧 `surface-frame` 画的是一整块静态背景，但真正的 terminal default background
只会通过逐 cell 的背景色体现；结果就是“字符所在 cell 有背景，空白 viewport 仍是宿主背景”，
视觉上形成断裂。

#### 6. 当前 renderer 还是逐 cell 直绘

当前 terminal host 直接遍历 `session-cells`，每个 cell 各自绘制背景和文本。
这条路线短期可用，但无法优雅承载：

- 整块 viewport 主题 ownership；
- 更接近 IDE 编辑区的整体排版；
- 行级 contextual tint；
- 后续更复杂的 selection / cursor / run-based text painting。

### 设计要点拆分

#### 设计点 1：远端提示输出抑制层

##### 方案 A：在远端输出进入 terminal 之前做全局 exact-match 过滤

内容：

- 在 SSH channel 到 terminal buffer 的单一入口增加一层 line-oriented suppression；
- 仅对 exact match
  `Activate the web console with: systemctl enable --now cockpit.socket`
  生效；
- 作为全局规则处理，而不是仅限某个 host 或某次会话。

实现复杂度：

- 中

与当前架构契合度：

- 高，输出入口集中，易于单点控制

交互一致性：

- 高，不会污染 row/cell/cursor/selection 的投影逻辑

可维护性：

- 中高，后续若再追加极少量 exact-match 抑制规则，也有统一入口

潜在风险：

- 需要处理分块输出与换行规范化；
- 如果用户未来主动输出完全相同的一行，也会被隐藏。

##### 方案 B：只在 surface projection 或渲染阶段做视觉隐藏

内容：

- 不动 transport / terminal buffer；
- 仅在投影到 Slint 之前或绘制时把该行藏掉。

实现复杂度：

- 低到中

与当前架构契合度：

- 中

交互一致性：

- 低，terminal 内部状态与屏幕展示可能脱节

可维护性：

- 低

潜在风险：

- 容易引入 selection、cursor、mouse row 与 scrollback 语义错位。

##### 最终决策

采用方案 A。

补充约束：

- 抑制范围为全局 exact-match；
- 不做模糊匹配，不做 host-specific 特判，不做前缀匹配。

#### 设计点 2：terminal 输入事件总线

##### 方案 A：沿用现有分裂 callback 结构，逐项补齐缺失输入

内容：

- 继续保留 `text-input`、`key-input`、`paste-requested`、`mouse-input`、`resize`；
- 缺什么补什么，把更多 key 和 wheel 一项项接进来。

实现复杂度：

- 中

与当前架构契合度：

- 中高

交互一致性：

- 中，只能逐步缓解缺项，不能根治 terminal-native mode 同步问题

可维护性：

- 中低，输入规则会继续散落在 Slint 与 Rust 两边

潜在风险：

- 容易反复出现“又有一个键不对”的回归；
- 仍可能绕过 live terminal 的 mode state。

##### 方案 B：建立统一 `TerminalInputEvent` 层，统一走 terminal-native API

内容：

- Slint 只负责捕获键盘、鼠标、滚轮、paste、resize；
- `bootstrap` 负责归一化为统一输入事件；
- `runtime` 统一调用 live terminal 的 `key_down()`、`send_paste()`、`mouse_event()`、`resize()`；
- named key 不再通过临时 `TerminalSession` 编码。

实现复杂度：

- 中高

与当前架构契合度：

- 高，符合 Slint host / bootstrap bridge / runtime state machine 的职责分层

交互一致性：

- 高，能够覆盖 shell 与全屏 TUI 的真实输入语义

可维护性：

- 高，后续追加 `focus_changed`、IME、更多鼠标模式时更稳定

潜在风险：

- 需要重整 terminal host 的 focus / key capture 结构；
- 不是“补一个 key mapping”级别的小修。

##### 最终决策

采用方案 B。

补充约束：

- 输入事件源必须覆盖普通字符、组合键、功能键、滚轮、拖拽、粘贴、resize；
- paste 必须通过 terminal-native bracketed paste 语义发送；
- named key 编码必须基于 live terminal state，而不是临时 session。

#### 设计点 3：terminal palette 与 viewport 背景 ownership

##### 方案 A：terminal palette 跟随 app theme，viewport 背景由 terminal 自己拥有

内容：

- 为 terminal 定义明确的 light palette 与 dark palette；
- 让 viewport 的整块默认背景、默认前景、cursor、selection 都从 terminal palette 派生；
- 宿主壳层只负责外框和工作区层级，不再替 terminal 决定默认底色。

实现复杂度：

- 中

与当前架构契合度：

- 高，`SessionTerminalConfig` 正是 palette 注入点

交互一致性：

- 高，light mode 下不再出现“浅色壳层里悬浮黑条”的割裂感

可维护性：

- 中高，终端主题与 app theme 的关系会更清楚

潜在风险：

- 需要认真校准 light palette 下的 ANSI 16 色，避免 prompt / diff / highlight 发灰或发脏。

##### 方案 B：terminal 永远保持 dark palette，只修 viewport 背景铺满

内容：

- 不为 light mode 定义独立 terminal theme；
- 只修复“terminal 默认背景必须完整铺满 viewport”这一 correctness 问题。

实现复杂度：

- 低到中

与当前架构契合度：

- 高

交互一致性：

- 中，视觉会比现在完整，但仍保留“亮色壳层 + 深色 terminal”的分离感

可维护性：

- 高

潜在风险：

- 无法满足当前明确的 light-mode 协调目标。

##### 最终决策

采用方案 A。

补充约束：

- app theme 为 light 时，terminal 也进入 light palette；
- terminal 默认背景必须完整拥有 viewport；
- `ThemeTokens` 只负责外部 shell surface，不再与 terminal 默认背景抢 ownership。

#### 设计点 4：presenter 级 renderer 与 VS Code 风格边界

##### 方案 A：继续沿用逐 cell 直绘，仅做配色和间距美化

内容：

- 保留当前 `session-cells -> Rectangle/Text` 的直绘模型；
- 只调字体、padding、cursor、selection、边框、配色。

实现复杂度：

- 低到中

与当前架构契合度：

- 高

交互一致性：

- 中，只能把现在的观感“修顺眼”，无法真正靠近 IDE editor 的整体感

可维护性：

- 中

潜在风险：

- 视觉上限较低；
- 很容易变成一轮又一轮的 cosmetic patch。

##### 方案 B：提升到 presenter 级 renderer，并使用 contextual tint

内容：

- 引入 terminal presenter 概念，由 presenter 负责：
  viewport 背景、行背景、text run、selection、cursor、行内默认 tint；
- 不再把“每个 cell 都是一个完整 UI 元素”当成唯一表达方式；
- 视觉目标参考 VS Code editor 区域，但不机械照搬；
- 行级视觉增强仅允许 contextual tint：
  只对 default-background 行做很轻的相邻色变化；
- 对 `vim`、`htop`、`less`、显式 ANSI 背景、alt-screen 场景，contextual tint 自动退让或关闭。

实现复杂度：

- 中高

与当前架构契合度：

- 高，符合“custom renderer on top of Slint”的长期方向

交互一致性：

- 高，既能提升 shell 观感，也不会粗暴破坏全屏 TUI 的自绘语义

可维护性：

- 高，后续扩展 selection、hyperlink、semantic decoration 时更有空间

潜在风险：

- renderer 分层会比当前复杂；
- 如果 presenter 责任边界不清，容易与 runtime projection 再次耦合。

##### 最终决策

采用方案 B。

补充约束：

- 视觉目标是“接近 VS Code editor 区域的协调感”，不是像素级复刻；
- 行交错只允许 contextual tint；
- 明确禁止全局强制 zebra striping。

### 方案对比摘要

| 设计点 | 方案 A | 方案 B | 最终选择 |
| --- | --- | --- | --- |
| 输出抑制层 | transport 前 exact-match 过滤 | projection / render 视觉隐藏 | A |
| 输入事件总线 | 沿用现有 callback 逐项补洞 | 统一 `TerminalInputEvent` + terminal-native API | B |
| palette 与背景 ownership | terminal 跟随 app theme，完整拥有 viewport | 终端固定深色，只补 viewport 背景 | A |
| renderer 风格边界 | 继续逐 cell 直绘 | presenter 级重构 + contextual tint | B |

### 最终决策

本轮确认后的最终决策如下：

- cockpit 提示行采用全局 exact-match 抑制；
- terminal 输入层重构为统一输入事件总线；
- live terminal 成为 key / paste / mouse / resize 的唯一语义出口；
- bracketed paste 必须走 terminal-native `send_paste()`；
- terminal 主题随 app theme 切换，提供明确的 light / dark terminal palette；
- terminal 默认背景必须完整拥有 viewport；
- renderer 提升到 presenter 级；
- VS Code 风格只作为气质参考，不做机械复刻；
- 行级美化只允许 contextual tint，不允许全局 zebra。

### 实施步骤

本节只记录高层实施顺序，不展开为 implementation plan。

1. 先建立 terminal 输入事件抽象，并收拢当前分裂的 Slint callback 语义。
2. 把 named key、paste、mouse、resize 全部改为由 live terminal state 驱动。
3. 在 SSH 输出入口增加 exact-match suppression，并保证不破坏 chunk 拼接与行边界。
4. 重新定义 terminal theme contract，让 palette 与 app theme 形成明确映射。
5. 让 terminal viewport 默认背景从 terminal palette 派生，并完全覆盖显示区域。
6. 引入 presenter 层，替代当前纯逐 cell 直绘的核心职责。
7. 在 presenter 层实现 contextual tint，并为显式背景 / alt-screen 场景设置退让规则。
8. 最后补齐输入、渲染、主题与回归验证。

### 风险与回滚策略

#### 主要风险

- 输入链路统一后，可能暴露出比当前更多的模式同步问题；
- light palette 如果调校不佳，会让 ANSI 颜色在亮色主题下显脏；
- presenter 重构如果边界不清，可能再次把 UI chrome 与 terminal 内容刷新耦合起来；
- contextual tint 若退让规则不足，可能干扰 `vim` / `htop` / `less` 等全屏 TUI 的视觉正确性。

#### 回滚策略

- 若统一输入事件层引发高风险回归，可先保留事件抽象，但临时只切换 key / paste 路径，鼠标与滚轮延后；
- 若 light palette 效果不稳定，可暂时保留 dark palette 作为 fallback，但不回退输入层重构；
- 若 presenter 级重构超出可控范围，可先保留 presenter 的 viewport / row ownership，再逐步替换 text painting；
- 若 contextual tint 对全屏 TUI 造成干扰，可先全局关闭 tint，但保留 presenter 与 palette 架构。

### 验证清单

- [ ] 远端输出中 exact match 的 cockpit 提示行不再显示
- [ ] 其他普通输出不受 suppression 误伤
- [ ] `Ctrl+C` 在无 selection 时能正确发送到远端
- [ ] `Ctrl+C` 在有 selection 时仍可复制选中内容
- [ ] `Ctrl` / `Alt` / `Shift` 组合键能覆盖常见 shell / TUI 场景
- [ ] `Enter` / `Backspace` / `Tab` / `Esc` 行为与标准终端一致
- [ ] 箭头键、`Home` / `End` / `PageUp` / `PageDown` / `Delete` / `F` 键可在 `vim` / `htop` 中正常工作
- [ ] 鼠标点击、拖拽、滚轮在 shell 与支持 mouse tracking 的 TUI 中行为正确
- [ ] bracketed paste 开启时，paste 以 bracketed 形式发送
- [ ] 窗口 resize 后 rows / cols 能同步到远端 PTY
- [ ] light mode 下 terminal viewport 不再出现“字符区黑底、空白区白底”的断裂
- [ ] terminal 默认背景完整覆盖 viewport
- [ ] 终端字体维持 mono 风格，整体观感接近 IDE 编辑区而非传统粗糙 cell 网格
- [ ] contextual tint 仅在合适场景生效，不干扰显式背景或 alt-screen TUI
