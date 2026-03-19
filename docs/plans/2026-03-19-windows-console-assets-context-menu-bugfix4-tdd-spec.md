# Windows Console Assets Context Menu Bugfix4 TDD Spec

日期: 2026-03-19
阶段: implementation -> test-driven-development handoff
状态: 自动化验证已通过，待进入下一轮测试补强

## 范围摘要

本轮已经把 `Windows Console` 资产区从“placeholder + inline rename”的创建路径，收敛为更接近 Explorer 的 modal 驱动流程：

- toolbar `Create` 与 context menu `Create` 都不再直接插入 placeholder 节点；
- `New Folder` 改为独立小型 modal，`confirm` 后才写入 `AssetTree`；
- `New SSH Connection` 改为独立大型 modal，带 tab 状态与 draft 字段；
- `Tree` 渲染宿主改为 `ListView`，行模型新增 `path_hint / show_disclosure / compact_flat_mode`；
- `Flat` 重新定义为“只显示 SSH 行”，并通过 `path_hint` 保留目录上下文；
- `AssetNodeRow` 已拆开 disclosure hit target 与整行 body hit target，避免点击冲突。

下一轮 TDD 不应再回到 placeholder create 语义，而应围绕 modal 状态机、row hit-test、Flat 搜索与未来异步接入边界继续补强。

## 核心 Struct 与状态

### Rust domain

- `src/shell/assets.rs`
  - `AssetNode`
    - `id: String`
    - `kind: ConsoleAssetKind`
    - `title: String`
    - `parent_id: Option<String>`
    - `children: Vec<String>`
    - `expanded: bool`
  - `VisibleAssetRow`
    - `id: String`
    - `kind: ConsoleAssetKind`
    - `label: String`
    - `depth: usize`
    - `has_children: bool`
    - `expanded: bool`
    - `path_hint: Option<String>`
    - `show_disclosure: bool`
  - `AssetTree`
    - canonical tree 真源；
    - 负责 root / child 插入、展开状态、`Tree` / `Flat` 投影、搜索投影、default naming 冲突消解。

- `src/shell/view_model.rs`
  - `AssetModalState`
    - `NewFolder { parent_id, draft_name }`
    - `NewSshConnection { parent_id, active_tab, draft }`
  - `AssetSshModalTab`
    - `Standard`
    - `Tunnel`
    - `Proxy`
    - `Environment`
    - `Advanced`
  - `AssetSshConnectionDraft`
    - `name`
    - `host`
    - `user`
    - `port`
    - `environment`
    - `proxy_method`
  - `ShellViewModel`
    - 本轮重点字段：
      - `asset_modal_state: Option<AssetModalState>`
      - `asset_view_mode: AssetViewMode`
      - `asset_search_query: String`
      - `selected_asset_ids: Vec<String>`
      - `focused_asset_id: Option<String>`
      - `editing_asset_id: Option<String>`
      - `editing_asset_text: String`
      - `context_menu_open: bool`
      - `context_target_asset_id: Option<String>`
      - `context_menu_target_kind: Option<ContextTargetKind>`
      - `console_asset_tree: AssetTree`

### Slint window model

- `ui/app-window.slint`
  - modal host properties：
    - `asset-modal-open`
    - `asset-modal-kind`
    - `asset-modal-can-confirm`
    - `asset-folder-modal-name`
    - `asset-ssh-modal-active-tab`
    - `asset-ssh-modal-name`
    - `asset-ssh-modal-host`
    - `asset-ssh-modal-user`
    - `asset-ssh-modal-port`
    - `asset-ssh-modal-environment`
    - `asset-ssh-modal-proxy-method`
  - Explorer row model：
    - `console-asset-items: [ConsoleAssetItem]`

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
    - `renaming`
    - `rename_text`
    - `path_hint`
    - `show_disclosure`
    - `compact_flat_mode`
  - `ListView` 已成为 Console 资产区的实际列表宿主。

- `ui/components/assets-folder-create-modal.slint`
  - folder draft UI；
  - `Escape` / `Return` 已直接映射到 close / confirm callback。

- `ui/components/assets-ssh-connection-modal.slint`
  - SSH draft UI；
  - 当前只要求 `name + host` 才可 confirm；
  - `tunnel / proxy / environment / advanced` 仍有 placeholder panel，但 tab 状态已接线。

- `ui/components/asset-node-row.slint`
  - row 视觉与 hit target 契约；
  - disclosure 独立点击区；
  - body 触摸区从 disclosure 右侧开始，避免命中重叠；
  - 当 `path_hint != ""` 时切换为两行布局。

