# Mica Term FemtoVG WGPU Experimental Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不改动正式发布链语义的前提下，为当前 `mica-term` 主应用增加一条严格独立的 `pure winit-femtovg-wgpu experimental` 构建路径，并覆盖 `Linux x64` 与 `Windows x64 MSVC` 两个平台。

**Architecture:** 保持现有 `src/main.rs -> app::logging -> app::bootstrap` 启动主链不变，只新增一层很薄的 `runtime profile + internal selector` 语义。正式包继续使用默认 `software` 路线；实验包改为 `app-level feature + Slint BackendSelector + explicit experimental identity + wrapper-only packaging`，绝不引入 `SLINT_BACKEND`、`winit-software` 或任何 fallback/workaround 泄漏。

**Tech Stack:** Rust, Cargo features, Slint 1.15.1, `backend-winit`, `renderer-software`, `renderer-femtovg-wgpu`, `unstable-wgpu-28`, Bash smoke scripts, Markdown docs

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-12-femtovg-wgpu-experimental-design.md`，实现时不得偏离已确认决策。
- 参考资料 `docs/plans/try-winit-femtovg-wgpu.md` 只作为背景，不得把 try 文档重新扩展成另一条默认路线。
- 每个任务都按 `@superpowers:test-driven-development` 执行：先写失败测试或 smoke，再做最小实现，再跑通过。
- 如果 Slint `BackendSelector`、`renderer-femtovg-wgpu` 或 `wgpu-28` API 与预期不符，不允许猜测，立即切换到 `@superpowers:systematic-debugging`。
- 本计划只增加实验链，不新建第二个 app binary；打包产物名可以带 experimental 后缀，但可执行文件名必须继续保持 `mica-term`。
- `build-release.sh` 必须继续只服务正式链；实验链只通过显式 wrapper 进入。

### Target Snapshot

完成后应满足以下用户可见结果：

- 默认正式链仍然是：
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-gnu`
- 新增实验链仅覆盖：
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-msvc`
- 实验链构建形态固定为：
  - `--no-default-features`
  - `--features femtovg-wgpu-experimental`
  - `backend = winit`
  - `renderer = femtovg-wgpu`
  - `require_wgpu_28(WGPUConfiguration::default())`
- 实验包启动失败时必须明确报错退出，不允许回退到 `software`。
- 实验包窗口标题、失败文案、日志元数据、stage dir、archive name 都要带明显 experimental 身份。

### Out Of Scope

- `wezterm-term` / `termwiz` 真实 terminal surface 接入
- `russh` / `SFTP` 连接逻辑
- `Tokio runtime` 重构
- macOS / Linux ARM64 / Android / iOS 实验扩展
- 将 `femtovg-wgpu` 提升为正式默认 renderer
- 新的 installer、签名、CI workflow、release pipeline 集成

## Task 1: 建立 `femtovg-wgpu` feature 拓扑与 runtime profile 语义

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/runtime_profile.rs`
- Modify: `tests/runtime_profile.rs`

**Step 1: Write the failing tests**

更新 `tests/runtime_profile.rs`，先把 formal / experimental 两条 profile 语义钉死：

```rust
use mica_term::app::runtime_profile::{
    AppBuildFlavor, AppRuntimeProfile, RendererMode,
};

#[test]
fn formal_profile_stays_on_software_renderer() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Formal);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert!(!profile.is_experimental());
    assert_eq!(profile.forced_backend(), None);
    assert_eq!(profile.forced_renderer(), None);
    assert!(!profile.requires_wgpu_28());
}

#[test]
fn femtovg_wgpu_experimental_profile_is_gpu_only() {
    let profile = AppRuntimeProfile::femtovg_wgpu_experimental();

    assert_eq!(
        profile.build_flavor,
        AppBuildFlavor::FemtoVgWgpuExperimental
    );
    assert_eq!(profile.renderer_mode, RendererMode::FemtoVgWgpu);
    assert!(profile.is_experimental());
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("femtovg-wgpu"));
    assert!(profile.requires_wgpu_28());
}
```

保留现有“Skia 已移除”的 source contract，但额外补充一条负向断言，禁止重新引入旧试验词汇：

```rust
assert!(!content.contains("SkiaExperimental"));
assert!(!content.contains("winit-skia-software"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test runtime_profile -q
```

Expected: FAIL，因为 `FemtoVgWgpuExperimental`、`FemtoVgWgpu`、`femtovg_wgpu_experimental()`、`forced_renderer()`、`requires_wgpu_28()` 还不存在。

