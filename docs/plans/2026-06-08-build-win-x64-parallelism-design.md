# Build Win x64 Parallelism Design

日期: 2026-06-08
执行者: Codex
状态: 只产出 design，待在新 worktree 中实现

## 设计结论摘要

本轮推荐的一阶段方案是：

- 在 `build-desktop.sh` 共享实现层新增公开环境变量 `BUILD_JOBS`
- 当 `BUILD_JOBS` 为正整数时，wrapper 向 `cargo build` 或 `cargo xwin build` 追加 `--jobs <N>`
- 默认不改，继续保持 Cargo 默认并行
- 帮助文案、构建日志和 smoke tests 一起锁定该行为
- 不采用“多开多个 cargo build 进程”方案
- 不在第一阶段默认启用 linker、sccache、7z 等额外优化
- 先把 profiling 指南与可观测性做扎实，再决定第二阶段是否值得追 linker / cache / 压缩优化

## 本地代码事实摘要

## 1. `build-win-x64.sh` 是 thin wrapper，且当前默认 target 是 MSVC，不是 GNU

- 帮助文案直接把默认 target 写成 `x86_64-pc-windows-msvc`：`build-win-x64.sh:18`, `build-win-x64.sh:19`
- wrapper 代码把 `TARGET` 默认设成 `x86_64-pc-windows-msvc`：`build-win-x64.sh:43`
- 如果用户把 `TARGET` 设成 `x86_64-pc-windows-gnu`，脚本会直接报错并要求改用 software wrapper：`build-win-x64.sh:45`, `build-win-x64.sh:47`, `build-win-x64.sh:48`
- wrapper 最终只是 `exec "$ROOT_DIR/build-desktop.sh" "$@"`：`build-win-x64.sh:71`

结论：

- 用户/历史直觉里的“build-win-x64 默认 GNU”与当前仓库事实冲突
- 本次并行度方案不能顺手改动该语义

## 2. Windows GNU 默认包装入口当前在 `build-win-x64-software.sh`

- 帮助文案把默认 target 写成 `x86_64-pc-windows-gnu`：`build-win-x64-software.sh:16`, `build-win-x64-software.sh:17`
- wrapper 代码把 `TARGET` 默认设成 `x86_64-pc-windows-gnu`：`build-win-x64-software.sh:40`
- 它同样只是设环境变量后再 `exec` 到 `build-desktop.sh`：`build-win-x64-software.sh:43`, `build-win-x64-software.sh:44`, `build-win-x64-software.sh:52`

结论：

- GNU / MSVC 的“路由逻辑”目前分散在 wrapper 语义里
- jobs 解析应该尽量收敛到共享层，而不是分别在 wrapper 里复制

## 3. `build-desktop.sh` 才是真正的共享构建入口

- 默认构建命令先设为 `CARGO_BUILD_CMD=(cargo build)`：`build-desktop.sh:342`
- 在 Linux-host Windows MSVC 分支中切换为 `CARGO_BUILD_CMD=(cargo xwin build)`：`build-desktop.sh:408`, `build-desktop.sh:414`, `build-desktop.sh:421`
- 最终实际执行点统一在：`build-desktop.sh:450`
- 构建参数统一从 `CARGO_BUILD_ARGS=... --target "$TARGET" --locked` 组装：`build-desktop.sh:440`

结论：

- 若要支持 jobs，应优先在 `build-desktop.sh` 共享执行点落地
- 这样同时覆盖普通 `cargo build` 与 `cargo xwin build`

## 4. 当前仓库没有自定义 jobs 配置

- `build-desktop.sh` 当前没有向 Cargo 追加 `--jobs`：`build-desktop.sh:440`, `build-desktop.sh:450`
- `.cargo/config.toml` 只设置了 `RUST_MIN_STACK`，没有 `build.jobs`：`.cargo/config.toml:1`, `.cargo/config.toml:5`
- `Cargo.toml` 没有 `[profile.release]` 的 LTO / `codegen-units` / `incremental` 自定义：`Cargo.toml:1`, `Cargo.toml:92`

结论：

