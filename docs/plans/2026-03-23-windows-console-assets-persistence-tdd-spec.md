# Windows Console Assets Persistence TDD Spec

日期: 2026-03-23
阶段: implementation complete -> TDD handoff
状态: 计划内 Task 已完成，当前进入后续测试扩展与真实桌面 smoke 阶段

## 范围摘要

本轮已为 `Windows Console` 资产区补齐完整的持久化基础链路：

- 独立的 persisted catalog domain，不再把 runtime `AssetTree` 直接当作磁盘 schema
- 统一 app root 解析，资产数据目录与 logging root 保持同源策略
- `redb` 持久化 store、schema version、升级备份与损坏隔离
- create / rename / delete / SSH connection 编辑后的同步落盘
- UI 会话态与 persisted state 严格分离
- `readme.md` 与 `verification.md` 的目录策略和验证记录补齐

当前保持 `Rust state -> bootstrap -> Slint` 的单向真相源，持久化仍为同步调用，没有引入后台 worker 或命令队列。

## 核心 Struct

### Persisted domain

- `src/app/assets_catalog/model.rs`
  - `PersistedAssetCatalog`
    - `schema_version`
    - `root_ids`
    - `nodes`
  - `PersistedAssetNode`
    - `id`
    - `parent_id`
    - `title`
    - `kind`
    - `child_ids`
    - `payload`
  - `PersistedAssetKind`
    - `Folder`
    - `SshConnection`
  - `PersistedAssetPayload`
    - `Folder`
    - `SshConnection(PersistedSshConnectionSpec)`
  - `PersistedSshConnectionSpec`
    - `host`
    - `user`
    - `port`
    - `environment`
    - `proxy_method`

### Runtime state

- `src/shell/assets.rs`
  - `AssetTree`
    - 运行时树结构、同父级命名校验、默认命名、可见行投影
  - `AssetNode`
  - `AssetNodePayload`
    - `Folder`
    - `SshConnection(AssetSshConnectionSpec)`
  - `AssetSshConnectionSpec`

- `src/shell/view_model.rs`
  - `ShellViewModel`
    - 持有 UI 真相源与 runtime `AssetTree`
  - `AssetModalState`
    - `NewFolder`
    - `NewSshConnection`
    - `RenameAsset`
    - `DeleteAssetConfirm`
  - `AssetSshConnectionDraft`
  - `AssetSshModalTab`

### Bootstrap / app layer

- `src/app/app_paths.rs`
  - `AppRootPathInputs`
  - `AppRootPaths`
  - `AppRootSource`
- `src/app/assets_catalog/redb_store.rs`
  - `RedbAssetCatalogStore`
  - `METADATA_TABLE`
  - `ASSET_RECORDS_TABLE`
- `src/app/bootstrap.rs`
  - `load_asset_catalog(...)`
  - `save_asset_catalog(...)`
  - `save_asset_catalog_if_available(...)`
  - `asset_catalog_repository_for_app(...)`

## Trait 与接口契约

### Repository contract

- `src/app/assets_catalog/repository.rs`
  - `AssetCatalogRepository`
    - `load(&self) -> Result<PersistedAssetCatalog>`
    - `save(&self, catalog: &PersistedAssetCatalog) -> Result<()>`

该 trait 的约束是：

- bootstrap 只依赖 trait，不把 `redb` 细节泄漏到 UI/state 层
- `load()` 返回 persisted domain，而不是 runtime `AssetTree`
- `save()` 只接受 persisted catalog，不接受 UI 会话态

### Mapper contract

- `src/app/assets_catalog/mapper.rs`
  - `catalog_to_asset_tree(catalog: &PersistedAssetCatalog) -> AssetTree`
  - `asset_tree_to_catalog(tree: &AssetTree) -> PersistedAssetCatalog`

关键约束：

- SSH 字段必须双向完整映射
- `expanded / search / selection / context-menu` 不进入 persisted catalog
- root 顺序、child 顺序、节点类型都必须保持稳定

### ShellViewModel contract

- `replace_console_asset_tree(&mut self, tree: AssetTree)`
  - 用启动加载结果整体替换 runtime tree
  - 清理 selection / focus / context menu / inline editing 等会话态
