# Asset Sync Background Service Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把当前散落在 `bootstrap` UI 装配层的资产同步编排收拢为 `VaultSyncService`，确保资产保存后的远端同步始终后台静默执行，并让 `Sync Settings` 能展示本地/远端最新同步状态。

**Architecture:** 继续沿用现有 `AppWindow -> bootstrap -> ShellViewModel -> vault engine/provider` 主链路，不重写 `SyncEngine`、provider 或 merge。新增 `src/app/vault/sync_service.rs` 作为唯一同步编排入口，统一承接 `manual sync`、`debounced auto sync`、`periodic refresh`、`open modal -> refresh remote head`，并把 service 结果投影回现有 `SyncModalViewState` 与 Slint modal。实现上优先保留现有 slow-provider smoke test 契约，先锁失败测试，再最小改动收口运行时装配与 UI 状态。

**Tech Stack:** Rust, Slint, Tokio, existing vault/snapshot/recovery stack, `cargo test`, shell smoke scripts, `slint-renderer-software` render tests

---

## Input Design

- 设计文档固定为 `docs/plans/2026-04-02-asset-sync-background-service-design.md`
- 实现不得偏离以下已确认决策：
  - 抽 `VaultSyncService` 做收敛型结构升级
  - `Sync Settings` 打开时后台静默读取一次 primary remote head
  - modal 时间展示走极简状态卡
  - 同步成功静默，失败非阻塞暴露
- 本轮明确不做：
  - 重写 `SyncEngine`
  - 改写 provider 协议
  - 引入 sync history / revisions 列表页面
  - 把 mirror 单独做成首屏状态卡

## Execution Notes

- 使用 `@superpowers:test-driven-development` 执行每个任务：先写失败测试，再写最小实现，再跑通过。
- 如果 service 调度、timer 去重、UI 回流顺序出现意外，立即切换到 `@superpowers:systematic-debugging`，不要凭感觉修闭包链。
- 所有远端 I/O 必须脱离 UI 线程；不要再引入任何“保存后直接同步 provider”的前台路径。
- `VaultSyncService` 只收口“编排层”；不要顺手重构 `SyncEngine`、`merge`、`VaultProvider`。
- 时间展示层必须兼容：
  - 本地 durable metadata 的 epoch-millis 字符串
  - 远端 `committed_at` 可能出现的 ISO8601 字符串
- UI 成功反馈保持静默；如果实现过程中新增了成功 toast，视为偏离设计，必须删掉。

## Task Sequence Overview

1. 新增 `VaultSyncService` 合同、事件类型与最小单测。
2. 把 `manual / auto / periodic` 同步编排迁入 service，并修正显式 runtime handle 优先级。
3. 把资产/SSH/keychain 变更入口统一改为 service 的 dirty API，移除 UI 层直接操纵 scheduler 的细节。
4. 为 `Sync Settings` 增加“打开即后台刷新 primary remote head”的状态链路。
5. 补齐 modal 极简状态卡、时间格式化与 Slint/UI contract 回归测试。

### Task 1: 建立 `VaultSyncService` 合同与最小状态机

**Files:**
- Create: `src/app/vault/sync_service.rs`
- Modify: `src/app/vault/mod.rs`
- Create: `tests/vault_sync_service_spec.rs`

**Step 1: Write the failing unit tests**

先在 `tests/vault_sync_service_spec.rs` 锁定 service 的最小行为契约：

```rust
use mica_term::app::vault::sync_service::{
    VaultSyncIntent, VaultSyncService, VaultSyncServiceConfig,
};

#[test]
fn service_coalesces_duplicate_remote_head_refresh_requests() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    assert!(service.request(VaultSyncIntent::RefreshRemoteHead));
    assert!(!service.request(VaultSyncIntent::RefreshRemoteHead));
}

#[test]
fn service_tracks_dirty_state_without_dropping_manual_requests() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    assert!(service.request(VaultSyncIntent::LocalMutation));
    assert!(service.request(VaultSyncIntent::ManualSync));
}

#[test]
fn service_prefers_explicit_runtime_handle_when_present() {
    let runtime = mica_term::app::async_runtime::AppAsyncRuntime::new().unwrap();
    let service = VaultSyncService::new(
        VaultSyncServiceConfig::default().with_runtime_handle(Some(runtime.handle())),
    );

    assert!(service.can_run_in_background());
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_sync_service_spec -- --nocapture
```