- 当前并行度完全依赖 Cargo 默认值或用户外部环境
- 当前 repo 不存在“仓库已经手写固定 jobs”的隐藏逻辑

## 5. `build.rs` 本身包含潜在的串行长尾

- `build.rs` 显式开了一个命名线程来执行 `run_build`：`build.rs:16`, `build.rs:21`, `build.rs:23`
- `run_build` 中会执行 Slint codegen：`build.rs:36`, `build.rs:39`, `build.rs:40`
- Windows target 时还会编译图标资源：`build.rs:43`, `build.rs:46`, `build.rs:48`

结论：

- 即使 Cargo 在 crate 层面并行，单个 crate 的 `build.rs` / codegen / link 仍可能成为长尾
- “htop 看起来只有一个进程”并不能直接否定 Cargo 并行

## 6. Linux-host Windows MSVC 路径已经有较重的 cargo-xwin / LLVM shim 逻辑

- `build-desktop.sh` 会写入共享 shim 目录 `target/cargo-xwin-tools`：`build-desktop.sh:230`, `build-desktop.sh:236`, `build-desktop.sh:240`
- 会写 `target/cargo-xwin-patched-registry` 来修 ICU import archive：`build-desktop.sh:250`, `build-desktop.sh:259`, `build-desktop.sh:265`
- 会写 `target/cargo-xwin-libs` 并注入 `Advapi32.lib` 大小写 shim：`build-desktop.sh:288`, `build-desktop.sh:293`, `build-desktop.sh:295`
- `build.rs` / `build_support/xwin_link.rs` 也会再补一层 `Advapi32.lib` shim：`build.rs:29`, `build.rs:33`, `build_support/xwin_link.rs:37`, `build_support/xwin_link.rs:46`, `build_support/xwin_link.rs:49`

结论：

- 当前仓库并不适合“多开多个 cargo build 写同一个 `target` 目录”
- 除了 Cargo 自身的锁竞争，还会叠加 shim 目录与 patched registry 的共享写风险

## 7. `build-release.sh` 当前是严格串行聚合

- 只支持 `MODE=fail-fast` / `MODE=best-effort`：`build-release.sh:15`, `build-release.sh:58`
- 目标数组固定为 Linux x64 + Windows GNU software：`build-release.sh:66`, `build-release.sh:67`, `build-release.sh:68`
- 通过 `for target in ...; do` 逐个串行执行：`build-release.sh:74`, `build-release.sh:75`
- `MODE=fail-fast` 在第一处失败立刻退出：`build-release.sh:81`, `build-release.sh:82`

结论：

- release 聚合层当前强调确定性，而不是多 target 并发 fan-out
- 本轮性能工作不应顺手把 release 聚合也改成并发

## 8. 当前 `.zip` 产物契约是稳定且被测试锁定的

- Windows GNU / MSVC 都要求 `zip`：`build-desktop.sh:399`, `build-desktop.sh:409`
- Windows 归档分支就是 `.zip`：`build-desktop.sh:470`, `build-desktop.sh:474`
- `tests/build_win_x64_script_smoke.sh` 明确锁定 `.zip`：`tests/build_win_x64_script_smoke.sh:28`
- `readme.md` 也公开了 Windows wrapper 输出是 `.zip`：`readme.md:38`, `readme.md:45`, `readme.md:86`

结论：

- 第一阶段不应把默认产物改成 `.7z`
- 压缩器优化最多是后续可选路径

## 只读 profiling 与证据链

## 1. 端到端 warm-ish 包装实测

本地运行：

```bash
TIMEFORMAT=$'real %3R\nuser %3U\nsys %3S'; { time ./build-win-x64.sh; }
```

观测：

- 总 real 时间约 `64.533s`
- Cargo 阶段打印 `Finished release profile [optimized] target(s) in 59.47s`
- 说明端到端时间的绝大部分花在编译，而不是 stage/copy/zip
- 同次输出还出现 `Blocking waiting for file lock on artifact directory`，说明 warm run 里还叠加了 artifact lock 等待

推断：

- 用户体感里的“慢”主要在 Cargo 构建阶段
- 单靠优化 zip 不可能解决主问题

## 2. fresh `cargo xwin build` 默认并行实测

