# SFTP Two-Layer Workspace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把当前“右侧唯一 SFTP 面板”升级为“两层 SFTP”模型：右侧 `Quick Browser` 负责轻量浏览，中部 `SFTP Workspace Tab` 负责更完整的文件工作流，并让两者通过独立 `FileBrowserSession` 解耦。

**Architecture:** 先把工作区实体从“终端 session = tab”改成“workspace tab = terminal | sftp | launcher”，再引入独立 `FileBrowserSession` 与 `HostProfileRef`，让右侧 quick browser 和中部 SFTP tab 都基于可复制的浏览会话工作。最后分别改右侧 UI、主工作区 UI 与 bootstrap/binder，把 `Expand to Workspace`、锁定模式和断连重连串起来。

**Tech Stack:** Rust, Slint, Tokio, `russh`, `russh-sftp`, existing `SessionManager`, `SftpBrowserController`, `cargo test`, `cargo check`

---

## Input Design

- 设计基线固定为 `docs/plans/2026-04-15-sftp-two-layer-workspace-design.md`
- 已确认的产品决策，不允许在实现时漂移：
  - 右侧是 `Quick Browser`，不是唯一主 SFTP 入口
  - 中部新增 `SFTP Workspace Tab`
  - `Quick Browser` 默认 `Follow Active Terminal`
  - `Expand to Workspace` 会创建新的独立 SFTP tab，并默认锁定
  - terminal tab 关闭后，SFTP tab 保留并进入 `Disconnected / Reconnect`
  - reconnect 只恢复文件浏览，不自动新开 terminal tab
  - 第一阶段不做系统级独立窗口
  - 第一阶段不强推 heavyweight `ConnectionId`；优先 `HostProfileRef`
  - `Permissions` 列只预留，不作为 MVP 阻塞项

## Execution Notes

- 每个 task 都先用 `@superpowers:test-driven-development`：先写失败测试/结构 smoke，再做最小实现，再跑通过。
- 如果 workspace tab、SFTP projection、Slint callback 接线出现不符合预期的回归，不允许猜测，立即切换 `@superpowers:systematic-debugging`。
- 实现期间不要碰当前用户已修改但与本任务无关的文件：`tests/assets_modal_render_spec.rs`、`tests/terminal_layout_harfbuzz_spec.rs`。
- 尽量保持现有对外路径稳定；优先在现有模块下增量扩展，而不是大范围重命名路径。
- 快速浏览与工作区浏览必须基于不同 `FileBrowserSession`；禁止回退成共享全局可变 SFTP 状态。
- 如果开始编码，优先在独立 worktree 中执行；当前文档阶段先不改动代码。

## Task Sequence Overview

1. 冻结新实体 contract：`WorkspaceTabId`、`FileBrowserSessionId`、`HostProfileRef`、`WorkspaceTab::Sftp`
2. 把 workspace 状态从“active session”重构为“active workspace tab + optional active terminal session”
3. 引入独立 `FileBrowserSession` registry，并让 quick browser / workspace SFTP tab 能复制会话状态
4. 把右侧面板改造成 `Quick Browser`，补齐 `Expand`、badge、follow/locked、path UX
5. 在主工作区加入 `SFTP Workspace Tab` UI 分支和更完整表格
6. 接通 expand、锁定、terminal close、SFTP reconnect 生命周期
7. 做回归验证、收口文档和最终提交

### Task 1: Freeze the new SFTP identities and workspace-tab contracts

