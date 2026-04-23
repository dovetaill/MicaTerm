# Terminal TUI Correctness Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复终端 viewport/winsize、alt-screen/TUI 贴底与滚动边界、以及 alt-screen overlay 隔离，优先恢复 Codex/lazygit/vim/less/htop 这类 TUI 的正确性。

**Architecture:** 先用测试锁定 rows/cols contract、alt-screen scroll contract、semantic projection contract，再以最小侵入方式统一 UI 与 Rust 的 terminal geometry contract，并在 alt-screen 下对本地 scrollback 与 semantic/overview projection 做硬边界短路。保留普通 shell scrollback、selection、semantic highlight 的既有行为，只在 alt-screen/TUI 场景收紧。

**Tech Stack:** Rust, Slint, wezterm_term/alacritty_terminal adapters, cargo test

---

### Task 1: 锁定 contract 测试

**Files:**
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

1. 先写 alt-screen scroll/clamp、semantic/overview 短路、surface resize snapped contract、terminal defaults viewport contract 的失败测试。
2. 运行最小测试集，确认新测试先红。

### Task 2: 统一 viewport -> rows/cols -> winsize contract

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/bootstrap.rs`

1. 让 host 的 resize 事件改为基于 snapped content rect 计算 rows/cols。
2. 在 `TerminalRuntimeDefaults` 中保存 live viewport defaults（rows/cols/pixel size），并在 bootstrap 的 resize callback 中同步更新。
3. 在 SSH `request_pty` 前读取 live defaults，避免首帧固定 80x24。

### Task 3: 修 full-screen TUI scroll boundary 与 alt-screen overlay 隔离

**Files:**
- Modify: `src/app/terminal_core/wezterm_adapter.rs`
- Modify: `src/app/terminal_core/alacritty_adapter.rs`
- Modify: `src/app/terminal_semantic/mod.rs`

1. alt-screen 下禁用本地 scrollback / viewport_offset。
2. alt-screen 下禁用 command blocks / overview markers / semantic projection，避免 overlay 残留到 TUI frame。
3. 跑针对性测试，确认普通 shell scrollback 未回归。

### Task 4: 回归验证

**Files:**
- Test only

1. 运行定向 `cargo test` 集合覆盖 terminal session/core/bootstrap/semantic contract。
2. 若仍有 redraw/残影测试失败，再决定是否补 presenter/native invalidation 边界。