本地生成 isolated target-dir 的 fresh profiling 构建，并保留 `cargo --timings` HTML：

- `Total units: 1133`
- `Max concurrency: 34 (jobs=32 ncpu=32)`
- `Total time: 438.9s (7m 18.9s)`

关键含义：

- Cargo 默认并行在这台 32 线程机器上已经被充分使用
- “CPU 没吃满”不是因为 Cargo 完全没并行
- 更像是后半段关键路径收敛到少数不可并行单元

## 3. fresh `-j1` 对照实测

同条件下运行：

- `cargo xwin build -j1 ... --timings`
- `Total time: 1427.4s (23m 47.4s)`
- `Max concurrency: 2 (jobs=1 ncpu=32)`

关键含义：

- 把 jobs 压成 1 会让构建时间从 7m18s 恶化到 23m47s
- 这直接证明默认并行是有效的
- 第一阶段正确方向不是“多开多个 cargo”，而是“允许显式覆盖 jobs 并把 resolved state 打印出来”

## 4. 进程采样结果

只读采样中，构建早期曾看到：

- 最多约 30 个 `rustc` 子进程

构建后半段则长期变成：

- 只有 1 个 `rustc`

关键含义：

- 这是典型的 crate DAG “前宽后窄”形态
- 前半段依赖很多，可并行
- 后半段逐步收敛到单一关键路径，常见于最终 crate 的 codegen / link / build.rs 长尾

## 5. timing 报告中最重的构建单元

fresh 默认并行 timing 报告前几项显示：

- `mica-term v0.1.0`：`363.6s`
- `aws-lc-sys build script (run)`：`50.5s`
- `windows v0.58.0`：`21.9s`
- `read-fonts v0.35.0`：`21.8s`
- `aws-sdk-s3 v1.127.0`：`19.9s`
- `zstd-sys build script (run)`：`19.9s`

其中根 crate `mica-term` 自身占据最重关键路径，且包含大量 frontend/codegen 时间。

推断：

- 当前瓶颈并不主要在 `cargo-xwin` 包装层本身
- 更可能落在 root crate、Slint/Skia 路径、最终 codegen / link 阶段

## 外部调研摘要

## 1. Cargo 原生并行与 jobs 控制

官方文档确认：

- `build.jobs` / `CARGO_BUILD_JOBS` / `cargo build --jobs N` 是 Cargo 原生并行度控制面
- 默认值是逻辑 CPU 数
- `--jobs` 可覆盖 config / env
- `cargo build --timings` 能输出 HTML 报告，展示单位构建时间和并发曲线

来源：

- Cargo config: <https://doc.rust-lang.org/cargo/reference/config.html>
- Cargo build command: <https://doc.rust-lang.org/cargo/commands/cargo-build.html>
- Cargo timings: <https://doc.rust-lang.org/cargo/reference/timings.html>
- Cargo env vars: <https://doc.rust-lang.org/cargo/reference/environment-variables.html>

## 2. Cargo build script 与 jobserver

官方文档确认：

- 每个 build script 默认只继承一个 job slot
- 若 build script 想额外并行，必须通过 jobserver 协调
- `CARGO_MAKEFLAGS` / jobserver 是 Cargo 与 `rustc` 的协同并行协议

来源：

- Build scripts: <https://doc.rust-lang.org/cargo/reference/build-scripts.html>
- rustc jobserver: <https://doc.rust-lang.org/rustc/jobserver.html>

这与本地 `build.rs` 可能形成串行长尾的观察一致。

## 3. cargo-xwin

官方 README 确认：

- `cargo-xwin` 是 Linux/macOS 上交叉编译 Windows MSVC 的工具层
- 常规用法仍是 `cargo xwin build --target x86_64-pc-windows-msvc`
- 它依赖 clang / LLVM 工具链
- 文档没有建议“多开多个 cargo xwin build 共享同一 target 目录”

来源：

- <https://github.com/rust-cross/cargo-xwin>

这支持“jobs 应走 Cargo 原生层，而不是自己并发多开 cargo-xwin”的结论。

## 4. sccache

官方 README 确认：