- `console_asset_tree(&self) -> &AssetTree`
  - 只读暴露给 bootstrap 做持久化投影
- `confirm_asset_modal(&mut self) -> bool`
  - 成功发生业务 mutation 时返回 `true`
- `confirm_delete_asset(&mut self) -> bool`
  - 成功删除时返回 `true`

这个布尔返回值是 bootstrap 的保存门槛，避免纯 UI 刷新或无效 confirm 触发落盘。

## Slint callbacks / global state / bindings

### AppWindow properties

- `asset-modal-open`
- `asset-modal-kind`
- `asset-modal-validation-message`
- `asset-modal-can-confirm`
- `asset-folder-modal-name`
- `asset-ssh-modal-active-tab`
- `asset-ssh-modal-name`
- `asset-ssh-modal-host`
- `asset-ssh-modal-user`
- `asset-ssh-modal-port`
- `asset-ssh-modal-environment`
- `asset-ssh-modal-proxy-method`
- `asset-rename-modal-open`
- `asset-rename-modal-name`
- `asset-rename-modal-validation-message`
- `asset-rename-modal-can-confirm`
- `asset-delete-confirm-modal-open`
- `asset-delete-confirm-target-label`
- `asset-delete-confirm-descendant-count`
- `console-asset-items`

### AppWindow callbacks

- `assets-create-action-selected`
- `confirm-asset-modal-requested`
- `close-asset-modal-requested`
- `asset-folder-modal-name-changed`
- `asset-ssh-modal-tab-selected`
- `asset-ssh-modal-draft-changed`
- `asset-rename-modal-name-changed`
- `confirm-asset-rename-requested`
- `confirm-delete-asset-requested`
- `asset-selected`
- `toggle-expanded-requested`

### Global state 说明

- 本轮没有新增 Slint `global` singleton
- 窗口级资产状态仍由 `AppWindow` property 承载
- `console-asset-items` 通过 `ModelRc<VecModel<ConsoleAssetItem>>` 从 Rust 投影

## Tokio task / channel / actor 交互关系

本轮没有引入 Tokio task、channel 或 actor。当前是同步持久化链路：

1. Slint callback 进入 `bootstrap`
2. `bootstrap` 调用 `ShellViewModel`
3. view-model 返回 mutation 是否成功
4. 若成功，`bootstrap` 调用 `asset_tree_to_catalog(...)`
5. 通过 `AssetCatalogRepository::save(...)` 同步写盘
6. 然后再 `sync_console_assets()` / `sync_asset_modal_state()`

后续如果要引入 Tokio / actor，需要保持以下边界：

- repository 调用不能直接在后台线程更新 Slint model
- 后台落盘完成后的 UI 回写必须通过 `slint::invoke_from_event_loop`
- 保存队列若异步化，必须定义“最新状态覆盖旧状态”或“串行命令确认”策略，不能允许无界堆积

## 状态流转说明

### 启动加载

1. `bind_top_status_bar_with_profile()` 解析 `UiPreferencesStore`
2. 同时解析默认 `AssetCatalogRepository`
3. `load_asset_catalog(repo)` 读取 persisted catalog
4. `catalog_to_asset_tree(...)` 转成 runtime tree
5. `ShellViewModel::replace_console_asset_tree(...)` 注入初始树
6. bootstrap 首次执行 `sync_console_assets()`，首帧 UI 直接反映已加载资产

### Create folder / SSH

1. UI 打开 modal，维护 draft
2. `confirm_asset_modal()` 校验命名和 SSH host
3. view-model 在 runtime tree 上创建节点
4. bootstrap 看到返回 `true`
5. bootstrap 执行 `save_asset_catalog(...)`
6. 持久化成功或失败后，继续同步 UI 列表与 modal state

### Rename

1. context menu 打开 rename modal
2. draft 变化只更新内存验证状态
3. confirm 成功后 runtime tree 更新标题
4. bootstrap 保存最新 catalog
5. UI 列表刷新，modal 关闭

### Delete

1. context menu 打开 delete confirm modal
2. view-model 计算 `descendant_count`
3. confirm 后 runtime tree 删除 subtree
4. focus/selection 回落到下一个兄弟、上一个兄弟或父节点
5. bootstrap 保存更新后的 catalog