**Files:**
- Create: `tests/workspace_sftp_tab_contract_smoke.sh`
- Create: `tests/sftp_file_browser_session_spec.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `src/app/sftp/model.rs`
- Modify: `src/app/sftp/mod.rs`
- Create: `src/app/sftp/browser_session.rs`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing tests**

- 新增结构 smoke，断言：
  - `WorkspaceTabKind` 包含 `Sftp`
  - `WorkspaceTab` 不再只靠 `session_id` 做唯一身份
  - `src/app/sftp/browser_session.rs` 存在
- 新增 `tests/sftp_file_browser_session_spec.rs`，锁定以下最小 contract：

```rust
#[test]
fn cloned_workspace_browser_inherits_host_and_path_but_not_shared_selection() {
    let quick = FileBrowserSession::quick_browser(
        HostProfileRef::new("asset-prod"),
        "/srv/app",
    );

    let expanded = quick.clone_for_workspace();

    assert_eq!(expanded.host_profile_ref, quick.host_profile_ref);
    assert_eq!(expanded.current_path, "/srv/app");
    assert_ne!(expanded.file_browser_session_id, quick.file_browser_session_id);
    assert!(expanded.selected_entry_ids.is_empty());
}
```

**Step 2: Run tests to verify they fail**

Run:
```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
cargo test --test workspace_tabs_spec --test sftp_file_browser_session_spec -q
```

Expected: FAIL，因为 `WorkspaceTab::Sftp`、`FileBrowserSession` 与新 id contract 还不存在。

**Step 3: Write minimal implementation**

- 在 `src/app/sftp/browser_session.rs` 新增最小模型：

```rust
pub type FileBrowserSessionId = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProfileRef {
    pub asset_id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBrowserSession {
    pub file_browser_session_id: FileBrowserSessionId,
    pub host_profile_ref: HostProfileRef,
    pub current_path: String,
    pub selected_entry_ids: Vec<String>,
}
```

- 在 `src/shell/tabs.rs` 把 `WorkspaceTab` 改成可承载 terminal / sftp / launcher 的基础结构，并补一个最小 `WorkspaceTab::sftp(...)` 构造器。
- 在 `src/app/sftp/mod.rs` re-export 新模型，确保后续 task 可以直接引用。

**Step 4: Run tests to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:
```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add tests/workspace_sftp_tab_contract_smoke.sh tests/sftp_file_browser_session_spec.rs src/shell/tabs.rs src/app/sftp/model.rs src/app/sftp/mod.rs src/app/sftp/browser_session.rs tests/workspace_tabs_spec.rs

git commit -m "refactor: add sftp browser session contracts"
```

### Task 2: Move workspace state from session-centric to tab-centric

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Modify: `src/shell/view_model/asset_modal_executor.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Create: `tests/workspace_sftp_projection_spec.rs`

**Step 1: Write the failing tests**

- 在 `tests/workspace_sftp_projection_spec.rs` 锁定：
  - active workspace identity 变成 `tab_id`
  - terminal tab 与 SFTP tab 可以同时存在
  - active tab 切到 SFTP 时，terminal surface accessor 不应误报
  - 关闭 SFTP tab 时 fallback 行为仍正常
- 扩充 `tests/workspace_tabs_spec.rs`，覆盖：

```rust
#[test]
fn workspace_can_activate_sftp_tab_without_losing_terminal_tab_identity() {
    let terminal_tab = WorkspaceTab::from_session(&sample_handle(...));
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![terminal_tab.clone(), sftp_tab.clone()]);

    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));
    assert_eq!(view_model.active_workspace_tab().unwrap().title, "Files: Prod");
    assert!(view_model.active_workspace_terminal_session_id().is_none());
}
```

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test --test workspace_tabs_spec --test workspace_sftp_projection_spec -q
```

Expected: FAIL，因为当前 view model 仍以 `active_workspace_session_id` 为核心。

**Step 3: Write minimal implementation**

- 在 `ShellViewModel` 新增：
  - `active_workspace_tab_id`
  - `active_workspace_terminal_session_id()`
  - `activate_workspace_tab(...)`
  - `close_workspace_tab(...)`
- 让 `active_workspace_tab()` 基于 `tab_id` 查找，而不是 `session_id`
- 在 `workspace.rs` 中区分：

```rust
match active_tab.kind {
    WorkspaceTabKind::Terminal => "terminal",
    WorkspaceTabKind::Sftp => "sftp",
    WorkspaceTabKind::Launcher => "welcome",
}
```

- 让 `asset_modal_executor.rs` 这类必须依赖 terminal session 的代码改用新的 `active_workspace_terminal_session_id()` helper。
- 在 `workspace_terminal.rs` 中保留终端 tab projection，但不要再覆盖已有的 `WorkspaceTab::Sftp`。

**Step 4: Run tests to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:
```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/shell/view_model.rs src/shell/view_model/workspace.rs src/shell/view_model/asset_modal_executor.rs src/app/bootstrap/workspace_terminal.rs tests/workspace_tabs_spec.rs tests/workspace_sftp_projection_spec.rs

git commit -m "refactor: make workspace tabs independent from terminal sessions"
```

### Task 3: Introduce a real `FileBrowserSession` registry and cloneable view state

**Files:**
- Modify: `src/app/sftp/browser_state.rs`
- Modify: `src/app/sftp/browser_controller.rs`
- Modify: `src/app/sftp/model.rs`
- Modify: `src/app/sftp/session_binding.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `tests/sftp_browser_controller_spec.rs`
- Modify: `tests/sftp_panel_state_spec.rs`
- Create: `tests/sftp_quick_browser_session_spec.rs`

**Step 1: Write the failing tests**

- 在 `tests/sftp_quick_browser_session_spec.rs` 锁定：
  - quick browser 有独立 `browser_session_id`
  - workspace expand 会 clone 新 session
  - sort / selection 不会和 quick browser 共享可变引用
  - follow mode 与 locked mode 可以独立存在
- 扩展 `tests/sftp_panel_state_spec.rs`，让原来的 `sftp_sessions`/全局 sort state 断言改成针对 `FileBrowserSession` / `QuickBrowserState`。
- 扩展 `tests/sftp_browser_controller_spec.rs`，让 read-dir 请求按 `file_browser_session_id` 路由，而不是隐式按 terminal session。

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test --test sftp_panel_state_spec --test sftp_browser_controller_spec --test sftp_quick_browser_session_spec -q
```

Expected: FAIL，因为当前 quick browser 仍复用 `sftp_sessions` + 全局 sort/column state。

**Step 3: Write minimal implementation**

- 在 `src/app/sftp/browser_state.rs` 把 `SftpBrowserSessionState` 提升为 `FileBrowserSession` 可用的核心状态。
- 在 `ShellViewModel` 中引入：

```rust
pub file_browser_sessions: HashMap<FileBrowserSessionId, FileBrowserSession>;
pub quick_browser_session_id: Option<FileBrowserSessionId>;
pub quick_browser_state: QuickBrowserState;
```

- 把以下旧字段迁移出去：
  - `sftp_sessions`
  - `sftp_panel_sort_state`
  - `sftp_panel_column_layout`
- 让 `ShellViewModel::active_sftp_session_state()` 改为：
  - quick browser path：读取 `quick_browser_session_id`
  - workspace SFTP path：读取 active `WorkspaceTab::Sftp.file_browser_session_id`
- 在 controller / binding 中增加 clone helper，例如：

```rust
impl FileBrowserSession {
    pub fn clone_for_workspace(&self) -> Self {
        Self {
            file_browser_session_id: new_browser_session_id(),
            host_profile_ref: self.host_profile_ref.clone(),
            current_path: self.current_path.clone(),
            sort_state: self.sort_state,
            selected_entry_ids: Vec::new(),
            ..self.clone_without_identity()
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:
```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/app/sftp/browser_state.rs src/app/sftp/browser_controller.rs src/app/sftp/model.rs src/app/sftp/session_binding.rs src/shell/view_model.rs src/shell/view_model/sftp.rs tests/sftp_browser_controller_spec.rs tests/sftp_panel_state_spec.rs tests/sftp_quick_browser_session_spec.rs

git commit -m "refactor: decouple file browser sessions from terminal tabs"
```

### Task 4: Rebuild the right panel as a true `Quick Browser`

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `tests/sftp_right_panel_render_spec.rs`
- Modify: `tests/sftp_follow_cwd_spec.rs`
- Create: `tests/sftp_quick_browser_render_spec.rs`

**Step 1: Write the failing tests**

- 在 `tests/sftp_quick_browser_render_spec.rs` 锁定新 UI contract：
  - 顶部出现 `Expand`
  - 顶部出现 connection badge
  - 顶部存在 `Follow` / `Locked` 状态切换
  - 路径区不再是单纯文本，至少暴露 breadcrumb/path-edit 模式状态
- 扩展 `tests/sftp_right_panel_render_spec.rs`，断言低频图标没有重新堆回主 toolbar。
- 扩展 `tests/sftp_follow_cwd_spec.rs`，验证 quick browser 在 `Follow Active Terminal` 和 `Locked to Host/Profile` 之间切换时行为正确。

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test --test sftp_right_panel_render_spec --test sftp_follow_cwd_spec --test sftp_quick_browser_render_spec -q
```

Expected: FAIL，因为现有右栏还没有 expand、badge、locked 模式和混合路径控件。

**Step 3: Write minimal implementation**

- 在 `right-panel.slint` 顶部工具条只保留高频动作，并新增：

```slint
callback sftp-panel-expand-requested();
callback sftp-panel-binding-mode-toggle-requested();

in property <string> sftp-panel-connection-badge: "";
in property <string> sftp-panel-binding-mode-label: "Follow";
in property <bool> sftp-panel-path-editing: false;
```

- 把路径区改成 breadcrumb + 可切换的 inline input 模式。
- 在 `src/shell/view_model/sftp.rs` 增加 quick browser 专属 getter/setter：
  - `quick_browser_connection_badge()`
  - `quick_browser_binding_mode_label()`
  - `toggle_quick_browser_binding_mode()`
  - `begin_quick_browser_path_edit()`
- 在 `src/app/bootstrap/sftp.rs` 把新 UI property / callback 同步到 window。

**Step 4: Run tests to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:
```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add ui/shell/right-panel.slint src/app/bootstrap/sftp.rs src/shell/view_model/sftp.rs tests/sftp_right_panel_render_spec.rs tests/sftp_follow_cwd_spec.rs tests/sftp_quick_browser_render_spec.rs

git commit -m "feat: turn the sftp side panel into a quick browser"
```

### Task 5: Add an `SFTP Workspace Tab` surface to the main workspace

**Files:**
- Create: `ui/shell/sftp-workspace-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Create: `tests/sftp_workspace_tab_render_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing tests**

- 新增 `tests/sftp_workspace_tab_render_spec.rs`，锁定：
  - `WorkspacePane` 能根据 active tab kind 在 `TerminalSessionHost` 和 `SftpWorkspaceHost` 之间切换
  - SFTP tab 表头至少有 `Name / Type / Size / Modified`
  - tab 标题格式为 `Files: <host label>`
- 扩展 `tests/workspace_tabs_spec.rs`，断言 `WorkspaceTabItem` / UI projection 能显示 terminal tab 和 SFTP tab。

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test --test workspace_tabs_spec --test sftp_workspace_tab_render_spec -q
```

Expected: FAIL，因为 workspace 还只能渲染 terminal host。

**Step 3: Write minimal implementation**

- 新建 `ui/shell/sftp-workspace-host.slint`，先提供：
  - 顶部轻量工具条
  - 表格列 `Name / Type / Size / Modified`
  - 父级导航、刷新、多选基础事件
- 在 `workspace-pane.slint` 增加分支：

```slint
if root.workspace-session-host-mode == "sftp" : sftp-host := SftpWorkspaceHost { ... }
else : session-host := TerminalSessionHost { ... }
```

- 在 `tabbar.slint` / projection 中让 tab title 能显示 `Files: Prod` 这种标题，不要把 SFTP tab 当成特殊异常 case。

**Step 4: Run tests to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:
```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add ui/shell/sftp-workspace-host.slint ui/shell/workspace-pane.slint ui/shell/tabbar.slint src/app/bootstrap/workspace_terminal.rs src/shell/tabs.rs src/shell/view_model/workspace.rs tests/sftp_workspace_tab_render_spec.rs tests/workspace_tabs_spec.rs

git commit -m "feat: add sftp workspace tabs to the main workspace"
```

### Task 6: Implement `Expand to Workspace`, locked lifecycle, and SFTP-only reconnect

**Files:**
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/app/sftp/browser_controller.rs`
- Modify: `src/app/sftp/session_binding.rs`
- Modify: `src/app/sftp/runtime.rs`
- Create: `tests/sftp_expand_to_workspace_spec.rs`
- Create: `tests/sftp_workspace_disconnect_spec.rs`
- Modify: `tests/sftp_follow_cwd_spec.rs`
- Modify: `tests/sftp_runtime_spec.rs`

**Step 1: Write the failing tests**

- 在 `tests/sftp_expand_to_workspace_spec.rs` 锁定：
  - 从 quick browser expand 会创建新的 `WorkspaceTab::Sftp`
  - 新 tab 继承当前 host/profile 与 path
  - expand 后的 tab 默认 locked，不跟随 terminal tab 切换
- 在 `tests/sftp_workspace_disconnect_spec.rs` 锁定：
  - 关联 terminal 关闭后，SFTP tab 仍保留
  - SFTP tab 状态变成 `Disconnected`
  - 调用 reconnect 只恢复 SFTP 浏览，不自动新建 terminal tab
- 扩展 `tests/sftp_runtime_spec.rs` 和 `tests/sftp_follow_cwd_spec.rs` 让 runtime/binding 层覆盖 locked + reconnect 分支。

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test --test sftp_expand_to_workspace_spec --test sftp_workspace_disconnect_spec --test sftp_follow_cwd_spec --test sftp_runtime_spec -q
```

Expected: FAIL，因为 expand/locked/reconnect 生命周期还没接通。

**Step 3: Write minimal implementation**

- 在 `src/shell/view_model/sftp.rs` 增加：

```rust
pub fn expand_quick_browser_to_workspace(&mut self) -> Option<WorkspaceTabId> { ... }
pub fn reconnect_active_sftp_workspace(&mut self) -> bool { ... }
```

- `expand_quick_browser_to_workspace()` 要做的事：
  - 读取 quick browser 的 `FileBrowserSession`
  - clone 成新的 workspace browser session
  - 创建 `WorkspaceTab::Sftp`
  - 激活该 tab
- terminal 关闭时：
  - 如果 workspace SFTP tab 依赖的 transport 消失，标记 browser session 为 `Disconnected`
  - 不关闭 tab
- reconnect 时：
  - 只为该 `FileBrowserSession` 重新建立 SFTP browsing chain
  - 不调用 terminal-launch 路径

**Step 4: Run tests to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:
```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/shell/view_model/sftp.rs src/app/bootstrap/sftp.rs src/app/bootstrap/workspace_terminal.rs src/app/sftp/browser_controller.rs src/app/sftp/session_binding.rs src/app/sftp/runtime.rs tests/sftp_expand_to_workspace_spec.rs tests/sftp_workspace_disconnect_spec.rs tests/sftp_follow_cwd_spec.rs tests/sftp_runtime_spec.rs

git commit -m "feat: support expandable sftp workspace tabs and reconnect"
```

### Task 7: Run the full regression pass and tighten the MVP contract

**Files:**
- Modify as needed: `src/shell/tabs.rs`
- Modify as needed: `src/shell/view_model.rs`
- Modify as needed: `src/shell/view_model/workspace.rs`
- Modify as needed: `src/shell/view_model/sftp.rs`
- Modify as needed: `src/app/bootstrap/workspace_terminal.rs`
- Modify as needed: `src/app/bootstrap/sftp.rs`
- Modify as needed: `ui/shell/right-panel.slint`
- Modify as needed: `ui/shell/workspace-pane.slint`
- Modify as needed: `ui/shell/tabbar.slint`
- Modify as needed: `ui/shell/sftp-workspace-host.slint`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/sftp_panel_state_spec.rs`
- Test: `tests/sftp_browser_controller_spec.rs`
- Test: `tests/sftp_right_panel_render_spec.rs`
- Test: `tests/sftp_follow_cwd_spec.rs`
- Test: `tests/sftp_runtime_spec.rs`
- Test: `tests/sftp_transfer_flow_spec.rs`
- Test: `tests/sftp_context_menu_spec.rs`
- Test: `tests/sftp_queue_spec.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Run the targeted regression suite**

Run:
```bash
cargo test --test workspace_tabs_spec --test workspace_sftp_projection_spec --test sftp_file_browser_session_spec --test sftp_panel_state_spec --test sftp_browser_controller_spec --test sftp_quick_browser_session_spec --test sftp_quick_browser_render_spec --test sftp_workspace_tab_render_spec --test sftp_expand_to_workspace_spec --test sftp_workspace_disconnect_spec --test sftp_follow_cwd_spec --test sftp_runtime_spec --test sftp_transfer_flow_spec --test sftp_context_menu_spec --test sftp_queue_spec -q
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 2: Run compile verification**

Run:
```bash
cargo check --workspace
```

Expected: PASS

**Step 3: Fix only the smallest regressions**

- 只修正被上面回归捕获的问题
- 不在这个阶段加第二阶段功能
- 如果 `Permissions` 列、split view、transfer queue UI 冒出来，明确回退，保持 MVP 范围

**Step 4: Final commit**

```bash
git add src/shell/tabs.rs src/shell/view_model.rs src/shell/view_model/workspace.rs src/shell/view_model/sftp.rs src/app/bootstrap/workspace_terminal.rs src/app/bootstrap/sftp.rs src/app/sftp/browser_session.rs src/app/sftp/browser_state.rs src/app/sftp/browser_controller.rs src/app/sftp/model.rs src/app/sftp/session_binding.rs src/app/sftp/runtime.rs src/app/sftp/mod.rs ui/shell/right-panel.slint ui/shell/workspace-pane.slint ui/shell/tabbar.slint ui/shell/sftp-workspace-host.slint tests/workspace_tabs_spec.rs tests/workspace_sftp_projection_spec.rs tests/sftp_file_browser_session_spec.rs tests/sftp_panel_state_spec.rs tests/sftp_browser_controller_spec.rs tests/sftp_quick_browser_session_spec.rs tests/sftp_quick_browser_render_spec.rs tests/sftp_workspace_tab_render_spec.rs tests/sftp_expand_to_workspace_spec.rs tests/sftp_workspace_disconnect_spec.rs tests/sftp_follow_cwd_spec.rs tests/sftp_runtime_spec.rs tests/sftp_transfer_flow_spec.rs tests/sftp_context_menu_spec.rs tests/sftp_queue_spec.rs tests/workspace_sftp_tab_contract_smoke.sh

git commit -m "feat: add two-layer sftp workspace model"
```

## Done Criteria

实现完成后必须满足：

- 右侧 quick browser 可以继续跟随 active terminal
- quick browser 可以切换到 locked host/profile 模式
- quick browser 可以一键 `Expand to Workspace`
- expand 后在中部出现独立 `SFTP Workspace Tab`
- 该 tab 继承同一 host/profile 与 path
- 切换其它 terminal tab 时，该 SFTP tab 不会被强制切换
- 关闭原 terminal tab 后，SFTP tab 保留并显示 `Disconnected / Reconnect`
- reconnect 只恢复文件浏览，不自动打开 terminal tab
- 整体仍保持 terminal-first，而不是退化成双栏文件管理器
