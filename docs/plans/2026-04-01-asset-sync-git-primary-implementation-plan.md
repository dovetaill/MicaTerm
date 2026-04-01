# Asset Sync Git Primary Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将当前基于 `gist/snippet` 的弱一致 snapshot 同步路径，迁移为“`Gitee` 普通 Git 私有仓库 primary + 资产级三方 merge + attach-time merge + HTTPS/SSH 双认证”的正式多设备同步闭环。

**Architecture:** 继续沿用现有 `AppWindow -> bootstrap -> ShellViewModel -> vault engine/provider` 主链路，不另起新系统。先冻结 Git primary 的 schema、transport contract 与 UI draft，再引入稳定 `device_id` 和 `GitRepoProvider`，随后把当前 snapshot-level latest-wins 升级为资产级三方 merge，并把首次 attach、冲突可见化、旧 gist 路径降级都接入同一条同步流水线。

**Tech Stack:** Rust, Slint, Tokio, existing vault/snapshot/recovery stack, pure-Rust Git backend (`gix` + `gix-credentials`), `cargo test`, shell smoke scripts

---

## Input Design

- 设计文档固定为 `docs/plans/2026-04-01-asset-sync-git-primary-design.md`
- 实现不得偏离以下已确认决策：
  - 同步粒度是 `资产级三方 merge`
  - 强一致 primary 是 `普通 Git 私有仓库 backend`
  - 首发正式 UI 只暴露 `Gitee 普通私有仓库`
  - 首次 attach 走 `attach-time merge`
  - 当前阶段不引入 `PostgreSQL` 或用户系统
  - 同一 vault 的多设备由 `vault_id + device_id` 建模
  - Git 认证同时支持：
    - `HTTPS credentials`
    - `SSH key`
  - `gist/snippet` 不再做 primary，只保留 `import/export / backup` 路径

## Execution Notes

- 使用 `@superpowers:test-driven-development` 执行每个任务：先写失败测试，再写最小实现，再跑通过。
- 如果 Git 认证、fast-forward 冲突或 merge 结果与预期不符，立即切换到 `@superpowers:systematic-debugging`，不要凭感觉修改同步逻辑。
- 首发不要引入系统 Git 依赖；默认使用 crate 内 Git backend，不要求用户机器预装 `git`。
- `VaultHead.vault_revision` 不再依赖当前的 `rev-000x` 递增字符串；Git branch 的提交祖先关系与逻辑 vault revision 要分层处理，避免把 Git commit OID 直接写进参与内容寻址的 `head` 文件导致递归依赖。
- 旧 `gitee_gist` / `github_gist` / `gitlab_snippet` provider 在迁移期保留代码，但正式 primary 入口必须下线。
- `known_hosts`、`keychain_catalog`、SSH secrets 的 merge 范围必须明确；不要只 merge `asset_catalog` 然后把引用关系留给后面修。

## Task Sequence Overview

1. 冻结 Git primary、device identity、logical revision 与 transport revision 的 schema contract。
2. 把 sync modal / bootstrap draft 从 `gist id + PAT` 重构为 `Git repo + dual auth`。
3. 新增稳定 `device_id`、远端 repo cache 布局和 bootstrap 持久化辅助。
4. 实现 `GitRepoProvider` 与 `Gitee` 普通私有仓库首发路径，支持 `HTTPS` 与 `SSH key`。
5. 引入资产级三方 merge、tombstone、conflict result contract，覆盖 asset/keychain/secret/known_hosts 边界。
6. 用 merge engine 重写 attach-time merge 与日常 sync 流程，替换当前 snapshot-level latest-wins。
7. 将冲突和 recovery 结果正式投影到 UI，并把旧 `gist/snippet` 路径降级为 backup/import-export。
8. 完成回归验证、兼容策略确认与迁移收尾。

