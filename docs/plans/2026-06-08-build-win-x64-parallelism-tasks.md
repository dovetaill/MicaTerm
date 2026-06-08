# Build Win x64 Parallelism Tasks

日期: 2026-06-08
执行者: Codex
状态: 未来正式实现任务拆解；本轮不执行

## 任务总则

本任务清单面向后续正式实现阶段。

要求：

- 在新的 `.worktrees` 目录中执行
- 先补 smoke tests，再做最小实现
- 每个阶段都保留 fresh profiling 对照
- 第一阶段只做 `BUILD_JOBS -> cargo --jobs` 的共享层薄映射
- 不顺手改默认 target、linker、LTO、`codegen-units`、`incremental`、`.zip` 契约或 release 聚合语义

## Task 1：增加 / 更新 smoke tests，先失败

目标：先把未来行为写成会失败的契约测试，再动实现。

### 建议修改 / 新增的测试文件

- 修改：`tests/build_desktop_script_smoke.sh`
- 修改：`tests/build_win_x64_script_smoke.sh`
- 视实现方式二选一：
  - 新增：`tests/build_desktop_jobs_smoke.sh`
  - 或在现有 fake-cargo/fake-rustup 风格 smoke 基础上扩展一份 jobs 专项断言

### 必须新增的断言

1. `build-desktop.sh --help` 或 `build-win-x64.sh --help` 中出现并行说明：
   - `BUILD_JOBS`
   - 说明未设置时使用 Cargo 默认并行
   - 说明该设置影响 `cargo build` / `cargo xwin build`

2. 设置 `BUILD_JOBS=32` 时：
   - 普通 Cargo 路径会把 `--jobs 32` 传给 `cargo build`
   - Windows MSVC 路径会把 `--jobs 32` 传给 `cargo xwin build`

3. 未设置 `BUILD_JOBS` 时：
   - 旧行为兼容
   - wrapper 不自行追加 `--jobs`

4. 无效 `BUILD_JOBS` 必须被拒绝：
   - `BUILD_JOBS=0`
   - `BUILD_JOBS=-1`
   - `BUILD_JOBS=abc`
   - 报错必须清晰指出：`BUILD_JOBS` 仅接受正整数

5. 当前 target 语义不得漂移：
   - `build-win-x64.sh` 仍默认 `x86_64-pc-windows-msvc`
   - Windows GNU 默认包装入口仍是 `build-win-x64-software.sh`

### 推荐测试实现方式

优先使用 fake `cargo` / fake `rustup` / fake `zip` 的 shell smoke，原因是：

- 当前仓库已经有类似的 fake tool shim 测试风格
- 能精确断言最终命令行是否包含 `--jobs`
- 不需要真实跑一遍完整 Windows 打包

### 推荐验证命令

```bash
bash tests/build_desktop_script_smoke.sh
bash tests/build_win_x64_script_smoke.sh
# 若新增 jobs 专项 smoke：
bash tests/build_desktop_jobs_smoke.sh
```

预期：

- 第一次补测试时应先失败
- 失败原因应直接指向 help / 参数透传 / 非法值处理尚未实现

## Task 2：最小实现 jobs 参数解析

目标：只做共享层的最小必要实现，不扩散范围。

### 实现边界

- 优先在 `build-desktop.sh` 共享实现中实现
- `build-win-x64.sh` 继续保持 wrapper 语义
- `build-win-x64-software.sh` 继续保持 wrapper 语义
- `build-release.sh` 不改变串行聚合语义

### 建议实现策略

1. 读取 `BUILD_JOBS`
2. 校验它是否为正整数
3. 若合法，则在 `CARGO_BUILD_ARGS` 中追加：

```bash
--jobs <N>
```

4. 若未设置：
   - 不追加 `--jobs`
   - 保持 Cargo 默认并行

5. 如 `CARGO_BUILD_JOBS` 已由调用方提供：
   - 允许继续生效
   - 构建日志中应尽量打印该信息
   - 但 repo 文档主推荐入口仍是 `BUILD_JOBS`

### 推荐日志字段

未来实现至少打印：

- resolved driver
- resolved target
- resolved profile
- resolved features
- resolved jobs

建议输出风格示例：

```text
==> Building mica-term for x86_64-pc-windows-msvc (release)
==> Build driver: cargo xwin build
==> Features: slint-renderer-skia,terminal-native-renderer
==> Jobs: BUILD_JOBS=32 -> --jobs 32
```

未设置时建议类似：

```text
==> Jobs: default (Cargo decides)
```

### 推荐验证命令

```bash
bash tests/build_desktop_script_smoke.sh
bash tests/build_win_x64_script_smoke.sh
# 若新增 jobs 专项 smoke：
bash tests/build_desktop_jobs_smoke.sh
```

## Task 3：更新 README / build docs

目标：把新的 jobs 控制与 profiling 指南明确公开。

### 文档必须新增的示例

```bash
BUILD_JOBS=32 ./build-win-x64.sh
BUILD_JOBS=$(nproc) ./build-win-x64.sh
```

