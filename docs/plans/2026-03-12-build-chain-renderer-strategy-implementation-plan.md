# Build Chain And Renderer Strategy Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不打乱现有 `Rust + Slint + winit` 壳层的前提下，落地 `Debian` 正式发布链与 `Windows MSVC pure Skia experimental` 实验链，确保正式链稳定、实验链可验证、renderer 选择真源在程序内部。

**Architecture:** 保持现有 `src/main.rs -> app::logging -> app::bootstrap` 启动主链，新增一个很薄的 `runtime profile` 层统一表达“正式包 / 实验包”身份与 renderer 策略。正式链继续沿用 `software renderer` 与现有 Windows workaround；实验链改为 `pure Skia + app-level feature + internal backend lock + explicit failure path`，同时把 Debian 正式总控脚本放在现有 `build-desktop.sh` 之上，而不是重写整个打包系统。

**Tech Stack:** Rust, Cargo features, Slint 1.15.1, `backend-winit`, `renderer-software`, `renderer-skia`, shell smoke scripts, `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-12-build-chain-renderer-strategy-design.md`，实现时不得偏离已确认决策。
- 本计划只落主方案，不实现 `docs/plans/try-winit-femtovg-wgpu.md` 中的旁路尝试。
- 每个任务都按 `@superpowers:test-driven-development` 执行：先写失败测试或 smoke，再做最小实现，再跑通过。
- 如果 Slint backend / renderer 选择行为与预期不符，不允许猜测，立即切换到 `@superpowers:systematic-debugging`。
- 计划默认在独立 worktree 执行；若继续在当前工作区执行，改动范围也必须只覆盖本计划列出的文件。
- 本轮目标是构建链和 renderer 策略，不顺手改 `terminal runtime`、`SSH/SFTP`、`WelcomeView` 结构或 Fluent 视觉细节。

### Target Snapshot

完成后应满足以下用户可见结果：

- Debian 上有一个正式总控入口，可以一次发起：
  - `Linux x64` 正式包
  - `Windows x64 GNU` 正式包
- 正式 `Linux x64` 包运行时使用 `winit-software`
- 正式 `Windows x64 GNU` 包运行时使用 `winit-software`，继续保留当前 `offscreen workaround`
- `Windows MSVC Skia Experimental` 包是 `pure Skia` 形态，不再携带默认 `software renderer`
- 实验包启动时程序内部强制 `winit-skia-software`
- 若外部设置冲突的 `SLINT_BACKEND`，实验包会忽略并写日志
- 若实验包无法成功初始化 `winit-skia-software`，会明确提示后退出，不会静默降级
- 日志、诊断或窗口标识可以清楚区分正式包与 `Skia Experimental`

### Out of Scope

- `wezterm-term` / `termwiz` 接入
- `russh` / `SFTP` 连接逻辑
- `winit-femtovg-wgpu` 真正实现
- 删除当前 Windows `offscreen workaround`
- CI 工作流或 release pipeline 平台接入

## Task 1: 建立 runtime profile 与 experimental feature 语义

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/runtime_profile.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/lib.rs`
- Create: `tests/runtime_profile.rs`
- Modify: `tests/build_win_x64_skia_script_smoke.sh`

**Step 1: Write the failing tests**

创建 `tests/runtime_profile.rs`，定义“正式包 / 实验包”的纯 Rust 契约：

```rust
use mica_term::app::runtime_profile::{
    AppBuildFlavor, AppRuntimeProfile, RendererMode,
};

#[test]
fn formal_profile_defaults_to_software_renderer() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Formal);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert!(!profile.is_experimental());
}

