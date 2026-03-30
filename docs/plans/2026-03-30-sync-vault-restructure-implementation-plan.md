# Sync & Vault Restructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将当前不可用的 `Sync & Vault` 右侧栏重构为“titlebar 独立 `Sync` 按钮 + 状态驱动 modal + Gitee PAT 首发 + 资产域核心数据同步”的可用正式产品闭环。

**Architecture:** 保持现有 `AppWindow -> bootstrap -> ShellViewModel -> vault engine/provider` 主链路，不另起新系统。先冻结 titlebar / menu / modal 的产品契约，再把 `vault_panel_state` 迁移为独立 `SyncModalViewState`，然后收敛 remote-first 初始化、Gitee PAT provider、snapshot round-trip 与 lock/unlock 生命周期，最后移除旧正式入口并完成回归验证。

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `termwiz`, `russh`, `keyring`, vault provider stack, `cargo test`, shell smoke scripts

---

## Input Design

- 设计文档固定为 [2026-03-30-sync-vault-restructure-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-30-sync-vault-restructure-design.md)
- 实现不得偏离以下已确认决策：
  - `Sync` 是独立 titlebar 按钮，不属于 `Settings`
  - 正式 UI 不再使用右侧 `vault` panel
  - 正式 UI 隐藏 `Appearance`
  - modal 采用状态驱动，交互参考现有 `SSH` modal
  - 首次启用采用 remote-first
  - 正式 UI 首发只暴露 `Gitee`
  - `Gitee` 首发认证只支持 `PAT`
  - sync 范围覆盖资产域核心数据，排除 `ui_preferences`

## Execution Notes

- 使用 `@superpowers:test-driven-development` 执行每个任务：先写失败测试，再写最小实现，再跑通过。
- 若 modal 状态跳转或 vault 生命周期出现意外行为，不允许猜测，必须切换到 `@superpowers:systematic-debugging`。
- 默认在独立 worktree 中执行；如果在当前工作区执行，也必须严格限制改动范围为本计划列出的文件。
- 首发阶段不要顺手加入 `OAuth`、自建 sync host、多 provider 正式 UI、`ui_preferences` 同步或“临时兼容右侧 panel”。

## Task Sequence Overview

1. 冻结正式入口契约：titlebar 独立 `Sync` 按钮、隐藏 `Appearance`、拆除 `Settings -> vault` 正式路径。
2. 创建独立 `Sync` modal 与状态机契约，替换静态 `vault_panel_state` 的 UI 表达。
3. 打通 remote-first 首次启用与所有 modal 动作接线，消除死按钮。
4. 实现 Gitee PAT-only 首发 provider 闭环，同时保留内部通用 provider 架构。
5. 修正 snapshot / restore / clear contract，保证资产域核心数据 round-trip，明确排除 `ui_preferences`。
6. 下线旧正式入口并完成回归验证。

### Task 1: 冻结 titlebar / menu / settings 的正式入口契约

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/components/titlebar-menu.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/titlebar_layout_spec.rs`
- Test: `tests/titlebar_render_spec.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/top_status_bar_ui_contract_smoke.sh`
- Test: `tests/vault_settings_smoke.rs`
- Test: `tests/vault_settings_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

把现有“`Settings` 打开 `Sync & Vault` 右侧栏”的断言替换为新产品契约：

```rust
#[test]
fn titlebar_exposes_sync_as_a_first_class_action() {
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_sync_modal_open());
    app.invoke_open_sync_modal_requested();
    assert!(app.get_sync_modal_open());
}

#[test]
fn settings_no_longer_routes_into_vault_flow() {
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_settings_panel_requested();
    assert_ne!(app.get_right_panel_view().as_str(), "vault");
}
```

把 UI contract shell 脚本切换为：