- `sccache` 推荐通过 `RUSTC_WRAPPER` 或 Cargo `rustc-wrapper` 接入
- 它更像缓存层 / CI 层能力，不是 repo 构建脚本必须强绑的第一阶段依赖
- Rust 编译可利用 jobserver，cargo + `RUSTC_WRAPPER=sccache` 通常会自动配合

来源：

- <https://github.com/mozilla/sccache>
- <https://github.com/mozilla/sccache/blob/main/docs/Rust.md>

这支持把 `sccache` 作为后续 opt-in，而不是第一阶段默认值。

## 5. linker 方向

Rust Performance Book 与 rustc 文档都表明：

- 更快 linker 可能显著改善编译尾部
- 但是否值得切换默认 linker，应以 profiling 与目标平台兼容性为前提
- Windows 也可考虑 lld，但仓库当前 Linux-host MSVC 已经通过 `lld-link` shim 工作，不宜在并行度任务里顺手扩大 linker 变更面

来源：

- Rust Performance Book: <https://nnethercote.github.io/perf-book/build-configuration.html>
- rustc codegen options: <https://doc.rust-lang.org/rustc/codegen-options/index.html>

## 6. zip / 7z

外部资料显示：

- 7-Zip 可以通过 `-mmt` 进行多线程压缩
- 但当前系统自带的 Info-ZIP `zip 3.0` 文档没有多线程控制契约
- 7-Zip 适合作为后续可选压缩增强路径，而不是替换现有 `.zip` 默认合同

来源：

- 7-Zip method switch: <https://7-zip.opensource.jp/chm/cmdline/switches/method.htm>
- 7-Zip SDK: <https://www.7-zip.org/sdk.html>
- Debian zip manpage: <https://manpages.debian.org/unstable/zip/zip.1.en.html>

## subagent 使用情况与真实性说明

- 当前 Codex 环境暴露了通用 subagent 工具，因此“subagent 机制”本身可用。
- 但工具接口没有暴露可验证的模型标识，因此无法确认这些 subagent 是否真的是 `gpt-5.4 xh`。
- 本轮仍按用户要求，实际发起了多角色 subagent 辩论。
- 首轮多 agent 中，部分 agent 因 `429 Too Many Requests`、`402 Payment Required`、usage limit 而失败。
- 之后通过缩短 prompt、减少上下文、关闭旧 agent 的方式恢复出一批有效 subagent 结果。
- 因此本节辩论记录是：
  - 一部分来自真实 subagent 返回
  - 一部分由主 agent 基于同一证据链手动补完收敛

这比谎称“全部由 gpt-5.4 xh 成功完成”更真实，也更符合本轮文档任务要求。

## 多专家辩论记录

## 第一轮：各专家独立陈述

### Rust / Cargo 构建系统专家

主张：

- jobs 控制应放在 `build-desktop.sh` 共享层
- 应映射到单次 Cargo 调用的 `--jobs` 或 `CARGO_BUILD_JOBS`
- 不应在 wrapper 层复制第二套实现

证据：

- 真正执行点只在 `build-desktop.sh:342`, `build-desktop.sh:421`, `build-desktop.sh:450`
- `build-win-x64.sh` 只是 `exec` 到共享脚本：`build-win-x64.sh:71`
- 当前 repo 没有任何 `BUILD_JOBS` 逻辑：`build-desktop.sh:440`, `.cargo/config.toml:1`

风险提示：

- 多开 cargo build 到同一个 `target` 目录会与 shim 目录、patched registry 共享写冲突

对其他方案的反驳：

- “htop 只见一个前台进程”不能证明 Cargo 没并行
- `build.rs` 单线程 +关键路径收敛本身就会制造单进程长尾观感

### Linux / Windows cross-compilation 专家

主张：

- 必须先尊重当前 wrapper 真实 target 语义
- jobs 应落在 `build-desktop.sh` 共享层，而不是按 ABI 分裂实现
- linker 优化先不进第一阶段

证据：

