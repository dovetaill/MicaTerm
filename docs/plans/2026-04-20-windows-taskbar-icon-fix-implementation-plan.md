# Windows Taskbar Icon Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 让打包后的 Windows 应用稳定保留任务栏图标，不再因运行时空图标同步退回默认图标，并留下可回放的启动诊断日志。

**Architecture:** 在 Slint 应用层显式提供非空窗口图标；在 vendored winit backend 层阻断“空图标清空 Windows 原生图标”的链路；在应用启动阶段记录 Windows icon handle 诊断，便于后续追踪。

**Tech Stack:** Rust、Slint、vendored `i-slint-backend-winit`、Windows Win32 icon APIs、现有 shell/rust 契约测试。

---

### Task 1: 用失败测试锁定 Windows 图标契约

**Files:**
- Create: `tests/windows_icon_runtime_spec.rs`
- Modify: `tests/windows_icon_integration_smoke.sh`
- Reference: `ui/app-window.slint`
- Reference: `src/app/bootstrap.rs`
- Reference: `vendor/i-slint-backend-winit/winitwindowadapter.rs`

**Step 1: Write the failing test**
- 增加 source-level Rust 测试，断言 `AppWindow` 显式声明运行时 icon、Windows 图标诊断模块存在、vendored backend 具有“空图标不清空原生 icon”的分支。
- 修正现有 shell smoke，使它验证当前真实契约，而不是过时地要求 `build-desktop.sh` 内联 `.ico` 路径。

**Step 2: Run test to verify it fails**
Run: `cargo test --test windows_icon_runtime_spec -- --nocapture`
Run: `bash tests/windows_icon_integration_smoke.sh`
Expected: FAIL，因为当前应用层没有显式 icon，diagnostics 模块不存在，vendored backend 也没有空图标保护。

### Task 2: 实现应用层与 backend 层的完整修复

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/mod.rs`
- Create: `src/app/windows_icon.rs`
- Modify: `vendor/i-slint-backend-winit/winitwindowadapter.rs`

**Step 1: Add runtime Slint window icon**
- 给 `AppWindow` 直接声明应用图标资源，确保运行时窗口创建拿到非空 icon。

**Step 2: Add Windows icon diagnostics**
- 新建 `src/app/windows_icon.rs`，封装 Win32 small/big/class icon handle 读取与 `tracing` 日志输出。
- 在 `run_with_profile()` 中于窗口创建后、事件循环运行前打点两次，记录运行时图标状态。

**Step 3: Prevent empty icon sync from clearing packaged Windows icons**
- 在 vendored winit backend 里检测空 `Window.icon`。
- Windows 上若为空则保留现有原生图标，不主动清空；非空时仍正常同步 runtime icon。

### Task 3: 验证并更新文档

**Files:**
- Modify: `docs/plans/2026-04-20-windows-taskbar-icon-fix-design.md`
- Modify: `docs/plans/2026-04-20-windows-taskbar-icon-fix-implementation-plan.md`

**Step 1: Run focused verification**
Run: `cargo test --test windows_icon_runtime_spec -- --nocapture`
Run: `bash tests/windows_icon_integration_smoke.sh`
Expected: PASS

**Step 2: Run regression verification**
Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: PASS

**Step 3: Commit**
Run: `git add docs/plans/2026-04-20-windows-taskbar-icon-fix-design.md docs/plans/2026-04-20-windows-taskbar-icon-fix-implementation-plan.md ui/app-window.slint src/app/bootstrap.rs src/app/mod.rs src/app/windows_icon.rs vendor/i-slint-backend-winit/winitwindowadapter.rs tests/windows_icon_runtime_spec.rs tests/windows_icon_integration_smoke.sh`
Run: `git commit -m "fix: stabilize windows taskbar icon"`
Expected: 一个包含完整修复、诊断和测试契约的提交。
