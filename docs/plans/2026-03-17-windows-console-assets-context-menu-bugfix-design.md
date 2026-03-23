# Windows Console 资产列表右键菜单 Bugfix Design

日期: 2026-03-17
执行者: Codex
状态: 已确认方案，待按需进入 implementation plan

## 背景

本轮任务聚焦 `Windows Console` 资产列表区域的三个直接问题：

1. 当前资产列表区域右键无效；
2. 当前资产列表下方仍显示 `Prod SSH`、`Favorites`、`Jump Host` 三个无意义 demo 项；
3. `New SSH` 等“新建”入口第一版只需要创建一个名称占位，暂不接入真实 SSH 业务逻辑。

本轮不是 terminal runtime、renderer、SSH/SFTP 生命周期或持久化方案的大改造，而是一次针对 `AssetsSidebar` 壳层交互的聚焦收敛。

## 当前现状与问题定位

### 代码现状

- 当前主工作区仍未接入真实 terminal widget，`src/main.rs` 只负责选择 `winit + femtovg-wgpu` renderer；`ui/app-window.slint` 主区域仍是 `WelcomeView` 占位。
- 当前资产列表是壳层原型，`ui/shell/assets-sidebar.slint` 通过 `for item in root.console-asset-items` 渲染 `AssetNodeRow`，并不是真实 SSH/Folder 树。
- 当前右键菜单基础设施已经存在：
  - Rust 侧有 `src/shell/context_menu.rs` 的 action tree / placement / visible columns；
  - 状态由 `src/shell/view_model.rs` 持有；
  - `src/app/bootstrap.rs` 已经把 `asset-context-menu-requested`、`row-hovered`、`key-pressed` 等桥接到 `ShellViewModel`。

### 根因定位

- demo 数据来源已经明确：`src/app/bootstrap.rs` 中 `default_console_asset_items()` 直接注入了 `Prod SSH`、`Favorites`、`Jump Host`。
- 当前 item row 自身有右键入口：`ui/components/asset-node-row.slint` 在 `PointerEventButton.right` 时发出 `context-menu-requested(...)`。
- 但 blank area 没有独立右键入口；现有结构只覆盖“点在某一行上”的场景，没有覆盖“列表空白区右键”。
- 当前测试以直接调用 `AppWindow` callback 为主，验证了 Rust 状态机与 overlay 投影，但没有真正守住“面板 pointer 输入链路是否完整打通”这一层。

### Git 调研

最近相关提交显示当前右键菜单能力是刚引入的壳层功能，而不是长期稳定模块：

- `07692e3 feat: add windows console assets context menu`
- `eba141d Merge branch 'feature/windows-console-assets-context-menu'`

结论是：本轮应继续沿用现有 `AppWindow overlay + ShellViewModel + bootstrap callback bridge` 路线修正，不应切换到另一套菜单体系。

## 目标

- 移除当前无意义的 demo 资产项，恢复为真正的空态壳层。
- 让 `Windows Console` 资产区同时支持：
  - item right-click；
  - blank-area right-click。
- 第一版把“新建”信息架构收敛为最小集合：
  - `New Folder`
  - `New SSH Connection`
- `New Folder` / `New SSH Connection` 第一版只创建一个待命名占位项，并立即进入 inline rename。
- 保持既有架构一致性：
  - Rust 继续持有状态与菜单裁决；
  - Slint 继续负责 overlay 与视觉渲染；
  - 不扩散到 terminal、SSH runtime、SFTP、数据库或跨平台抽象重构。

## 边界

### 本轮覆盖

- 资产区空态表现
- 资产区 blank-area 右键入口
- 资产区 item 右键入口修正
- 右键菜单最小 IA 收敛
- “新建后立即 inline rename”的交互壳层

### 本轮不覆盖

- 真实 SSH 连接配置创建
- `wezterm-term`、`termwiz`、`russh`、`russh-sftp` 接入
- 真实资产树、文件夹树、虚拟滚动
- 数据库存储与真实持久化 schema
- 完整菜单键盘系统扩展
- 多协议新建入口恢复

## 方案对比

### 设计点 1：初始资产列表数据源

| 方案 | 描述 | 优点 | 缺点 |
| --- | --- | --- | --- |
| `1A` | 保留 demo 列表，但换成更像占位数据的名字 | 成本最低，便于继续点选测试 | 仍是假数据，用户感知差，问题本质未解决 |
| `1B` | 改为空列表 + empty state | 最符合当前诉求，信息干净，后续真实数据接入最自然 | 必须同步补齐 blank-area 右键入口 |
| `1C` | 保留一个系统占位行 | 不会让界面过空 | 容易和真实资产混淆，交互语义不干净 |

最终选择：`1B`

### 设计点 2：右键入口修复方式