- `titlebar` 存在独立 `Sync` 按钮与 `open-sync-modal-requested`
- `TitlebarMenu` 不再暴露 `Appearance`
- `Settings` 不再驱动 `vault` 右侧栏

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test titlebar_layout_spec --test titlebar_render_spec --test top_status_bar_smoke -q
cargo test --test vault_settings_smoke -q
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/vault_settings_ui_contract_smoke.sh
```

Expected:

- Rust tests仍显示 `Settings -> vault` 旧行为
- shell smoke 仍能 grep 到 `Appearance` 和 `vault` 右侧栏契约

**Step 3: Write minimal implementation**

在窗口和 titlebar 层先建立正式入口契约：

```slint
// ui/app-window.slint
callback open-sync-modal-requested();
in-out property <bool> sync-modal-open: false;
```

```slint
// ui/shell/titlebar.slint
sync-button := TitlebarIconButton {
    clicked => { root.open-sync-modal-requested(); }
}
```

```slint
// ui/components/titlebar-menu.slint
// Keep Settings only.
// Remove Appearance from formal menu.
```

同时在 `bootstrap` / `view_model` 里去掉“Settings 打开 vault panel”作为正式行为，改为只打开真实设置面。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test titlebar_layout_spec --test titlebar_render_spec --test top_status_bar_smoke -q
cargo test --test vault_settings_smoke -q
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/vault_settings_ui_contract_smoke.sh
```

Expected:

- `Sync` 已成为正式一级动作
- `Settings` 不再把 `right_panel_view` 切到 `vault`
- 正式菜单不再暴露 `Appearance`

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint ui/components/titlebar-menu.slint ui/app-window.slint src/app/bootstrap.rs src/shell/view_model.rs tests/titlebar_layout_spec.rs tests/titlebar_render_spec.rs tests/top_status_bar_smoke.rs tests/top_status_bar_ui_contract_smoke.sh tests/vault_settings_smoke.rs tests/vault_settings_ui_contract_smoke.sh
git commit -m "feat: promote sync to a first-class titlebar action"
```

### Task 2: 建立独立 `Sync` modal 与状态驱动 view model

**Files:**
- Create: `ui/components/sync-vault-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/sync_vault_modal_smoke.rs`
- Test: `tests/assets_modal_render_spec.rs`
- Test: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

新增 modal 合约测试，先锁定“状态驱动，而不是静态 dashboard”：

```rust
#[test]
fn sync_modal_defaults_to_not_configured_state() {
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();
    assert_eq!(app.get_sync_modal_mode().as_str(), "not-configured");
    assert_eq!(app.get_sync_modal_primary_action_label().as_str(), "Set up sync");
}

#[test]
fn sync_modal_does_not_reuse_right_panel_vault_copy() {
    let source = std::fs::read_to_string("ui/components/sync-vault-modal.slint").unwrap();
    assert!(!source.contains("Primary remote"));
    assert!(!source.contains("Mirror remote"));
    assert!(!source.contains("primary-action := Rectangle"));
}
```

新增 shell 合约脚本，检查：

- `SyncVaultModal` 被 `AppWindow` 正式挂载
- 存在 `sync-modal-mode`
- 存在独立 close / primary / secondary action callback
- 不复用右侧 panel 的静态 `primary/secondary/tertiary` 按钮文案模型

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sync_vault_modal_smoke -q
cargo test --test assets_modal_render_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- 新 modal 文件不存在
- `AppWindow` 仍只有右侧栏 `vault` 属性，没有 modal 契约

**Step 3: Write minimal implementation**

在 `ShellViewModel` 中新增独立 modal 状态，而不是继续挤在 `vault_panel_state`：

```rust
pub enum SyncModalMode {
    NotConfigured,
    Locked,
    UnlockedButRemoteIncomplete,
    Ready,
    SyncError,
}

