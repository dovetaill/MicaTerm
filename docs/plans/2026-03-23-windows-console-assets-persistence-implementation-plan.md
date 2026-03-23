# Windows Console Assets Naming And Persistence Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 `Windows Console` 资产列表补齐统一命名行为、SSH 连接字段持久化、`redb` 本地存储、logging-style 数据目录解析，以及 README 目录说明，且不把 UI 会话态写入磁盘。

**Architecture:** 继续保持 `Rust domain -> bootstrap bridge -> Slint` 单向真相源。持久化能力放在 `app` 层，通过可复用 root resolver、独立 persisted catalog domain、`AssetCatalogRepository` 和 `redb` store 接入；`ShellViewModel` 仍只负责运行时树状态与 modal/UI 投影，不直接持有 DB handle，也不把 deferred `Tokio task / command queue` 作为当前实现基线。

**Tech Stack:** Rust, Slint, Tokio runtime, `redb`, `directories`, cargo test, shell smoke scripts

---

## 执行前提

- 当前计划只覆盖已确认主方案：
  - `Folder 1` / `Folder 1-1`
  - `SSH Connection 1` / `SSH Connection 1-1`
  - 同父级跨类型唯一
  - UI 的 `expanded / search / selection / context-menu` 不持久化
  - logging-style root abstraction
  - `redb` 作为首选持久化介质
- 当前实现基线是同步 `repository/store` 写入。
- `Tokio task / command queue` 仅保留在 [20260323-todo.md](/home/wwwroot/mica-term/20260323-todo.md) 作为 deferred Option B，不要在本轮实现中提前引入 ack、flush、rollback 状态机。
- 文档说明文件路径是 [readme.md](/home/wwwroot/mica-term/readme.md)，不是 `README.md`。
- 严格按 TDD 执行，每个任务先写失败测试，再做最小实现，再跑验证。

## Task 1: 抽出可复用的 app root resolver

**Files:**
- Create: `src/app/app_paths.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/logging/paths.rs`
- Create: `tests/app_paths.rs`
- Modify: `tests/logging_paths.rs`

**Step 1: Write the failing tests**

在 `tests/app_paths.rs` 增加：

```rust
#[test]
fn app_root_prefers_explicit_override() {}

#[test]
fn app_root_uses_executable_dir_when_portable_marker_exists() {}

#[test]
fn app_root_uses_platform_local_data_when_marker_is_absent() {}

#[test]
fn app_root_creates_data_logs_and_crash_directories() {}
```

在 `tests/logging_paths.rs` 增加一个回归测试，锁定 logging 仍然通过同一 root source 解析：

```rust
#[test]
fn logging_paths_stay_aligned_with_shared_app_root_resolution() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test app_paths --test logging_paths
```

Expected:

- FAIL because `src/app/app_paths.rs` 还不存在；
- FAIL because logging 还没有复用共享 root resolver。

**Step 3: Write the minimal implementation**

在 `src/app/app_paths.rs` 中建立通用 root resolver：

```rust
pub enum AppRootSource {
    EnvOverride,
    PortableMarker,
    StandardLocalData,
}

pub struct AppRootPathInputs {
    pub env_root_dir: Option<PathBuf>,
    pub executable_dir: PathBuf,
    pub standard_local_data_dir: PathBuf,
    pub portable_marker_name: &'static str,
}

pub struct AppRootPaths {
    pub root_source: AppRootSource,
    pub root_dir: PathBuf,
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub crash_dir: PathBuf,
}

pub fn resolve_app_root_paths(inputs: &AppRootPathInputs) -> Result<AppRootPaths> {
    // override -> portable marker -> standard local data
    // create data/logs/crash directories eagerly
}
```

在 `src/app/logging/paths.rs` 改为 thin adapter：

```rust
pub fn resolve_logging_paths(inputs: &LoggingPathInputs) -> Result<LoggingPaths> {
    let app_paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: inputs.env_log_dir.clone(),
        executable_dir: inputs.executable_dir.clone(),
        standard_local_data_dir: inputs.standard_local_data_dir.clone(),
        portable_marker_name: inputs.portable_marker_name,
    })?;
    Ok(LoggingPaths {
        root_source: map_root_source(app_paths.root_source),
        root_dir: app_paths.root_dir,
        logs_dir: app_paths.logs_dir,
        crash_dir: app_paths.crash_dir,
    })
}
```

实现要求：

