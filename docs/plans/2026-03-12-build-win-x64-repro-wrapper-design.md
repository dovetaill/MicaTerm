# Build Win X64 Repro Wrapper Design

日期: 2026-03-12  
执行者: Codex  
状态: 已确认方案，待进入实现

## 背景

当前仓库已经有独立最小 repro binary：

- `windows_theme_repro`

但它的打包仍需要用户手动传入：

- `APP_NAME=windows-theme-repro`
- `BIN_NAME=windows_theme_repro`

这会增加使用成本，也不符合现有 Windows wrapper 脚本的调用习惯。

## 目标

- 新增一个专用包装脚本：`build-win-x64-repro.sh`
- 默认打包 `windows_theme_repro`
- 不修改 `build-desktop.sh` 的核心逻辑
- 不改变现有 `build-win-x64.sh` 作为正式包入口的语义

## 方案对比

### 方案 A：新增独立 repro wrapper

- 新增 `build-win-x64-repro.sh`
- 固定：
  - `TARGET=x86_64-pc-windows-gnu`
  - `APP_NAME=windows-theme-repro`
  - `BIN_NAME=windows_theme_repro`
- 内部直接转发到 `build-desktop.sh`

优点：

- 改动最小
- 与现有 wrapper 风格一致
- 不会把正式包和 repro 包重新耦合

缺点：

- 多一个很薄的脚本文件

### 方案 B：给 `build-win-x64.sh` 增加模式开关

优点：

- 文件更少

缺点：

- 会让正式包入口承担两种语义
- 后续更容易把正式构建和 repro 构建混淆

最终选择：`方案 A`

## 设计

- 新增 `build-win-x64-repro.sh`
- 脚本只做环境变量固定和转发，不新增自己的打包逻辑
- 新增 `tests/build_win_x64_repro_script_smoke.sh`
  - 验证脚本存在
  - `bash -n` 通过
  - 文本中固定了 `TARGET`、`APP_NAME`、`BIN_NAME`
  - `--help` 输出仍能透传自 `build-desktop.sh`
- README 增加最小 repro 构建入口说明

## 验收标准

- 可以直接执行 `./build-win-x64-repro.sh`
- 产物名称符合：
  - `dist/windows-theme-repro-x86_64-pc-windows-gnu-release.zip`
- smoke test 通过
- `cargo check --workspace` 与 `cargo clippy --workspace -- -D warnings` 仍通过