| 方案 | 描述 | 优点 | 缺点 |
| --- | --- | --- | --- |
| `2A` | 保留现有自绘 overlay，只补 item + blank-area 双入口 | 与现有架构最一致，改动最小，视觉最稳定 | 需要小心 blank-area touch layer 不要抢 row 命中 |
| `2B` | 改用 Slint 原生 `ContextMenuArea` / `PopupWindow` | 部分基础语义更原生 | 会削弱当前自绘菜单的一致性与可控性 |
| `2C` | 改成 panel 统一命中层，统一判断 item/blank-area | 真源统一，未来树模型扩展更强 | 对当前 bugfix 来说过重 |

最终选择：`2A`

### 设计点 3：第一版新建信息架构

| 方案 | 描述 | 优点 | 缺点 |
| --- | --- | --- | --- |
| `3A` | 保留完整 `New Connection -> SSH / Local Terminal / Serial / Telnet / SSH Tunnel` | 提前定型未来 IA | 第一版大部分入口只是 planned，显得虚 |
| `3B` | 仅保留 `New Folder` 和 `New SSH Connection` | 最小、最清晰、最符合当前目标 | 未来恢复多协议时需要再扩一轮 IA |
| `3C` | 保留总入口 `New Connection`，点开二级菜单 | 兼顾未来扩展 | 对当前问题来说仍偏重 |

最终选择：`3B`

### 设计点 4：新建动作第一版落地深度

| 方案 | 描述 | 优点 | 缺点 |
| --- | --- | --- | --- |
| `4A` | 直接插入默认名，后续再 rename | 成本最低 | 用户仍要再做一步操作 |
| `4B` | 插入占位项后立即 inline rename | 最贴近桌面端体验，符合“只新建一个名称”诉求 | 需要补一个轻量编辑态 |
| `4C` | 弹极简浮层 / 小对话框，只收一个 name | 状态清晰 | 比 inline rename 更重，打断感更强 |

最终选择：`4B`

## 最终决策

### 决策摘要

- 移除 `default_console_asset_items()` 的 demo 输出，资产区默认回到空列表。
- `Windows Console` 面板在无资产时展示 empty state，而不是伪装成真实资产列表。
- 保留当前自绘右键菜单体系，不切换到 Slint 原生菜单。
- 在 `AssetsSidebar` 中补一条 blank-area right-click 路径，与现有 row right-click 并存。
- 把新建入口收敛为：
  - `New Folder`
  - `New SSH Connection`
- 点击上述动作后，只创建一个内存中的占位资产项，并立即进入 inline rename。
- 第一版不实现 SSH 连接详情、认证、host、port、jump host、folder metadata 等业务逻辑。

### 目标状态

用户首次进入 `Window Console` 时，看到的是：

- 一个干净的 empty state；
- 可以在空白区右键，弹出最小菜单；
- 可以点击顶部 `Create` 或通过右键菜单创建 `Folder` / `SSH`；
- 创建后列表立即出现一个待命名项，焦点直接进入 inline rename；
- 命名完成后，该项作为纯壳层资产存在，供后续真实业务接入。

## 目标架构

### 1. 资产区结构

资产区保持当前 `AssetsSidebar -> panel-content-host -> AssetNodeRow list` 的分层，不引入新的复杂容器。变化点只有两个：

- 当 `console-asset-items` 为空时，渲染 empty state；
- 在空列表区域或列表剩余空白区域增加 blank-area right-click touch target。

### 2. 右键入口策略

- `item right-click`
  - 继续由 `AssetNodeRow` 发出 `context-menu-requested(item-id, item-kind, x, y)`；
  - 仍沿用当前 row 级坐标与 `ShellViewModel::open_context_menu_for_target(...)`。
- `blank-area right-click`
  - 由 `AssetsSidebar` 的面板内容层新增独立 pointer target；
  - 发出 `asset-context-menu-requested("", "blank", x, y)`；
  - Rust 侧继续通过 `parse_context_target_kind("blank") -> BlankArea` 进入现有 resolver。

### 3. 菜单 IA 收敛

本轮不再延续“完整连接协议集合”的展示策略，而是把第一版 IA 明确压缩到用户现在真正能理解和使用的最小集合。

建议：

- blank-area 菜单第一组只保留：
  - `New Folder`
  - `New SSH Connection`
- item 菜单中如果涉及“创建子项”或“同级创建”，也只保留这两个入口；
- 其他未来动作不在本轮 design 中继续扩散。

这意味着当前 `context_menu.rs` 中的 `new_connection_submenu(...)` 不再是本轮设计真源；本轮设计真源是最小新建动作集，而不是未来完整协议菜单。

### 4. 占位资产与 inline rename

新建动作第一版采用“先插入、再改名”的桌面端壳层语义：

