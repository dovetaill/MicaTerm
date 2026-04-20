# Windows Taskbar Icon Fix Design

## Goal

修复打包后的 Windows 可执行文件偶发丢失任务栏图标、退回默认图标的问题，同时补齐可追踪的诊断日志，避免后续再出现“exe 资源图标在，但运行时窗口图标被清空”的隐性回归。

## Problem Summary

当前仓库已经通过 `build.rs` 将 `assets/icons/windows/mica-term.ico` 编进 exe 资源段，但运行时窗口图标链路仍然存在两个缺口：

1. `ui/app-window.slint` 的 `AppWindow` 没有显式设置 `Window.icon`，导致 Slint 运行时默认仍是空图像。
2. vendored `i-slint-backend-winit` 会把 Slint 的 `Window.icon` 同步到 Windows 的 `ICON_SMALL` / `ICON_BIG`。当 Slint 图标为空时，这条同步链路等价于把窗口图标清空，覆盖掉 exe 资源提供的默认图标。

这会形成“exe 资源图标正常，但任务栏偶尔显示默认图标”的不稳定表现，具体是否退回默认图标取决于 Explorer 抓取的是资源阶段还是运行时同步后的状态。

## Root Cause

### 1. 打包资源图标与运行时窗口图标是两条链路

`build.rs` 中的 `winresource` 只保证 exe 带有原生图标资源；它不能阻止运行时窗口系统再次主动设置或清空 `WM_SETICON` / `ICON_SMALL` / `ICON_BIG`。

### 2. Slint `Window.icon` 默认值为空

Slint 文档中 `Window.icon` 的默认值就是 empty image。当前 `AppWindow` 没有覆盖它，因此桌面后端看到的是“空图标”。

### 3. vendored winit backend 会在 Windows 上同步 small/big icon

当前 vendored `i-slint-backend-winit` 在 Windows 上会同时调用：

- `window.set_window_icon(...)` -> `ICON_SMALL`
- `window.set_taskbar_icon(...)` -> `ICON_BIG`

当传入的是空图标转换结果时，实际效果就是把窗口图标清掉。

## Approved Fix Direction

### 1. 在应用层显式声明运行时窗口图标

在 `ui/app-window.slint` 里给 `AppWindow` 直接设置内嵌图标资源，优先使用仓库里已经存在的应用图标矢量资源，让 Slint 从窗口创建开始就拥有稳定的非空图像。

### 2. 在 vendored backend 层阻止“空 Slint 图标清空 Windows 原生图标”

在 `vendor/i-slint-backend-winit/winitwindowadapter.rs` 增加空图标识别逻辑：

- Windows 上如果 Slint `Window.icon` 为空，则保留现有原生图标，不再主动调用 `set_window_icon(None)` / `set_taskbar_icon(None)`。
- 如果 Slint `Window.icon` 非空，则继续按原路径同步 small/big icon。

这可以同时覆盖：

- 当前 `AppWindow` 未设置 icon 的历史回归；
- 未来资源解析失败、返回空图像时对 exe 资源图标的误清空。

### 3. 在应用启动阶段补 Windows 图标诊断日志

新增一个 Windows 专用诊断模块，在应用创建窗口后记录：

- `HWND`
- `WM_GETICON(ICON_SMALL)` / `WM_GETICON(ICON_BIG)`
- `GCLP_HICONSM` / `GCLP_HICON`
- 当前阶段标记（如 `after_window_new`、`before_window_run`）

这样即使以后再出现任务栏图标异常，也能从日志里直接区分“资源没带上”还是“运行时被清空/覆盖”。

## Scope

### Modify

- `ui/app-window.slint`
- `src/app/bootstrap.rs`
- `src/app/mod.rs`
- `vendor/i-slint-backend-winit/winitwindowadapter.rs`
- `tests/windows_icon_integration_smoke.sh`

### Create

- `src/app/windows_icon.rs`
- `tests/windows_icon_runtime_spec.rs`
- `docs/plans/2026-04-20-windows-taskbar-icon-fix-implementation-plan.md`

## Non-Goals

- 不改打包 zip 的目录结构，也不要求分发目录额外携带一个独立 `.ico` 文件。
- 不引入新的 Windows 安装器或快捷方式元数据逻辑。
- 不修改非 Windows 平台的窗口图标行为。

## Success Criteria

- `AppWindow` 在 Slint 层有显式、非空的运行时图标。
- Windows vendored backend 不会再因为空 `Window.icon` 主动清空 exe 原生图标。
- 启动日志能记录 Windows small/big/class icon 句柄状态。
- Windows 图标相关测试契约覆盖应用层、backend 层和日志层。
