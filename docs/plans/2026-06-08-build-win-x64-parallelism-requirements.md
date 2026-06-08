# Build Win x64 Parallelism Requirements

日期: 2026-06-08
执行者: Codex
状态: 只产出 requirements，待在新 worktree 中实现

## 任务边界

本轮只做只读调研、证据整理与方案设计，不修改业务代码、脚本代码、测试代码、`Cargo.toml`、`Cargo.lock` 或配置文件。

正式实现必须另开新窗口，并在对应的 `.worktrees` 目录中执行。

## 问题陈述

当前运行 `./build-win-x64.sh` 打 Windows x64 包时，用户观察到整体耗时偏长，且在 `htop` 中常常只看到一个前台进程，直觉上像是 CPU 没有被充分利用。

本轮调研确认：

- 当前入口 `./build-win-x64.sh` 实际上是一个薄 wrapper，本体构建发生在 `build-desktop.sh` 中。
- Cargo 默认本身就会并行编译多个 crate；“只看到一个进程”并不自动等价于“没有并行”。
- 在 fresh `cargo xwin build` 采样中，Cargo 并发峰值已经达到 `jobs=32 / ncpu=32`，但构建后半段仍会收敛到单 `rustc` 长尾，因此真正问题更像是“需要显式覆盖并行度、强化可观测性，并把瓶颈定位方法写清楚”，而不是“盲目多开多个 cargo 进程”。

## 事实冲突审计

用户 prompt 中存在一条历史直觉：`build-win-x64.sh` 默认应面向 `x86_64-pc-windows-gnu`。

但本地代码与 smoke tests 的当前事实是：

- `build-win-x64.sh` 默认 target 是 `x86_64-pc-windows-msvc`
- `build-win-x64-software.sh` 默认 target 是 `x86_64-pc-windows-gnu`
- `build-win-x64.sh` 会显式拒绝 `x86_64-pc-windows-gnu` + Skia 组合

因此，本 requirements 以“当前仓库真实语义”为准：

- 未来实现不得在本次并行度优化中顺手改动现有 GNU / MSVC wrapper 语义。
- `build-win-x64.sh` 继续保持当前 MSVC 主线路由。
- Windows GNU 默认包装入口继续由 `build-win-x64-software.sh` 承担。

如需重新统一 Windows wrapper 语义，应另开独立设计与实现任务，不混入本次构建性能工作。

## 用户目标

- 在 AMD Ryzen 9 7950X3D 这类高核数机器上，尽量吃满可安全使用的 CPU 资源。
- 允许用户显式控制 Cargo 并行度，避免只能依赖 Cargo 隐式默认值。
- 不牺牲脚本可维护性、发布确定性、跨 target 兼容性与现有 smoke 契约。
- 不把“性能优化”演变成“重写构建系统”或“改变现有 Windows 打包语义”。

## 非目标

- 本轮不改代码。
- 不重写构建系统。
- 不改变 renderer / runtime 语义。
- 不绕过 `--locked`。
- 不采用“无证据多开多个 cargo 进程并发写同一个 `target` 目录”的危险方案。
- 不在没有 profiling 证据前，直接修改 `release` profile、LTO、`codegen-units`、`incremental`、`strip` 等发布质量相关默认值。
- 不引入 `7z`、`pigz`、`zstd`、`sccache` 等新的强依赖作为第一阶段前提条件。

## 功能需求

## 1. 显式并行度控制

未来正式实现必须支持显式控制 Cargo 并行度。

本轮设计选择的公开脚本入口为：

- `BUILD_JOBS=<positive-integer>`

同时允许高级用户继续直接使用 Cargo 原生环境变量：

- `CARGO_BUILD_JOBS=<cargo-native-value>`

但仓库文档与 wrapper 的主推荐入口应为 `BUILD_JOBS`，因为：

- 用户使用成本最低：`BUILD_JOBS=32 ./build-win-x64.sh`
- wrapper 可以打印明确的 resolved jobs 决策
- 不需要新增命令行参数解析复杂度

### 1.1 `BUILD_JOBS` 取值约束

- 第一阶段仅接受正整数
- `0`、负数、非数字必须报错
- 本轮不为 `BUILD_JOBS` 设计 `auto` / `default` / 负数偏移语义
- 如用户需要 Cargo 原生更复杂语义，可继续直接设置 `CARGO_BUILD_JOBS`

## 2. help 输出必须说明并行参数

未来实现后，至少以下帮助入口必须说明并行度控制：

- `./build-desktop.sh --help`
- `./build-win-x64.sh --help`
- 如 `build-win-x64-software.sh` 仍是用户常用 Windows GNU 入口，也应同步说明

help 文案至少要覆盖：