**Step 3: Write the minimal implementation**

在 `Cargo.toml` 中建立分层 feature：

```toml
[features]
default = ["slint-renderer-software"]
slint-renderer-software = ["slint/renderer-software"]
slint-renderer-femtovg-wgpu = [
  "slint/renderer-femtovg-wgpu",
  "slint/unstable-wgpu-28",
]
femtovg-wgpu-experimental = ["slint-renderer-femtovg-wgpu"]
```

在 `src/app/runtime_profile.rs` 中扩展 profile 枚举与辅助方法：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppBuildFlavor {
    Formal,
    FemtoVgWgpuExperimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererMode {
    Software,
    FemtoVgWgpu,
}

impl AppRuntimeProfile {
    pub fn femtovg_wgpu_experimental() -> Self {
        Self {
            build_flavor: AppBuildFlavor::FemtoVgWgpuExperimental,
            renderer_mode: RendererMode::FemtoVgWgpu,
        }
    }

    pub fn is_experimental(self) -> bool {
        matches!(self.build_flavor, AppBuildFlavor::FemtoVgWgpuExperimental)
    }

    pub fn forced_backend(self) -> Option<&'static str> {
        match self.renderer_mode {
            RendererMode::Software => None,
            RendererMode::FemtoVgWgpu => Some("winit"),
        }
    }

    pub fn forced_renderer(self) -> Option<&'static str> {
        match self.renderer_mode {
            RendererMode::Software => None,
            RendererMode::FemtoVgWgpu => Some("femtovg-wgpu"),
        }
    }

    pub fn requires_wgpu_28(self) -> bool {
        matches!(self.renderer_mode, RendererMode::FemtoVgWgpu)
    }
}
```

不要在这一任务里引入 target 白名单判断，也不要把 profile 绑定到脚本环境变量；这里只负责把“身份”和“renderer 语义”表达清楚。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test runtime_profile -q
```

Expected: PASS，formal / experimental 的纯 Rust 语义稳定。

**Step 5: Commit**

```bash
git add Cargo.toml src/app/runtime_profile.rs tests/runtime_profile.rs
git commit -m "feat: define femtovg wgpu experimental profile"
```

## Task 2: 在主入口加入严格的 `BackendSelector` 锁定路径

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`
- Create: `tests/femtovg_wgpu_contract_smoke.sh`

**Step 1: Write the failing tests**

更新 `tests/bootstrap_profile_smoke.rs`，把实验 profile 的启动契约从 runtime 层再钉一次：

```rust
use mica_term::app::runtime_profile::AppRuntimeProfile;

#[test]
fn formal_profile_does_not_request_selector_lock() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.forced_backend(), None);
    assert_eq!(profile.forced_renderer(), None);
    assert!(!profile.requires_wgpu_28());
}

#[test]
fn experimental_profile_requests_internal_selector_lock() {
    let profile = AppRuntimeProfile::femtovg_wgpu_experimental();

    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("femtovg-wgpu"));
    assert!(profile.requires_wgpu_28());
}
```

创建 `tests/femtovg_wgpu_contract_smoke.sh`，用源码契约锁住“程序内部 selector、不是环境变量”这一点：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN_FILE="$ROOT_DIR/src/main.rs"

grep -F 'BackendSelector::new' "$MAIN_FILE" >/dev/null
grep -F 'backend_name("winit".into())' "$MAIN_FILE" >/dev/null
grep -F 'renderer_name("femtovg-wgpu".into())' "$MAIN_FILE" >/dev/null
grep -F 'require_wgpu_28' "$MAIN_FILE" >/dev/null
grep -F 'femtovg-wgpu-experimental' "$MAIN_FILE" >/dev/null

if rg -n 'SLINT_BACKEND|set_var\\("SLINT_BACKEND"' "$MAIN_FILE" >/dev/null; then
  echo "unexpected SLINT_BACKEND override path remains in $MAIN_FILE" >&2
  exit 1
fi
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_profile_smoke -q
bash tests/femtovg_wgpu_contract_smoke.sh
```

Expected:

- `bootstrap_profile_smoke` 先因为新的 profile API 尚未在启动主链中落地而只完成一半
- `femtovg_wgpu_contract_smoke.sh` 直接 FAIL，因为 `src/main.rs` 里还没有 `BackendSelector`

**Step 3: Write the minimal implementation**

在 `src/main.rs` 中增加 feature-gated profile 选择与 selector 入口。先把 profile 选择封装成一个小函数：

