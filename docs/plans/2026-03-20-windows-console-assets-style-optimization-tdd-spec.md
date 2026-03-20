# Windows Console Assets Style Optimization TDD Spec

日期：2026-03-20
状态：已完成代码落地，供下一阶段 `test-driven-development` 使用

## 1. 目标与范围

本轮已经完成 `Windows Console` 资产区的 5 个实现任务，范围集中在：

- toolbar 语义与稳定宽度契约
- explorer 单行高密度 row 契约
- create modal 英文化与打开后 focus 调度
- context menu hover / corridor 稳定性
- explorer / menu 专用视觉对比度 token

本规范只总结已经落地的结构真源、桥接接口、Slint callbacks 与后续测试重点，不重新设计 terminal core、SSH runtime、SFTP、持久化或 renderer。

## 2. 核心 Rust 结构与状态真源

### 2.1 `AssetsToolbarDescriptor`

文件：`src/shell/sidebar.rs`

当前关键字段：

- `show_tree_controls: bool`
- `tree_controls_enabled: bool`
- `tree_expansion_tooltip: &'static str`
- `view_mode_tooltip: &'static str`

约束：

- `Console` 目的地下，`show_tree_controls` 在 `Tree / Flat` 两种模式都保持 `true`
- 只有 `Tree` 模式时 `tree_controls_enabled == true`
- `Flat` 模式时 `tree_expansion_tooltip == "Switch to Tree View to expand folders"`

建议测试：

- descriptor 纯单元测试覆盖 `Tree / Flat` 两套分支
- tooltip 文案测试不要只测布尔值，必须同时测 explanatory copy

### 2.2 `ShellViewModel`

文件：`src/shell/view_model.rs`

本轮直接相关的状态：

- `asset_view_mode`
- `asset_tree_fully_expanded`
- `asset_modal_state`
- `context_menu_open`
- `context_menu_open_path`
- `context_menu_origin_x / context_menu_origin_y`
- `context_menu_child_flows_left`
- `context_menu_feedback_text`

新增/关键方法：

- `truncate_context_menu_open_path(len)`
- `toggle_asset_view_mode()`
- `toggle_asset_tree_expansion()`
- `open_new_folder_modal(...)`
- `open_new_ssh_modal(...)`
- `handle_context_menu_leaf_action(...)`

约束：

- `Flat` 模式下调用 `toggle_asset_tree_expansion()` 不应改变树展开状态
- `context_menu_open_path` 只表示结构性 submenu 展开路径，不再承载所有瞬时 hover 态
- modal 的草稿与 active tab 仍由 `ShellViewModel` 真源维护，UI 只做渲染与输入回传

建议测试：

- `context_menu_open_path` 的收缩与保留必须分开覆盖一级、二级 submenu
- modal reopen 时必须验证 `ShellViewModel -> AppWindow` 的 reset 语义

### 2.3 `ContextMenuActionNode` / `Rect`

文件：`src/shell/context_menu.rs`

关键函数：

- `visible_columns_for_path(...)`
- `resolve_root_menu_origin(...)`
- `should_keep_corridor_open(...)`
- `context_menu_column_offset(...)`

约束：

- `should_keep_corridor_open(...)` 负责 corridor keep-open 的几何判断
- `context_menu_column_offset(...)` 负责将 column index 映射到 overlay 内的实际 x 偏移
- `Rect` 计算必须与 `flow-left` / `flow-right` 两种排列保持一致

建议测试：

- corridor 测试必须同时覆盖 `keep-open == true` 与 `keep-open == false`
- 后续若加入三级 submenu，建议新增 `flow-left` 多列 corridor 用例

## 3. Slint 数据桥接与 UI 模型

### 3.1 `ConsoleAssetItem`

文件：`ui/shell/assets-sidebar.slint`

关键字段：

- `depth`
- `path_hint`
- `show_disclosure`
- `compact_flat_mode`
- `focused`
- `renaming`

约束：

- `compact_flat_mode` 仍由 Rust bridge 提供，Slint 不自行推断
- `path_hint` 在 `Flat` 模式下仍显示，但只占单行尾部弱提示区域
- `AssetNodeRow` 单行高度固定为 `28px`

### 3.2 `AssetsContextMenuItem`

文件：`ui/components/assets-context-menu-column.slint`

本轮新增字段：

- `open: bool`

用途：

- 让已经打开子菜单的父项保持稳定高亮
- 将“结构已展开”与“鼠标瞬时 hover”分离

建议测试：

- 后续组件测试应分别断言 `visual-hover` 与 `open` 两种视觉状态

## 4. 未新增 trait 接口的说明

本轮没有新增 Rust trait 接口。

保持不变的接口：

- `PlatformWindowEffects`

说明：

- 本轮改动集中在 `ShellViewModel -> bootstrap -> Slint` 的桥接链路
- 焦点调度、context menu corridor、toolbar descriptor 都通过已有函数式桥接完成，没有引入新的 trait 抽象层

## 5. 关键 Slint callbacks / 事件通道

### 5.1 Toolbar / Explorer

文件：`ui/app-window.slint`、`ui/shell/sidebar.slint`、`ui/shell/assets-sidebar.slint`