### Task 1: 冻结 Git primary 与元数据契约

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/vault/model.rs`
- Modify: `src/app/vault/bootstrap.rs`
- Modify: `src/app/vault/provider/mod.rs`
- Modify: `src/app/vault/mod.rs`
- Modify: `tests/vault_model_spec.rs`
- Modify: `tests/vault_bootstrap_spec.rs`
- Create: `tests/vault_provider_git_repo_spec.rs`

**Step 1: Write the failing contract tests**

在 `tests/vault_model_spec.rs` 与 `tests/vault_bootstrap_spec.rs` 中先锁定以下新契约：

- `ProviderKind` 包含 `GitRepo`
- `BootstrapRemoteLocator` 支持 `GitRepo { remote_url, branch, host_kind }`
- `ProviderAuthKind` 支持面向 Git repo 的 `HttpsCredentials` 与 `SshKey`
- 本地 bootstrap state 记录：
  - 稳定 `device_id`
  - 当前逻辑 `vault_revision`
  - 当前 transport `git_head_oid` 或等价 hint
- `VaultHead.vault_revision` 不再要求匹配 `rev-000x`

新增 `tests/vault_provider_git_repo_spec.rs`，先写纯模型级断言：

```rust
#[test]
fn git_repo_remote_round_trip_preserves_branch_and_auth_mode() {
    let remote = BootstrapRemoteConfig {
        remote_id: "remote-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind: GitHostKind::Gitee,
            remote_url: "git@gitee.com:demo/mica-vault.git".into(),
            branch: "mica-vault".into(),
        },
        credential_ref: Some("vault/bootstrap/remote-primary".into()),
        auth_kind: ProviderAuthKind::SshKey,
        last_health: None,
    };

    assert_eq!(remote.provider, ProviderKind::GitRepo);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_model_spec --test vault_bootstrap_spec --test vault_provider_git_repo_spec -- --nocapture
```

Expected:

- FAIL，因为 `GitRepo` locator / auth / device identity / transport revision hint 相关类型尚不存在。

**Step 3: Implement the minimal contract surface**

在 `src/app/vault/model.rs` 中补齐：

- `GitHostKind`
- `BootstrapRemoteLocator::GitRepo`
- `ProviderKind::GitRepo`
- `ProviderAuthKind::HttpsCredentials`
- `ProviderAuthKind::SshKey`
- 用于 sync modal 的 `GitRepoRemoteDraft`

在 `src/app/vault/bootstrap.rs` 中补齐本地 durable state 最小字段：

- `device_id`
- `logical_revision`
- `transport_revision_hint`

在 `src/app/vault/provider/mod.rs` 中补充 Git primary 所需的 provider contract 注释与占位约束，并把后续 `GitRepoProvider` 预留进模块树。

在 `Cargo.toml` 中先加入最小依赖集：

- `gix`
- `gix-credentials`

此任务只冻结 schema 与 contract，不实现 fetch/push。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_model_spec --test vault_bootstrap_spec --test vault_provider_git_repo_spec -- --nocapture
```

Expected:

- PASS，模型和 bootstrap schema 已能序列化/反序列化并表达 Git primary contract。

**Step 5: Commit**

```bash
git add Cargo.toml src/app/vault/model.rs src/app/vault/bootstrap.rs src/app/vault/provider/mod.rs src/app/vault/mod.rs tests/vault_model_spec.rs tests/vault_bootstrap_spec.rs tests/vault_provider_git_repo_spec.rs
git commit -m "feat: lock git primary sync contracts"
```

### Task 2: 将 sync settings 从 gist draft 重构为 Git repo + 双认证 draft

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/sync-vault-modal.slint`
- Modify: `tests/sync_vault_modal_smoke.rs`
- Modify: `tests/vault_settings_smoke.rs`
- Modify: `tests/vault_settings_ui_contract_smoke.sh`

**Step 1: Write the failing UI and state tests**

先把当前 `primary_gist_id / primary_pat / mirror_gist_id / mirror_pat` 契约替换为 Git repo draft：

```rust
#[test]
fn sync_modal_defaults_to_gitee_git_repo_primary_fields() {
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();
    assert_eq!(app.get_sync_modal_provider_label().as_str(), "Gitee");
    assert_eq!(app.get_sync_modal_git_remote_url().as_str(), "");
    assert_eq!(app.get_sync_modal_git_auth_mode().as_str(), "https");
}

