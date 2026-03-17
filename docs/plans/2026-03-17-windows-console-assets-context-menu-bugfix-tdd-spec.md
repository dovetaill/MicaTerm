# Windows Console Assets Context Menu Bugfix TDD Spec

Date: 2026-03-17
Status: Ready for `test-driven-development`

## Scope

本轮实现已经完成以下壳层能力：

- `Windows Console` 资产区默认从空态启动，不再注入 demo 资产
- blank-area 右键在 empty state 与列表尾部空白区都可用
- 第一版 create IA 只保留 `New Folder` 与 `New SSH Connection`
- toolbar / context menu create action 会创建 placeholder 资产
- placeholder 资产创建后立即进入 inline rename
- rename draft / commit / cancel 已打通 `Slint -> Rust view model -> Slint` 双向桥接

本轮仍未覆盖：

- 真实 SSH 连接配置、host / port / auth 表单
- 持久化、数据库 schema、runtime actor / Tokio channel
- terminal session lifecycle 与 SFTP 逻辑

## Core Rust State

### `src/shell/assets.rs`

- `ConsoleAssetKind`
  - `from_create_action_id(&str) -> Option<Self>`
  - `placeholder_label(self) -> &'static str`
- `MockConsoleAssetItem`
  - 当前仍是壳层资产真源，字段为 `id` / `kind` / `label`

### `src/shell/view_model.rs`

关键状态：

- `console_asset_items: Vec<MockConsoleAssetItem>`
- `selected_asset_ids: Vec<String>`
- `renaming_asset_id: Option<String>`
- `renaming_asset_text: String`
- `next_console_asset_serial: u64`
- `context_menu_open`, `context_menu_target_kind`, `context_menu_open_path`

关键方法：

- `handle_assets_create_action(&mut self, action_id: &str)`
- `handle_context_menu_leaf_action(&mut self, action_id: &str)`
- `update_asset_rename_draft(&mut self, asset_id: &str, text: String)`
- `commit_asset_rename(&mut self, asset_id: &str, text: String)`
- `cancel_asset_rename(&mut self, asset_id: &str)`

当前行为契约：

- `new-folder` -> 创建 `ConsoleAssetKind::Folder`
- `new-ssh-connection` -> 创建 `ConsoleAssetKind::SshConnection`
- 新建后自动：
  - 追加 `draft-asset-{serial}`
  - 选中新资产
  - 写入 `renaming_asset_id`
  - 写入 `renaming_asset_text`
- rename commit 只更新内存 `label`
- rename cancel 不删除 placeholder，只退出编辑态

### `src/shell/context_menu.rs`

blank-area 场景最小 IA：

- `new-folder`
- `new-ssh-connection`

SSH / Folder 场景当前仍保留 planned action 路径，但 create 分支已改为平铺的最小集合。

## Slint Data Contract

### `ui/shell/assets-sidebar.slint`

`ConsoleAssetItem` 当前字段：

- `id: string`
- `kind: string`
- `label: string`
- `selected: bool`
- `renaming: bool`
- `rename_text: string`

新增 callback：

- `asset-rename-text-changed(string, string)`
- `asset-rename-commit-requested(string, string)`
- `asset-rename-cancel-requested(string)`

blank-area 命中层：

- `empty-state-context-touch`
- `list-blank-fill-context-touch`

### `ui/components/asset-node-row.slint`

新增输入契约：

- `renaming: bool`
- `rename-text: string`

新增 callback：

- `rename-text-changed(string, string)`
- `rename-commit-requested(string, string)`
- `rename-cancel-requested(string)`

当前编辑态行为：

- `TextInput.edited` -> draft 更新
- `Key.Return` -> commit
- `Key.Escape` -> cancel
- `changed has-focus` 丢失焦点 -> commit
- `TouchArea` 在 `renaming` 期间禁用，避免编辑态吞掉输入

### `ui/shell/sidebar.slint` / `ui/app-window.slint`

rename callback 已逐层透传：

- `AssetsSidebar -> Sidebar -> AppWindow`
- `AppWindow` 暴露：
  - `asset-rename-text-changed(string, string)`
  - `asset-rename-commit-requested(string, string)`
  - `asset-rename-cancel-requested(string)`

## Bootstrap Bridge

### `src/app/bootstrap.rs`

当前桥接职责：

- `console_asset_items_for(state)` 负责把 Rust state 投影为 Slint `ConsoleAssetItem`
- rename 投影逻辑：
  - 当前行 `id == renaming_asset_id` 时，`renaming = true`
  - 当前行 `rename_text = renaming_asset_text`
  - 其他行 `renaming = false`
- 已绑定 callback：
  - `on_assets_create_action_selected`
  - `on_asset_rename_text_changed`
  - `on_asset_rename_commit_requested`
  - `on_asset_rename_cancel_requested`
  - `on_asset_context_menu_requested`
  - `on_assets_context_menu_action_invoked`

## Existing Automated Coverage

- `tests/assets_context_menu_spec.rs`
  - blank-area 最小 IA
  - flat menu columns
  - planned SSH action feedback
  - menu placement / corridor geometry
- `tests/assets_context_menu_smoke.rs`
  - bootstrap 空态
  - blank-area 右键
  - create action -> placeholder 投影
  - rename callback round-trip
- `tests/shell_view_model.rs`
  - create action -> placeholder + rename state
  - rename draft / commit / cancel
- `tests/assets_context_menu_ui_contract_smoke.sh`
  - empty state copy
  - blank-area touch targets
  - rename callback / `TextInput` 契约

## Recommended Next TDD Focus

下一阶段建议从以下行为继续补测试：

1. 重命名空字符串、纯空白字符串时的处理策略
2. 重命名过程中再次触发 create action 的状态覆盖策略
3. 多个 placeholder 连续创建时的 serial / selection 稳定性
4. 焦点切换导致的重复 commit 是否会产生多次状态写入
5. row right-click 与 blank-area fill right-click 在长列表下是否存在误命中

## Edge Cases And Risks

### UI / Interaction

- `blank-area` 命中层若布局回归，可能重新抢占 row 事件
- `TextInput` 失焦自动 commit 可能与 `Return` commit 形成重复提交路径
- 如果 `renaming_asset_id` 指向已不存在的项，UI 会退出显示但 Rust state 可能保留脏编辑态

### Concurrency / Async

- 当前实现未引入 Tokio task、channel 或 actor mailbox，因此不存在本轮新增的 channel blocking / data race 面风险
- 若下一阶段把 rename / create 接入异步持久化，必须重点测试：
  - UI optimistic state 与异步回写顺序竞争
  - 取消编辑后异步回调反向覆盖 UI
  - 主线程 Slint 更新是否始终通过 UI-safe bridge 进入

### Data Integrity

- 当前 placeholder id 使用 `draft-asset-{serial}`，仅在进程内唯一，不具备持久化语义
- rename cancel 只退出编辑态，不回滚 placeholder 创建；如果未来引入“未命名项自动清理”，需要新增明确测试

## Suggested Entry Commands For Next Phase

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test shell_view_model -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo check --workspace
cargo clippy --workspace -- -D warnings
```