pub struct SyncModalViewState {
    pub open: bool,
    pub mode: SyncModalMode,
    pub title: String,
    pub headline: String,
    pub status_text: String,
    pub error_text: String,
    pub provider_label: String,
    pub target_label: String,
    pub primary_action_label: String,
    pub secondary_action_label: String,
}
```

在 Slint 侧让 modal 用 `if root.sync-modal-mode == "..."` 切换内容区，不允许继续沿用静态 dashboard 卡片。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sync_vault_modal_smoke -q
cargo test --test assets_modal_render_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- modal 正式存在且挂在 `AppWindow`
- UI 结构是状态驱动，而不是旧右侧栏平移

**Step 5: Commit**

```bash
git add ui/components/sync-vault-modal.slint ui/app-window.slint src/shell/view_model.rs src/app/bootstrap.rs tests/sync_vault_modal_smoke.rs tests/assets_modal_render_spec.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: add dedicated sync vault modal state machine"
```

### Task 3: 打通 remote-first 首次启用和所有 modal 动作接线

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/vault/bootstrap.rs`
- Modify: `src/app/vault/model.rs`
- Test: `tests/sync_vault_modal_smoke.rs`
- Test: `tests/vault_bootstrap_spec.rs`
- Test: `tests/vault_sync_engine_spec.rs`

**Step 1: Write the failing tests**

新增“首次启用必须先有远端”的行为测试：

```rust
#[test]
fn first_enable_flow_requires_a_remote_before_local_vault_is_created() {
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "not-configured");
    assert!(app.get_sync_modal_error_text().contains("Configure a Gitee remote first"));
}
```

新增动作接线测试，覆盖：

- create/setup
- unlock
- sync now
- lock
- close

并断言每个动作都会更新 modal state，而不是只是打印日志或停留原状。

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sync_vault_modal_smoke -q
cargo test --test vault_bootstrap_spec -q
cargo test --test vault_sync_engine_spec -q
```

Expected:

- 当前 `create_local_vault_from_shell_state()` 仍可在没有远端时创建本地 vault
- modal callbacks 尚未全部接线

**Step 3: Write minimal implementation**

把正式启用语义改为 remote-first：

```rust
fn create_local_vault_from_shell_state(...) -> Result<()> {
    ensure_primary_remote_is_present_and_valid(&bundle)?;
    // only then export snapshot + wrap vault key + persist bootstrap state
}
```

在 `bootstrap` 中集中处理 modal action：

```rust
window.on_sync_modal_primary_action_requested(move || { ... });
window.on_sync_modal_unlock_requested(move |password| { ... });
window.on_sync_modal_sync_now_requested(move || { ... });
window.on_sync_modal_lock_requested(move || { ... });
window.on_sync_modal_close_requested(move || { ... });
```

要求：

- 所有可见按钮都必须有真实 action
- action 完成后必须 `sync_sync_modal_state(...)`
- 错误态必须返回 `SyncError` 或 `NotConfigured` / `Locked` 等可恢复状态

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sync_vault_modal_smoke -q
cargo test --test vault_bootstrap_spec -q
cargo test --test vault_sync_engine_spec -q
```

Expected:

- 正式 UI 不再允许“本地已创建但不可 sync”的假完成状态
- 所有 modal 动作都有状态变化和错误反馈

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs src/app/vault/bootstrap.rs src/app/vault/model.rs tests/sync_vault_modal_smoke.rs tests/vault_bootstrap_spec.rs tests/vault_sync_engine_spec.rs
git commit -m "feat: enforce remote-first sync onboarding"
```

### Task 4: 实现 Gitee PAT-only 首发 provider，并保持内部通用 provider 架构

**Files:**
- Modify: `src/app/vault/provider/gitee_gist.rs`
- Modify: `src/app/vault/model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/vault/provider/mod.rs`
- Test: `tests/vault_provider_gitee_spec.rs`
- Test: `tests/vault_bootstrap_spec.rs`

**Step 1: Write the failing tests**

先把现有 `PAT + OAuth` 断言收窄为首发正式契约：

```rust
#[test]
fn gitee_bootstrap_config_supports_pat_only_for_first_release() {
    let pat = GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
        .expect("parse gitee pat config");

    assert!(matches!(pat.auth, GiteeGistAuth::PersonalAccessToken { .. }));
    assert!(GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pkce)).is_err());
}
```

再补一个 UI / bootstrap 层测试，确保正式 UI 只暴露：

- `Personal Access Token`
- `Gist ID` 或 “create new gist” 目标字段

而不展示 `GitHub` / `S3` / `GitLab` / `OAuth` 入口。

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_provider_gitee_spec -q
cargo test --test vault_bootstrap_spec -q
```

Expected:

- 现有测试仍允许 `ProviderAuthKind::Pkce`
- UI / bootstrap 仍有通用 provider 杂项暴露

**Step 3: Write minimal implementation**

保持内部 provider registry，但把正式首发面收敛为 Gitee PAT：

```rust
pub struct GiteeRemoteDraft {
    pub personal_access_token: String,
    pub gist_id: String,
    pub create_new_gist: bool,
}
```

```rust
impl TryFrom<&BootstrapRemoteConfig> for GiteeGistProviderConfig {
    type Error = anyhow::Error;

    fn try_from(value: &BootstrapRemoteConfig) -> Result<Self, Self::Error> {
        ensure!(matches!(value.auth_kind, ProviderAuthKind::Pat), "first release supports PAT only");
        ...
    }
}
```

要求：

- 正式 UI 只出现 `Gitee`
- 内部 `ProviderKind`、provider factory、sync engine 仍保持通用扩展能力

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_provider_gitee_spec -q
cargo test --test vault_bootstrap_spec -q
```

Expected:

- `Gitee` provider 首发正式支持 `PAT`
- `Pkce` / `OAuth` 作为未实现能力被显式拒绝，而不是半支持

**Step 5: Commit**

```bash
git add src/app/vault/provider/gitee_gist.rs src/app/vault/model.rs src/app/bootstrap.rs src/app/vault/provider/mod.rs tests/vault_provider_gitee_spec.rs tests/vault_bootstrap_spec.rs
git commit -m "feat: land gitee pat-only sync provider flow"
```

### Task 5: 修正 snapshot / restore / clear contract，覆盖资产域核心数据并排除 `ui_preferences`

**Files:**
- Modify: `src/app/vault/snapshot.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/assets_catalog.rs`
- Modify: `src/app/keychain/mod.rs`
- Test: `tests/vault_snapshot_spec.rs`
- Test: `tests/vault_bootstrap_spec.rs`
- Test: `tests/assets_catalog_store.rs`

**Step 1: Write the failing tests**

先锁定新 contract：

```rust
#[test]
fn export_vault_snapshot_excludes_ui_preferences_for_first_release() {
    let snapshot = export_vault_snapshot(...).expect("export snapshot");
    assert_eq!(snapshot.ui_preferences.theme_mode, None);
    assert_eq!(snapshot.ui_preferences.always_on_top, None);
}
```

再补两个关键生命周期测试：

```rust
#[test]
fn unlock_restores_console_snippet_and_keychain_projection() {
    // create local vault, lock, unlock
    // assert console assets, snippets, keychain identities/keys all reappear
}

#[test]
fn lock_clears_decrypted_keychain_and_asset_state() {
    // after lock, assert console/snippet/keychain decrypted state cleared together
}
```

还要补 round-trip 覆盖：

- `SSH connections`
- SSH secret bundles
- keychain identity / key secret bundles
- `snippets`
- 文件 / 文件夹目录结构

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_snapshot_spec -q
cargo test --test vault_bootstrap_spec -q
cargo test --test assets_catalog_store -q
```

Expected:

- 现有 snapshot 仍导出 `ui_preferences`
- unlock / lock 对 keychain UI projection 的恢复与清理不完整

**Step 3: Write minimal implementation**

在 snapshot 导出层明确排除 `ui_preferences`：

```rust
let ui_preferences = SnapshotUiPreferences {
    theme_mode: None,
    always_on_top: None,
};
```

在 bootstrap 生命周期中修正：

```rust
fn apply_vault_snapshot_to_shell(...) -> Result<()> {
    // rebuild console assets
    // rebuild snippet assets
    // rebuild keychain catalog + projection
}

fn clear_vault_decrypted_state(...) -> Result<()> {
    // clear console/snippet/keychain decrypted state together
}
```

要求：