### 文档必须解释的差异

- Cargo 默认会并行编译多个 crate
- `BUILD_JOBS` 是 repo wrapper 的显式覆盖旋钮
- `CARGO_BUILD_JOBS` 是 Cargo 原生高级入口
- 未设置 `BUILD_JOBS` 时，wrapper 不改变旧默认行为

### 文档必须解释的事实冲突

- 当前 repo 中 `build-win-x64.sh` 默认是 MSVC，不是 GNU
- Windows GNU 默认包装入口是 `build-win-x64-software.sh`
- 本次性能工作不改变这条现有语义

### 推荐验证命令

```bash
rg -n "BUILD_JOBS|CARGO_BUILD_JOBS|Cargo default parallelism|x86_64-pc-windows-msvc|x86_64-pc-windows-gnu" readme.md build-desktop.sh build-win-x64.sh build-win-x64-software.sh
```

## Task 4：profiling 与验证

目标：证明瓶颈到底在哪，并对比 jobs 覆盖的真实收益。

### 必跑对照组

1. 默认：

```bash
time ./build-win-x64.sh
```

2. 显式逻辑核数：

```bash
BUILD_JOBS=$(nproc) time ./build-win-x64.sh
```

3. 显式固定 32：

```bash
BUILD_JOBS=32 time ./build-win-x64.sh
```

4. fresh cargo timings 基线：

```bash
cargo clean
cargo xwin build --release --target x86_64-pc-windows-msvc --locked --no-default-features --features slint-renderer-skia,terminal-native-renderer --timings
```

5. fresh 串行对照：

```bash
cargo clean
cargo xwin build -j1 --release --target x86_64-pc-windows-msvc --locked --no-default-features --features slint-renderer-skia,terminal-native-renderer --timings
```

### 运行中采样建议

```bash
while sleep 1; do
  date +%T
  pgrep -af 'rustc|clang|lld-link|link.exe'
  echo
 done | tee /tmp/mica-build-procs.log
```

### 工具可用性说明

- 如果主机有 `/usr/bin/time -v`，优先使用它
- 如果像本轮环境一样没有 `/usr/bin/time -v`，则使用 shell 内建 `time` 或 `time -p`
- 如果安装了 `hyperfine`，可追加 benchmark；若未安装，不应把它当成实现前提

### 成果记录要求

- 保存每次 `cargo --timings` 的 HTML 报告
- 比较：
  - `Total time`
  - `Max concurrency`
  - 根 crate 长尾是否缩短
  - 末尾单 `rustc` / 链接阶段是否缩短

### zip 阶段 follow-up 门槛

只有当 zip 阶段满足以下任一条件时，才单独开后续优化任务：

- 持续超过端到端 wall time 的 10%
- 或持续超过 15s

否则 zip 继续维持当前 `.zip` 路径，不优先动。

## Task 5：后续可选优化，不在第一阶段实现

这些方向都值得保留，但必须在 Task 4 的 profiling 证据支持下再开独立任务。

### 5.1 linker / 链接尾巴优化

候选方向：

- 更细化的 `lld-link` 评估
- target-specific linker flags
- 仅在有证据时评估 link-time flags

不在第一阶段直接修改的项：

- 默认 LTO
- 默认 `codegen-units=1`
- 默认 `strip`
- 默认 `panic=abort`

### 5.2 `sccache`

候选方向：

- CI 中通过 `RUSTC_WRAPPER=sccache` 注入
- 本地开发文档提供 opt-in 指南
- 评估 `SCCACHE_BASEDIRS` 与不同 checkout 路径复用效果

### 5.3 parallel zip / 7z

候选方向：

- 保持 `.zip` 合同，额外提供可选 `7z -mmt`
- 或在具备依赖时提供 opt-in parallel zip 路径

前提：

- 不改变默认 `.zip` 合同
- 不强依赖 7z

### 5.4 CI matrix cache / xwin cache 预热

候选方向：

- 预热 `cargo-xwin` cache
- 复用 Cargo registry / target cache
- 明确 Windows MSVC cross-build 的 cache key 设计

### 5.5 release profile 调整

只有当 profiling 能明确证明收益，才考虑独立任务研究：

- `lto`
- `codegen-units`
- `incremental`
- `debug`
- `strip`

## 实施完成后的最小验收清单

- `BUILD_JOBS=32` 能透传到 `cargo build` / `cargo xwin build`
- 未设置 `BUILD_JOBS` 时旧行为兼容
- 非法 `BUILD_JOBS` 早失败且报错清楚
- help 文案提到 parallelism 入口
- build log 打印 resolved jobs
- `build-desktop.sh` 继续是共享实现
- `build-win-x64.sh` 继续保持 MSVC 默认语义
- Windows GNU 默认包装入口继续是 `build-win-x64-software.sh`
- `.zip` 产物契约不变
- profiling 对比结果能证明实现没有把默认性能倒退

## 本轮说明

本文件只定义未来任务，不代表这些任务已经执行。

本轮没有修改任何业务代码、脚本代码、测试代码或配置文件。
