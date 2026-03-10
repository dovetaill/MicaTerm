# Mica Term Desktop Build Matrix Design

日期: 2026-03-10  
执行者: Codex

## 背景

当前仓库只有 `build-win-x64.sh`, 且文档只覆盖 Windows x64 构建。代码主体本身并未锁死到单一平台, 但构建入口、打包产物、宿主机约束与验证策略都还没有形成可复用的桌面多平台方案。

本次设计的目标不是一次性做完整发布系统, 而是先把桌面平台构建矩阵扩到 Linux x64/arm64、macOS、Windows ARM64, 形成统一入口与一致的归档产物, 为后续 CI、签名与平台安装器打基础。

## 目标

- 提供一个统一的桌面构建入口, 覆盖 Linux x64/arm64、macOS、Windows ARM64
- 保留现有 `build-win-x64.sh` 用法, 避免破坏已有调用方式
- 明确每个 target 的宿主机与工具链前提, 让脚本失败得更早、更可读
- 统一产物目录命名与归档命名, 便于本地和 CI 消费
- 用最小 smoke test 覆盖脚本入口、帮助文本与关键 target 路由

## 边界

### 本次覆盖

- 统一构建脚本 `build-desktop.sh`
- Windows 兼容包装脚本
- 目标矩阵与宿主机约束
- 归档格式与 staging 目录规则
- README 文档更新
- shell smoke test 更新与新增

### 本次不覆盖

- Android 构建
- `.app`、`.dmg`、`.msix`、`.deb`、`.rpm` 等平台安装器
- macOS 签名、notarization
- Windows 签名
- GitHub Actions / GitLab CI matrix
- 运行时平台差异修复

## 方案对比

### 方案 A: 单一统一入口 + 兼容包装脚本

- 新增 `build-desktop.sh` 作为唯一真实实现
- `build-win-x64.sh` 保留, 内部只转发到统一入口
- 所有公共逻辑集中在一个脚本里维护

优点:

- 最适合当前仓库阶段, 公共逻辑不会复制
- README 和 smoke test 可以围绕一个事实来源更新
- 后续接 CI 时, matrix 只需要切 target 和 host 条件

缺点:

- 脚本内部需要维护一组清晰的 target 分支

### 方案 B: 按平台拆分独立脚本

- `build-linux.sh`
- `build-macos.sh`
- `build-win.sh`

优点:

- 平台专属逻辑更直观

缺点:

- 现在就拆会带来大量重复逻辑
- target、打包目录、README 拷贝与参数校验更容易漂移

### 方案 C: 只做 CI matrix, 不提供统一本地入口

优点:

- 适合发布完全依赖 CI 的团队

缺点:

- 不适合当前仓库阶段
- 本地复现和调试成本更高

最终选择: `方案 A`

## 设计

### 1. 构建入口

新增 repo-root `build-desktop.sh`, 作为桌面目标的统一入口。脚本保留现有 `TARGET` / `PROFILE` / `APP_NAME` / `BIN_NAME` / `DIST_DIR` 风格, 以减少使用心智负担。

`build-win-x64.sh` 不再承载真实逻辑, 只做兼容包装:

- 默认把 `TARGET` 设为 `x86_64-pc-windows-gnu`
- 继续接受 `TARGET=x86_64-pc-windows-msvc ./build-win-x64.sh`
- 直接转发到 `build-desktop.sh`

这样既保留历史入口, 又避免 Windows 与其他桌面平台逻辑分叉。

### 2. 目标矩阵

首批支持的 target:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `aarch64-pc-windows-msvc`
- 兼容保留:
  - `x86_64-pc-windows-gnu`
  - `x86_64-pc-windows-msvc`

宿主机约束:

- Linux GNU targets:
  - 可在 Linux 主机上本地构建
  - `aarch64-unknown-linux-gnu` 需要外部 linker
- macOS targets:
  - 仅声明为 macOS 主机或对应 CI runner 上的受支持目标
  - 当前 Linux 主机不承诺可交叉产出可运行 macOS 二进制
- Windows MSVC targets:
  - 仅声明为 Windows MSVC 环境支持
- Windows GNU x64:
  - 继续保留 Linux + MinGW-w64 的交叉构建路径

脚本需要对这些差异给出 fail-fast 错误信息, 而不是让 `cargo build` 在深处报模糊错误。

### 3. 产物与打包

统一 staging 目录:

- `dist/<app>-<target>-<profile>/`

统一可执行文件命名:

- Windows: `<bin>.exe`
- 其他桌面平台: `<bin>`

归档格式:

- Windows: `zip`
- Linux/macOS: `tar.gz`

统一归档命名:

- Windows: `dist/<app>-<target>-<profile>.zip`
- Linux/macOS: `dist/<app>-<target>-<profile>.tar.gz`

Staging 内容保持最小:

- 主可执行文件
- `README.md` 复制为大写形式

本阶段不引入额外资源清单、动态库收集或平台安装器结构。

### 4. 测试与验证

测试策略分两层:

- Shell smoke test:
  - `build-desktop.sh` 存在且 `bash -n` 通过
  - `--help` 输出包含所有首批支持 target
  - `--help` 输出包含 `zip` / `tar.gz` 产物说明
- 兼容性 smoke test:
  - `build-win-x64.sh --help` 仍然可用
  - 帮助输出仍包含 Windows x64 历史入口语义

代码级验证继续沿用仓库现有 Rust 命令:

- `cargo test -q`
- `cargo fmt --check`
- `cargo check --workspace`

如 `clippy` 在当前基线未纳入统一验收, 本次不额外扩大验收范围。

### 5. README 策略

`readme.md` 从“Windows Build”扩展为“Desktop Build”, 明确区分:

- 统一入口命令
- 兼容 Windows x64 包装命令
- 各 target 的工具链前提
- 哪些 target 需要在哪类宿主机上构建

文档要避免暗示“任意主机都可交叉打满全矩阵”, 否则会制造错误预期。

## 风险与约束

- `window-vibrancy` 依赖当前虽然未见显式调用, 但未来若启用, Linux 侧需要条件编译约束
- macOS 与 Windows ARM64 的真正可用性依赖对应宿主机或 CI runner, 当前 Linux 主机只能提供脚本与文档层验证
- `aarch64-unknown-linux-gnu` 的交叉编译依赖系统 linker, 不能只靠 `rustup target add`

## 验收标准

- 仓库存在统一桌面构建入口 `build-desktop.sh`
- `build-win-x64.sh` 仍可作为兼容入口使用
- README 明确列出支持矩阵与宿主机前提
- smoke tests 能覆盖统一入口与兼容入口
- 当前主机上的基线 Rust 测试与检查仍通过
