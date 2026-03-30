# Assets Snippets Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在现有 `Assets Sidebar`、本地 `redb` catalog 和 `VaultSnapshot` 主路径上，把 `Snippets` 从占位导航升级为可管理、可持久化、可同步的 `Package + Snippet` 模块，并接通 `create / edit / delete / paste / run` 基础交互。

**Architecture:** 沿用当前 `Rust state -> bootstrap bridge -> Slint` 的单向真相源模式，不新开第二套 snippets 基础设施。实现时先扩展共享 schema 与 mapper，再扩展 `ShellViewModel` 和 snippets 树投影，随后补 Slint 面板、modal、context menu 和 bootstrap 动作桥接，最后串联本地存储、vault snapshot 与回归验证，确保 `Window Console` 现有 explorer 和 SSH runtime 不回归。

**Tech Stack:** Rust, Slint, redb, VaultSnapshot, cargo test, shell smoke scripts

**Execution Rules:** 每个任务都先走 `@superpowers:test-driven-development`。完成所有任务前，必须执行 `@superpowers:verification-before-completion`，并收集测试输出后再宣称完成。

---

## 执行前提

- 本计划只针对 [assets-snippets-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-28-assets-snippets-design.md) 的已确认方案。
- 实施时禁止把 snippets 拆成平行于 `assets_catalog` / `vault` 的第二套系统。
- 实施时禁止把 `Package` 简化成任意层 `Folder`。
- 默认交互必须保持：
  - 单击选中
  - 双击 `Paste`
  - `Run` 为显式动作
- 若实现过程中发现共享 schema 风险过高，先停在对应任务并回到设计文档中记录偏差，不要私自改方案。

### 任务依赖顺序

1. 扩展共享领域模型与持久化 schema
2. 扩展 snippets 运行时树投影与 view model
3. 接入 snippets 左侧 UI、create popover 与 modal
4. 桥接 bootstrap、context menu 与 paste/run 动作
5. 完成本地存储、vault snapshot 映射与全量回归

---

### Task 1: 扩展共享领域模型与持久化 schema

**Files:**
- Modify: `src/shell/assets.rs`
- Modify: `src/app/assets_catalog/model.rs`
- Modify: `src/app/assets_catalog/mapper.rs`
- Modify: `src/app/assets_catalog/redb_store.rs`
- Modify: `src/app/vault/model.rs`
- Test: `tests/assets_catalog_domain.rs`
- Test: `tests/assets_catalog_store.rs`
- Test: `tests/vault_snapshot_spec.rs`

**Step 1: Write the failing tests**

先在现有测试里补 snippets 领域约束：

- `tests/assets_catalog_domain.rs`
  - 断言运行时与持久化模型现在支持 `SnippetPackage` 与 `Snippet`
  - 断言 snippets 节点必须带 `domain = snippets`
  - 断言 package 节点不能持有 snippet 以外的子类型
- `tests/assets_catalog_store.rs`
  - 断言包含根层未分组 snippet、一个 package、package 内 snippet 的 catalog 能 round-trip
- `tests/vault_snapshot_spec.rs`
  - 断言同一份 snapshot 可同时包含 console 资产和 snippets 资产

示例断言：

```rust
assert_eq!(stored.kind, PersistedAssetKind::Snippet);
assert_eq!(stored.domain, PersistedAssetDomain::Snippets);
assert_eq!(roundtrip.nodes["snippet-1"].title, "Deploy prod");
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_catalog_store --test vault_snapshot_spec -- --nocapture
```

Expected:

- FAIL，因为当前 schema 和 mapper 还没有 `SnippetPackage`、`Snippet`、`domain` 或等价域隔离字段。

**Step 3: Write the minimal implementation**

在共享模型中补齐最小领域扩展：

- 在 `src/shell/assets.rs` 增加：
  - `AssetDomain`
  - `SnippetPackage`
  - `Snippet`
  - snippets payload 结构，例如：

```rust
pub struct AssetSnippetSpec {
    pub script: String,
    pub package_id: Option<String>,
}
```