1. 用户触发 `New Folder` 或 `New SSH Connection`；
2. 菜单关闭；
3. Rust 侧向 `console_asset_items` 追加一个新的 placeholder item；
4. 该 item 带有类型：
   - `folder`
   - `ssh`
5. 同步设置一个“当前正在重命名的资产 id”状态；
6. Slint 将该行切换为 inline rename 模式；
7. 用户输入名称并确认；
8. 第一版只更新内存状态，不做真实连接创建。

## 数据流

### 空白区右键

1. 用户在资产区空白处右键；
2. blank-area touch target 发出 `asset-context-menu-requested("", "blank", x, y)`；
3. Rust 侧打开 `BlankArea` 菜单；
4. overlay 在当前根窗口坐标系中定位；
5. 用户选择 `New Folder` 或 `New SSH Connection`；
6. Rust 创建 placeholder item；
7. UI 切换到 inline rename。

### 行右键

1. 用户在已有资产项上右键；
2. `AssetNodeRow` 发出 `asset-context-menu-requested(item-id, item-kind, x, y)`；
3. Rust 按 item kind 打开对应菜单；
4. 如果选择创建动作，也仍然走 placeholder + inline rename。

### 命名提交

1. 用户输入名称并确认；
2. Rust 更新当前 placeholder item 的 `label`；
3. 清空“正在重命名”状态；
4. 本轮不触发任何 SSH 业务 side effect。

## 实施步骤

> 这里是 design 级实施顺序，用于界定范围；详细 TDD / commit 颗粒度在 implementation plan 中再展开。

1. 清理当前 demo 数据注入，资产区默认进入空态。
2. 为 `AssetsSidebar` 定义空态文案与空态布局。
3. 在 panel content 中补 blank-area right-click 命中层。
4. 收敛右键菜单 IA 到 `New Folder` / `New SSH Connection`。
5. 为 `ShellViewModel` 增加“创建占位项 + inline rename”所需的最小状态。
6. 在资产行组件中增加 inline rename 渲染分支。
7. 增加 UI 级 smoke 与 view model 级测试，守住：
   - 空态；
   - blank-area right-click；
   - create action；
   - inline rename round trip。

## 风险与回滚

### 风险 1：blank-area 命中层抢走 row 事件

- 风险：如果 blank-area touch area 覆盖层级不当，会导致 item right-click 失效或 click/select 异常。
- 缓解：
  - 保持 row 自身命中优先；
  - blank-area target 只覆盖未被 row 占据的区域，或在宿主层明确避开行区域。

### 风险 2：空态与列表态切换后，菜单锚点坐标不稳定

- 风险：空态布局、空列表容器、滚动区域切换后，右键坐标可能与 overlay 定位不一致。
- 缓解：
  - 继续统一使用根窗口坐标；
  - 保持 `bootstrap.rs` 的 `update_context_menu_placement(...)` 为唯一 placement 真源。

### 风险 3：inline rename 引入过多“真实编辑器”语义

- 风险：若第一版把 inline rename 做成完整表单，会把任务扩散到输入法、校验、焦点恢复等大题。
- 缓解：
  - 第一版只支持最小文本编辑闭环；
  - 不做复杂校验，只保证可提交和可取消。

### 回滚策略

若实现阶段发现 inline rename 复杂度明显超出预期，可按以下顺序回滚，且不影响已确认的大方向：

1. 保留 `1B + 2A + 3B`；
2. 将 `4B inline rename` 临时降级为 `4A 默认名插入`；
3. 后续单独补“rename editing state”设计，不回退到 demo 数据或完整多协议 IA。

## 验证清单

- [ ] 启动后不再显示 `Prod SSH`、`Favorites`、`Jump Host`
- [ ] `Window Console` 在空列表时显示 empty state
- [ ] 空白区域右键可以打开 blank-area 菜单
- [ ] 资产行右键仍然可以打开 item 菜单
- [ ] 菜单中第一版新建入口只剩 `New Folder` 与 `New SSH Connection`
- [ ] 触发 `New Folder` 后插入一个 folder placeholder，并立即进入 inline rename
- [ ] 触发 `New SSH Connection` 后插入一个 ssh placeholder，并立即进入 inline rename
- [ ] 命名提交后只更新壳层资产名称，不触发真实 SSH 业务逻辑
- [ ] overlay 定位、关闭、hover path 行为不因空态/列表态切换而退化

## 后续文档

- 若进入实现阶段，应基于本文档补充：
  - `docs/plans/2026-03-17-windows-console-assets-context-menu-bugfix-implementation-plan.md`
- implementation plan 需要进一步细化：
  - 具体文件路径
  - TDD 顺序
  - smoke / cargo test 命令
  - 提交粒度