#[test]
fn sync_modal_can_switch_between_https_and_ssh_auth_modes() {
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.set_sync_modal_git_auth_mode("ssh".into());
    assert!(app.get_sync_modal_git_ssh_private_key().len() >= 0);
}
```

在 `tests/vault_settings_ui_contract_smoke.sh` 里把旧 gist contract 改成以下检查：

- 存在 `sync-modal-git-remote-url`
- 存在 `sync-modal-git-branch`
- 存在 `sync-modal-git-auth-mode`
- 存在 `sync-modal-git-https-username`
- 存在 `sync-modal-git-https-secret`
- 存在 `sync-modal-git-ssh-private-key`
- 不再暴露 `sync-modal-primary-gist-id`
- 不再暴露 `sync-modal-primary-pat`

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test sync_vault_modal_smoke --test vault_settings_smoke -- --nocapture
bash tests/vault_settings_ui_contract_smoke.sh
```

Expected:

- FAIL，因为 UI 和 `ShellViewModel` 仍是 gist/PAT 字段。

**Step 3: Implement the minimal draft migration**

在 `src/shell/view_model.rs` 的 `SyncModalViewState` 中新增并接线：

- `git_remote_url`
- `git_branch`
- `git_auth_mode`
- `git_https_username`
- `git_https_secret`
- `git_ssh_private_key`
- `git_ssh_passphrase`

在 `ui/app-window.slint` / `ui/components/sync-vault-modal.slint` 中把输入区切到：

- `Git remote URL`
- `Branch`
- `Auth mode` (`https` / `ssh`)
- 条件渲染 `HTTPS credentials` 和 `SSH key` 区域

在 `src/app/bootstrap.rs` 中把 draft 解析从 gist 模式切到 Git repo 模式：

```rust
BootstrapRemoteLocator::GitRepo { ... }
```

首发 UI 仍固定 provider label 为 `Gitee`，但 draft 和模型不再绑定 gist。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test sync_vault_modal_smoke --test vault_settings_smoke -- --nocapture
bash tests/vault_settings_ui_contract_smoke.sh
```

Expected:

- PASS，sync settings 已转为 Gitee Git repo + dual auth draft。

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs ui/app-window.slint ui/components/sync-vault-modal.slint tests/sync_vault_modal_smoke.rs tests/vault_settings_smoke.rs tests/vault_settings_ui_contract_smoke.sh
git commit -m "feat: migrate sync settings to git repo drafts"
```

### Task 3: 加入稳定 `device_id` 与本地 Git remote cache 辅助

**Files:**
- Create: `src/app/vault/device_identity.rs`
- Modify: `src/app/vault/mod.rs`
- Modify: `src/app/vault/bootstrap.rs`
- Modify: `src/app/bootstrap.rs`
- Create: `tests/vault_device_identity_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing persistence tests**

新增 `tests/vault_device_identity_spec.rs`，锁定：

- 同一数据目录下生成的 `device_id` 可重复读取
- 删除文件后才会重新生成
- Git repo cache 根目录按 `remote_id` 稳定命名

示例：

```rust
#[test]
fn device_id_persists_for_the_same_vault_root() {
    let root = sample_vault_runtime_root("device-id");
    let first = load_or_create_device_id(root.as_path()).unwrap();
    let second = load_or_create_device_id(root.as_path()).unwrap();
    assert_eq!(first, second);
}
```

在 `tests/bootstrap_smoke.rs` 中新增场景：

- 首次创建本地 vault 后会写入 `device_id`
- 自动恢复不会刷新已有 `device_id`

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_device_identity_spec --test bootstrap_smoke -- --nocapture
```

Expected:

- FAIL，因为当前没有独立 `device_id` helper，也没有本地 remote cache contract。

**Step 3: Implement minimal persistence helpers**

创建 `src/app/vault/device_identity.rs`：

- `load_or_create_device_id(root: &Path) -> Result<String>`
- `git_remote_cache_dir(root: &Path, remote_id: &str) -> PathBuf`

在 `src/app/vault/bootstrap.rs` / `src/app/bootstrap.rs` 中接入：