关键 callbacks：

- `toggle-assets-view-mode-requested()`
- `toggle-assets-tree-expansion-requested()`
- `assets-create-action-selected(string)`
- `asset-selected(string)`
- `toggle-expanded-requested(string)`

测试重点：

- `Flat -> Tree` 来回切换后，tree controls 的 visible/enabled 契约必须稳定
- explorer 单行 row 的点击、重命名、右键行为不能因行高变化失效

### 5.2 Modal

文件：`ui/app-window.slint`

关键 callbacks：

- `close-asset-modal-requested()`
- `confirm-asset-modal-requested()`
- `asset-folder-modal-name-changed(string)`
- `asset-ssh-modal-tab-selected(string)`
- `asset-ssh-modal-draft-changed(string, string)`

关键属性桥接：

- `asset-modal-open`
- `asset-modal-kind`
- `asset-modal-focus-sequence`

测试重点：

- reopen 后 `asset-ssh-modal-active-tab` 必须回到 `standard`
- `focus-sequence` 变化后应触发对应 modal 的首字段聚焦

### 5.3 Context Menu

文件：`ui/app-window.slint`、`ui/components/assets-context-menu-overlay.slint`

关键 callbacks：

- `assets-context-menu-action-invoked(string)`
- `assets-context-menu-row-hovered(int, int)`
- `assets-context-menu-pointer-moved(length, length)`
- `close-assets-context-menu-requested()`
- `assets-context-menu-key-pressed(string)`

测试重点：

- 叶子项 hover 不应立即重写 Rust `open_path`
- 带 children 的行 hover 才能触发结构展开
- pointer 在 parent / child column corridor 中移动时，不应过早关闭 submenu

## 6. 视觉 token 契约

文件：`ui/theme/tokens.slint`

新增 token：

- `explorer-row-hover-surface`
- `explorer-row-selected-surface`
- `menu-row-hover-surface`
- `menu-row-open-surface`

约束：

- `AssetNodeRow` 不再直接使用 `ThemeTokens.control-hover-surface`
- `AssetsContextMenuRow` 不再直接使用 `ThemeTokens.control-hover-surface`
- activity rail 和 toolbar icon button 仍留在 generic control token 体系

建议测试：

- shell smoke 脚本继续保留“禁止回退到 generic token”的负向断言
- 后续若引入截图测试，应分别覆盖 dark / light 两套主题

## 7. 重点边缘情况（Edge Cases）

### 7.1 `invoke_from_event_loop` 焦点调度

- `schedule_asset_modal_focus(...)` 是异步调度
- 如果 modal 已经在事件循环回调触发前关闭，必须依赖 `asset_modal_open` 守卫跳过 focus
- 后续若改成多 modal 并行，必须重新审视这个 guard

### 7.2 `path_hint` 与长 label 截断

- 当前 `path_hint` 固定为尾部弱化区域
- label 与 hint 都较长时可能出现视觉拥挤
- 下一轮测试建议增加超长 folder / ssh 名称的 UI 合约用例

### 7.3 submenu corridor 与 flow-left

- 当前 corridor 逻辑已经接入，但主要覆盖 `flow-right` 直觉路径
- 当 root menu 因靠近右边界而左翻时，建议追加更细的 pointer 轨迹测试

### 7.4 视觉 hover 与结构 open_path 脱耦

- Slint 的 `visual-hover` 现在只是视觉态
- Rust `context_menu_open_path` 只表示结构展开态
- 后续测试若只验证 UI 颜色、不验证 `open_path`，容易漏掉结构回退错误

### 7.5 `planned` action 的反馈路径

- `planned` action 仍保持可点击，但不会关闭菜单
- 它会更新 `context_menu_feedback_text`
- 后续测试应持续保证 planned action 不会误触发 leaf action 关闭路径

## 8. 建议的下一阶段 TDD 任务

1. 为 `focus-sequence` 增加更细粒度的 bridge-level 回归测试，验证 folder / ssh 两个 modal 都能在 reopen 后再次触发 focus。
2. 为 `flow-left` submenu 增加 corridor 几何测试，覆盖右侧边缘打开菜单后的 keep-open 路径。
3. 为 explorer/menu dark/light 主题增加截图或像素级快照测试，避免 token 被后续回退。
4. 为超长 `label + path_hint` 增加布局 smoke，确认单行契约下的截断和对齐仍稳定。
5. 在真实 Windows 11 GUI 环境补人工验收，重点验证 hover 对比度、submenu 横移容错与 modal focus 观感。

## 9. 交接结论

本轮代码落地已经把 `Windows Console` 资产区的结构桥接、样式契约和交互稳定性收敛到一套更清晰的分层：

- Rust 继续负责结构真源与几何决策
- Slint 负责本地视觉 hover 与单行高密度呈现
- modal focus 通过事件循环调度桥接，而不是直接依赖条件 overlay id

下一阶段如果进入更严格的 `test-driven-development`，应优先围绕：

- Rust 结构状态是否被错误回退
- Slint 视觉态是否再次与结构态耦合
- dark / light 主题对比度是否被后续样式改动削弱