## 已落地的关键函数契约

### Tree / projection

- `AssetTree::insert_root(kind, title) -> String`
- `AssetTree::insert_child(parent_id, kind, title) -> String`
- `AssetTree::set_expanded(node_id, expanded)`
- `AssetTree::set_all_expanded(expanded)`
- `AssetTree::project_visible_rows(view_mode, search_query) -> Vec<VisibleAssetRow>`
- `AssetTree::next_default_name_for_parent(parent_id, kind) -> String`
- `resolve_committed_name(kind, draft, items) -> String`

### View-model

- `ShellViewModel::open_new_folder_modal(parent_id)`
  - 仅切 modal state，不直接插入节点；
  - 会关闭 context menu / create menu，并结束活跃 rename。

- `ShellViewModel::update_new_folder_modal_name(value)`

- `ShellViewModel::open_new_ssh_modal(parent_id)`
  - 初始化 `AssetSshConnectionDraft`；
  - 默认 `port = "22"`。

- `ShellViewModel::update_ssh_modal_field(field, value)`
- `ShellViewModel::select_ssh_modal_tab(tab)`
- `ShellViewModel::can_confirm_asset_modal() -> bool`
- `ShellViewModel::confirm_asset_modal()`
  - 只有 confirm 时才真正写入 `AssetTree`；
  - root / child 由 `parent_id` 决定；
  - confirm 后更新 `selected_asset_ids / focused_asset_id / context_target_asset_id`；
  - confirm 后清空 `asset_modal_state`。

- `ShellViewModel::cancel_asset_modal()`
- `ShellViewModel::visible_console_asset_rows() -> Vec<VisibleAssetRow>`
- `ShellViewModel::handle_assets_create_action(action_id)`
  - toolbar create 只负责打开 root-targeted modal。

- `ShellViewModel::handle_context_menu_leaf_action(action_id)`
  - folder target -> child-targeted modal；
  - blank / root target -> root-targeted modal；
  - non-create planned action 仍走 feedback pill。

- `ShellViewModel::toggle_folder_expanded(asset_id)`
- `ShellViewModel::begin_asset_rename_session(asset_id, initial_text)`
- `ShellViewModel::commit_active_asset_rename()`
- `ShellViewModel::handle_blank_area_click()`
- `ShellViewModel::select_asset(asset_id)`

### Bootstrap sync bridge

- `src/app/bootstrap.rs`
  - `sync_asset_modal_state(window, state)`
    - 把 `AssetModalState` 映射到 `AppWindow` modal properties；
    - 在 folder / ssh / none 三种分支下重置未使用字段，避免脏数据残留。

## 当前没有新增 Trait

本轮没有引入新的 trait interface。

如果下一轮要把资产创建、保存、同步扩展到 Tokio actor / persistence / remote refresh，建议先定义清晰的 trait 边界，再进入 TDD 与实现。例如：

- `AssetRepository`
- `AssetCommandBus`
- `AssetSyncPort`

否则 modal 状态机、UI 状态与异步结果会直接耦合进 `ShellViewModel`，测试会迅速失控。

## Slint callbacks / bridge contracts

### Modal callbacks

- `assets-create-action-selected(string)`
- `close-asset-modal-requested()`
- `confirm-asset-modal-requested()`
- `asset-folder-modal-name-changed(string)`
- `asset-ssh-modal-tab-selected(string)`
- `asset-ssh-modal-draft-changed(string, string)`

### Explorer row callbacks

- `asset-selected(string)`
- `toggle-expanded-requested(string)`
- `asset-context-menu-requested(string, string, length, length)`
- `asset-rename-text-changed(string, string)`
- `asset-rename-commit-requested(string, string)`
- `asset-rename-cancel-requested(string)`

### Toolbar / search / shell interaction callbacks