- 初始化 bootstrap state 时生成并持久化 `device_id`
- 解锁、恢复、sync 都复用同一 `device_id`

本地 Git repo cache 路径约定为：

```text
<vault_root>/git-remotes/<remote_id>/
```

先不做真实 clone/fetch，只冻结路径与持久化语义。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_device_identity_spec --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS，`device_id` 与 repo cache 路径语义稳定。

**Step 5: Commit**

```bash
git add src/app/vault/device_identity.rs src/app/vault/mod.rs src/app/vault/bootstrap.rs src/app/bootstrap.rs tests/vault_device_identity_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: add stable vault device identity"
```

### Task 4: 实现 `GitRepoProvider` 与 `Gitee` 普通私有仓库首发接入

**Files:**
- Create: `src/app/vault/provider/git_repo.rs`
- Create: `src/app/vault/auth/git.rs`
- Modify: `src/app/vault/auth/mod.rs`
- Modify: `src/app/vault/provider/mod.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/vault_provider_git_repo_spec.rs`
- Modify: `tests/vault_sync_engine_spec.rs`

**Step 1: Write the failing provider tests**

在 `tests/vault_provider_git_repo_spec.rs` 中增加真实 provider contract 测试：

- `GitRepoProvider` 能用 `remote_url + branch` 读取远端 head
- `HTTPS credentials` 路径能把用户名/secret 转成 transport auth plan
- `SSH key` 路径能把私钥/passphrase 转成 transport auth plan
- 非 fast-forward push 返回 conflict
- `Gitee` host 校验能拒绝非首发 host 暴露

示例：

```rust
#[test]
fn git_repo_provider_rejects_non_fast_forward_push() {
    let provider = sample_git_repo_provider();
    let err = provider.push_revision(sample_write_request_against_stale_transport_hint())
        .expect_err("stale push should conflict");

    assert!(err.to_string().contains("non-fast-forward"));
}
```

在 `tests/vault_sync_engine_spec.rs` 中补充：

- primary 为 `GitRepoProvider` 时，`conditional_head_write` 不再是 gist 风格布尔位，而是 transport-level fast-forward 校验

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_provider_git_repo_spec --test vault_sync_engine_spec -- --nocapture
```

Expected:

- FAIL，因为 `GitRepoProvider` 与 Git auth transport 还不存在。

**Step 3: Implement the minimal provider**

创建 `src/app/vault/auth/git.rs`：

- `GitAuthMode`
- `GitTransportAuthPlan`
- `build_https_auth_plan(...)`
- `build_ssh_auth_plan(...)`

创建 `src/app/vault/provider/git_repo.rs`，实现：

- 基于本地 repo cache 的 open/init
- fetch remote branch
- 从最新 commit tree 读取：
  - `vault-head.json`
  - `vault-manifest.bin`
  - `vault-snapshot.bin`
- 写新 commit tree
- push branch，要求 fast-forward only

首发 repo tree 固定为：

```text
vault-head.json
vault-manifest.bin
vault-snapshot.bin
```

不要在 Git repo 里继续沿用 `vault-rev-xxxx-pack-0000.bin` 这类 gist 时代布局；Git commit history 本身就是版本历史。

在 `src/app/bootstrap.rs` 的 provider factory 中接入 `ProviderKind::GitRepo`，但 UI host 限定首发只接受 `Gitee`。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_provider_git_repo_spec --test vault_sync_engine_spec -- --nocapture
```

Expected:

- PASS，Git repo primary 已能完成基础 fetch/read/write/push/冲突检测。

**Step 5: Commit**

```bash
git add src/app/vault/provider/git_repo.rs src/app/vault/auth/git.rs src/app/vault/auth/mod.rs src/app/vault/provider/mod.rs src/app/bootstrap.rs tests/vault_provider_git_repo_spec.rs tests/vault_sync_engine_spec.rs
git commit -m "feat: add git repo primary provider"
```

### Task 5: 引入资产级三方 merge contract

