# Assets Sidebar Toolbar Bugfix Design

日期: 2026-03-16
执行者: Codex
状态: 方案已确认，待进入实现规划

## 背景

`AssetsSidebar` 顶部工具区已在今天早些时候完成首轮接入，但当前落地结果与目标视觉明显不一致：

- Search 采用 `inline row` 展开，观感像“挤出一行输入框”，不像竞品那种贴顶的浮层式搜索条
- Search 在空值时点击其他区域没有稳定收起
- `Create` 仍然带文字，不符合当前顶部工具区必须采用纯 icon button 的明确约束
- `Create` 下拉菜单的视觉宽度、锚点方向、图文对齐仍不自然
- 左侧 `AssetsSidebar` 宽度偏紧，header 呼吸感不足

本轮目标不是重做整套 sidebar，而是在不修改终端 runtime、不变更 renderer 主路径的前提下，对 `AssetsSidebar` 顶部工具区做一次聚焦于交互与视觉契约的方案收敛。

## 调研结论

### 相关 Git 历史

- `53c4b1d feat: implement assets sidebar toolbar shell`
- `1d4de33 fix: complete assets sidebar toolbar bugfix`
- `9266180 Stabilize Windows femtovg-wgpu mainline on DX12`

结论：

- 当前问题直接来自 `53c4b1d` 的首版 toolbar 方案，以及 `1d4de33` 对 `Create` popup 宿主迁移后的第二轮补丁
- 当前主线路径仍是 `winit + femtovg-wgpu + wgpu-28`，本轮不涉及 terminal widget、renderer 或 DX12 路线变更
- 现有 `Create` 菜单已提升到根窗口 host，但搜索交互仍停留在 `AssetsSidebar` 内联行方案

### 关键代码位置

- [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint)
- [ui/components/assets-create-menu.slint](/home/wwwroot/mica-term/ui/components/assets-create-menu.slint)
- [ui/components/sidebar-toolbar-icon-button.slint](/home/wwwroot/mica-term/ui/components/sidebar-toolbar-icon-button.slint)
- [ui/components/titlebar-menu.slint](/home/wwwroot/mica-term/ui/components/titlebar-menu.slint)
- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint)
- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs)
- [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs)
- [src/shell/metrics.rs](/home/wwwroot/mica-term/src/shell/metrics.rs)
- [src/shell/layout.rs](/home/wwwroot/mica-term/src/shell/layout.rs)

### 现状证据

1. 当前 Search 位于 `AssetsSidebar` 内部，是 `if root.asset-search-expanded : Rectangle` 的内联展开行，不是 popup。
2. 当前 Search 收起依赖 `TextInput.has-focus`，只能处理部分失焦路径，不能可靠覆盖所有 click-away 场景。
3. 当前 `Create` 是 `Add + Create + Chevron` 的 104px 复合按钮，直接违背“顶部图标按钮栏不要文字”的约束。
4. 当前 `Create` 菜单虽然已提升到根窗口，但锚点仍只是简单地放在按钮下方，菜单展开方向、宽度和行内容对齐未与触发器形成统一几何语言。
5. 当前 `AssetsSidebar` 宽度仍固定为 `256px`，header 在 3 个 icon button 加触发器并排时偏拥挤。
6. 标题栏已有 [titlebar-menu.slint](/home/wwwroot/mica-term/ui/components/titlebar-menu.slint) 这套 root popup 模式，可作为 `Create` 菜单几何与交互的参考，而不是继续发展第二套不一致菜单语义。

### 官方能力确认

通过官方文档检索，以下能力已确认可用：

- `PopupWindow` 支持 `close-on-click-outside` 与 `no-auto-close`
- `TextInput` 支持 `focus()` / `clear-focus()`
- `absolute-position` 可作为根窗口级锚点计算输入

参考：

- <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/>
- <https://docs.slint.dev/latest/docs/slint/guide/development/focus/>

结论：

- 本轮问题不是 Slint 做不到，而是当前方案选择不对
- Search 与 `Create` 都应转向“根窗口锚定 overlay”的统一思路

## 目标

- 将 Search 从内联展开行改为更贴近竞品的 anchored search popup
- 明确 Search 的 click-away 关闭规则：空值可自动关闭，非空保持展开
- 将 `Create` 触发器改为纯 icon button，移除文字
- 调整 `Create` 菜单的锚点方向、宽度与行布局，使其与 trigger 几何语言一致
- 将 `AssetsSidebar` 宽度从 `256px` 提升到更舒展的档位
- 保持现有 `AppWindow -> Sidebar -> AssetsSidebar` 架构不变

## 边界

### 本文档覆盖

- Search 触发器与 popup 形态
- Search 关闭规则
- `Create` 触发器形态
- `Create` 菜单锚点方向、宽度与行对齐
- `AssetsSidebar` 宽度调整
- 风险、回滚与验证标准

### 本文档不覆盖