- 在 `src/app/assets_catalog/model.rs` 与 `src/app/vault/model.rs` 增加对应的：
  - `PersistedAssetDomain` / `VaultAssetDomain`
  - `PersistedAssetKind::SnippetPackage`
  - `PersistedAssetKind::Snippet`
  - 对应 payload
- 在 `src/app/assets_catalog/mapper.rs` 和 `src/app/assets_catalog/redb_store.rs` 完成最小 round-trip 支持

约束要求：

- `Window Console` 现有 `Folder / SshConnection` 语义不变
- snippets 进入共享 schema，但必须能按 `domain` 或等价逻辑根区分域
- `Package` 单层约束先通过类型和父子校验接口表达

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_catalog_store --test vault_snapshot_spec -- --nocapture
```

Expected:

- PASS，且已有 console 资产 round-trip 断言仍通过。

**Step 5: Commit**

```bash
git add src/shell/assets.rs src/app/assets_catalog/model.rs src/app/assets_catalog/mapper.rs src/app/assets_catalog/redb_store.rs src/app/vault/model.rs tests/assets_catalog_domain.rs tests/assets_catalog_store.rs tests/vault_snapshot_spec.rs
git commit -m "feat: extend asset schema for snippets"
```

---

### Task 2: 扩展 snippets 运行时树投影与 `ShellViewModel`

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/sidebar.rs`
- Test: `tests/shell_view_model.rs`
- Test: `tests/assets_explorer_projection.rs`
- Test: `tests/assets_sidebar_toolbar_spec.rs`

**Step 1: Write the failing tests**

补 view model 和投影测试，锁定 snippets 域行为：

- `tests/shell_view_model.rs`
  - snippets 根层允许未分组 snippet
  - package 不能嵌套 package
  - snippets create/edit/delete 会修改 snippets 域，而不会污染 console 域
  - 双击 snippets 行默认触发 `Paste` 动作入口，而不是 `Run`
- `tests/assets_explorer_projection.rs`
  - `active_sidebar_destination = snippets` 时只投影 snippets 域
  - 根层 package 和未分组 snippet 同时显示
  - package 展开后显示内部 snippet
- `tests/assets_sidebar_toolbar_spec.rs`
  - snippets toolbar 不再是单一直连 `new-snippet`
  - snippets create 按钮改为 popover 模式

示例断言：

```rust
assert_eq!(rows[0].kind, ConsoleAssetKind::SnippetPackage);
assert_eq!(rows[1].kind, ConsoleAssetKind::Snippet);
assert!(descriptor.uses_create_popover);
assert_eq!(view_model.pending_snippet_activation(), Some(SnippetActivation::Paste));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test shell_view_model --test assets_explorer_projection --test assets_sidebar_toolbar_spec -- --nocapture
```

Expected:

- FAIL，因为当前 `console_asset_tree`、toolbar descriptor 和 snippets 动作入口都还不存在真实实现。

**Step 3: Write the minimal implementation**

在 `src/shell/view_model.rs` 做最小扩展：

- 为 snippets 建立真实状态，而不是继续占位字符串
- 提供 snippets 树投影接口，例如：

```rust
pub fn visible_snippet_rows(&self) -> Vec<VisibleAssetRow>
pub fn handle_snippet_create_action(&mut self, action_id: &str)
pub fn begin_snippet_activation(&mut self, snippet_id: &str, mode: SnippetActivation)
```

- 保持 console/snippets 通过 `active_sidebar_destination` 和 `domain` 隔离
- 在 `src/shell/sidebar.rs` 把 snippets toolbar descriptor 改为：
  - `uses_create_popover = true`
  - create menu 入口供 `New Snippet` 和 `New Package` 共用

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test shell_view_model --test assets_explorer_projection --test assets_sidebar_toolbar_spec -- --nocapture
```

Expected:

- PASS，且现有 console 投影与 toolbar 行为不回归。

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/shell/sidebar.rs tests/shell_view_model.rs tests/assets_explorer_projection.rs tests/assets_sidebar_toolbar_spec.rs
git commit -m "feat: add snippets runtime projection"
```

---

### Task 3: 接入 snippets 左侧 UI、create popover 与 modal