- `build-win-x64.sh` 当前默认 `x86_64-pc-windows-msvc`：`build-win-x64.sh:43`
- `build-win-x64-software.sh` 当前默认 `x86_64-pc-windows-gnu`：`build-win-x64-software.sh:40`
- Linux-host MSVC 实际 driver 是 `cargo xwin build`：`build-desktop.sh:421`
- 现有 `lld-link` shim 主要承担正确性修复，不是性能增强：`build-desktop.sh:246`, `build-desktop.sh:257`, `build-desktop.sh:417`

风险提示：

- 在并行度任务里顺手改 linker，容易把“性能方案”升级为“交叉编译链语义变更”

对其他方案的反驳：

- 不能因为用户想优化 Windows x64 就假设当前主线还是 GNU；本地事实已经不是这样

### CI / Release 工程专家

主张：

- `BUILD_JOBS` 更适合作为环境变量入口，而不是新增复杂 CLI 参数
- help / log / smoke 契约必须一起补
- `build-release.sh` 的串行聚合语义不应被这次性能工作改变

证据：

- `build-desktop.sh` 和 `build-release.sh` 当前除了 `--help` 不接受其他 CLI 参数：`build-desktop.sh:328`, `build-desktop.sh:333`, `build-release.sh:49`, `build-release.sh:54`
- help 契约已被多个 smoke 脚本锁定：`tests/build_desktop_script_smoke.sh:16`, `tests/build_win_x64_script_smoke.sh:17`
- release 聚合是明确串行：`build-release.sh:74`

风险提示：

- 先加 CLI 参数会扩大接口变更面和 smoke 维护成本
- 并行化 release 聚合会恶化失败归因与重现性

对其他方案的反驳：

- “让 build-release.sh 并行跑两个 target”不属于本轮最小收益/最小风险范围

### 性能 profiling 专家

主张：

- 关键问题不是 Cargo 默认没并行
- 关键问题是“前半段已并行，后半段单 `rustc` 长尾还很长”
- 第一阶段必须保留 timings + 端到端 time + 运行中进程采样

证据：

- fresh 默认并行 `7m18.9s`
- fresh `-j1` `23m47.4s`
- `Max concurrency: 34 (jobs=32 ncpu=32)`
- 进程采样早期约 30 个 `rustc`，后半段长期只剩 1 个
- timing 报告中根 crate `mica-term` 自身占据最长关键路径

风险提示：

- 只看总时长会误判；必须盯住关键路径尾巴是否缩短

对其他方案的反驳：

- “再多开几个 cargo”不会缩短已经收敛到单 crate / 单链接路径的关键尾巴

### 保守维护者 / 回归风险审查专家

主张：

- 第一阶段必须拒绝一次性改动 jobs 默认值、默认 target、压缩器、缓存层和 linker 语义
- `BUILD_JOBS` 不得硬编码 32
- 7z / sccache / linker 都更适合作为后续可选项

证据：

- Windows `.zip` 契约已被脚本、README、smoke tests 固定：`build-desktop.sh:470`, `readme.md:86`, `tests/build_win_x64_script_smoke.sh:28`
- Linux-host MSVC 有共享 shim 写路径，默认并发更危险：`build-desktop.sh:159`, `build-desktop.sh:230`, `build-desktop.sh:250`, `build-desktop.sh:288`
- 当前脚本没有任何 jobs 覆盖，所以硬编码 32 会变成未经验证的新默认：`build-desktop.sh:342`, `build-desktop.sh:421`, `build-desktop.sh:450`

风险提示：

- 高核数机器能跑 32，不代表小内存 CI、其他 host、其他 target 都应被强推 32

对其他方案的反驳：

- “自动根据 7950X3D 直接取 32 作为默认”是把单机经验写死进通用脚本

## 第二轮：交叉质询与收敛

### 议题 1：要不要在 `build-win-x64.sh` 自己加 jobs？

收敛结果：

- 否
- 共享层 `build-desktop.sh` 才是唯一正确执行点
- wrapper 只需继续负责 target / feature / flavor 语义

理由：

- 否则 Windows GNU / MSVC / Linux / macOS 会出现重复逻辑
- 后续 `build-win-x64-software.sh` 也会被迫再抄一份

### 议题 2：公开入口应该选 `BUILD_JOBS` 还是 CLI `--jobs`？