```rust
fn select_runtime_profile() -> AppRuntimeProfile {
    #[cfg(feature = "femtovg-wgpu-experimental")]
    {
        return AppRuntimeProfile::femtovg_wgpu_experimental();
    }

    #[cfg(not(feature = "femtovg-wgpu-experimental"))]
    {
        AppRuntimeProfile::formal()
    }
}
```

再加一个只负责 experimental selector 的小函数，避免把 `main()` 弄脏：

```rust
#[cfg(feature = "femtovg-wgpu-experimental")]
fn apply_renderer_selector(profile: AppRuntimeProfile) -> anyhow::Result<()> {
    use anyhow::Context;
    use slint::{BackendSelector, WGPUConfiguration};

    if !profile.is_experimental() {
        return Ok(());
    }

    BackendSelector::new()
        .backend_name(profile.forced_backend().unwrap().into())
        .renderer_name(profile.forced_renderer().unwrap().into())
        .require_wgpu_28(WGPUConfiguration::default())
        .select()
        .map_err(anyhow::Error::from)
        .context("failed to select winit-femtovg-wgpu backend")
}

#[cfg(not(feature = "femtovg-wgpu-experimental"))]
fn apply_renderer_selector(_profile: AppRuntimeProfile) -> anyhow::Result<()> {
    Ok(())
}
```

然后把 `main()` 入口改成：

```rust
let profile = select_runtime_profile();
let logging = ...;
mica_term::app::logging::runtime::emit_runtime_profile_metadata(profile);
apply_renderer_selector(profile)?;
mica_term::app::bootstrap::run_with_profile(profile)?;
```

关键约束：

- `apply_renderer_selector(profile)` 必须发生在任何 `AppWindow::new()` 之前
- 不能写 `std::env::set_var("SLINT_BACKEND", ...)`
- 不能保留 `software` fallback 分支

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_profile_smoke -q
bash tests/femtovg_wgpu_contract_smoke.sh
cargo check --no-default-features --features femtovg-wgpu-experimental -q
```

Expected:

- 两个测试均 PASS
- experimental feature 至少在当前 host 上能编译通过

**Step 5: Commit**

```bash
git add src/main.rs tests/bootstrap_profile_smoke.rs tests/femtovg_wgpu_contract_smoke.sh
git commit -m "feat: lock experimental builds to winit femtovg wgpu"
```

## Task 3: 暴露实验包标题、失败文案与日志身份

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/logging/runtime.rs`
- Modify: `ui/app-window.slint`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `tests/panic_logging.rs`
- Modify: `tests/logging_runtime.rs`

**Step 1: Write the failing tests**

更新 `tests/top_status_bar_smoke.rs`，钉住标题和 Slint title binding：

```rust
#[test]
fn runtime_window_title_marks_femtovg_wgpu_experimental_build() {
    assert_eq!(
        runtime_window_title(AppRuntimeProfile::femtovg_wgpu_experimental()),
        "Mica Term [FemtoVG WGPU Experimental]"
    );
}

#[test]
fn app_window_title_is_runtime_bound() {
    let content = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(content.contains("in property <string> window-title"));
    assert!(content.contains("title: root.window-title;"));
}
```

更新 `tests/panic_logging.rs`，补实验失败文案契约：

```rust
#[test]
fn startup_failure_message_is_explicit_for_femtovg_wgpu_profile() {
    let message = startup_failure_message(
        AppRuntimeProfile::femtovg_wgpu_experimental(),
        "mock init failure",
    )
    .expect("experimental profile should expose a startup message");

    assert!(message.contains("FemtoVG WGPU Experimental"));
    assert!(message.contains("winit-femtovg-wgpu"));
    assert!(message.contains("mock init failure"));
}
```

更新 `tests/logging_runtime.rs`，要求 runtime metadata 清楚记录 experimental 身份：

```rust
#[test]
fn debug_logging_can_emit_femtovg_wgpu_runtime_profile_metadata() {
    // keep existing temp-dir logging harness
    emit_runtime_profile_metadata(AppRuntimeProfile::femtovg_wgpu_experimental());
    // assert log contains:
    // - FemtoVgWgpuExperimental
    // - FemtoVgWgpu
    // - Some("winit")
    // - Some("femtovg-wgpu")
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test top_status_bar_smoke --test panic_logging --test logging_runtime -q
```

Expected: FAIL，因为当前标题仍是静态 `"Mica Term"`，失败文案仍为空，日志也还没有记录 `forced_renderer`。

