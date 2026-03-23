# Windows Console Assets Explorer TDD Spec

日期: 2026-03-23
阶段: implementation complete -> TDD handoff
状态: 当前计划任务已完成，后续进入增量测试与桌面 GUI 验证

## 来源文档

- Design: `docs/plans/2026-03-20-windows-console-assets-explorer-design.md`
- Implementation Plan: `docs/plans/2026-03-20-windows-console-assets-explorer-implementation-plan.md`
- Verification Record: `verification.md`

## 范围摘要

本轮已把 `Windows Console` 资产 Explorer 的核心交互从旧的 inline rename 路径收敛到统一 modal workflow，并补齐：

- disclosure `none / collapsed / expanded` 三态投影
- 同父级统一唯一命名校验
- rename modal 输入校验与 confirm gating
- folder 递归删除确认与删除后 focus fallback
- Slint explorer row 契约清理，移除旧 inline rename UI contract

当前仍然保持 `Rust state -> bootstrap bridge -> Slint shell` 的单向真相源模型，UI 不持有独立业务状态。

## 核心 Struct 与状态

### Rust domain

- `src/shell/assets.rs`
  - `AssetDisclosureState`
    - `None`
    - `Collapsed`
    - `Expanded`
  - `AssetNameValidation`
    - `Valid`
    - `Empty`
    - `Duplicate`
  - `VisibleAssetRow`
    - `id`
    - `kind`
    - `label`
    - `depth`
    - `has_children`
    - `expanded`
    - `disclosure_state`
    - `path_hint`
  - `RemovedAssetSummary`
    - `removed_ids`
    - `descendant_count`
  - `AssetTree`
    - 负责 canonical tree、projection、同父级唯一命名、默认命名和 subtree remove

- `src/shell/view_model.rs`
  - `AssetModalState`
    - `NewFolder`
    - `NewSshConnection`
    - `RenameAsset`
    - `DeleteAssetConfirm`
  - `ShellViewModel`
    - 负责 selection、focus、context target、modal state、toolbar/search state
    - 负责把 `AssetTree` 领域操作组合成可投影 UI 状态

### Slint window model

- `ui/shell/assets-sidebar.slint`
  - `ConsoleAssetItem`
    - `id`
    - `kind`
    - `label`
    - `depth`
    - `has_children`
    - `expanded`
    - `selected`
    - `focused`
    - `disclosure_state`
    - `path_hint`
    - `compact_flat_mode`

- `ui/app-window.slint`
  - `asset-rename-modal-open`
  - `asset-rename-modal-name`
  - `asset-rename-modal-validation-message`
  - `asset-rename-modal-can-confirm`
  - `asset-delete-confirm-modal-open`
  - `asset-delete-confirm-target-label`
  - `asset-delete-confirm-descendant-count`
  - `asset-modal-focus-sequence`

### Bootstrap bridge

- `src/app/bootstrap.rs`
  - `sync_console_assets()`
    - 把 `VisibleAssetRow` 投影成 `ConsoleAssetItem`
    - 在 bridge 层把 `AssetDisclosureState::{None, Collapsed, Expanded}` 映射为 Slint `string`
  - `sync_asset_modal_state()`
    - 负责 `AssetModalState` 到 `AppWindow` modal properties 的唯一投影
    - 保证 create / rename / delete 三条 modal 路径不会同时处于激活状态
  - `slint::invoke_from_event_loop(...)`
    - 用于 modal focus sequence 等需要回到 UI 线程的刷新点

## Trait 与接口契约

本轮没有新增 trait。接口契约主要通过 Rust domain API、view-model 方法和 Slint callback/property 体现。

### Domain / view-model 关键接口

- `AssetTree::validate_name_in_parent(parent_id, candidate, exclude_id) -> AssetNameValidation`
- `AssetTree::next_default_name_from_base(base, siblings) -> String`
- `AssetTree::descendant_count(node_id) -> Option<usize>`
- `AssetTree::remove_subtree(node_id) -> Option<RemovedAssetSummary>`
- `ShellViewModel::open_rename_asset_modal(asset_id)`
- `ShellViewModel::update_rename_asset_modal_name(value)`
- `ShellViewModel::asset_rename_modal_validation_message() -> String`
- `ShellViewModel::open_delete_asset_confirm(asset_id)`
- `ShellViewModel::confirm_delete_asset()`
- `ShellViewModel::confirm_asset_modal()`
- `ShellViewModel::cancel_asset_modal()`

### 接口前后条件

- `AssetTree::validate_name_in_parent(...)`
  - 输入会先 `trim()`
  - `exclude_id` 用于 rename 原值豁免
  - 只在同父节点作用域内检查，不跨父节点扩散