收敛结果：

- 第一阶段主公开入口选 `BUILD_JOBS`
- 底层仍映射到 Cargo 原生 `--jobs`
- `CARGO_BUILD_JOBS` 作为高级用户兼容入口保留

理由：

- 当前脚本 CLI 接口极窄，只认 `--help`
- 引入新的 wrapper CLI 参数会扩大 help / smoke 变更面
- 用环境变量更贴合仓库现有 wrapper 风格

### 议题 3：默认要不要做脚本侧 auto 策略？

收敛结果：

- 第一阶段不要
- 默认继续保持 Cargo 默认并行

理由：

- fresh profiling 已显示 Cargo 默认并行在 32 线程主机上能跑到 `Max concurrency 34`
- `-j1` 明显更慢，说明默认值不是问题根因
- 先补覆盖旋钮、日志和 profiling 指南，收益最大、风险最小

### 议题 4：linker / sccache / 7z 是否该进入第一阶段？

收敛结果：

- 不进入第一阶段默认方案
- 只记录为后续可选优化

理由：

- 当前真正问题先要证明是否是 jobs、关键路径、链接或压缩哪一段主导
- 默认启用这些能力都会增加 host 依赖、故障面或语义变更面

### 议题 5：多开多个 cargo build 进程要不要作为“吃满 CPU”方案？

收敛结果：

- 明确拒绝

理由：

- Cargo 原生并行已经存在
- 当前仓库有共享 shim / patched-registry 写路径
- 同一 `target` 目录并发构建存在锁竞争、缓存污染、无收益甚至更慢的风险

## 方案对比

## A. 只在 `build-win-x64.sh` 里加并行逻辑

优点：

- 改动面看起来最小
- 对用户点名入口最直接

缺点：

- 实际构建不在这里执行
- 会与 `build-win-x64-software.sh` / `build-desktop.sh` 形成逻辑分叉
- Windows GNU、Windows MSVC、Linux/macOS 路径都无法共享

回归风险：

- wrapper 语义重复
- 后续继续复制 jobs 解析逻辑

与当前仓库架构契合度：低

是否进入最终方案：否

## B. 在 `build-desktop.sh` 共享实现里加 `BUILD_JOBS`，wrapper 只透传

优点：

- 符合当前共享构建入口架构
- 同时覆盖 `cargo build` 和 `cargo xwin build`
- wrapper 不需要复制实现
- 与现有 thin-wrapper 语义最一致

缺点：

- 需要同步更新帮助文案与 smoke 契约
- 需要谨慎设计日志输出与非法值处理

回归风险：中低

与当前仓库架构契合度：高

是否进入最终方案：是

## C. 只使用 Cargo 原生 `--jobs` / `CARGO_BUILD_JOBS`

优点：

- 完全走 Rust 原生能力
- 概念最正统

缺点：

- 仅依赖 Cargo 原生环境变量的话，repo wrapper 无法提供清晰的用户提示与统一日志
- 直接加 wrapper CLI `--jobs` 会扩大接口变更面

回归风险：低

与当前仓库架构契合度：中高

是否进入最终方案：部分进入

最终采用方式：

- 底层机制使用 Cargo 原生 `--jobs`
- repo 公开入口采用 `BUILD_JOBS` 映射到该原生能力

## D. 并行启动多个 `cargo build` 进程

优点：

- 表面上更“吃满 CPU”

缺点：

- 与 Cargo 原生并行重复
- 同一 `target` 目录有锁竞争
- 当前仓库还有共享 `cargo-xwin` shim / patched-registry 写路径
- 无法自然解决关键路径后半段单 `rustc` 长尾

回归风险：高

与当前仓库架构契合度：低

是否进入最终方案：否，明确拒绝

## E. 优化 linker，例如 lld / mold / clang 或 target-specific linker

优点：

- 若尾巴主要卡在链接，可能显著收益
- 属于成熟方向

缺点：

- 当前 Linux-host MSVC 已有 `lld-link` 正确性 shim，进一步改 linker 默认值会扩大变更面
- `mold` 不适用于当前 Windows MSVC 主链
- 没有 profiling 证据前，不应把它当第一阶段主方案