**Files:**
- Create: `src/app/vault/merge.rs`
- Modify: `src/app/vault/model.rs`
- Modify: `src/app/keychain/model.rs`
- Modify: `src/app/vault/snapshot.rs`
- Modify: `src/app/assets_catalog/mapper.rs`
- Modify: `tests/vault_snapshot_spec.rs`
- Create: `tests/vault_merge_spec.rs`

**Step 1: Write the failing merge tests**

在 `tests/vault_merge_spec.rs` 中先锁定 4 类关键场景：

1. A/B 各自新增不同资产 -> merge 结果 union
2. A 删除资产，B 修改同一资产 -> 产生 conflict result，不静默覆盖
3. keychain identity 与引用它的 SSH asset 同步新增 -> 结果保持引用完整
4. `known_hosts` 走保守 union，不参与 destructive overwrite

示例：

```rust
#[test]
fn merge_unions_assets_added_on_different_devices() {
    let result = merge_snapshots(base(), local_with_asset("asset-a"), remote_with_asset("asset-b"));

    assert!(result.merged.asset_catalog.nodes.contains_key("asset-a"));
    assert!(result.merged.asset_catalog.nodes.contains_key("asset-b"));
    assert!(result.conflicts.is_empty());
}
```

在 `tests/vault_snapshot_spec.rs` 中补充：

- snapshot round-trip 保留 merge metadata / tombstone metadata

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_merge_spec --test vault_snapshot_spec -- --nocapture
```

Expected:

- FAIL，因为当前没有三方 merge engine，也没有 merge metadata/tombstone contract。

**Step 3: Implement the minimal merge engine**

创建 `src/app/vault/merge.rs`：

- `MergeInput { base, local, remote, device_id }`
- `MergeResult { merged, conflicts, recovery_actions }`
- `merge_snapshots(...)`

在 `src/app/vault/model.rs` 与 `src/app/keychain/model.rs` 中补充最小 merge metadata：

- `last_modified_at`
- `last_modified_by_device`
- `deleted_at` / tombstone

在 `src/app/vault/snapshot.rs` 中确保这些元数据会进出 snapshot。

明确首发 merge 范围：

- `asset_catalog`：资产级三方 merge
- `keychain_catalog`：节点级三方 merge
- SSH secret bundles：按引用节点和修改来源跟随 merge
- `known_hosts`：union-only
- `sync_preferences` / `ui_preferences`：不参与冲突裁决

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_merge_spec --test vault_snapshot_spec -- --nocapture
```

Expected:

- PASS，三方 merge 已能稳定处理新增、删除/修改冲突、引用完整性和 `known_hosts` union。

**Step 5: Commit**

```bash
git add src/app/vault/merge.rs src/app/vault/model.rs src/app/keychain/model.rs src/app/vault/snapshot.rs src/app/assets_catalog/mapper.rs tests/vault_merge_spec.rs tests/vault_snapshot_spec.rs
git commit -m "feat: add asset-level three-way merge"
```

### Task 6: 用 merge engine 重写 attach-time merge 与日常 sync

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/vault/engine.rs`
- Modify: `src/app/vault/sync_decision.rs`
- Modify: `src/app/vault/recovery.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Create: `tests/vault_attach_merge_spec.rs`
- Modify: `tests/vault_sync_decision_spec.rs`

**Step 1: Write the failing integration tests**

在 `tests/vault_attach_merge_spec.rs` 中新增：

- 本地无 bootstrap，但本地有资产、远端也有资产 -> 走 attach-time merge，不直接 remote-first overwrite
- attach 完成后会生成新的 merged revision 并 push 到 Git primary

在 `tests/bootstrap_smoke.rs` 中新增：

- 设备 C 先新增本地资产，再接入已有远端仓库 -> 当前资产集包含本地与远端 union
- 日常 sync 遇到同资产冲突 -> 不再 snapshot-level latest-wins，而是进入 merge engine

在 `tests/vault_sync_decision_spec.rs` 中替换旧预期：