- `data_dir` 固定为 `<root>/data`
- `logs_dir` 固定为 `<root>/logs`
- `crash_dir` 固定为 `<root>/crash`
- 明确 working directory 不参与解析

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test app_paths --test logging_paths
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/app_paths.rs src/app/mod.rs src/app/logging/paths.rs tests/app_paths.rs tests/logging_paths.rs
git commit -m "refactor: add shared app root path resolver"
```

## Task 2: 定义 persisted asset catalog domain 与 runtime mapper

**Files:**
- Create: `src/app/assets_catalog/mod.rs`
- Create: `src/app/assets_catalog/model.rs`
- Create: `src/app/assets_catalog/mapper.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/shell/assets.rs`
- Create: `tests/assets_catalog_domain.rs`
- Modify: `tests/assets_explorer_projection.rs`

**Step 1: Write the failing tests**

在 `tests/assets_catalog_domain.rs` 增加：

```rust
#[test]
fn persisted_catalog_round_trips_tree_order_and_node_kind() {}

#[test]
fn persisted_catalog_preserves_ssh_connection_fields() {}

#[test]
fn persisted_catalog_excludes_ui_session_state() {}

#[test]
fn empty_catalog_maps_to_empty_runtime_tree() {}
```

在 `tests/assets_explorer_projection.rs` 增加一个回归测试，确保运行时 `expanded` 仍只来自内存状态，不是 persisted schema 的一部分：

```rust
#[test]
fn expanded_state_remains_runtime_only_after_catalog_mapping() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_explorer_projection
```

Expected:

- FAIL because persisted catalog types 和 mapper 还不存在；
- FAIL because当前 runtime tree 还没有明确的持久化边界。

**Step 3: Write the minimal implementation**

在 `src/app/assets_catalog/model.rs` 定义 persisted schema：

```rust
pub const ASSET_CATALOG_SCHEMA_VERSION: u32 = 1;

pub struct PersistedAssetCatalog {
    pub schema_version: u32,
    pub root_ids: Vec<String>,
    pub nodes: BTreeMap<String, PersistedAssetNode>,
}

pub struct PersistedAssetNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub kind: PersistedAssetKind,
    pub child_ids: Vec<String>,
    pub payload: PersistedAssetPayload,
}

pub enum PersistedAssetKind {
    Folder,
    SshConnection,
}

pub enum PersistedAssetPayload {
    Folder,
    SshConnection(PersistedSshConnectionSpec),
}

pub struct PersistedSshConnectionSpec {
    pub host: String,
    pub user: String,
    pub port: String,
    pub environment: String,
    pub proxy_method: String,
}
```

在 `src/app/assets_catalog/mapper.rs` 建立映射：

```rust
pub fn catalog_to_asset_tree(catalog: &PersistedAssetCatalog) -> AssetTree
pub fn asset_tree_to_catalog(tree: &AssetTree) -> PersistedAssetCatalog
```

实现要求：

- `AssetTree` 继续作为 runtime tree，不直接等同 persisted schema
- mapper 只能映射业务态：
  - 树结构
  - 节点类型
  - SSH connection 字段
- mapper 不得映射 UI 会话态：
  - `expanded`
  - `asset_search_query`
  - `selected_asset_ids`
  - `focused_asset_id`
  - context menu / modal state

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_explorer_projection
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/assets_catalog/mod.rs src/app/assets_catalog/model.rs src/app/assets_catalog/mapper.rs src/app/mod.rs src/shell/assets.rs tests/assets_catalog_domain.rs tests/assets_explorer_projection.rs
git commit -m "feat: define persisted asset catalog domain"
```

## Task 3: 统一 create / rename 命名校验与默认命名

**Files:**
- Modify: `src/shell/assets.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/assets-folder-create-modal.slint`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn new_folder_modal_prefills_next_dash_suffix_name() {}

#[test]
fn new_ssh_modal_prefills_next_dash_suffix_name() {}

#[test]
fn create_validation_rejects_duplicate_name_across_kinds() {}

#[test]
fn conflicting_manual_input_keeps_user_text_and_disables_confirm() {}

#[test]
fn editing_existing_name_to_original_value_remains_valid() {}
```

在 `tests/assets_modal_smoke.rs` 增加：

```rust
#[test]
fn create_modals_project_inline_validation_message_and_confirm_state() {}
```

在 `tests/assets_modal_ui_contract_smoke.sh` 增加 grep 断言，锁定 Slint 新属性：

```bash
rg -n "validation-message" ui/components/assets-folder-create-modal.slint
rg -n "validation-message" ui/components/assets-ssh-connection-modal.slint
rg -n "can-confirm" ui/components/assets-folder-create-modal.slint
rg -n "can-confirm" ui/components/assets-ssh-connection-modal.slint
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test shell_view_model --test assets_modal_smoke
```

Run:

```bash
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because create modal 还没有 inline validation projection；
- FAIL because default naming 还没有统一采用 dash-suffix；
- FAIL because `New SSH Connection` 还没有复用 rename 同等级校验语义。

**Step 3: Write the minimal implementation**

在 `src/shell/assets.rs` 锁定默认命名与校验函数：