回归风险：中高

与当前仓库架构契合度：中

是否进入最终方案：不进入第一阶段；保留为后续优化

## F. 使用 `sccache` / cache 策略

优点：

- 对重复构建、CI、多人环境很有价值
- 与 Cargo / Rust 生态契合

缺点：

- 更偏缓存层能力，而非 repo 脚本本体行为
- 需要额外安装、环境配置与 cache 生命周期管理
- 对首次 cold build 没法解决关键路径本质问题

回归风险：中

与当前仓库架构契合度：中

是否进入最终方案：不进入第一阶段；保留为后续 opt-in

## G. 优化 zip 压缩，例如 `7z -mmt`、`pigz`、`zstd`，或保持 zip 单线程

优点：

- 若压缩阶段占比高，可以较低成本提速
- 7-Zip 有成熟多线程能力

缺点：

- 当前 Windows 产物契约是 `.zip`
- 系统自带 `zip 3.0` 没有多线程契约
- 7z / pigz / zstd 不是当前脚本基础依赖
- 本地 warm run 里压缩阶段看起来不是主瓶颈

回归风险：中

与当前仓库架构契合度：中低

是否进入最终方案：不进入第一阶段；仅在 profiling 证明 zip 明显占比后再开后续任务

## 最终推荐方案

### 一阶段正式推荐

1. 在 `build-desktop.sh` 共享实现层引入 `BUILD_JOBS`
2. 当 `BUILD_JOBS` 为正整数时，向 `cargo build` 或 `cargo xwin build` 追加 `--jobs <N>`
3. 默认不设 jobs 时，保持 Cargo 默认并行
4. 在 help 与 build log 中显式打印 parallelism 使用方式
5. 用 smoke tests 锁定 help、日志和参数透传
6. 不修改现有 GNU / MSVC wrapper 语义
7. 不修改 `.zip` 产物契约
8. 不在第一阶段默认启用 linker / sccache / 7z 方案

### 关键论证

- 该方案不破坏统一 `build-desktop.sh` 管线
- 该方案使用 Cargo 原生并行能力，而不是手写危险的多 cargo 进程
- 该方案允许显式 `BUILD_JOBS=32` 或 `BUILD_JOBS=$(nproc)`
- 该方案能通过 smoke test 锁定 help / log / 参数透传
- 该方案把 linker / parallel zip / sccache 作为可选后续优化，符合 profiling 优先原则

## 对用户观察的直接解释

- Cargo 默认本身会并行编译多个 crate；本轮 fresh profiling 已直接看到 `Max concurrency 34 (jobs=32 ncpu=32)`。
- `htop` 里只看到一个长期前台进程，并不表示没有并行，因为：
  - 前台通常只有一个 `cargo` / `cargo xwin`
  - 真实并行体现在大量短生命周期 `rustc` 子进程与内部线程
  - 后半段关键路径会收敛到单 `rustc` / 单链接长尾
  - `build.rs` / Slint codegen / Windows resource embedding 本来也可能形成串行段
- 多开 cargo build 到同一个 `target` 目录并不是第一方案，因为会引入锁竞争、缓存污染和共享 shim 写风险。
- 对 7950X3D，文档可以建议 `BUILD_JOBS=32` 或 `BUILD_JOBS=$(nproc)`，但必须保留用户自行调节能力，不能硬编码。
- `release` profile、LTO、`codegen-units`、debug info、`strip`、`incremental` 在本轮只作为调研项，不在没有证据时直接改默认发布质量。

## 实施前置约束

未来正式实现必须先补 smoke tests，再补最小实现，再跑 profiling 对比。

这既符合本轮用户要求，也符合 TDD 思路：

- 先定义未来会失败的契约测试
- 再做最小实现
- 最后用 fresh profiling 证明收益与边界

## 本轮未实施项

- 没有修改任何业务代码、脚本代码、测试代码、`Cargo.toml`、`Cargo.lock` 或配置文件
- 没有引入新的构建依赖
- 没有创建 `.worktrees` 中的正式实现分支
- 没有把并行优化宣称为“已完成”

本轮交付仅是：requirements / design / tasks