- 双端都变更时不再仅比较时间直接 `Push` / `Pull`
- 新 contract 应至少能返回 `MergeThenPush`、`PullOnly`、`PushOnly`、`Noop`

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_attach_merge_spec --test bootstrap_smoke --test vault_sync_decision_spec -- --nocapture
```

Expected:

- FAIL，因为当前仍是 snapshot-level latest-wins 和 remote-first recover。

**Step 3: Implement minimal pipeline changes**

在 `src/app/vault/sync_decision.rs` 中把当前 `Push / Pull / Noop` 提升为 merge-aware 计划：

```rust
pub enum SyncAction {
    Noop,
    PullOnly,
    PushOnly,
    MergeThenPush,
}
```

在 `src/app/bootstrap.rs` 中改写两个关键入口：

- `recover_local_vault_from_primary_remote(...)`
- `sync_local_vault(...)`

新流程：

1. fetch remote head + transport revision hint
2. 导出当前本地 snapshot
3. 判断是否：
   - 空远端首推
   - 纯 pull
   - 纯 push
   - merge
4. 需要 merge 时调用 `merge_snapshots(...)`
5. 如果 merge 结果有 conflict，先持久化 recovery / conflict records
6. 提交 merged snapshot 到 Git primary

attach-time merge 特别规则：

- 本地无 bootstrap state 且远端非空时，若当前 shell 已有资产或 keychain 数据，则不再 remote-first 覆盖，必须走 `merge_snapshots(None, local, remote)`

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_attach_merge_spec --test bootstrap_smoke --test vault_sync_decision_spec -- --nocapture
```

Expected:

- PASS，attach-time merge 与日常 sync 都已切到 merge-aware pipeline。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/vault/engine.rs src/app/vault/sync_decision.rs src/app/vault/recovery.rs tests/vault_attach_merge_spec.rs tests/bootstrap_smoke.rs tests/vault_sync_decision_spec.rs
git commit -m "feat: wire attach-time merge into sync pipeline"
```

### Task 7: 把 conflict / recovery 正式投影到 UI，并下线 gist primary 入口

**Files:**
- Create: `src/app/vault/conflict_inbox.rs`
- Modify: `src/app/vault/mod.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/sync-vault-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/sync_vault_modal_smoke.rs`
- Create: `tests/vault_conflict_inbox_spec.rs`
- Modify: `tests/vault_provider_gitee_spec.rs`
- Modify: `tests/vault_settings_smoke.rs`

**Step 1: Write the failing visibility tests**

在 `tests/vault_conflict_inbox_spec.rs` 中锁定：

- merge 结果中的 conflicts 会进入本地 inbox
- inbox entry 至少包含：
  - `asset_id` 或逻辑目标
  - `conflict_kind`
  - `local_device_id`
  - `remote_device_id`
  - `captured_at`

在 `tests/sync_vault_modal_smoke.rs` / `tests/vault_settings_smoke.rs` 中锁定：

- sync modal 会显示 conflict/recovery summary
- 正式 primary 选项里不再出现 `Gitee Gist`

在 `tests/vault_provider_gitee_spec.rs` 中替换期望：

- `gitee_gist` 仍可读取/写入 backup/import-export payload
- 但不再被 provider factory 用作正式 primary

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test vault_conflict_inbox_spec --test sync_vault_modal_smoke --test vault_settings_smoke --test vault_provider_gitee_spec -- --nocapture
```

Expected:

- FAIL，因为当前没有 conflict inbox，也仍默认使用 gist provider 首发路径。

**Step 3: Implement minimal inbox and provider downgrade**

创建 `src/app/vault/conflict_inbox.rs`：

- `ConflictInboxEntry`
- `persist_conflict_entries(...)`
- `load_conflict_entries(...)`

在 `src/app/bootstrap.rs` 中把 merge/recovery 结果写入 inbox，并在 `ShellViewModel` 中投影：

- conflict count
- latest conflict summary
- review action availability

在 `ui/components/sync-vault-modal.slint` / `ui/app-window.slint` 中加入：

- `sync-modal-conflict-count`
- `sync-modal-conflict-summary`