- `wezterm-term` / `termwiz` / `russh` / `russh-sftp`
- 真实 terminal widget
- 真实资产树数据、搜索算法、创建向导
- 全局主题 token 重构
- 顶部状态栏、右侧面板、窗口 frame 策略

## 设计点与方案对比

### 设计点 1：Search 形态

#### 方案 S1A：保留 `inline search row`

优点：

- 改动最小
- 不增加新的 popup 层级
- 与现有状态结构兼容

缺点：

- 视觉上仍像“展开一行”，不接近竞品
- 会把列表内容整体下压
- click-away 规则更难做干净

#### 方案 S1B：改为 anchored search popup

优点：

- 视觉最接近参考图
- 不会挤压列表内容
- 更容易和 `Create` 菜单统一为根窗口 overlay 体系

缺点：

- 需要新增 search anchor 与 popup 状态同步
- 相比内联行多一层几何管理

最终选择：`S1B`

### 设计点 2：Search 关闭规则

#### 方案 S2A：任何 outside click 都关闭

优点：

- 规则最简单
- 与一般 popup 行为一致

缺点：

- 用户带着过滤结果点击列表时，搜索框会立刻消失
- 过滤上下文感被打断

#### 方案 S2B：空值 outside click 自动关闭，非空保持展开

优点：

- 更符合“搜索是临时工作台”的桌面工具直觉
- 保留 query 的可见上下文
- 与你当前要求完全一致

缺点：

- 不能只依赖 `PopupClosePolicy`
- 需要 Search popup 与状态机配合

最终选择：`S2B`

### 设计点 3：`Create` 触发器形态

#### 方案 S3A：纯 icon button

优点：

- 与顶部其余工具按钮语言完全统一
- 满足“顶部图标按钮栏不要文字”的硬约束
- 减轻 header 宽度压力

缺点：

- 首次可发现性略低于文字按钮

#### 方案 S3B：icon 内嵌 dropdown 提示

优点：

- 更明确表达“这是菜单入口”

缺点：

- 在 `28px` 档位中容易显得拥挤
- 需要额外自绘复合图形

最终选择：`S3A`

### 设计点 4：`Create` 菜单锚点与几何语言

#### 方案 S4A：保持当前向下展开，只修 item 样式

优点：

- 实现成本最低

缺点：

- 菜单仍容易“飘”向主工作区
- 触发器与菜单之间缺乏统一几何语言
- 只能治表，不治根

#### 方案 S4B：根窗口 popup，按 trigger 右缘或 sidebar 内边距对齐，统一宽度和行布局

优点：

- 几何关系更稳定
- 菜单更像侧栏自身控件，不像飘出的独立窗
- 可以统一 item 的 icon 列、text baseline、padding、菜单宽度

缺点：

- 要同时调整 anchor 策略与 menu 内容布局

#### 方案 S4C：退回 sidebar 内部 overlay

优点：

- containment 直觉最强

缺点：

- 会重新碰到 clip、z-order、click-away 和宿主约束问题
- 与当前 root popup 路线冲突

最终选择：`S4B`

### 设计点 5：`AssetsSidebar` 宽度

#### 方案 S5A：从 `256px` 提升到 `272px`

优点：

- 对整体 layout 契约冲击最小
- 几乎不影响现有窗口阈值

缺点：

- 体感提升有限

#### 方案 S5B：从 `256px` 提升到 `288px`

优点：

- header 与未来树节点都更舒展
- 能明显缓解当前侧栏偏窄的问题
- 为 anchored popup 留出更自然的视觉边界

缺点：

- `FULL_LAYOUT_MIN_WIDTH` 会同步增大
- 窄窗口下右侧面板更早退出

#### 方案 S5C：改为可拖拽宽度

优点：

- 长期体验最好

缺点：

- 已超出当前 bugfix 范围

最终选择：`S5B`

## 最终决策

本轮确认的设计组合为：

- `S1B`: Search 采用 anchored search popup，而不是内联展开行
- `S2B`: Search 为空时 outside click 自动关闭；非空时保持展开
- `S3A`: `Create` 改为纯 icon trigger，不保留文字
- `S4B`: `Create` 菜单继续使用根窗口 popup，但按 trigger 右缘或 sidebar 内边距对齐，并重做菜单宽度与行布局
- `S5B`: `AssetsSidebar` 宽度由 `256px` 调整为 `288px`

## 最终设计

### 1. Search

- Search button 继续位于 header 工具区
- 点击后不再挤出一行 `TextInput`，而是在 header 下方打开 anchored popup
- popup 视觉目标参考竞品：更聚焦、更贴边、更像“当前面板的即时搜索器”
- popup 打开后应自动 focus 到输入框
- 点击外部区域时：
  - 若 `query == ""`，关闭 popup
  - 若 `query != ""`，保持 popup 打开，只丢失焦点，不清空 query
- 后续可预留 `Esc` 行为：
  - 空值时关闭
  - 非空时先清空，再次 `Esc` 才关闭