Expected:

- FAIL，因为 `sync_service` 模块和 `VaultSyncIntent` / `VaultSyncService` 还不存在。

**Step 3: Write the minimal implementation**

在 `src/app/vault/sync_service.rs` 里先落最小 contract：

```rust
pub enum VaultSyncIntent {
    ManualSync,
    LocalMutation,
    PeriodicRefresh,
    RefreshRemoteHead,
}

pub struct VaultSyncServiceConfig {
    pub runtime_handle: Option<tokio::runtime::Handle>,
}

pub struct VaultSyncService {
    runtime_handle: Option<tokio::runtime::Handle>,
    remote_head_refresh_in_flight: std::sync::atomic::AtomicBool,
}

impl VaultSyncService {
    pub fn new(config: VaultSyncServiceConfig) -> Self { /* ... */ }
    pub fn request(&self, intent: VaultSyncIntent) -> bool { /* ... */ }
    pub fn can_run_in_background(&self) -> bool { self.runtime_handle.is_some() }
}
```

在 `src/app/vault/mod.rs` 导出 `pub mod sync_service;`。

这一任务只冻结 service API 与最小去重语义，不迁移 `bootstrap` 里的实际同步逻辑。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_sync_service_spec -- --nocapture
```

Expected:

- PASS，service 最小 contract、显式 runtime handle、remote head refresh 去重语义已锁定。

**Step 5: Commit**

```bash
git add src/app/vault/mod.rs src/app/vault/sync_service.rs tests/vault_sync_service_spec.rs
git commit -m "feat: add vault sync service contract"
```

### Task 2: 把 `manual / auto / periodic` 同步编排迁入 service

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/vault_sync_service_spec.rs`

**Step 1: Write the failing integration tests**

先保留并扩大现有 slow-provider 契约，确保重构后仍满足“快速返回，不阻塞 UI”：

```rust
#[test]
fn manual_sync_modal_returns_before_slow_primary_write_completes() { /* existing smoke */ }

#[test]
fn debounced_auto_sync_returns_before_slow_primary_write_completes() { /* existing smoke */ }

#[test]
fn periodic_sync_returns_before_slow_primary_refresh_completes() { /* existing smoke */ }

#[test]
fn service_background_mode_uses_explicit_runtime_handle_even_without_session_runtime_guard() {
    let runtime = mica_term::app::async_runtime::AppAsyncRuntime::new().unwrap();
    let service = VaultSyncService::new(
        VaultSyncServiceConfig::default().with_runtime_handle(Some(runtime.handle())),
    );

    assert!(service.can_run_in_background());
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_sync_service_spec service_background_mode_uses_explicit_runtime_handle_even_without_session_runtime_guard -- --exact
cargo test --test bootstrap_smoke manual_sync_modal_returns_before_slow_primary_write_completes -- --exact
cargo test --test bootstrap_smoke debounced_auto_sync_returns_before_slow_primary_write_completes -- --exact
cargo test --test bootstrap_smoke periodic_sync_returns_before_slow_primary_refresh_completes -- --exact
```

Expected:

- FAIL，直到 `bootstrap` 改为经 `VaultSyncService` 统一调度，并显式优先使用传入的 `async_runtime_handle`。

**Step 3: Write the minimal implementation**

在 `src/app/bootstrap.rs` 做三件事：

1. 把当前这些类型和闭包收敛到 service 调用：
   - `VaultSyncSchedulerState`
   - `VaultSyncBackgroundMessage`
   - `run_vault_sync`