在 provider factory / settings 流程里移除 gist provider 的正式 primary 暴露，只保留 Git repo primary。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test vault_conflict_inbox_spec --test sync_vault_modal_smoke --test vault_settings_smoke --test vault_provider_gitee_spec -- --nocapture
```

Expected:

- PASS，冲突与 recovery 已有正式 UI 入口，gist primary 已从正式路径移除。

**Step 5: Commit**

```bash
git add src/app/vault/conflict_inbox.rs src/app/vault/mod.rs src/shell/view_model.rs src/app/bootstrap.rs ui/components/sync-vault-modal.slint ui/app-window.slint tests/vault_conflict_inbox_spec.rs tests/sync_vault_modal_smoke.rs tests/vault_settings_smoke.rs tests/vault_provider_gitee_spec.rs
git commit -m "feat: surface sync conflicts and retire gist primary"
```

### Task 8: 全量回归、兼容收尾与文档确认

**Files:**
- Modify: `verification.md`
- Modify: `docs/plans/2026-04-01-asset-sync-git-primary-design.md`
- Modify: `docs/plans/2026-04-01-asset-sync-git-primary-implementation-plan.md`
- Reference: `tests/bootstrap_smoke.rs`
- Reference: `tests/vault_provider_git_repo_spec.rs`
- Reference: `tests/vault_merge_spec.rs`
- Reference: `tests/vault_attach_merge_spec.rs`
- Reference: `tests/sync_vault_modal_smoke.rs`

**Step 1: Run the focused regression suite**

Run:

```bash
cargo test --test vault_model_spec --test vault_bootstrap_spec --test vault_device_identity_spec --test vault_provider_git_repo_spec --test vault_merge_spec --test vault_attach_merge_spec --test vault_sync_engine_spec --test vault_sync_decision_spec --test vault_snapshot_spec --test bootstrap_smoke --test sync_vault_modal_smoke --test vault_settings_smoke -- --nocapture
bash tests/vault_settings_ui_contract_smoke.sh
```

Expected:

- PASS，核心模型、provider、merge、attach、bootstrap、UI contract 全部通过。

**Step 2: Run secondary compatibility checks**

Run:

```bash
cargo test --test vault_provider_gitee_spec --test vault_provider_github_spec --test vault_provider_gitlab_spec --test vault_provider_s3_spec -- --nocapture
```

Expected:

- PASS，旧 provider 仍可作为 backup/import-export 路径存在，不影响新 primary。

**Step 3: Write verification notes**

在 `verification.md` 中记录：

- Git primary 首发路径
- dual auth 验证结果
- attach-time merge 核心场景
- conflict inbox 可见化结果
- 兼容旧 gist provider 的回归结果

同时回写设计/计划文档中的任何最终落地差异，但不要扩 scope。

**Step 4: Commit**

```bash
git add verification.md docs/plans/2026-04-01-asset-sync-git-primary-design.md docs/plans/2026-04-01-asset-sync-git-primary-implementation-plan.md
git commit -m "docs: verify git primary asset sync rollout"
```

## Final Verification Gate

在声称功能完成前，必须至少复核以下场景：

1. 设备 A、B 离线期间各自新增不同资产，重新同步后两边都能看到 union。
2. 设备 C 先录本地资产，再 attach 到已有远端仓库，不会 remote-first 覆盖。
3. `Gitee` 普通私有仓库 primary 的 stale push 会因 non-fast-forward 被拒绝。
4. `HTTPS credentials` 与 `SSH key` 两种 auth 模式都能走通。
5. 冲突结果不是静默落盘，而是能在正式 UI 中看到 summary。
6. 正式 primary 设置路径不再出现 `gist/snippet` 选项。

## Notes for the Implementer

- 如果 `gix` 的 SSH key 注入在 Windows 上出现库能力缺口，不要把系统 Git 依赖直接硬编码进产品路径；应先把 transport auth 隔离在 `src/app/vault/auth/git.rs`，确保 backend 替换只影响一层。
- 如果发现 keychain/secret merge 复杂度高于预期，优先保证“引用完整 + 不静默丢数据”，而不是冒险做过度自动合并。
- 如果 attach-time merge 的无共同 base 情况无法安全自动提交，允许先落“生成 merge preview + conflict inbox”，但不能回退成 remote-first 覆盖。