**Step 3: Write the minimal implementation**

在 `ui/app-window.slint` 中把静态 title 改成 runtime-bound property：

```slint
in property <string> window-title: "Mica Term";
title: root.window-title;
```

在 `src/app/bootstrap.rs` 中把 experimental 标识集中在已有 helper 上：

```rust
pub fn runtime_window_title(profile: AppRuntimeProfile) -> String {
    if profile.is_experimental() {
        "Mica Term [FemtoVG WGPU Experimental]".into()
    } else {
        app_title().to_owned()
    }
}

pub fn startup_failure_message(profile: AppRuntimeProfile, err: &str) -> Option<String> {
    if profile.is_experimental() {
        Some(format!(
            "Mica Term FemtoVG WGPU Experimental failed to initialize winit-femtovg-wgpu: {err}"
        ))
    } else {
        None
    }
}
```

并在 `run_with_profile(profile)` 中真正把标题写回 UI：

```rust
let window = AppWindow::new()?;
window.set_window_title(runtime_window_title(profile).into());
bind_top_status_bar_with_profile(&window, profile);
window.run()?;
```

在 `src/app/logging/runtime.rs` 中把 runtime metadata 扩展为：

```rust
tracing::info!(
    target: "app.renderer",
    build_flavor = ?profile.build_flavor,
    renderer_mode = ?profile.renderer_mode,
    forced_backend = ?profile.forced_backend(),
    forced_renderer = ?profile.forced_renderer(),
    wgpu_28_required = profile.requires_wgpu_28(),
    "initialized runtime profile"
);
```

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test top_status_bar_smoke --test panic_logging --test logging_runtime -q
```

Expected: PASS，experimental 包的窗口标题、失败提示和日志诊断都可见。

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/logging/runtime.rs ui/app-window.slint \
  tests/top_status_bar_smoke.rs tests/panic_logging.rs tests/logging_runtime.rs
git commit -m "feat: add femtovg wgpu experimental runtime identity"
```

## Task 4: 新增 Linux / Windows experimental wrapper，并保持正式总控纯净

**Files:**
- Create: `build-linux-x64-femtovg-wgpu.sh`
- Create: `build-win-x64-femtovg-wgpu.sh`
- Create: `tests/build_linux_x64_femtovg_wgpu_script_smoke.sh`
- Create: `tests/build_win_x64_femtovg_wgpu_script_smoke.sh`
- Modify: `tests/build_release_script_smoke.sh`

**Step 1: Write the failing smoke tests**

创建 `tests/build_linux_x64_femtovg_wgpu_script_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-linux-x64-femtovg-wgpu.sh"

[[ -f "$SCRIPT_PATH" ]] || {
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
}

bash -n "$SCRIPT_PATH"
HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F 'x86_64-unknown-linux-gnu' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'mica-term-femtovg-wgpu-experimental' <<<"$HELP_OUTPUT" >/dev/null
grep -F '--no-default-features' <<<"$HELP_OUTPUT" >/dev/null
grep -F '.tar.gz' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'femtovg-wgpu-experimental' "$SCRIPT_PATH" >/dev/null
```

创建 `tests/build_win_x64_femtovg_wgpu_script_smoke.sh`，除 target / archive 外与 Linux 脚本对齐：

```bash
grep -F 'x86_64-pc-windows-msvc' <<<"$HELP_OUTPUT" >/dev/null
grep -F '.zip' <<<"$HELP_OUTPUT" >/dev/null
grep -F 'femtovg-wgpu-experimental' "$SCRIPT_PATH" >/dev/null
```

修改 `tests/build_release_script_smoke.sh`，增加负向断言，保证正式总控不混入实验入口：

```bash
if rg -n 'femtovg-wgpu-experimental|build-linux-x64-femtovg-wgpu|build-win-x64-femtovg-wgpu|x86_64-pc-windows-msvc' "$SCRIPT_PATH" >/dev/null; then
  echo "formal release script must not expose femtovg-wgpu experimental entrypoints" >&2
  exit 1
fi
```

**Step 2: Run smoke tests to verify they fail**

Run:

```bash
bash tests/build_linux_x64_femtovg_wgpu_script_smoke.sh
bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh
bash tests/build_release_script_smoke.sh
```

Expected:

- 前两个 smoke 因 wrapper 文件缺失而 FAIL
- `build_release_script_smoke.sh` 因负向契约还没补而 FAIL 或保持待完善状态

**Step 3: Write the minimal implementation**