- `AssetTree::remove_subtree(...)`
  - 返回值中的 `removed_ids` 必须覆盖根节点与全部后代
  - `descendant_count` 不包含删除根本身，只统计 nested descendants
- `ShellViewModel::open_rename_asset_modal(...)`
  - 仅在目标资产存在时进入 `RenameAsset`
  - 会同步更新 `focused_asset_id`、`selected_asset_ids`、`context_target_asset_id`
- `ShellViewModel::confirm_delete_asset()`
  - 仅当 `AssetModalState` 为 `DeleteAssetConfirm` 时执行删除
  - 删除成功后必须同时清理 selection / focus / context target / modal state

### UI contract 关键约束

- `AssetNodeRow` 只负责：
  - selection
  - disclosure toggle
  - context-menu request
  - row metadata rendering
- `AssetNodeRow` 不再负责：
  - inline rename input
  - rename draft 保存
  - rename commit/cancel callback
- `AssetsSidebar` / `Sidebar` 不再透传 inline rename callbacks
- `AppWindow` 不再暴露：
  - `asset-rename-active`
  - `asset-rename-text-changed(...)`
  - `asset-rename-commit-requested(...)`
  - `asset-rename-cancel-requested(...)`
  - `dismiss-active-asset-rename-requested()`

## Slint callbacks / global state / bindings

### Rename modal

- `ui/components/assets-rename-modal.slint`
  - properties:
    - `item-name`
    - `validation-message`
    - `can-confirm`
  - callbacks:
    - `name-changed(string)`
    - `confirm-requested()`
    - `close-requested()`

### Delete confirm modal

- `ui/components/assets-delete-confirm-modal.slint`
  - properties:
    - `target-label`
    - `descendant-count`
  - callbacks:
    - `confirm-requested()`
    - `close-requested()`

### AppWindow bridge

- `asset-rename-modal-name-changed(string)`
- `confirm-asset-rename-requested()`
- `confirm-delete-asset-requested()`
- `close-asset-modal-requested()`

### Explorer row binding

- `AssetNodeRow`
  - `disclosure-state: "none" | "collapsed" | "expanded"`
  - `selected-requested(string)`
  - `toggle-expanded-requested(string)`
  - `context-menu-requested(string, string, length, length)`
- `ConsoleAssetItem.disclosure_state`
  - 必须只来源于 Rust projection，UI 不自行推导
- `chevron-right-icon` / `chevron-down-icon`
  - 仅由 `disclosure-state` 选择，不再依赖旧 `show_disclosure + expanded` 布尔组合

### Global state 说明

- 本轮没有新增 Slint `global` singleton。
- 窗口级状态集中挂在 `AppWindow` property 上，由 `src/app/bootstrap.rs` 统一同步。
- `ConsoleAssetItem` 列表仍通过 `ModelRc<VecModel<...>>` 从 Rust 投影到 Slint。

## Tokio / channel / actor 交互关系

本轮没有新增 Tokio task、channel 或 actor。当前实现仍是同步 shell 状态流：

- Slint callback 进入 `bootstrap`
- `bootstrap` 直接调用 `ShellViewModel`
- `ShellViewModel` 修改 `AssetTree` / modal state
- `bootstrap` 再调用 `sync_console_assets()`、`sync_asset_modal_state()` 回写窗口 property

与 UI 线程相关的唯一约束仍是：

- 窗口焦点序列和异步回调若跨线程进入 UI，必须使用 `slint::invoke_from_event_loop`
- 后续若真实 SSH/SFTP runtime 通过 Tokio 推送树变更，不能直接跨线程写 Slint model，必须先切回 UI 线程

## 状态流转说明

### Rename

1. context menu `rename-asset` 叶子动作进入 `ShellViewModel::open_rename_asset_modal`
2. `AssetModalState` 切到 `RenameAsset`
3. `bootstrap` 通过 `sync_asset_modal_state()` 投影 rename modal props
4. `AssetsRenameModal` 触发 `name-changed`
5. `bootstrap` 调用 `update_rename_asset_modal_name`
6. view-model 根据 `AssetNameValidation` 更新 validation message 和 `can_confirm`
7. `confirm-asset-rename-requested()` 触发后，view-model 提交 rename 并清空 modal state
8. `sync_console_assets()` 刷新 row label / selection / focus

### Delete

