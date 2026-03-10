# Icon Logo TDD Handoff

日期: 2026-03-10
执行者: Codex

## Scope

本文档记录 `Mica Term` 图标与 logo 实现阶段已经落地的资源、构建接线、验证入口与后续测试风险，作为下一轮 `test-driven-development` 的输入基线。

本轮改动覆盖以下范围：

- `assets/icons/` 下的 4 组 SVG 源资产
- `scripts/export-icons.sh` 导出脚本
- `assets/icons/png/` 与 `assets/icons/windows/` 的受控导出产物
- `build.rs` 中的 Windows icon 嵌入逻辑
- `build-win-x64.sh` 中的 Windows 打包 icon staging 逻辑
- `tests/` 下的 3 个 shell smoke tests

## Asset Surface

### Source SVG assets

- `assets/icons/mica-term-logo.svg`
  - 用于品牌 logo
  - `viewBox="0 0 720 256"`
  - 包含 `#4ea1ff` 品牌蓝与 `id="m-frame"` 主图形
- `assets/icons/mica-term-app.svg`
  - 用于 app icon 主版
  - `viewBox="0 0 256 256"`
  - 包含 `id="m-frame"`
- `assets/icons/mica-term-taskbar.svg`
  - 用于任务栏小尺寸特化版
  - `viewBox="0 0 256 256"`
  - 包含 `id="taskbar-m-frame"`
- `assets/icons/mica-term-mark.svg`
  - 用于单色 mark
  - `viewBox="0 0 256 256"`
  - 使用 `fill="currentColor"`

### Generated raster assets

- `assets/icons/png/mica-term-16.png`
- `assets/icons/png/mica-term-20.png`
- `assets/icons/png/mica-term-24.png`
- `assets/icons/png/mica-term-32.png`
- `assets/icons/png/mica-term-40.png`
- `assets/icons/png/mica-term-48.png`
- `assets/icons/png/mica-term-64.png`
- `assets/icons/png/mica-term-128.png`
- `assets/icons/png/mica-term-256.png`
- `assets/icons/windows/mica-term.ico`

## Build And Script Integration Surface

### Rust build integration

- `build.rs`
  - 入口函数：`main()`
  - 继续执行 `slint_build::compile("ui/app-window.slint")`
  - 当 `CARGO_CFG_TARGET_OS == "windows"` 时，创建 `winresource::WindowsResource`
  - 通过 `set_icon("assets/icons/windows/mica-term.ico")` 将 `.ico` 嵌入 Windows 二进制

### Cargo build dependency

- `Cargo.toml`
  - `build-dependencies`
  - `slint-build = "1.15.1"`
  - `winresource = "0.1.19"`

说明：

- 当前项目内没有新增自定义 `struct` 或 `trait`
- 本轮唯一新增并实际使用的核心构建类型是外部 crate 的 `winresource::WindowsResource`
- `Cargo.lock` 已包含对应解析后的依赖版本

### Export script contract

- 脚本：`scripts/export-icons.sh`
- shell 入口：`#!/usr/bin/env bash`
- 默认输入目录：`SOURCE_DIR="${SOURCE_DIR:-$ROOT_DIR/assets/icons}"`
- 默认输出目录：`OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/assets/icons}"`
- 固定输入文件：
  - `mica-term-app.svg`
  - `mica-term-taskbar.svg`
- 固定输出目录：
  - `png/`
  - `windows/`
- 固定导出尺寸：
  - `16 20 24 32 40 48 64 128 256`
- 尺寸选择规则：
  - `<= 32` 使用 `mica-term-taskbar.svg`
  - `> 32` 使用 `mica-term-app.svg`
- 外部命令依赖：
  - `rsvg-convert`
  - `magick`

### Windows packaging contract

- 脚本：`build-win-x64.sh`
- 新增路径变量：`ICON_PATH="$ROOT_DIR/assets/icons/windows/mica-term.ico"`
- staging 行为：
  - 复制 `target/<target>/<profile>/<bin>.exe`
  - 若 `ICON_PATH` 存在，则复制 `mica-term.ico` 到 staging 目录
  - 若 `readme.md` 存在，则复制为 `README.md`
- 最终压缩包：
  - `dist/mica-term-x86_64-pc-windows-gnu-release.zip`

## Slint Surface

### Slint components

- 本轮没有修改任何 `.slint` 文件
- `ui/app-window.slint` 仍然是 `build.rs` 的编译入口

### Slint callbacks and bindings