创建 Linux wrapper：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: ./build-linux-x64-femtovg-wgpu.sh [--help]

Experimental target:
  x86_64-unknown-linux-gnu

Cargo shape:
  --no-default-features
  --features femtovg-wgpu-experimental

Output:
  dist/mica-term-femtovg-wgpu-experimental-x86_64-unknown-linux-gnu-release.tar.gz
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

export TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
export APP_NAME="${APP_NAME:-mica-term-femtovg-wgpu-experimental}"
export BIN_NAME="${BIN_NAME:-mica-term}"
export CARGO_NO_DEFAULT_FEATURES="${CARGO_NO_DEFAULT_FEATURES:-1}"
export CARGO_FEATURES="${CARGO_FEATURES:-femtovg-wgpu-experimental}"
exec "$ROOT_DIR/build-desktop.sh" "$@"
```

创建 Windows wrapper，唯一差别是：

```bash
export TARGET="${TARGET:-x86_64-pc-windows-msvc}"
```

并且 help 输出必须写清：

- 只能在 Windows MSVC / Git Bash 环境使用
- 输出为 `dist/mica-term-femtovg-wgpu-experimental-x86_64-pc-windows-msvc-release.zip`

此任务不要改 `build-release.sh` 的实现逻辑，只允许通过 smoke 测试把它锁为“正式链纯净”。

**Step 4: Run smoke tests to verify they pass**

Run:

```bash
bash tests/build_linux_x64_femtovg_wgpu_script_smoke.sh
bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh
bash tests/build_desktop_script_smoke.sh
bash tests/build_win_x64_script_smoke.sh
bash tests/build_release_script_smoke.sh
```

Expected: PASS，wrapper 正常存在且正式脚本未被污染。

**Step 5: Commit**

```bash
git add build-linux-x64-femtovg-wgpu.sh build-win-x64-femtovg-wgpu.sh \
  tests/build_linux_x64_femtovg_wgpu_script_smoke.sh \
  tests/build_win_x64_femtovg_wgpu_script_smoke.sh \
  tests/build_release_script_smoke.sh
git commit -m "build: add femtovg wgpu experimental wrappers"
```

## Task 5: 收口 README、verification 报告与最终验证矩阵

**Files:**
- Modify: `readme.md`
- Modify: `verification.md`
- Modify: `docs/plans/2026-03-12-femtovg-wgpu-experimental-implementation-plan.md`
- Reference: `docs/plans/2026-03-12-femtovg-wgpu-experimental-design.md`

**Step 1: Write the failing documentation expectation**

先用最轻量的 grep 验证 README 还没有完整记录实验链：

```bash
rg -n 'build-linux-x64-femtovg-wgpu.sh|build-win-x64-femtovg-wgpu.sh|femtovg-wgpu-experimental|x86_64-pc-windows-msvc' readme.md
```

Expected: FAIL 或结果不完整，因为 README 还没有把 experimental wrapper、目标边界和产物命名写清楚。

**Step 2: Prepare the verification report skeleton**

在 `verification.md` 中预留一个新的章节：

```md
## FemtoVG WGPU Experimental Verification

### Source Documents

- Design: `docs/plans/2026-03-12-femtovg-wgpu-experimental-design.md`
- Implementation Plan: `docs/plans/2026-03-12-femtovg-wgpu-experimental-implementation-plan.md`
```

不要提前宣称通过，只先建立报告骨架。

**Step 3: Write the minimal documentation updates**

更新 `readme.md`，新增一个独立章节 `## FemtoVG WGPU Experimental`，至少写清：