1. context menu `delete-asset` 动作进入 `ShellViewModel::open_delete_asset_confirm`
2. view-model 计算 `descendant_count`
3. `bootstrap` 投影 delete confirm modal
4. 用户确认后触发 `confirm-delete-asset-requested()`
5. `ShellViewModel::confirm_delete_asset()` 调用 `AssetTree::remove_subtree`
6. 删除结果通过 `RemovedAssetSummary` 回传
7. view-model 根据剩余兄弟 / 父节点执行 focus fallback
8. `sync_console_assets()` 回写最新列表

### Disclosure projection

1. `AssetTree::visible_rows()` 生成 `VisibleAssetRow`
2. `VisibleAssetRow.disclosure_state` 表达 `none / collapsed / expanded`
3. `sync_console_assets()` 把 enum 映射到 `ConsoleAssetItem.disclosure_state`
4. `AssetNodeRow` 依据 `disclosure-state` 选择：
   - `none`: 不渲染箭头，但保留占位
   - `collapsed`: 渲染右箭头
   - `expanded`: 渲染下箭头

## 关键错误处理策略

- 名称为空：
  - `AssetNameValidation::Empty`
  - confirm disabled
- 同父级重名：
  - `AssetNameValidation::Duplicate`
  - rename/create 都走同一校验语义
- rename 为原值：
  - 视为有效，不报 duplicate
- 删除目标不存在：
  - `open_delete_asset_confirm` / `confirm_delete_asset` 直接 return，不制造脏状态
- rename/delete modal 关闭：
  - 统一走 `close-asset-modal-requested()` 清理 `asset_modal_state`
- modal 投影互斥：
  - `sync_asset_modal_state()` 对非当前 modal 路径统一写回 empty/false，避免双路径残留
- UI row disclosure：
  - `disclosure_state == "none"` 时仍保留固定占位，避免对齐跳动

## 潜在边缘情况

- Tokio channel 阻塞或消息堆积
  - 本轮未引入 channel，但后续若 runtime 通过 channel 推送资产树变更，需要限制缓冲与丢弃策略，避免 UI 长时间消费旧快照。
- UI 线程更新时机不正确
  - 若未来异步来源直接改 Slint property 而未通过 `slint::invoke_from_event_loop`，会导致 UI 线程违规更新。
- 数据竞争或共享状态不一致
  - 当前主状态集中在 `Rc<RefCell<ShellViewModel>>`，未来若混入后台任务共享资产树，必须避免多处真相源并发写入。
- 资源释放时序问题
  - 若窗口销毁后仍持有旧 `Weak<AppWindow>` 回调，需要在回调边界安全 unwrap 或提前停止源任务。
- 异步任务取消或界面关闭后的悬挂回调
  - 未来若 rename/delete 进入真正异步持久化或网络层，必须在完成回调前确认窗口仍存活、modal 仍属于当前会话。
- Slint model 更新与实际数据源不同步
  - `ConsoleAssetItem` 必须始终从 `visible_console_asset_rows()` 重新投影，不能在 UI 本地增量修改 row 状态。
- focus fallback 边界
  - 删除最后一个 root row 时应清空 `selection/focus`，不能残留已删除 id。
- disclosure 显示边界
  - flat mode、leaf ssh row、空 children folder 不应错误显示 disclosure toggle。
- modal close 与 stale target
  - rename/delete 关闭后，不能让旧 `context_target_asset_id` 残留影响下一次 create/rename/delete。

## 后续测试建议

### 单元测试

- `AssetTree::remove_subtree()` 更深层级矩阵
- `validate_name_in_parent()` 覆盖更多 base/suffix 组合
- disclosure projection 在 tree/flat/search 三种视图下的稳定性

### 集成测试

- `ShellViewModel` 上下文菜单 -> modal -> confirm -> projection 全链路矩阵
- rename 与 delete 在连续操作下的 selection/focus 演化
- create/rename/delete 混合执行后 `context_target_asset_id` 清理
- `sync_asset_modal_state()` 针对 `NewFolder / NewSshConnection / RenameAsset / DeleteAssetConfirm / None` 五种分支的 property 互斥投影

### UI 交互测试

- rename modal 输入错误态、禁用确认按钮、Escape/Enter 键盘行为
- delete confirm modal 文案在空 folder / 非空 folder / ssh row 三种场景的差异
- disclosure 图标在 Win11 实机上的对齐、hover、pressed 反馈
- context menu 打开 modal 后 overlay stacking 与 click-outside close 行为

### 手工 GUI 验证

- Windows 11 上实际检查 rename/delete modal 焦点、键盘提交、关闭顺序
- 检查删除后 row 高亮与 focus ring 的视觉回落
- 检查右箭头 / 下箭头在 Fluent 风格下的视觉密度与颜色表现