2. 构造 service 时显式选择 runtime：
   - 先用 `bind_top_status_bar_with_profile_and_async_handle()` 传入的 `async_runtime_handle`
   - 仅在没有显式 handle 时，才 fallback 到 `session_runtime_guard`
3. 保留现有 `sync_local_vault()` / `refresh_local_vault_from_primary_remote_if_changed()` 作为工作函数，由 service 负责何时后台执行、何时回主线程投递结果。

最小实现目标类似：

```rust
let vault_sync_runtime_handle = explicit_async_runtime_handle
    .clone()
    .or_else(|| session_runtime_guard.as_ref().map(AppAsyncRuntime::handle));

let vault_sync_service = Rc::new(VaultSyncService::new(
    VaultSyncServiceConfig::default().with_runtime_handle(vault_sync_runtime_handle),
));
```

这一任务结束后，`manual / debounced auto / periodic` 都必须经 service 统一排队和执行。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_sync_service_spec -- --nocapture
cargo test --test bootstrap_smoke manual_sync_modal_returns_before_slow_primary_write_completes -- --exact
cargo test --test bootstrap_smoke debounced_auto_sync_returns_before_slow_primary_write_completes -- --exact
cargo test --test bootstrap_smoke periodic_sync_returns_before_slow_primary_refresh_completes -- --exact
```

Expected:

- PASS，说明显式 runtime handle 生效，三类慢 provider 场景都保持后台快速返回。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/vault_sync_service_spec.rs
git commit -m "refactor: route vault sync scheduling through service"
```

### Task 3: 统一资产与 SSH/keychain 变更入口的 dirty API

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing mutation-path tests**

继续锁住现有“变更后自动入队同步”的行为，防止 service 抽离后漏掉某个入口：

```rust
#[test]
fn asset_mutation_syncs_without_auto_sync_toggle() { /* existing smoke */ }

#[test]
fn back_to_back_mutations_share_one_debounced_auto_sync_upload() { /* existing smoke */ }

#[test]
fn periodic_auto_sync_retries_failed_dirty_changes() { /* existing smoke */ }
```

如缺少覆盖，再新增一个最小 SSH 保存入口回归：

```rust
#[test]
fn saving_ssh_asset_marks_service_dirty_without_blocking_modal_close() {
    // save ssh asset, close modal immediately, wait for debounced provider write
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke asset_mutation_syncs_without_auto_sync_toggle -- --exact
cargo test --test bootstrap_smoke back_to_back_mutations_share_one_debounced_auto_sync_upload -- --exact
cargo test --test bootstrap_smoke periodic_auto_sync_retries_failed_dirty_changes -- --exact
```

Expected:

- FAIL，直到各类资产变更入口都改为调用同一条 service dirty API。

**Step 3: Write the minimal implementation**

把所有直接调用 `mark_local_vault_dirty_and_arm_sync(...)` 的路径，统一收敛为 service 接口，例如：

```rust
vault_sync_service.request_local_mutation(
    &mut state,
    &mut vault,
    credential_store_ref.as_ref(),
);
```

覆盖至少这些路径：

- 资产新建
- 资产重命名
- 资产删除
- SSH 资产保存 / save-and-connect
- keychain folder / identity / SSH key 变更

完成后删除或内联旧的 `mark_local_vault_dirty_and_arm_sync()`，不要保留“双轨调度”。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke asset_mutation_syncs_without_auto_sync_toggle -- --exact
cargo test --test bootstrap_smoke back_to_back_mutations_share_one_debounced_auto_sync_upload -- --exact
cargo test --test bootstrap_smoke periodic_auto_sync_retries_failed_dirty_changes -- --exact
```

Expected:

- PASS，且多次快速本地变更仍只折叠成一次 debounced upload。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs
git commit -m "refactor: route asset mutations through vault sync service"
```