- `./build-linux-x64-femtovg-wgpu.sh`
- `./build-win-x64-femtovg-wgpu.sh`
- 两个 wrapper 只生成 experimental 包，不属于 `build-release.sh`
- executable name 仍是 `mica-term`
- stage/archive name 带 `mica-term-femtovg-wgpu-experimental-*`
- Linux / Windows 目标分别是：
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-msvc`

在 `verification.md` 中，等最终命令全部执行完后再补：

- Commands Executed
- Automated Results
- Linux Manual Checklist
- Windows Manual Checklist
- Notes

最后把本 implementation plan 自己 `git add` 进去，保证执行记录里保留最终落地版本。

**Step 4: Run final verification**

在 Linux 主机先跑自动化矩阵：

```bash
cargo fmt --check
cargo check --workspace
cargo check --no-default-features --features femtovg-wgpu-experimental -q
cargo test --workspace -q
cargo clippy --workspace --all-targets -- -D warnings
bash tests/femtovg_wgpu_contract_smoke.sh
bash tests/build_linux_x64_femtovg_wgpu_script_smoke.sh
bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh
bash tests/build_release_script_smoke.sh
```

在 Windows MSVC / Git Bash 环境补 host-specific 验证：

```bash
cargo check --target x86_64-pc-windows-msvc --no-default-features --features femtovg-wgpu-experimental -q
./build-win-x64-femtovg-wgpu.sh --help
```

如果有图形桌面，再补人工 GUI smoke：

- Linux:
  - `cargo run --no-default-features --features femtovg-wgpu-experimental`
- Windows:
  - `cargo run --target x86_64-pc-windows-msvc --no-default-features --features femtovg-wgpu-experimental`

手工检查项至少包括：

- 启动成功
- 标题显示 `FemtoVG WGPU Experimental`
- `resize / maximize / restore` 正常
- `theme toggle` 正常
- 失败时 stderr 与日志都能看到明确 experimental 提示

**Step 5: Commit**

```bash
git add readme.md verification.md \
  docs/plans/2026-03-12-femtovg-wgpu-experimental-implementation-plan.md
git commit -m "docs: record femtovg wgpu experimental verification flow"
```

## Recommended Execution Order

1. Task 1
2. Task 2
3. Task 3
4. Task 4
5. Task 5

Task 4 可以在 Task 2 稳定之后并行准备，但不要在 Task 1 未完成前就写 wrapper，因为 wrapper 的 feature 名与 archive naming 依赖 Task 1 的最终语义。

## Verification Gates

- Gate 1: `tests/runtime_profile.rs` 通过后，才能进入启动链改造
- Gate 2: `tests/femtovg_wgpu_contract_smoke.sh` 通过后，才能宣称 selector 真源在程序内部
- Gate 3: 标题 / 失败提示 / 日志测试通过后，才能开始补 README 和 verification 报告
- Gate 4: wrapper smoke 全通过后，才能更新 README 中的实验入口说明
- Gate 5: Linux 自动化矩阵与 Windows host-specific `cargo check` 都完成后，才能写 verification 结果

## Rollback Guidance

- 如果 `renderer-femtovg-wgpu` 或 `wgpu-28` API 在当前 Slint 版本上无法稳定组合，回滚 experimental feature 与 wrapper，但不要碰正式链。
- 如果 selector 路径可以编译但在真实机上持续启动失败，保留设计与实现文档，回滚代码入口，继续把这条路线留在 try / design 层。
- 如果 wrapper 命名或产物命名影响既有脚本，优先回滚 wrapper，不允许把 experimental 逻辑并入 `build-release.sh`。
- 任何回滚都不允许变成“experimental 悄悄退回 software renderer”的混合实现。

## Handoff Notes

- 不要顺手改 `WelcomeView`、`terminal stack`、`SSH/SFTP` 或 Fluent 视觉细节。
- 不要恢复旧 `Skia experimental` 入口；`tests/build_win_x64_skia_script_smoke.sh` 应继续保持“旧路径不存在”的负向保护。
- 不要把 `SLINT_BACKEND` 当作临时捷径；本计划的核心就是程序内部 selector 真源。
- 所有实验身份都要可观察：标题、失败提示、日志、wrapper help、archive name 至少要覆盖其中四项以上。

## Execution Outcome

- Executed in `.worktrees/femtovg-wgpu-experimental` on 2026-03-13.
- Linux automation matrix passed:
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo check --no-default-features --features femtovg-wgpu-experimental -q`
  - `cargo test --workspace -q`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `bash tests/femtovg_wgpu_contract_smoke.sh`
  - `bash tests/build_linux_x64_femtovg_wgpu_script_smoke.sh`
  - `bash tests/build_win_x64_femtovg_wgpu_script_smoke.sh`
  - `bash tests/build_release_script_smoke.sh`
- Windows MSVC experimental cross-target validation passed on the Linux host with:
  - Rust target `x86_64-pc-windows-msvc`
  - `llvm-19`
  - `clang-19`
  - temporary tool shim at `/tmp/mica-term-toolshim`
- A local `[patch.crates-io] gpu-allocator` override is part of the implementation because `gpu-allocator 0.28.0` otherwise resolves `windows 0.58.0` and conflicts with `wgpu-hal 28` on `x86_64-pc-windows-msvc`.
- Manual Linux / Windows GUI smoke is still pending because this environment has no desktop session and no real Windows host.