**Files:**
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/components/assets-create-menu.slint`
- Modify: `ui/app-window.slint`
- Create: `ui/components/assets-snippet-modal.slint`
- Create: `ui/components/assets-snippet-package-modal.slint`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_modal_ui_contract_smoke.sh`
- Test: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Write the failing tests**

补 UI 契约和 smoke 测试：

- `tests/assets_sidebar_toolbar_smoke.rs`
  - 切到 snippets 后 create menu 包含 `New Snippet` 与 `New Package`
- `tests/assets_modal_smoke.rs`
  - `New Snippet` 打开 snippet modal，字段包含 `name/script/package`
  - `New Package` 打开轻量 modal，只包含 name
- `tests/assets_modal_ui_contract_smoke.sh`
  - snippets modal 文案、字段 id、按钮文案与验证区存在

示例断言：

```rust
assert_eq!(app.get_asset_modal_kind().as_str(), "new-snippet");
assert_eq!(app.get_asset_snippet_modal_script().as_str(), "");
assert!(menu_text.contains("New Package"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke --test assets_sidebar_toolbar_smoke -- --nocapture
./tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL，因为当前 snippets 面板没有真实树列表，也没有 snippet/package modal 组件。

**Step 3: Write the minimal implementation**

在 Slint 层做最小接线：

- `ui/shell/assets-sidebar.slint`
  - 把 `snippets` 分支从静态文案改为真实 tree 列表分支
- `ui/components/assets-create-menu.slint`
  - 在 snippets 上下文里显示两项：
    - `New Snippet`
    - `New Package`
- 新建：
  - `ui/components/assets-snippet-modal.slint`
  - `ui/components/assets-snippet-package-modal.slint`
- `ui/app-window.slint`
  - 暴露 snippets modal 所需属性与回调

组件要求：

- 沿用当前方角、Fluent icon、校验消息区、footer 按钮语言
- 不在本任务引入右侧详情编辑器

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_modal_smoke --test assets_sidebar_toolbar_smoke -- --nocapture
./tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- PASS，且现有 folder/ssh modal 不回归。

**Step 5: Commit**

```bash
git add ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/components/assets-create-menu.slint ui/app-window.slint ui/components/assets-snippet-modal.slint ui/components/assets-snippet-package-modal.slint tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh tests/assets_sidebar_toolbar_smoke.rs
git commit -m "feat: add snippets sidebar and modals"
```

---

### Task 4: 桥接 bootstrap、context menu 与 `Paste / Run` 动作

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/context_menu.rs`
- Modify: `ui/app-window.slint`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/assets_context_menu_spec.rs`
- Test: `tests/assets_context_menu_smoke.rs`

**Step 1: Write the failing tests**

补桥接层与 context menu 测试：

- `tests/bootstrap_smoke.rs`
  - snippets 的 create/edit/delete 回调能从 `AppWindow` 走到 `ShellViewModel`
  - 双击 snippets 行触发 `Paste`
- `tests/assets_context_menu_spec.rs`
  - snippets 行 context menu 包含：
    - `Paste`
    - `Run`
    - `Edit`
    - `Delete`
  - package 行不应有 `Run`
- `tests/assets_context_menu_smoke.rs`
  - snippets 的显式 `Run` 动作与 `Paste` 动作都能进入对应 view model 分支

示例断言：

```rust
assert_eq!(action.id, "run-snippet");
assert_eq!(action.state, ContextMenuActionState::Enabled);
assert_eq!(view_model.last_snippet_activation(), Some(SnippetActivation::Paste));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- FAIL，因为 bootstrap 还没有 snippets 专用属性同步和动作桥接，context menu 也没有 snippets 行为。

**Step 3: Write the minimal implementation**

在 `src/app/bootstrap.rs` 补 snippets 桥接：

- snippets create menu action 分发
- snippets modal 字段同步
- snippets 行激活默认走 `Paste`

在 `src/shell/context_menu.rs` 增加 snippets 动作树：

```rust
"paste-snippet"
"run-snippet"
"edit-snippet"
"delete-snippet"
"new-package"
```

在 `src/shell/view_model.rs` 增加动作分发：