- `BUILD_JOBS=<N>` 的用法
- 未设置时使用 Cargo 默认并行度
- 该设置同时影响 `cargo build` 与 `cargo xwin build` 路径

## 3. 构建日志必须打印 resolved jobs 决策

未来实现后，构建日志必须在执行 Cargo 前打印：

- 实际 driver：`cargo build` 或 `cargo xwin build`
- `target`
- `profile`
- `features`
- resolved jobs 决策

至少应能区分：

- `jobs=default`
- `jobs=BUILD_JOBS:32`
- `jobs=CARGO_BUILD_JOBS:<value>`

目的不是美化日志，而是让 profiling、CI 排障与 smoke test 有可锁定的可观测输出。

## 4. 默认行为

默认行为必须二选一并给出论证：

- 保持 Cargo 默认并行
- 或采用安全的 auto 策略

本轮 requirements 接受设计文档的推荐结论：

- 第一阶段保持 Cargo 默认并行
- 不在 wrapper 侧新增脚本级 auto 调度器

理由：

- fresh build 已观测到 Cargo 默认并发峰值达到 `jobs=32 / ncpu=32`
- 同条件 `-j1` 明显更慢
- 当前真正的长尾更像后半段关键路径收敛，而不是“默认并行没启用”
- 先加显式覆盖旋钮与日志，比先发明新的 auto 策略更稳

## 5. 用户可直接利用高核数机器

未来实现后，必须支持以下用法：

```bash
BUILD_JOBS=32 ./build-win-x64.sh
BUILD_JOBS=$(nproc) ./build-win-x64.sh
```

并应在文档中明确：

- 对 7950X3D 这类机器，`BUILD_JOBS=32` 或 `BUILD_JOBS=$(nproc)` 是合理起点
- 但不应硬编码到脚本默认值中
- 具体最佳值取决于 RAM、是否同时跑其他任务、以及尾部长链接阶段占比

## 6. 压缩阶段优化只能作为可选后续项

如果 profiling 证明压缩阶段是显著瓶颈，则设计可以提供可选 parallel zip / 7z 路径。

但第一阶段必须满足：

- 不强依赖非基础工具
- 不改变当前 `.zip` 产物契约
- 不把 Windows 打包默认产物切成 `.7z`

## 7. 必须有 profiling 指南

未来实现文档必须给出 profiling 指南，能区分慢点落在：

- Rust 编译阶段
- `build.rs` / Slint codegen / resource embedding 阶段
- 链接阶段
- `cargo-xwin` / MinGW 工具链阶段
- stage / copy 阶段
- zip 压缩阶段
- `build-release.sh` 聚合多个目标时的串行阶段

至少应提供以下命令或等价命令：

```bash
time ./build-win-x64.sh
cargo xwin build --timings ...
cargo build --timings ...
```

如主机缺少 `/usr/bin/time -v` 或 `hyperfine`，文档应明确可用替代方案，而不是假装这些工具总存在。

## 8. 共享实现约束

未来实现必须优先落在 `build-desktop.sh` 共享层。

- `build-win-x64.sh` 保持 thin wrapper / 环境注入语义
- `build-win-x64-software.sh` 也不应复制 jobs 解析逻辑
- `build-release.sh` 继续保持共享实现之上的聚合入口，而不是重新发明并行构建控制面

## 验收标准

- 文档清楚说明未来实现后的命令示例。
- smoke test 能锁定 jobs 参数透传。
- 不设置 jobs 时兼容旧行为。
- 设置 jobs 时 `cargo build` / `cargo xwin build` 能收到参数。
- 当前 Windows target 默认语义不变：
  - `build-win-x64.sh` 继续保持 `x86_64-pc-windows-msvc`
  - Windows GNU 默认包装入口继续由 `build-win-x64-software.sh` 承担
- `build-desktop.sh` 仍为共享实现。
- `.zip` 产物契约不变。
- `--locked` 不被移除。
- 实现文档必须告诉用户如何用 profiling 证明瓶颈到底是不是并行度本身。

## 未来实现后的最小示例

```bash
# 保持 Cargo 默认并行
./build-win-x64.sh

# 显式指定 32 jobs
BUILD_JOBS=32 ./build-win-x64.sh

# 自动取当前机器逻辑核数
BUILD_JOBS=$(nproc) ./build-win-x64.sh

# 高级用法：直接使用 Cargo 原生环境变量
CARGO_BUILD_JOBS=24 ./build-win-x64.sh
```

## 结论

本次性能工作的一阶段 requirements 不要求“发明新的并行构建系统”，而要求：

- 共享层暴露一个安全、透明、可测试的 jobs 覆盖旋钮
- 默认继续信任 Cargo 默认并行
- 用日志与 profiling 把瓶颈定位清楚
- 把 linker / cache / 7z 这类更激进的优化留到有证据的后续阶段