### Task 4: 为 `Sync Settings` 接入后台 remote head 刷新链路

**Files:**
- Modify: `src/app/vault/sync_service.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/sync_vault_modal_smoke.rs`
- Modify: `tests/vault_settings_smoke.rs`

**Step 1: Write the failing modal refresh tests**

先锁住“modal 打开不阻塞，但后台会刷新 remote head”的新契约：

```rust
#[test]
fn opening_sync_settings_refreshes_primary_head_in_background() {
    // arrange a slow primary read_head
    // open modal
    // assert modal opens immediately
    // then wait until remote revision/time fields update
}

#[test]
fn remote_head_refresh_failure_keeps_sync_settings_non_blocking() {
    // arrange read_head error
    // open modal
    // assert modal still opens and error lands in modal state later
}
```

再在 `tests/vault_settings_smoke.rs` 加一个快速 UI 契约：

```rust
#[test]
fn sync_settings_opens_before_remote_head_refresh_completes() {
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();
    assert!(app.get_sync_modal_open());
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sync_vault_modal_smoke opening_sync_settings_refreshes_primary_head_in_background -- --exact
cargo test --test sync_vault_modal_smoke remote_head_refresh_failure_keeps_sync_settings_non_blocking -- --exact
cargo test --test vault_settings_smoke sync_settings_opens_before_remote_head_refresh_completes -- --exact
```

Expected:

- FAIL，因为当前 modal 打开不会触发 remote head refresh，也没有远端状态字段可更新。

**Step 3: Write the minimal implementation**

在 `VaultSyncService` 里补 remote-head 刷新工作项与结果消息：

```rust
pub enum VaultSyncIntent {
    /* existing */
    RefreshRemoteHead,
}

pub struct RemoteHeadSnapshot {
    pub revision: Option<String>,
    pub committed_at: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}
```

在 `src/shell/view_model.rs` 给 `SyncModalViewState` 增加最小状态字段：

- `local_last_sync_text`
- `remote_last_update_text`
- `primary_revision_text`
- `remote_status_text`
- `remote_status_loading`

在 `src/app/bootstrap.rs` 打开 modal 的回调里：

1. 先同步本地缓存状态到 modal；
2. 立即 `state.open_sync_modal()`；
3. 再调用 service 的 `RefreshRemoteHead` 请求；
4. 当结果返回时，仅更新 modal 状态，不重开、不阻塞。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sync_vault_modal_smoke opening_sync_settings_refreshes_primary_head_in_background -- --exact
cargo test --test sync_vault_modal_smoke remote_head_refresh_failure_keeps_sync_settings_non_blocking -- --exact
cargo test --test vault_settings_smoke sync_settings_opens_before_remote_head_refresh_completes -- --exact
```

Expected:

- PASS，modal 会先打开，远端状态稍后异步落入 view model。

**Step 5: Commit**

```bash
git add src/app/vault/sync_service.rs src/shell/view_model.rs src/app/bootstrap.rs tests/sync_vault_modal_smoke.rs tests/vault_settings_smoke.rs
git commit -m "feat: refresh sync settings remote head in background"
```

### Task 5: 渲染极简状态卡并补齐时间格式化/UI contract

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/sync-vault-modal.slint`
- Modify: `tests/assets_modal_render_spec.rs`
- Modify: `tests/sync_vault_modal_smoke.rs`
- Modify: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing UI and render tests**

先锁定时间展示 contract：

```rust
#[test]
fn sync_modal_shows_local_and_remote_sync_timestamps() {
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.set_sync_modal_open(true);
    app.set_sync_modal_local_last_sync_text("2026-04-02 10:30".into());
    app.set_sync_modal_remote_last_update_text("2026-04-02 10:31".into());
    app.set_sync_modal_primary_revision_text("rev-0042".into());

    assert_eq!(app.get_sync_modal_local_last_sync_text().as_str(), "2026-04-02 10:30");
}
```