- `Paste` 只生成待粘贴动作
- `Run` 走显式执行分支
- 不允许 package 触发 `Run`

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- PASS，且现有 console context menu 与 SSH create/connect 行为不回归。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs src/shell/context_menu.rs ui/app-window.slint tests/bootstrap_smoke.rs tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs
git commit -m "feat: wire snippets actions through bootstrap"
```

---

### Task 5: 完成本地存储、vault snapshot 映射与全量回归

**Files:**
- Modify: `src/app/assets_catalog/mapper.rs`
- Modify: `src/app/assets_catalog/redb_store.rs`
- Modify: `src/app/vault/snapshot.rs`
- Modify: `src/app/vault/model.rs`
- Test: `tests/assets_catalog_store.rs`
- Test: `tests/assets_persistence_contract_smoke.sh`
- Test: `tests/vault_snapshot_spec.rs`
- Test: `tests/assets_explorer_smoke.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

补最终回归测试：

- `tests/assets_catalog_store.rs`
  - snippets 保存后重启可加载
- `tests/assets_persistence_contract_smoke.sh`
  - snippets 字段进入持久化输出
- `tests/vault_snapshot_spec.rs`
  - snippets 进入 snapshot export/import round-trip
- `tests/assets_explorer_smoke.rs`
  - snippets 不影响 console explorer 现有行为
- `tests/ssh_connect_tabs_ui_contract_smoke.sh`
  - snippets 集成后 SSH modal/runtime 合约仍通过

示例断言：

```rust
assert_eq!(restored_snippet.script, "kubectl rollout restart deploy/api");
assert_eq!(snapshot.asset_catalog.nodes["snippet-1"].title, "Restart API");
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_catalog_store --test vault_snapshot_spec --test assets_explorer_smoke -- --nocapture
./tests/assets_persistence_contract_smoke.sh
./tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- FAIL，因为 snippets 还没有真正进入 store、snapshot 与跨层回归路径。

**Step 3: Write the minimal implementation**

收尾共享路径：

- `src/app/assets_catalog/mapper.rs`
  - 完成 snippets 与 runtime/vault 的双向映射
- `src/app/assets_catalog/redb_store.rs`
  - 完成 snippets payload 的编码、解码和 schema 升级
- `src/app/vault/snapshot.rs`
  - 完成 snippets export/import

要求：

- 保存/加载 snippets 时不影响 console 节点顺序和 payload
- vault snapshot round-trip 后，package 与未分组 snippet 的根层结构保持不变

**Step 4: Run the verification suite**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_catalog_store --test shell_view_model --test assets_explorer_projection --test assets_modal_smoke --test assets_sidebar_toolbar_smoke --test bootstrap_smoke --test assets_context_menu_spec --test assets_context_menu_smoke --test vault_snapshot_spec -- --nocapture
./tests/assets_modal_ui_contract_smoke.sh
./tests/assets_persistence_contract_smoke.sh
./tests/assets_context_menu_ui_contract_smoke.sh
./tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- 全部 PASS
- snippets 功能就位
- console explorer、SSH create/connect/runtime 合约无回归

**Step 5: Commit**

```bash
git add src/app/assets_catalog/mapper.rs src/app/assets_catalog/redb_store.rs src/app/vault/snapshot.rs src/app/vault/model.rs tests/assets_catalog_store.rs tests/assets_persistence_contract_smoke.sh tests/vault_snapshot_spec.rs tests/assets_explorer_smoke.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: persist and sync snippets assets"
```

---

## 完成定义

只有满足以下条件，才能宣称 `assets-snippets` 实现完成：

- `Snippets` 从占位导航升级为真实树模块；
- `Package` 单层约束生效；
- 根层支持未分组 snippet；
- 双击默认 `Paste`，`Run` 只通过显式动作触发；
- snippets 已进入共享本地持久化与 `VaultSnapshot` 主路径；
- console explorer 与 SSH runtime 相关测试无回归。

## 最终验证提醒

实现结束前，必须执行：

- `@superpowers:verification-before-completion`

并把通过的测试命令与输出摘要写回交付说明。