#[test]
fn skia_experimental_profile_is_pure_skia() {
    let profile = AppRuntimeProfile::skia_experimental();

    assert_eq!(profile.build_flavor, AppBuildFlavor::SkiaExperimental);
    assert_eq!(profile.renderer_mode, RendererMode::SkiaSoftware);
    assert!(profile.is_experimental());
    assert!(profile.requires_backend_lock());
}
```

修改 `tests/build_win_x64_skia_script_smoke.sh`，补“纯 Skia 形态”契约：

```bash
grep -F 'CARGO_NO_DEFAULT_FEATURES' "$SCRIPT_PATH" >/dev/null
grep -F 'windows-skia-experimental' "$SCRIPT_PATH" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test runtime_profile -q`  
Expected: FAIL with unresolved imports such as `runtime_profile`, `AppBuildFlavor`, or `RendererMode`.

Run: `bash tests/build_win_x64_skia_script_smoke.sh`  
Expected: FAIL because the script does not yet assert a pure-Skia build shape.

**Step 3: Write minimal implementation**

创建 `src/app/runtime_profile.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppBuildFlavor {
    Formal,
    SkiaExperimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererMode {
    Software,
    SkiaSoftware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppRuntimeProfile {
    pub build_flavor: AppBuildFlavor,
    pub renderer_mode: RendererMode,
}

impl AppRuntimeProfile {
    pub fn formal() -> Self {
        Self {
            build_flavor: AppBuildFlavor::Formal,
            renderer_mode: RendererMode::Software,
        }
    }

    pub fn skia_experimental() -> Self {
        Self {
            build_flavor: AppBuildFlavor::SkiaExperimental,
            renderer_mode: RendererMode::SkiaSoftware,
        }
    }

    pub fn is_experimental(self) -> bool {
        matches!(self.build_flavor, AppBuildFlavor::SkiaExperimental)
    }

    pub fn requires_backend_lock(self) -> bool {
        self.is_experimental()
    }
}
```

修改 `src/app/mod.rs`：

```rust
pub mod bootstrap;
pub mod logging;
pub mod runtime_profile;
pub mod ui_preferences;
pub mod window_effects;
pub mod windowing;
```

修改 `Cargo.toml`，把 renderer feature 与 app-level experimental feature 明确拆开：

```toml
[features]
default = ["slint-renderer-software"]
slint-renderer-software = ["slint/renderer-software"]
slint-renderer-skia = ["slint/renderer-skia"]
windows-skia-experimental = ["slint-renderer-skia"]
```

如果后续实现阶段需要避免 feature 语义冲突，可把 `windows-skia-experimental` 调整为 “app-level experimental + renderer-skia” 的组合 feature，但第一步先建立清晰命名。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test runtime_profile -q`  
Expected: PASS，说明 runtime profile 语义已经稳定。

Run: `bash tests/build_win_x64_skia_script_smoke.sh`  
Expected: PASS 或转入下一步最小脚本调整所需的明确失败项。

**Step 5: Commit**

```bash
git add Cargo.toml src/app/runtime_profile.rs src/app/mod.rs tests/runtime_profile.rs \
  tests/build_win_x64_skia_script_smoke.sh
git commit -m "feat: define build profile and experimental feature model"
```

## Task 2: 让 experimental 包在程序内部真锁定 `winit-skia-software`

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/runtime_profile.rs`
- Create: `tests/bootstrap_profile_smoke.rs`
- Modify: `tests/window_theme_contract_smoke.sh`

**Step 1: Write the failing tests**

创建 `tests/bootstrap_profile_smoke.rs`，验证 renderer 决策行为：

```rust
use mica_term::app::runtime_profile::{AppRuntimeProfile, RendererMode};

#[test]
fn formal_profile_does_not_require_backend_lock() {
    let profile = AppRuntimeProfile::formal();

    assert!(!profile.requires_backend_lock());
}

#[test]
fn skia_experimental_profile_requires_winit_skia_software() {
    let profile = AppRuntimeProfile::skia_experimental();

    assert!(profile.requires_backend_lock());
    assert_eq!(profile.renderer_mode, RendererMode::SkiaSoftware);
}
```

修改 `tests/window_theme_contract_smoke.sh`，检查实验逻辑没有通过 `target_os = "windows"` 粗暴分叉 renderer：

```bash
grep -F 'winit-skia-software' "$ROOT_DIR/src/main.rs" >/dev/null
grep -F 'SLINT_BACKEND' "$ROOT_DIR/src/main.rs" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test bootstrap_profile_smoke -q`  
Expected: FAIL because startup profile/backends are not yet expressed in code.

Run: `bash tests/window_theme_contract_smoke.sh`  
Expected: FAIL because `src/main.rs` does not yet contain backend-lock logic.

**Step 3: Write minimal implementation**

在 `src/app/runtime_profile.rs` 中增加启动决策辅助：

```rust
impl AppRuntimeProfile {
    pub fn forced_backend(self) -> Option<&'static str> {
        match self.renderer_mode {
            RendererMode::Software => None,
            RendererMode::SkiaSoftware => Some("winit-skia-software"),
        }
    }
}
```

修改 `src/main.rs`，在 logging 初始化和 `bootstrap::run()` 之前插入 backend 真锁定：

```rust
fn select_runtime_profile() -> mica_term::app::runtime_profile::AppRuntimeProfile {
    #[cfg(feature = "windows-skia-experimental")]
    {
        return mica_term::app::runtime_profile::AppRuntimeProfile::skia_experimental();
    }

    #[cfg(not(feature = "windows-skia-experimental"))]
    {
        mica_term::app::runtime_profile::AppRuntimeProfile::formal()
    }
}

fn apply_renderer_lock(profile: mica_term::app::runtime_profile::AppRuntimeProfile) {
    if let Some(backend) = profile.forced_backend() {
        if std::env::var("SLINT_BACKEND").ok().as_deref() != Some(backend) {
            tracing::warn!(
                target: "app.renderer",
                requested = ?std::env::var("SLINT_BACKEND").ok(),
                forced = backend,
                "overriding conflicting SLINT_BACKEND for experimental profile"
            );
        }
        std::env::set_var("SLINT_BACKEND", backend);
    }
}
```

如果实现阶段需要避免直接依赖环境变量副作用，也可以在这里改用 `slint::BackendSelector` 作为内部真源，但要求保持“程序内部决定 renderer”这一原则不变。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test runtime_profile --test bootstrap_profile_smoke -q`  
Expected: PASS，profile 和 forced backend 决策闭环。

Run: `bash tests/window_theme_contract_smoke.sh`  
Expected: PASS，能看到 experimental backend lock 的源码契约。

**Step 5: Commit**

```bash
git add src/main.rs src/app/runtime_profile.rs src/app/bootstrap.rs \
  tests/bootstrap_profile_smoke.rs tests/window_theme_contract_smoke.sh
git commit -m "feat: lock experimental builds to winit-skia-software"
```

## Task 3: 补实验包失败提示、诊断标记与日志可观测性

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app/logging/config.rs`
- Modify: `src/app/logging/runtime.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/logging_runtime.rs`
- Modify: `tests/panic_logging.rs`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing tests**

修改 `tests/logging_runtime.rs`，验证 runtime metadata 可被记录：

```rust
use mica_term::app::runtime_profile::AppRuntimeProfile;

#[test]
fn debug_logging_can_emit_runtime_profile_metadata() {
    let profile = AppRuntimeProfile::skia_experimental();
    assert!(profile.is_experimental());
}
```

修改 `tests/top_status_bar_smoke.rs`，为窗口或诊断标识补一个轻量契约：

```rust
#[test]
fn app_title_can_be_extended_for_experimental_profile() {
    assert_eq!(mica_term::app::bootstrap::app_title(), "Mica Term");
}
```

如果实现阶段决定把 experimental 标记落在窗口标题衍生函数而不是静态标题本身，则测试应同步改成该衍生函数契约。

**Step 2: Run tests to verify they fail**

Run: `cargo test --test logging_runtime --test top_status_bar_smoke -q`  
Expected: FAIL because renderer/profile diagnostics and failure-path metadata are not yet exposed.

**Step 3: Write minimal implementation**

在 `src/main.rs` 中，为 experimental 启动失败加明确提示：

```rust
if let Err(err) = mica_term::app::bootstrap::run() {
    if profile.is_experimental() {
        eprintln!(
            "Mica Term Skia Experimental failed to initialize winit-skia-software: {err}"
        );
    }
    // keep existing fatal logging path
}
```

在 `src/app/logging/runtime.rs` 或 `src/app/bootstrap.rs` 中追加启动诊断：

```rust
tracing::info!(
    target: "app.renderer",
    build_flavor = ?profile.build_flavor,
    renderer_mode = ?profile.renderer_mode,
    forced_backend = ?profile.forced_backend(),
    "initialized runtime profile"
);
```

如果需要 UI 可见标记，优先新增一个衍生函数，例如：

```rust
pub fn runtime_window_title(profile: AppRuntimeProfile) -> String {
    if profile.is_experimental() {
        "Mica Term [Skia Experimental]".into()
    } else {
        "Mica Term".into()
    }
}
```

不要在这一任务里修改 Slint 结构；仅通过 Rust 侧标题或日志暴露标记即可。

**Step 4: Run tests to verify they pass**

Run: `cargo test --test logging_runtime --test panic_logging --test top_status_bar_smoke -q`  
Expected: PASS，且 experimental 失败提示与 runtime diagnostics 路径可见。

**Step 5: Commit**

```bash
git add src/main.rs src/app/logging/config.rs src/app/logging/runtime.rs src/app/bootstrap.rs \
  tests/logging_runtime.rs tests/panic_logging.rs tests/top_status_bar_smoke.rs
git commit -m "feat: add experimental renderer diagnostics and failure path"
```

## Task 4: 新增 Debian 正式总控脚本并收敛 build 契约

**Files:**
- Create: `build-release.sh`
- Modify: `build-desktop.sh`
- Modify: `build-win-x64.sh`
- Modify: `build-win-x64-skia.sh`
- Modify: `readme.md`
- Create: `tests/build_release_script_smoke.sh`
- Modify: `tests/build_desktop_script_smoke.sh`
- Modify: `tests/build_win_x64_script_smoke.sh`
- Modify: `tests/build_win_x64_skia_script_smoke.sh`

**Step 1: Write the failing smoke tests**

创建 `tests/build_release_script_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-release.sh"

[[ -f "$SCRIPT_PATH" ]]
bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"
grep -F 'fail-fast' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'best-effort' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-unknown-linux-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'x86_64-pc-windows-gnu' <<<"$HELP_OUTPUT" >/dev/null
```

修改 `tests/build_win_x64_skia_script_smoke.sh`，要求实验脚本体现 pure-Skia 语义：

```bash
grep -F 'CARGO_NO_DEFAULT_FEATURES="${CARGO_NO_DEFAULT_FEATURES:-1}"' "$SCRIPT_PATH" >/dev/null
grep -F 'TARGET="${TARGET:-x86_64-pc-windows-msvc}"' "$SCRIPT_PATH" >/dev/null
```

**Step 2: Run smoke tests to verify they fail**

Run: `bash tests/build_release_script_smoke.sh`  
Expected: FAIL because the release aggregator script does not exist yet.

Run: `bash tests/build_win_x64_skia_script_smoke.sh`  
Expected: FAIL because the Skia wrapper does not yet enforce pure-Skia defaults.

**Step 3: Write minimal implementation**

创建 `build-release.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODE="${MODE:-fail-fast}"

usage() {
  cat <<EOF
Usage: $(basename "$0") [--help]

Formal Debian release aggregator.

Modes:
  MODE=fail-fast   Stop on first failure (default)
  MODE=best-effort Continue both targets and report summary
EOF
}
```

然后把两条正式链明确写死为：

```bash
formal_targets=(
  "x86_64-unknown-linux-gnu"
  "x86_64-pc-windows-gnu"
)
```

修改 `build-win-x64-skia.sh`，让它默认成为 pure-Skia 包：

```bash
export TARGET="${TARGET:-x86_64-pc-windows-msvc}"
export CARGO_NO_DEFAULT_FEATURES="${CARGO_NO_DEFAULT_FEATURES:-1}"
export CARGO_FEATURES="${CARGO_FEATURES:-windows-skia-experimental}"
```

修改 `readme.md`，把链路分成：

- Formal Release
- Windows Skia Experimental
- Try / future renderer exploration

不要把 `winit-femtovg-wgpu` 提升为当前默认方案。

**Step 4: Run smoke tests to verify they pass**

Run: `bash tests/build_desktop_script_smoke.sh`  
Expected: PASS

Run: `bash tests/build_win_x64_script_smoke.sh`  
Expected: PASS

Run: `bash tests/build_win_x64_skia_script_smoke.sh`  
Expected: PASS，证明实验脚本已经是 pure-Skia 入口。

Run: `bash tests/build_release_script_smoke.sh`  
Expected: PASS，证明 Debian 总控入口存在且具备双模式说明。

**Step 5: Commit**

```bash
git add build-release.sh build-desktop.sh build-win-x64.sh build-win-x64-skia.sh readme.md \
  tests/build_release_script_smoke.sh tests/build_desktop_script_smoke.sh \
  tests/build_win_x64_script_smoke.sh tests/build_win_x64_skia_script_smoke.sh
git commit -m "feat: add formal release aggregator and pure skia wrapper"
```

## Task 5: 收口验证矩阵、文档与回滚说明

**Files:**
- Modify: `verification.md`
- Modify: `readme.md`
- Modify: `docs/plans/2026-03-12-build-chain-renderer-strategy-design.md`
- Reference: `docs/plans/try-winit-femtovg-wgpu.md`

**Step 1: Write the failing verification checklist**

先在 `verification.md` 中创建新的占位章节：

```md
## Build Chain And Renderer Strategy Verification

- [ ] formal linux build path documented
- [ ] formal windows gnu build path documented
- [ ] skia experimental path documented
- [ ] experimental backend conflict override documented
- [ ] explicit skia init failure path documented
```

如果实现阶段更适合把这部分放到新验证文档，也可以调整，但必须有独立验证矩阵。

**Step 2: Run documentation smoke**

Run: `rg -n "Skia Experimental|build-release.sh|best-effort|winit-skia-software" readme.md verification.md docs/plans/2026-03-12-build-chain-renderer-strategy-design.md`  
Expected: FAIL before文档收口，因为这些关键词还没有完整落位。

**Step 3: Write minimal implementation**

在 `verification.md` 中记录：

- Formal Linux build command
- Formal Windows GNU build command
- Experimental Windows MSVC Skia build command
- 关键 smoke/test 命令
- 风险、回滚、失败观察项

在 `readme.md` 中确保构建矩阵读者可直接理解：

- 哪个是正式链
- 哪个是实验链
- 哪个是 try 文档

在 design 文档中只补必要的“implementation plan 已生成”引用，不要重写已确认决策。

**Step 4: Run final verification**

Run:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace -q
cargo clippy --workspace --all-targets -- -D warnings
bash tests/build_desktop_script_smoke.sh
bash tests/build_win_x64_script_smoke.sh
bash tests/build_win_x64_skia_script_smoke.sh
bash tests/build_release_script_smoke.sh
bash tests/window_theme_contract_smoke.sh
```

Expected:

- 所有 Rust 测试通过
- 所有 smoke 通过
- 正式链与实验链文档边界清晰
- `try-winit-femtovg-wgpu.md` 仍然只作为旁路探索存在

**Step 5: Commit**

```bash
git add verification.md readme.md docs/plans/2026-03-12-build-chain-renderer-strategy-design.md \
  docs/plans/2026-03-12-build-chain-renderer-strategy-implementation-plan.md
git commit -m "docs: verify build chain and renderer strategy"
```

## Recommended Execution Order

1. Task 1
2. Task 2
3. Task 3
4. Task 4
5. Task 5

Task 4 can be developed in parallel with Tasks 2-3 only after Task 1 stabilizes the feature/profile semantics.

## Verification Gates

- Gate 1: `runtime_profile` tests pass before any startup-path refactor
- Gate 2: experimental backend lock is in place before adding failure-path UX
- Gate 3: pure-Skia wrapper script and Debian release aggregator smoke pass before README finalization
- Gate 4: full workspace checks pass before claiming the strategy is implemented

## Rollback Guidance

- If experimental backend locking proves unstable, keep `runtime_profile` scaffolding but stop building `Windows MSVC Skia Experimental` wrappers by default.
- If pure-Skia script semantics break downstream assumptions, revert `build-win-x64-skia.sh` to mixed-renderer behavior temporarily, but do not change the formal Debian release path.
- If Debian total-control script introduces maintenance friction, keep the wrapper but mark it internal-only; do not remove the existing per-target scripts.

## Handoff Notes

- Do not delete the existing Windows `offscreen workaround` during this plan.
- Do not promote `winit-femtovg-wgpu` beyond the try document during this plan.
- Keep all renderer decisions observable through logs or diagnostics so future Skia A/B work has trustworthy evidence.