### 纯 UI 会话行为

以下状态只存在于内存，不触发保存，也不进入 persisted schema：

- `expanded`
- `asset_search_query`
- `asset_search_expanded`
- `selected_asset_ids`
- `focused_asset_id`
- `context_menu_*`
- `asset_view_mode`

## 关键错误处理策略

- store 缺失文件
  - `load()` 返回空 catalog，不预创建无意义文件
- store 打开或读取失败
  - 原文件隔离到 `assets.corrupt-<timestamp>.redb`
  - 返回空 catalog
- schema 旧版本
  - 先复制 `assets.backup-<timestamp>.redb`
  - 再按当前 schema 重写
- schema 新于当前支持版本
  - 显式返回错误，不静默降级
- startup load 失败
  - bootstrap 记录 `failed to load asset catalog`
  - UI 回落为空资产树
- save 失败
  - bootstrap 记录 `failed to save asset catalog`
  - 不写 JSON fallback
  - 不把 UI session state 落到别的文件
- UI 无效 confirm
  - `confirm_asset_modal()` / `confirm_delete_asset()` 返回 `false`
  - bootstrap 不执行保存

## 潜在边缘情况（Edge Cases）

- Tokio channel 阻塞或消息堆积
  - 当前未使用 channel，但后续异步保存若使用 `mpsc`，必须限制缓冲并明确背压策略。
- UI 线程更新时机不正确
  - 后续若从后台线程直接更新 Slint property，会造成线程违规；必须切回 UI event loop。
- 数据竞争或共享状态不一致
  - 当前真相源集中在 `Rc<RefCell<ShellViewModel>>`；未来若后台也持有资产树副本，会产生双写风险。
- 资源释放时序问题
  - 若 repository、logging runtime、window 生命周期解耦，窗口关闭后可能仍有回调尝试访问失效句柄。
- 异步任务取消或界面关闭后的悬挂回调
  - 当前没有异步 worker；未来若引入后台保存，需要在回调前确认窗口仍存活、保存请求仍属于当前会话。
- Slint model 更新与实际数据源不同步
  - `console-asset-items` 必须始终从 `visible_console_asset_rows()` 重建，不应在 UI 层做局部增量真相源。
- 保存时混入 UI 会话态
  - 如果后续直接从 view-model 序列化而绕过 mapper，容易把 `expanded / search / selection` 错误写盘。
- schema 升级时文件锁未释放
  - `redb` 句柄在 upgrade / quarantine 分支前必须 drop，否则会触发 lock error。
- portable / standard root 误解为 working directory
  - 文档和测试已锁定“working directory 不影响资产数据位置”，后续不能回退为相对 cwd。

## 后续测试建议

### 单元测试

- `AssetTree` 新增“编辑已有 SSH 连接字段后重新保存”的细粒度测试
- `asset_tree_to_catalog()` 针对非法 payload 组合的防御测试
- `RedbAssetCatalogStore` 针对 metadata 缺失、部分表损坏、空 root_ids 的恢复测试
- `asset_catalog_repository_for_app()` 的 root 解析测试，覆盖 env override 与 portable marker 组合场景

### 集成测试

- 启动两次应用实例，验证首次创建资产后第二次启动能加载同样树结构
- 保存失败后再次启动，验证不会从任何 fallback 文件恢复出 UI 会话态
- 在同一父级下 create / rename / delete / create 的多步序列后，验证 `assets.redb` 顺序保持稳定

### UI 交互测试

- GUI 环境下验证启动后首帧就显示已持久化资产，而不是闪一下空列表
- 验证 SSH modal 编辑 `environment` / `proxy_method` 后重新打开应用能看到一致数据
- 验证树展开状态、搜索输入、选中状态在重启后不会被错误恢复
- 验证保存失败时 UI 仍保持刚完成的本地 mutation 展示，同时日志里出现错误记录

### 后续如果引入异步持久化

- 增加 save queue 背压测试
- 增加窗口关闭后取消未完成任务的测试
- 增加多次快速 mutation 只保留最终快照的测试
