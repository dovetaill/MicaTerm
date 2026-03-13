# Linux Host Windows GNU FemtoVG WGPU Wrapper Design

日期: 2026-03-13  
执行者: Codex  
状态: 已确认方案，待进入实现规划

## 背景

当前仓库已经存在两条与 Windows 打包相关的脚本路径：

- `build-win-x64.sh`
  - 语义是正式链的 Windows GNU wrapper
  - 目标默认是 `x86_64-pc-windows-gnu`
  - 适合在 Linux 主机上交叉打包 Windows 包
- `build-win-x64-femtovg-wgpu.sh`
  - 语义是 experimental 的 Windows MSVC wrapper
  - 目标固定是 `x86_64-pc-windows-msvc`
  - 明确要求在 Windows MSVC / Git Bash 环境运行

当前缺的不是 renderer 语义，而是一条“Linux 主机可直接打 experimental Windows 包”的便捷入口。

用户的真实需求是：

- 主力开发机是 Linux
- 仍然要打出 Windows 包
- renderer 语义必须与现有 experimental wrapper 一致
- 也就是继续固定：
  - `--no-default-features`
  - `--features femtovg-wgpu-experimental`
  - 程序内部 selector 真源
  - 不允许退回 `software`

因此，本轮不是修改 renderer 实现，而是补一条 host/target 更匹配 Linux 开发机的 wrapper。

## 目标

- 新增一个 Linux 主机专用的 experimental Windows GNU wrapper：
  - `build-win-x64-gnu-femtovg-wgpu.sh`
- 该脚本固定打包：
  - `x86_64-pc-windows-gnu`
- 该脚本必须继续使用纯 experimental renderer 语义：
  - `CARGO_NO_DEFAULT_FEATURES=1`
  - `CARGO_FEATURES=femtovg-wgpu-experimental`
- 保持可执行文件名仍为：
  - `mica-term.exe`
- 保持 archive / stage dir 的 experimental 身份仍为：
  - `mica-term-femtovg-wgpu-experimental-*`

## 非目标

- 不改动现有 `build-win-x64-femtovg-wgpu.sh` 的 MSVC 语义
- 不改动 `build-release.sh`
- 不新增 renderer fallback
- 不修改 `AppRuntimeProfile`、`BackendSelector`、日志或窗口标题逻辑
- 不把 GNU / MSVC 两条 experimental 路线混到同一个脚本里

## 方案对比

### 方案 A：新增独立 Linux GNU wrapper

做法：

- 新增 `build-win-x64-gnu-femtovg-wgpu.sh`
- 固定 `TARGET=x86_64-pc-windows-gnu`
- 其余 experimental 变量与 `build-win-x64-femtovg-wgpu.sh` 保持一致

优点：

- 与现有 `build-win-x64.sh` 的 host 语义一致
- 对 Linux 开发机最直接
- 与 MSVC wrapper 职责边界清晰
- README 和 smoke 测试都容易表达

缺点：

- 脚本数量增加一个

### 方案 B：把现有 MSVC wrapper 扩成 GNU / MSVC 双模式

优点：

- 少一个脚本

缺点：

- `build-win-x64-femtovg-wgpu.sh` 目前已经有非常明确的 MSVC-only 语义
- 混入 GNU 会让脚本名、help 文案、host 约束都变得含糊
- smoke 和文档边界更差

### 方案 C：新增一个通用 experimental Windows wrapper，再用环境变量切 target

优点：

- 灵活

缺点：

- 用户在 Linux 主机上更容易误用成 MSVC 目标
- 文档和验证矩阵会变复杂
- 与当前仓库“wrapper 显式表达目标边界”的风格不一致

**最终选择：方案 A**

## 设计决策

### 1. 新脚本命名

采用：

- `build-win-x64-gnu-femtovg-wgpu.sh`

原因：

- 明确表达 Linux 常用的 GNU 交叉目标
- 与 `build-win-x64-femtovg-wgpu.sh` 形成一对清晰命名
- 不会让人误认为它是 Windows 主机 MSVC 包装器

### 2. 固定目标

脚本默认并固定围绕：

- `TARGET=x86_64-pc-windows-gnu`

不在第一版中加入 `aarch64` 或其他 Windows target。

### 3. renderer 语义

该 wrapper 必须与现有 experimental wrapper 对齐：

- `APP_NAME=mica-term-femtovg-wgpu-experimental`
- `BIN_NAME=mica-term`
- `CARGO_NO_DEFAULT_FEATURES=1`
- `CARGO_FEATURES=femtovg-wgpu-experimental`

这保证：

- 打包脚本只是进入 experimental 构建链的入口
- 真正的 renderer 仍由程序内部 profile + selector 决定
- 不会通过脚本引入任何 `software` 退路

### 4. host 前提

该脚本面向 Linux 主机，依赖沿用 `build-desktop.sh` 的 GNU 交叉要求：

- 安装 Rust target：
  - `x86_64-pc-windows-gnu`
- 可用 MinGW linker：
  - `x86_64-w64-mingw32-gcc`

脚本自身不重复实现环境检查逻辑，而是复用 `build-desktop.sh`。

### 5. 输出命名

输出 archive 应固定为：

- `dist/mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-gnu-release.zip`

这与现有 Linux experimental archive 和 Windows MSVC experimental archive 维持一致的命名规则，只变更 target triple。

## 测试策略

新增一个 smoke：

- `tests/build_win_x64_gnu_femtovg_wgpu_script_smoke.sh`

至少验证：

- 脚本存在
- `bash -n` 通过
- `--help` 包含：
  - `x86_64-pc-windows-gnu`
  - `mica-term-femtovg-wgpu-experimental`
  - `--no-default-features`
  - `.zip`
- 脚本正文包含：
  - `femtovg-wgpu-experimental`

并继续复用现有 smoke 确保：

- `build-win-x64-femtovg-wgpu.sh` 仍然是 MSVC-only experimental wrapper
- `build-release.sh` 不被污染

## 风险与边界

### 风险 1：用户误解 GNU / MSVC 差异

缓解：

- 在 help 文案里明确：
  - 这个新脚本是 Windows GNU experimental package
  - 现有 `build-win-x64-femtovg-wgpu.sh` 是 Windows MSVC experimental package

### 风险 2：误把 GNU wrapper 当成正式链入口

缓解：

- README 中单独归类到 `FemtoVG WGPU Experimental`
- 不把它挂进 `build-release.sh`

### 风险 3：脚本语义回退到 formal build

缓解：

- smoke 必须检查 `CARGO_FEATURES=femtovg-wgpu-experimental`
- smoke 必须检查 `APP_NAME=mica-term-femtovg-wgpu-experimental`

## 最终结论

新增一条独立的：

- `build-win-x64-gnu-femtovg-wgpu.sh`

是当前最干净、最符合仓库风格、也最适合 Linux 主力开发机的方案。

它不会改变既有 MSVC experimental wrapper 的角色，只是补齐：

- `Linux host -> Windows GNU experimental package`

这条缺失的入口。