- `toggle-assets-search-requested()`
- `assets-search-query-changed(string)`
- `close-assets-search-requested()`
- `collapse-assets-search-requested()`
- `toggle-assets-view-mode-requested()`
- `toggle-assets-tree-expansion-requested()`
- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`
- `shell-interaction-requested()`
- `dismiss-active-asset-rename-requested()`

### Context menu callbacks

- `assets-context-menu-action-invoked(string)`
- `assets-context-menu-key-pressed(string)`
- `assets-context-menu-row-hovered(int, int)`
- `assets-context-menu-pointer-moved(length, length)`
- `close-assets-context-menu-requested()`

## 当前自动化验证基线

本轮已通过的自动化基线：

- Rust tests
  - `shell_view_model`
  - `assets_modal_smoke`
  - `assets_context_menu_smoke`
  - `assets_explorer_projection`
  - `assets_explorer_smoke`
  - `assets_sidebar_toolbar_spec`
  - `assets_sidebar_toolbar_smoke`
- UI contract smoke
  - `tests/assets_modal_ui_contract_smoke.sh`
  - `tests/assets_context_menu_ui_contract_smoke.sh`
  - `tests/assets_explorer_ui_contract_smoke.sh`
  - `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- compile / lint
  - `cargo check --workspace`
  - `cargo clippy --workspace -- -D warnings`

`verification.md` 中已经记录了 `Windows Console Assets Context Menu Bugfix4 Verification` 的结果，可作为下一轮回归基线。

## 下一轮测试建议

### 1. Modal 状态机矩阵补强

- folder modal
  - 空字符串、空白字符串、合法名称、重复名称
  - root create 与 folder child create
  - `Cancel -> reopen -> draft reset`
- SSH modal
  - `name` 空、`host` 空、`name + host` 都存在
  - tab 切换后 draft 是否保留
  - `Escape` 与 close button 是否保持同一语义

### 2. Bootstrap / Slint overlay contract 补强

- modal open / close 时是否重置未使用字段；
- folder modal 与 SSH modal 是否严格互斥；
- modal 打开时，context menu / inline rename / create menu 是否都已关闭；
- backdrop 行为是否满足“可拦截但不误关大 modal”的产品要求。

### 3. AssetNodeRow 交互回归

- disclosure click 与 body click 的命中区完全分离；
- `Tree` 模式下 folder row 的展开 / 收起不会误触 selection；
- `Flat` 模式下 `compact_flat_mode` 缩进契约稳定；
- `path_hint` 存在时的两行布局在 hover / focus / rename 间不抖动。

### 4. Projection / search matrix 补强

- `Flat` 必须永远忽略 folder row；
- `Flat` 搜索既能命中 SSH label，也能命中 `path_hint`；
- search 期间不应污染真实 `expanded` 状态；
- search 清空后 `Tree` 展开状态应恢复到搜索前的真实值。

### 5. Context target 优先级

- toolbar create 即使此前右键过 folder，也必须继续写入 root；
- context menu create 只在当前 target 为 folder 且 parent 仍存在时写入 child；
- stale `context_target_asset_id` 不应导致写入错误 parent。

### 6. 未来异步化前置测试

当前这一层仍是单线程 UI 状态，没有真实 Tokio channel / actor 参与资产创建，因此当前不存在实际 data race。

但如果下一轮开始接：

- 真实保存到 repository
- 背景同步 SSH/Folder 数据
- Actor / channel 驱动的 remote refresh

则必须先补以下测试与约束：

- 任何后台结果都不能直接从 Tokio 线程写 Slint window model；
- 必须通过 `slint::invoke_from_event_loop` 切回 UI 线程；
- channel 若使用 bounded queue，要验证高频刷新下不会阻塞 UI confirm path；
- modal confirm 后若后台保存失败，需定义明确的 rollback / toast / retry 契约；
- 后台刷新若在 modal 打开期间删除了 parent 节点，confirm 必须安全失败或回退到 root，而不能 panic。

## 需要重点盯防的边缘情况

- create 与 rename 必须继续保持分离，不要回退到 placeholder create；
- modal 打开时不应残留 inline rename；
- `context menu / modal / inline rename` 三者必须保持互斥；
- `Flat` 的 `path_hint` 是上下文补偿，不应再次演化成伪 folder row；
- folder child create 后，当前实现仍依赖显式展开 parent 才能看到 child，后续若要改成 auto-expand，必须先写测试再改；
- `resolve_committed_name()` 的默认命名与自定义命名冲突矩阵已经较复杂，下一轮修改前先锁住测试；
- 若后续引入 Tokio actor，不要在多个线程分别维护第二份 asset tree，避免 UI model 与 domain tree 分叉。

## 建议的下一阶段入口

建议下一轮直接以本文件作为 `test-driven-development` 输入，优先顺序如下：

1. modal 状态机与 overlay contract
2. `AssetNodeRow` hit-test / hover / focus regression
3. `Flat` 搜索与 `path_hint` matrix
4. 异步化前的 UI-thread ownership 约束测试