```rust
pub fn next_default_name_from_base(base: &str, siblings: &[AssetNode]) -> String {
    // "Folder 1" -> "Folder 1-1" -> "Folder 1-2"
    // "SSH Connection 1" -> "SSH Connection 1-1" -> "SSH Connection 1-2"
}

pub fn validate_name_in_parent(
    &self,
    parent_id: Option<&str>,
    candidate: &str,
    exclude_id: Option<&str>,
) -> AssetNameValidation
```

在 `src/shell/view_model.rs` 为 create / rename 统一提供投影：

```rust
fn create_asset_modal_validation(
    &self,
    parent_id: Option<&str>,
    draft_name: &str,
) -> AssetNameValidation

pub fn asset_create_modal_validation_message(&self) -> String
pub fn asset_create_modal_can_confirm(&self) -> bool
```

在 Slint modal 中补齐属性：

```slint
in property <string> validation-message: "";
in property <bool> can-confirm: false;
```

实现要求：

- create modal 打开时预填唯一默认名；
- 手动输入冲突值时显示 inline error 并禁用 confirm；
- 不再在 create submit 阶段静默重命名用户显式输入；
- 仅在空白草稿 fallback 时允许回落到默认名；
- rename 和 create 使用同一套同父级跨类型唯一校验。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test shell_view_model --test assets_modal_smoke
```

Run:

```bash
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/assets.rs src/shell/view_model.rs src/app/bootstrap.rs ui/components/assets-folder-create-modal.slint ui/components/assets-ssh-connection-modal.slint ui/app-window.slint tests/shell_view_model.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: unify asset create and rename validation"
```

## Task 4: 引入 `redb` repository/store 与恢复策略

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/assets_catalog/repository.rs`
- Create: `src/app/assets_catalog/redb_store.rs`
- Modify: `src/app/assets_catalog/mod.rs`
- Create: `tests/assets_catalog_store.rs`

**Step 1: Write the failing tests**

在 `tests/assets_catalog_store.rs` 增加：

```rust
#[test]
fn load_returns_empty_catalog_when_assets_file_is_missing() {}

#[test]
fn save_and_reload_preserves_tree_structure_and_ssh_fields() {}

#[test]
fn open_failure_quarantines_corrupt_file_with_timestamp_suffix() {}

#[test]
fn schema_upgrade_creates_backup_before_rewrite() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_catalog_store
```

Expected:

- FAIL because `redb` dependency 与 repository/store 还不存在；
- FAIL because `assets.redb`、backup、corrupt 隔离策略还未实现。

**Step 3: Write the minimal implementation**

先在 `Cargo.toml` 添加 `redb`。

在 `src/app/assets_catalog/repository.rs` 定义接口：

```rust
pub trait AssetCatalogRepository {
    fn load(&self) -> Result<PersistedAssetCatalog>;
    fn save(&self, catalog: &PersistedAssetCatalog) -> Result<()>;
}
```

在 `src/app/assets_catalog/redb_store.rs` 实现：

```rust
pub struct RedbAssetCatalogStore {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
}

impl RedbAssetCatalogStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            database_path: data_dir.join("assets.redb"),
            data_dir,
        }
    }
}
```

表布局要求：

- metadata table
  - `schema_version`
- asset records table
  - key: `asset_id`
  - value: versioned binary-encoded persisted node
- 可选 root order record

编码要求：

- 不能使用 JSON 作为 value 编码
- 使用二进制 codec，并在单元测试中锁定 round-trip

恢复要求：

- 文件不存在时返回空 catalog
- 打开失败时：
  - 原文件重命名为 `assets.corrupt-<timestamp>.redb`
  - 不静默覆盖损坏文件
- schema 升级前：
  - 创建 `assets.backup-<timestamp>.redb`
  - 再执行升级写入

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_catalog_store
```

Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml src/app/assets_catalog/repository.rs src/app/assets_catalog/redb_store.rs src/app/assets_catalog/mod.rs tests/assets_catalog_store.rs
git commit -m "feat: add redb asset catalog store"
```

## Task 5: 在 bootstrap/application 层接通启动加载与 mutation 持久化

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/assets_catalog/mapper.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

在 `tests/bootstrap_smoke.rs` 增加：

```rust
#[test]
fn bootstrap_loads_catalog_before_first_asset_projection_sync() {}

#[test]
fn create_rename_delete_and_ssh_edit_trigger_repository_save() {}

#[test]
fn save_failure_logs_error_without_persisting_ui_session_state() {}
```

在 `tests/assets_modal_smoke.rs` 增加：

```rust
#[test]
fn ssh_modal_confirm_updates_runtime_tree_and_persists_ssh_fields() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test assets_modal_smoke --test shell_view_model
```

Expected:

- FAIL because bootstrap 还没有 asset repository 注入与启动加载；
- FAIL because create / rename / delete / edit SSH connection 后还没有 persistence save hook。

**Step 3: Write the minimal implementation**

在 `src/app/bootstrap.rs` 增加 store 装配与保存辅助：

```rust
fn load_asset_catalog(
    repo: &dyn AssetCatalogRepository,
) -> PersistedAssetCatalog

fn save_asset_catalog(
    repo: &dyn AssetCatalogRepository,
    state: &ShellViewModel,
) -> Result<()>
```

在 `src/shell/view_model.rs` 只暴露狭窄运行时接口，不把 store 注入 view-model：

```rust
pub fn replace_console_asset_tree(&mut self, tree: AssetTree)
pub fn console_asset_tree(&self) -> &AssetTree
```

bootstrap 接线要求：

- 启动时：
  - resolve app root
  - 初始化 `RedbAssetCatalogStore`
  - load catalog
  - mapper -> runtime `AssetTree`
  - seed into `ShellViewModel`
- mutation 后持久化：
  - create folder
  - create SSH connection
  - rename
  - delete
  - edit SSH connection fields
- 只在业务 mutation 后保存
- search / selection / expand / context-menu 改变时不保存

错误处理要求：

- save 失败只记录日志，不把 UI session state 写入 fallback 文件
- 不在本任务实现 async queue / retry worker

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke --test assets_modal_smoke --test shell_view_model
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs src/app/assets_catalog/mapper.rs tests/bootstrap_smoke.rs tests/assets_modal_smoke.rs tests/shell_view_model.rs
git commit -m "feat: wire asset catalog persistence into bootstrap"
```

## Task 6: 更新 `readme.md` 并完成回归验证

**Files:**
- Modify: `readme.md`
- Create: `tests/assets_persistence_contract_smoke.sh`
- Modify: `verification.md`

**Step 1: Write the failing smoke checks**

创建 `tests/assets_persistence_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

rg -n "assets.redb" readme.md
rg -n "\\.mica-term-portable" readme.md
rg -n "working directory" readme.md
rg -n "data/" readme.md
```

**Step 2: Run smoke check to verify it fails**

Run:

```bash
bash tests/assets_persistence_contract_smoke.sh
```

Expected:

- FAIL because `readme.md` 还没有资产数据目录说明。

**Step 3: Write the minimal documentation**

在 `readme.md` 增加独立小节，明确：

- 资产数据目录不是相对 working directory
- portable 模式下，相对 executable dir
- 非 portable 模式下，走平台本地数据目录
- 资产主文件路径为 `<root>/data/assets.redb`
- `.mica-term-portable` 对 logging 与 asset data root 同时生效

在 `verification.md` 追加本轮验证记录：

```md
- cargo test --test app_paths --test logging_paths
- cargo test --test assets_catalog_domain --test assets_catalog_store
- cargo test --test shell_view_model --test assets_modal_smoke --test bootstrap_smoke
- bash tests/assets_modal_ui_contract_smoke.sh
- bash tests/assets_persistence_contract_smoke.sh
```

**Step 4: Run final regression**

Run:

```bash
cargo test --test app_paths --test logging_paths --test assets_catalog_domain --test assets_catalog_store --test shell_view_model --test assets_modal_smoke --test bootstrap_smoke
```

Run:

```bash
bash tests/assets_modal_ui_contract_smoke.sh
```

Run:

```bash
bash tests/assets_persistence_contract_smoke.sh
```

Run:

```bash
cargo fmt --check
```

Run:

```bash
cargo clippy --tests -- -D warnings
```

Expected: PASS

**Step 5: Commit**

```bash
git add readme.md tests/assets_persistence_contract_smoke.sh verification.md
git commit -m "docs: document asset persistence root behavior"
```

## Verification Checklist

- `Folder` 与 `SSH Connection` create modal 都预填 dash-suffix 默认名
- 手动输入重名时 inline error 与 confirm disabled 正常工作
- 同父级跨类型唯一约束在 create / rename 都生效
- `redb` 中保存了树结构、顺序、节点类型和 SSH 字段
- `expanded / search / selection / context-menu` 未进入 persisted catalog
- portable marker 与标准安装模式都能解析到正确 root
- `assets.redb` 缺失时能初始化空 catalog
- `assets.redb` 损坏时会被隔离而不是静默覆盖
- `readme.md` 已说明 working directory 不影响资产数据位置

## Deferred Follow-Up

以下内容不属于本计划的实施范围：

- `Tokio task / command queue`
- 持久化 ack / rollback / shutdown flush 状态机
- 崩溃前未落盘命令恢复
- 导入导出与远程同步共用 command pipeline

这些项已经记录在 [20260323-todo.md](/home/wwwroot/mica-term/20260323-todo.md)，若要推进，必须另开 design / implementation plan。