- 不能只“恢复数据结构”，必须同步回 `ShellViewModel` 投影
- 不能只清理 console/snippet，必须一并清理 keychain 投影与相关 secret 可见状态

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_snapshot_spec -q
cargo test --test vault_bootstrap_spec -q
cargo test --test assets_catalog_store -q
```

Expected:

- snapshot round-trip 覆盖资产域核心数据
- `ui_preferences` 不进入 sync contract
- lock / unlock 生命周期对 keychain / assets 表现一致

**Step 5: Commit**

```bash
git add src/app/vault/snapshot.rs src/app/bootstrap.rs src/shell/view_model.rs src/app/assets_catalog.rs src/app/keychain/mod.rs tests/vault_snapshot_spec.rs tests/vault_bootstrap_spec.rs tests/assets_catalog_store.rs
git commit -m "fix: complete vault snapshot restore and clear contract"
```

### Task 6: 下线旧正式入口并完成回归验证

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/components/vault-provider-card.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/vault_settings_smoke.rs`
- Test: `tests/vault_settings_ui_contract_smoke.sh`
- Test: `tests/sync_vault_modal_smoke.rs`
- Test: `tests/fluent_titlebar_assets_smoke.sh`

**Step 1: Write the failing tests**

把旧正式入口彻底写成失败条件：

```rust
#[test]
fn formal_ui_no_longer_contains_vault_right_panel_entry() {
    let source = std::fs::read_to_string("ui/shell/right-panel.slint").unwrap();
    assert!(!source.contains("text: \"Sync & Vault\""));
}
```

shell smoke 要求：

- `right-panel.slint` 不再包含正式 `vault` 文案
- `vault-provider-card.slint` 不再被正式 UI 依赖为 sync 主视图
- 打开 `Sync` modal 能完成一次 create/unlock/lock/sync 状态切换 smoke

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_settings_smoke --test sync_vault_modal_smoke -q
bash tests/vault_settings_ui_contract_smoke.sh
bash tests/fluent_titlebar_assets_smoke.sh
```

Expected:

- 旧正式入口还在 right panel
- 老 smoke 仍围绕 `Sync & Vault` right-panel contract

**Step 3: Write minimal implementation**

要求：

- 正式 UI 不再通过 right panel 暴露 `vault`
- 若 right panel 仍需为内部过渡保留，必须从正式 titlebar / menu / smoke 中完全摘掉
- 删除或改写所有把旧正式 UI 误当成现行行为的测试与文案

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test titlebar_layout_spec --test titlebar_render_spec --test top_status_bar_smoke --test vault_settings_smoke --test sync_vault_modal_smoke --test vault_provider_gitee_spec --test vault_bootstrap_spec --test vault_snapshot_spec -q
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/vault_settings_ui_contract_smoke.sh
bash tests/fluent_titlebar_assets_smoke.sh
cargo fmt --check
cargo check
```

Expected:

- 正式 UI 的所有契约都已转向 titlebar + modal
- `Gitee` PAT 首发路径和资产域 snapshot contract 全部通过回归
- 没有残留“看得到但点不了”的正式按钮

**Step 5: Commit**

```bash
git add ui/shell/right-panel.slint ui/components/vault-provider-card.slint ui/app-window.slint src/app/bootstrap.rs src/shell/view_model.rs tests/vault_settings_smoke.rs tests/vault_settings_ui_contract_smoke.sh tests/sync_vault_modal_smoke.rs tests/fluent_titlebar_assets_smoke.sh
git commit -m "refactor: retire legacy vault right-panel flow"
```

## Final Verification Checklist

- `Sync` 已成为 titlebar 独立一级按钮。
- `Settings` 不再承载 `Sync`。
- 正式 UI 已隐藏 `Appearance`。
- 正式 UI 不再依赖右侧 `vault` panel。
- `Sync` modal 以状态驱动方式工作，不再复用静态 dashboard。
- 首次启用必须配置远端，不能停留在“本地已创建但不可 sync”状态。
- modal 中所有可见动作都有真实接线。
- 正式 UI 首发只暴露 `Gitee`。
- `Gitee` 首发仅支持 `PAT`。
- snapshot round-trip 覆盖资产域核心数据。
- `ui_preferences` 不进入 sync payload，也不会在 restore 时被应用。
- lock / unlock 对 console / snippets / keychain 的恢复与清理一致。
- smoke 与 Rust 测试都不再把旧 right-panel contract 视为正式行为。