再在 `tests/assets_modal_render_spec.rs` 增加渲染检查：

```rust
#[test]
fn sync_modal_renders_sync_status_card_with_timestamps() {
    // set ready mode + timestamp props
    // assert modal body contains a non-empty status card region
}
```

并在 `tests/top_status_bar_ui_contract_smoke.sh` 增加字符串契约检查：

- `sync-modal-local-last-sync-text`
- `sync-modal-remote-last-update-text`
- `sync-modal-primary-revision-text`
- `sync-modal-remote-status-text`

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sync_vault_modal_smoke sync_modal_shows_local_and_remote_sync_timestamps -- --exact
cargo test --features slint-renderer-software --test assets_modal_render_spec sync_modal_renders_sync_status_card_with_timestamps -- --exact
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- FAIL，因为 `AppWindow`、`SyncModalViewState`、`SyncVaultModal` 还没有这些属性和状态卡。

**Step 3: Write the minimal implementation**

在 `src/app/bootstrap.rs` 增加统一时间格式化 helper，例如：

```rust
fn format_sync_timestamp_for_ui(raw: Option<&str>) -> String {
    // 1. try epoch millis
    // 2. try DateTime parse_from_rfc3339
    // 3. fallback to empty / Unknown
}
```

在 `ui/app-window.slint` 与 `ui/components/sync-vault-modal.slint` 补齐属性与状态卡：

- `local-last-sync-text`
- `remote-last-update-text`
- `primary-revision-text`
- `remote-status-text`
- `remote-status-loading`

在 modal body 中新增极简状态卡，放在现有 `status_text` / `summary-card` 之前或之后，但不要打乱现有配置表单布局。

要求：

- 成功状态只更新卡片内容，不新增成功 toast；
- 失败状态在卡片或现有 `error_text` 中可回看；
- 没有时间值时显示 `Never synced` / `Unknown`。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sync_vault_modal_smoke sync_modal_shows_local_and_remote_sync_timestamps -- --exact
cargo test --features slint-renderer-software --test assets_modal_render_spec sync_modal_renders_sync_status_card_with_timestamps -- --exact
bash tests/top_status_bar_ui_contract_smoke.sh
cargo test --test sync_vault_modal_smoke -- --nocapture
```

Expected:

- PASS，modal 既能显示本地/远端时间，又不破坏既有 sync settings 表单契约。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs ui/app-window.slint ui/components/sync-vault-modal.slint tests/assets_modal_render_spec.rs tests/sync_vault_modal_smoke.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: show sync status card in sync settings"
```

## Verification Sweep

在全部任务完成后，运行完整回归：

```bash
cargo test --test vault_sync_service_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test sync_vault_modal_smoke -- --nocapture
cargo test --test vault_settings_smoke -- --nocapture
cargo test --features slint-renderer-software --test assets_modal_render_spec -- --nocapture
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- `vault_sync_service_spec`: service contract、显式 runtime handle、remote refresh 去重全部通过
- `bootstrap_smoke`: manual / auto / periodic / mutation 回归全部通过
- `sync_vault_modal_smoke`: modal remote refresh 与状态展示全部通过
- `vault_settings_smoke`: titlebar sync entry 与 modal 打开契约保持稳定
- `assets_modal_render_spec`: 新状态卡不会破坏 modal 布局与 footer 可见性
- `top_status_bar_ui_contract_smoke.sh`: Rust/Slint contract 与新属性对齐

## Handoff Notes

- 如果 Task 2 完成后 slow-provider smoke 仍偶发超时，不要继续堆 sleep；先检查 service 是否仍有某条路径回到前台执行。
- 如果 Task 4 完成后 modal 打开变慢，先验证是否误把 remote head refresh 放进了打开前同步路径。
- 如果 Task 5 的 render test 失败，优先检查状态卡高度与 `ModalBodyScrollArea` 内容高度，而不是先改 footer。

