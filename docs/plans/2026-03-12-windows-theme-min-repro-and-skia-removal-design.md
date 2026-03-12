# Mica Term Windows Theme Minimal Repro And Skia Removal Design

日期: 2026-03-12  
执行者: Codex  
状态: 已确认方案，待进入实现规划

## 背景

当前仓库为了验证 Windows 下“主题切换时窗口部分超出屏幕导致屏外区域未完整刷新”的问题，引入了 `windows-skia-experimental`、`build-win-x64-skia.sh`、`winit-skia-software` backend lock 以及对应的 runtime profile、日志和测试。

已经确认的事实有两点：

- `SkiaExperimental` 构建在 Windows 上确实生效，运行日志能够看到 `winit-skia-software`
- 即使关闭 `ThemeRedrawRecovery`，问题仍然存在，说明当前缺陷并没有被 Skia 实验路径解决

因此当前最合理的方向不再是继续在正式应用路径上增加实验分支，而是：

- 把主线应用收敛回单一 workaround 正式路径
- 另建一个独立的最小 repro binary，只保留主题切换和超屏场景，用来稳定复现和后续向上游/底层定位问题

## 目标

- 删除主线应用中的所有 Windows Skia 实验路径
- 保留主线应用现有 workaround 方案，Windows 与 Linux 构建都只走这一条路径
- 新增一个独立最小 repro binary，仅聚焦“主题切换 + 超屏区域未完整刷新”
- 保证整个工作只发生在隔离 worktree 分支中，不合并回本地 `master`

## 边界

### 本次覆盖

- `Cargo.toml` feature 收敛
- Windows Skia 相关脚本、入口、profile、日志与测试清理
- 新增最小 repro Slint 窗口和独立入口
- README / verification / 测试文档同步收敛

### 本次不覆盖

- 尝试继续修复或优化 Skia renderer
- 调整正式应用现有 shell 结构、侧边栏、标题栏和原生窗口特效
- 引入新的渲染 backend 实验分支
- 将 repro 结果直接合并到 `master`

## 方案对比

### 方案 A: 独立最小 repro binary + 主线清理

- 新增 `src/bin/windows_theme_repro.rs`
- 新增独立的 `ui/windows-theme-repro.slint`
- 主线删除 `windows-skia-experimental` 与所有相关逻辑

优点:

- repro 与正式应用完全解耦
- 问题边界最清晰，后续更容易对外复现和报告
- 主线构建不再继续分叉

缺点:

- 需要维护一个额外的最小入口

### 方案 B: 把现有主应用裁剪成最小模式

- 不新增独立入口，只在现有 app 内增加一个“最小复现模式”

优点:

- 文件数量少

缺点:

- 会把 repro 与正式应用路径重新耦合
- workaround、window effects、复杂组件树都会污染结论

### 方案 C: 只删除 Skia，不做最小 repro

优点:

- 变更量最小

缺点:

- 无法保留稳定、可重复的最小复现场景
- 后续继续分析底层问题时缺少干净样本

最终选择: `方案 A`

## 设计

### 1. 主线应用收敛

主线应用只保留正式构建路径：

- 删除 `slint-renderer-skia`
- 删除 `windows-skia-experimental`
- 删除 `build-win-x64-skia.sh`
- 删除 `src/main.rs` 中 `winit-skia-software` backend lock
- 删除 `src/app/runtime_profile.rs` 中 `SkiaExperimental` / `SkiaSoftware` 分支

这样 Windows 和 Linux 的正式打包路径都会统一回当前 workaround 方案，不再保留实验构建入口。

### 2. 最小 repro 放置方式

新增一个独立 binary：

- `src/bin/windows_theme_repro.rs`

新增一个独立 Slint 文件：

- `ui/windows-theme-repro.slint`

必要时允许新增一个很薄的 glue 模块，但不允许复用主应用的 `bootstrap`、复杂 view model、window effects 或 runtime profile。目标是让 repro 只保留最少变量。

### 3. 最小 repro 交互模型

最小 repro 只包含以下内容：

- 标准窗口，不加无框、透明背景或 Mica
- 一个 `Toggle Theme` 按钮
- 一个当前主题文本标识
- 一整块占满主体区域的纯色面板
  - Dark: 黑色
  - Light: 白色

窗口默认高度会设置得足够大，便于在 Windows 上直接拖出屏幕底部，稳定观察超屏区域在主题切换后的刷新行为。

该 repro 不接入 `ThemeRedrawRecovery`。原因很明确：本工具的职责是复现底层现象，而不是把应用层 workaround 混进去。

### 4. 测试与验证策略

自动验证分两层：

- 主线收敛验证
  - Skia feature / 脚本 / runtime 文案都已删除
- repro 存在性验证
  - 独立 binary 可以参与编译

统一命令：

- `cargo test`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

Windows 手工验证只看两件事：

1. 正式应用已经不再提供 Skia experimental 构建入口
2. 最小 repro 在“窗口部分超出屏幕”时切换主题，是否仍会出现屏外区域未整体刷新

### 5. 分支与交付边界

整个工作只在新的隔离 worktree 分支中进行，例如：

- `feature/windows-theme-min-repro`

该分支用于：

- 收敛正式应用构建路径
- 保留最小 repro
- 输出后续 implementation plan 与 tdd spec

本轮不执行合并回 `master`。

## 风险与约束

- 最小 repro 使用标准窗口后，若问题不再出现，说明正式应用中的透明/无框/特效链路仍是重要变量；这不是失败，而是有效缩小范围
- 删除 Skia 相关逻辑会牵连测试、README、verification 等文档，需要一并收敛，否则会留下漂移
- 自动验证只能证明代码与构建路径正确，不能替代 Windows 真机上的图形现象确认

## 验收标准

- 仓库主线代码不再包含 `windows-skia-experimental` 相关入口
- `build-win-x64-skia.sh` 被删除
- Windows / Linux 正式构建只保留 workaround 路径
- 新增最小 repro binary，且可通过编译验证
- 设计文档、实现计划和后续 tdd 文档都留在隔离分支内
