# Terminal Smoke, Link Interaction, and Glyph Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为终端补齐可复用的 TUI smoke 验证资产、稳定终端 URL 交互、并修复连续 `-` 在 native terminal 渲染中的异常连线问题。

**Architecture:** 先建立 smoke 脚本与 checklist 作为统一验证入口，再在现有 workspace terminal 链路上补齐 URL hover/Ctrl/click gating，最后在 glyph 准备层做最小 visual-fit 修正。整个实现坚持小步 TDD，只补关键 contract/integration 覆盖，不引入完整 GUI E2E 平台。

**Tech Stack:** Rust, Slint, bash, cargo test, terminal renderer glyph preparation path

---

### Task 1: 建立 smoke 文档与脚本入口

**Files:**
- Create: `docs/terminal-tui-smoke-checklist.md`
- Create: `scripts/dev/terminal-tui-smoke.sh`
- Create: `tests/terminal_tui_smoke_fixture_spec.rs`

**Step 1: Write the failing test**

在 `tests/terminal_tui_smoke_fixture_spec.rs` 中新增 source/fixture contract 测试，至少覆盖：

- 存在 `scripts/dev/terminal-tui-smoke.sh`
- 脚本包含 `codex`、`vim`、`less`、`htop`、`links`、`glyphs` 场景标签
- checklist 文档包含“贴底 / alt-screen / spinner / link / glyph”观察项

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test terminal_tui_smoke_fixture_spec --quiet
```

Expected: FAIL，因为脚本和 checklist 还不存在。

**Step 3: Write minimal implementation**

- 新建 `docs/terminal-tui-smoke-checklist.md`
- 新建 `scripts/dev/terminal-tui-smoke.sh`
- 让脚本支持：
  - `all`
  - `codex`
  - `vim`
  - `less`
  - `htop`
  - `links`
  - `glyphs`
  - `progress`
- 没有命令时回退到 echo/printf 的内置说明或 fixture

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test terminal_tui_smoke_fixture_spec --quiet
bash scripts/dev/terminal-tui-smoke.sh all
```

Expected: 测试通过，脚本能打印 smoke 场景说明并正常退出。

**Step 5: Commit**

```bash
git add docs/terminal-tui-smoke-checklist.md scripts/dev/terminal-tui-smoke.sh tests/terminal_tui_smoke_fixture_spec.rs
git commit -m "test: add terminal tui smoke fixtures"
```

### Task 2: 为终端链接交互补 URL token trimming 与 gating 测试

**Files:**
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

新增测试覆盖：

- `https://example.com).` 这类 token 能正确裁剪为 URL 本体
- hover 命中 URL 时可查询，但只有 Ctrl 状态才进入“可点击”路径
- alt-screen 场景不触发链接打开
- 选区拖拽中不误触发链接点击

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test ssh_terminal_interaction_spec --test bootstrap_smoke --quiet
```

Expected: 新增链接交互用例先红。

**Step 3: Write minimal implementation**

- 在 `src/app/bootstrap/workspace_terminal.rs` 中补强 URL token trimming
- 在 `src/app/bootstrap.rs` 与现有 workspace terminal 回调链中补齐 Ctrl gating / alt-screen gating
- 不扩展到 `mailto:` 等非目标 scheme

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test ssh_terminal_interaction_spec --test bootstrap_smoke --quiet
```

Expected: 新增链接相关测试通过，既有 workspace terminal 交互测试不回归。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/bootstrap/workspace_terminal.rs tests/ssh_terminal_interaction_spec.rs tests/bootstrap_smoke.rs
git commit -m "fix: tighten terminal link interaction gating"
```

### Task 3: 补齐 Slint host 链接提示与指针状态契约

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing tests**

补 source-level contract 测试，断言：

- terminal host 在 URL hover 时能切 pointer cursor
- tooltip 文案区分 Ctrl 未按下 / 已按下两种状态
- Ctrl+click 链路仍通过现有 callback contract 走 Rust 层

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test native_terminal_surface_contract_spec --quiet
```

Expected: 新增 UI contract 断言失败。

**Step 3: Write minimal implementation**

- 在 `ui/shell/terminal-session-host.slint` 中收口 tooltip 文案、pointer cursor、Ctrl 状态表现
- 仅在线路既有 callback 基础上补齐，不新增多余 callback
- 保持 `workspace-pane` / `app-window` contract 不扩张

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test native_terminal_surface_contract_spec --quiet
```

Expected: source contract 通过。

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint tests/native_terminal_surface_contract_spec.rs
git commit -m "fix: align terminal link hover affordances"
```

### Task 4: 为重复 `-` 字形问题写回归测试

**Files:**
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Create or Modify: `tests/terminal_glyph_fit_spec.rs`

**Step 1: Write the failing test**

新增 glyph regression test，至少覆盖：

- `-----`、`drwx-----` 这类 cluster 会走预期 visual-fit 分类
- 修复只针对终端重复符号，不影响普通 body text cluster

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test terminal_glyph_fit_spec --quiet
```

Expected: FAIL，因为当前 ASCII `-` 未进入预期路径。

**Step 3: Write minimal implementation**

- 在 `src/app/terminal_renderer/wgpu_renderer.rs` 扩展 visual-fit 判定
- 只把终端高频重复符号纳入更保守的 grid-fit 范围
- 不修改 font backend / rustybuzz shaping 配置，除非测试证明该层不够

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test terminal_glyph_fit_spec --quiet
```

Expected: 新增 glyph regression 通过。

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/wgpu_renderer.rs tests/terminal_glyph_fit_spec.rs
git commit -m "fix: stabilize terminal glyph fit for repeated dashes"
```

### Task 5: 组合验证 smoke/link/glyph 改动

**Files:**
- Test only

**Step 1: Run targeted verification**

Run:

```bash
cargo check --quiet
cargo test --test terminal_tui_smoke_fixture_spec --test ssh_terminal_interaction_spec --test bootstrap_smoke --test native_terminal_surface_contract_spec --test terminal_glyph_fit_spec --quiet
bash scripts/dev/terminal-tui-smoke.sh all
```

Expected: 新增 smoke/link/glyph 相关测试通过，脚本能正常输出场景说明。

**Step 2: Run branch-local redesign checks that still matter**

Run:

```bash
cargo test --test terminal_theme_selection_spec --test theme_semantic_token_contract_spec --test theme_terminal_redesign_spec --quiet
bash tests/sidebar_ui_contract_smoke.sh
```

Expected: 记录当前 visual-highlight redesign 分支的真实状态；若仍有既有红灯，不把它们误报为 smoke/link/glyph 新回归。

**Step 3: Commit verification notes if docs changed**

```bash
git status --short
```

若验证阶段没有新文件变化，则无需 commit。