- 没有新增 Slint callback
- 没有新增 `ModelRc`
- 没有新增 `slint::invoke_from_event_loop`
- 没有新增 UI runtime state binding

说明：

- 本次图标实现只接入资源与构建链，没有进入运行时 UI 逻辑
- 因此下一轮若要在欢迎页、关于页或窗口角标中展示 logo，需要新增独立的 Slint 绑定测试，不应假定本轮已覆盖

## Current Testing Baseline

### Asset smoke tests

- `tests/icon_svg_assets_smoke.sh`
  - 验证 4 个 SVG 文件存在
  - 验证 `viewBox`
  - 验证关键 `id` 与颜色/填充属性

### Export pipeline smoke test

- `tests/export_icons_smoke.sh`
  - 验证 `scripts/export-icons.sh` 存在且 `bash -n` 可通过
  - 在临时目录下运行脚本
  - 验证 9 个 `png` 输出与 `mica-term.ico` 输出

### Windows integration smoke test

- `tests/windows_icon_integration_smoke.sh`
  - 验证 `Cargo.toml` 中存在 `winresource`
  - 验证 `build.rs` 引用了 `assets/icons/windows/mica-term.ico`
  - 验证 `build-win-x64.sh` 引用了 `assets/icons/windows/mica-term.ico`
  - 验证 `readme.md` 引用了 `scripts/export-icons.sh`
  - 验证仓库中已提交 `assets/icons/windows/mica-term.ico`

### Verified commands already executed

- `bash tests/icon_svg_assets_smoke.sh`
- `bash tests/export_icons_smoke.sh`
- `bash tests/windows_icon_integration_smoke.sh`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo test`
- `./build-win-x64.sh --help`
- `TARGET=x86_64-pc-windows-gnu PROFILE=release ./build-win-x64.sh`
- `unzip -l dist/mica-term-x86_64-pc-windows-gnu-release.zip | grep -F 'mica-term.ico'`

### Determinism check already executed

- 删除 `assets/icons/png/mica-term-*.png` 与 `assets/icons/windows/mica-term.ico`
- 重新执行 `./scripts/export-icons.sh`
- 比较重导出前后的 `sha256`
- `git diff -- assets/icons` 为空

## Edge Cases And Risks To Cover Next

- `scripts/export-icons.sh` 依赖 `rsvg-convert` 与 `magick`；当前 smoke test 默认假设二者已安装，后续应补充缺失依赖时的错误断言测试。
- 小尺寸图标当前通过“尺寸阈值切换 SVG”实现，而不是对 `16/20/24/32` 做逐尺寸手工微调；若设计继续演进，需补充尺寸级别视觉契约测试。
- `build-win-x64.sh` 仅在 `ICON_PATH` 存在时复制图标；这意味着打包阶段当前是“可缺失但继续”，后续可评估是否要提升为硬失败策略并补测试。
- `build.rs` 只在 Windows target 下嵌入 icon；Linux 本地 `cargo check` 无法覆盖 `.ico` 丢失时的真实 Windows 资源编译报错路径，后续可以增加更聚焦的 cross-target 回归验证。
- `Cargo.toml` 中声明的 `winresource` 版本范围与 `Cargo.lock` 实际解析版本可能继续变化；若未来需要完全可重复构建，应把依赖变更纳入专门的锁文件回归检查。
- 本轮没有把 logo 接入 Slint 运行时界面；欢迎页、关于页、标题栏或任务面板中若要展示品牌图形，需要新增 UI 组件与状态绑定测试。
- 当前没有 Tokio channel、actor 边界或 UI 线程切换逻辑参与图标处理，因此不存在新的数据竞争面；但如果未来引入异步图标加载或主题切换产物更新，应测试 channel backpressure、shutdown ordering 和 UI-thread handoff。

## Recommended Next TDD Targets

1. 为 `scripts/export-icons.sh` 增加缺失依赖与缺失源文件的失败路径测试，明确 stderr 文案和退出码语义。
2. 为导出的 `png`/`ico` 增加更稳定的回归校验策略，例如尺寸、文件存在性之外的哈希白名单或元数据断言。
3. 为 `build-win-x64.sh` 增加“图标缺失时是否允许继续打包”的行为测试，明确这是容错策略还是缺陷。
4. 若后续把 logo 放入 Slint UI，优先补组件挂载测试、资源路径测试和浅/深色主题下的视觉契约测试。
5. 若后续引入异步主题或动态图标刷新，补 `ModelRc`、`slint::invoke_from_event_loop` 与 Tokio channel 的线程安全测试，而不是在当前文档基础上默认这些能力已经存在。