### 2. `Create` Trigger

- `Create` 不再显示文字，只保留纯 icon button
- trigger 应与 Search / Tree / View 三个按钮保持同一尺寸体系
- 可通过 tooltip 提供文案 `Create`
- 不再使用“按钮本体承担品牌性 CTA”的思路，而是回归工具区动作入口

### 3. `Create` Menu

- 菜单宿主继续保留在根窗口 [app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint)
- 锚点不再只做“按钮左上角向下偏移”，而应改为：
  - 以 trigger 的右边缘对齐菜单右边缘，或
  - 以 sidebar 右内边距为约束，确保菜单不会视觉上漂向主工作区
- 菜单宽度应明显大于当前 icon trigger，但不能显得臃肿
- 菜单项采用统一结构：
  - 固定 leading icon 列
  - 固定文字列起始线
  - 固定行高
  - 固定左右 padding
- 首轮菜单项仍仅保留：
  - `New Folder`
  - `New SSH Connection`

### 4. Sidebar 宽度

- `AssetsSidebar` 从 `256px` 提升至 `288px`
- 这是本轮唯一的 layout budget 调整
- 对应的 Rust metrics 与 layout threshold 需要同步更新，避免 Slint 与 Rust 契约分叉

### 5. 统一 overlay 方向

- Search popup 与 `Create` menu 都应采用“根窗口锚定 overlay”体系
- 两者不必长得完全一样，但需要共享一致的几何规则：
  - 都从 header 区域出发
  - 都遵守 sidebar 的视觉边界
  - 都具备明确的 click-away 语义

## 实施步骤

1. 在 `AssetsSidebar` 中移除内联 search row 方案，改为导出 search anchor 几何。
2. 在 `AppWindow` 根层新增或接管 search popup 宿主，并与 `ShellViewModel` 的搜索状态联动。
3. 明确 Search popup 的关闭状态机，实现 `S2B` 规则。
4. 将 `Create` 触发器改为纯 icon button，并补 tooltip 文案。
5. 重做 `AssetsCreateMenu` 的锚点策略、菜单宽度、item 对齐方式。
6. 将 `AssetsSidebar` 宽度与相关 metrics/layout threshold 调整到 `288px`。
7. 补充或修订 UI contract / state smoke 测试，覆盖 search popup、create menu 和宽度契约。

## 风险与回滚

### 风险 1：Search popup 引入后，状态机比当前内联行更复杂

影响：

- 需要额外区分 `open`、`focused`、`query empty/non-empty`

缓解：

- 业务状态仍由 Rust `ShellViewModel` 持有
- UI 层只负责展示与本地焦点事件

回滚：

- 若 popup 行为在某个平台出现明显问题，可暂时保留 popup 外观，但降低规则为“只由按钮二次点击和 `Esc` 关闭”

### 风险 2：`Create` 菜单右缘对齐后，极窄窗口下可能贴近主工作区边界

影响：

- 某些窗口尺寸下菜单边界可能显得过于靠外

缓解：

- 用 sidebar 内边距作为二级约束，而不是只信任 trigger 右缘

回滚：

- 若右缘对齐在极端尺寸下表现不好，可退为“trigger 左缘 + 固定菜单宽度”，但保留统一 item 布局

### 风险 3：`AssetsSidebar` 加宽后改变整体 layout 阈值

影响：

- `FULL_LAYOUT_MIN_WIDTH` 与相关测试需要同步更新

缓解：

- 宽度调整只做一档，从 `256px` 到 `288px`
- 同步修正 [metrics.rs](/home/wwwroot/mica-term/src/shell/metrics.rs) 与 [layout.rs](/home/wwwroot/mica-term/src/shell/layout.rs)

回滚：

- 如果实测发现对窄窗口影响过大，可降回 `272px`，但不回退到 `256px`

## 验证清单

- Search 不再以内联行方式出现在 `AssetsSidebar` 中
- Search 点击后以 anchored popup 形式出现，视觉方向接近参考图
- Search 在 `query == ""` 时 outside click 会关闭
- Search 在 `query != ""` 时 outside click 不会强制关闭
- `Create` trigger 不再显示文字
- `Create` trigger 与其他 toolbar button 采用统一 icon-button 尺寸
- `Create` 菜单不再显得过窄或图文错位
- `Create` 菜单锚点不再视觉漂向主工作区
- `AssetsSidebar` 宽度契约已从 `256px` 调整为 `288px`
- Rust metrics、layout threshold、Slint 宽度声明三者保持一致
- 不触碰 terminal runtime、renderer 和 SSH/SFTP 业务逻辑

## 备注

本设计文档是对 `AssetsSidebar` 顶部工具区当前 bugfix 轮次的最终收敛。若后续需要进入实现阶段，应在本文件基础上另写 `implementation-plan`，而不是再回到早上那份首版 toolbar 设计文档继续追加。
