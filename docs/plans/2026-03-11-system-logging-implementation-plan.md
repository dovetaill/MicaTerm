# System Logging Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在当前 `Rust + Slint` 桌面壳层中落地已确认的 system logging 设计，建立统一 `tracing` 日志基建、标准/portable 日志路径解析、默认 `ERROR`/`PANIC` 落盘、崩溃 `crash/` 文件，以及 `14 天 + 64MB` 的清理策略。

**Architecture:** 新增 `src/app/logging/` 模块族，拆分为 `paths`、`config`、`runtime`、`cleanup`、`panic` 五个子模块。日志初始化在 UI 创建前完成，普通严重错误写入 `logs/`，panic/fatal 写入 `crash/`，当前启动链中的零散 `eprintln!` 迁移为统一 `tracing` 事件，且日志系统自身失败时只能降级，不能阻断应用启动。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, anyhow, directories 5, tracing, tracing-subscriber, tracing-appender, std::backtrace, filetime (dev), shell smoke tests, `cargo fmt`, `cargo check`, `cargo test`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-11-system-logging-design.md`，实现过程中不要扩展到 `Diagnostics` 页、`WER LocalDumps`、`MiniDumpWriteDump`、SSH/SFTP/terminal 业务日志。
- 执行时先使用 `@superpowers:using-git-worktrees` 创建独立 worktree，再在该 worktree 中实现。
- 每个任务严格按 `@superpowers:test-driven-development` 执行：先写失败测试，再写最小实现，再运行通过。
- 完成前使用 `@superpowers:verification-before-completion`，禁止先宣称完成再补验证。
- 如果 global subscriber、panic hook、子进程 smoke 行为与预期不一致，立即切换 `@superpowers:systematic-debugging`，不要猜。
- 当前仓库还没有 `src/app/logging/`，现有启动链只有 `src/app/bootstrap.rs` 中三个 `eprintln!` 触点；这一轮先把这些现有触点纳入统一 logging，不假设未来业务模块已经存在。
- `ui.tooltip` 相关临时调试能力不纳入长期 system logging 范围；如果执行过程中遇到旧调试代码，按设计稿直接删除或并入统一 `tracing`，不要保留独立文件 logger。

## Task 1: 新增 logging 模块壳层与日志路径解析

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/mod.rs`
- Create: `src/app/logging/mod.rs`
- Create: `src/app/logging/paths.rs`
- Test: `tests/logging_paths.rs`

**Step 1: Write the failing test**

创建 `tests/logging_paths.rs`：

```rust
use std::fs;
use std::path::PathBuf;

use mica_term::app::logging::paths::{
    LoggingPathInputs, LoggingRootSource, resolve_logging_paths,
};

#[test]
fn logging_paths_prefer_env_override_over_portable_and_standard_dirs() {
    let temp_root = std::env::temp_dir().join("mica-term").join("tests").join("logging-paths-override");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("portable-root")).unwrap();
    fs::write(temp_root.join("portable-root").join(".mica-term-portable"), "").unwrap();

    let paths = resolve_logging_paths(&LoggingPathInputs {
        env_log_dir: Some(temp_root.join("override-root")),
        executable_dir: temp_root.join("portable-root"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, LoggingRootSource::EnvOverride);
    assert_eq!(paths.root_dir, temp_root.join("override-root"));
    assert_eq!(paths.logs_dir, temp_root.join("override-root").join("logs"));
    assert_eq!(paths.crash_dir, temp_root.join("override-root").join("crash"));
}

#[test]
fn logging_paths_fall_back_to_portable_marker_when_present() {
    let temp_root = std::env::temp_dir().join("mica-term").join("tests").join("logging-paths-portable");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("portable-root")).unwrap();
    fs::write(temp_root.join("portable-root").join(".mica-term-portable"), "").unwrap();

    let paths = resolve_logging_paths(&LoggingPathInputs {
        env_log_dir: None,
        executable_dir: temp_root.join("portable-root"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, LoggingRootSource::PortableMarker);
    assert_eq!(paths.root_dir, temp_root.join("portable-root"));
}

#[test]
fn logging_paths_use_standard_local_data_dir_when_no_override_exists() {
    let temp_root = std::env::temp_dir().join("mica-term").join("tests").join("logging-paths-standard");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("app-bin")).unwrap();

    let paths = resolve_logging_paths(&LoggingPathInputs {
        env_log_dir: None,
        executable_dir: temp_root.join("app-bin"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, LoggingRootSource::StandardLocalData);
    assert_eq!(paths.root_dir, temp_root.join("standard-root"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test logging_paths -q`  
Expected: FAIL with missing module/items such as `app::logging`, `resolve_logging_paths`, or `LoggingPathInputs`.

**Step 3: Write minimal implementation**

在 `Cargo.toml` 增加依赖：

```toml
[dependencies]
tracing = "0.1"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }

[dev-dependencies]
filetime = "0.2"
```

在 `src/app/mod.rs` 导出 logging 模块：

```rust
pub mod bootstrap;
pub mod logging;
pub mod ui_preferences;
pub mod window_effects;
pub mod windowing;
```

创建 `src/app/logging/mod.rs`：

```rust
pub mod cleanup;
pub mod config;
pub mod panic;
pub mod paths;
pub mod runtime;
```

创建 `src/app/logging/paths.rs`：

```rust
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoggingRootSource {
    EnvOverride,
    PortableMarker,
    StandardLocalData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingPaths {
    pub root_source: LoggingRootSource,
    pub root_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub crash_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingPathInputs {
    pub env_log_dir: Option<PathBuf>,
    pub executable_dir: PathBuf,
    pub standard_local_data_dir: PathBuf,
    pub portable_marker_name: &'static str,
}

pub fn resolve_logging_paths(inputs: &LoggingPathInputs) -> Result<LoggingPaths> {
    let (root_source, root_dir) = if let Some(path) = &inputs.env_log_dir {
        (LoggingRootSource::EnvOverride, path.clone())
    } else if inputs
        .executable_dir
        .join(inputs.portable_marker_name)
        .exists()
    {
        (LoggingRootSource::PortableMarker, inputs.executable_dir.clone())
    } else {
        (
            LoggingRootSource::StandardLocalData,
            inputs.standard_local_data_dir.clone(),
        )
    };

    let logs_dir = root_dir.join("logs");
    let crash_dir = root_dir.join("crash");
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&crash_dir)?;

    Ok(LoggingPaths {
        root_source,
        root_dir,
        logs_dir,
        crash_dir,
    })
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test logging_paths -q`  
Expected: PASS, 并且临时目录下成功创建 `logs/` 与 `crash/`。

**Step 5: Commit**

```bash
git add Cargo.toml src/app/mod.rs src/app/logging/mod.rs src/app/logging/paths.rs tests/logging_paths.rs
git commit -m "feat: add logging path resolution"
```

## Task 2: 新增 logging config 与 tracing runtime，锁定默认 ERROR 过滤

**Files:**
- Modify: `src/app/logging/mod.rs`
- Create: `src/app/logging/config.rs`
- Create: `src/app/logging/runtime.rs`
- Test: `tests/logging_runtime.rs`

**Step 1: Write the failing test**

创建 `tests/logging_runtime.rs`：

```rust
use std::fs;

use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::LoggingPaths;
use mica_term::app::logging::runtime::build_test_logging_runtime;

#[test]
fn logging_runtime_writes_error_but_filters_debug_by_default() {
    let temp_root = std::env::temp_dir().join("mica-term").join("tests").join("logging-runtime-default");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: mica_term::app::logging::paths::LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::ErrorOnly);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        tracing::debug!(target: "ui.tooltip", "debug event should be filtered");
        tracing::error!(target: "app.lifecycle", "error event should be persisted");
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("error event should be persisted"));
    assert!(!content.contains("debug event should be filtered"));
}

#[test]
fn logging_runtime_keeps_debug_events_when_debug_mode_is_enabled() {
    let temp_root = std::env::temp_dir().join("mica-term").join("tests").join("logging-runtime-debug");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: mica_term::app::logging::paths::LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        tracing::debug!(target: "app.logging", "debug event should survive");
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("debug event should survive"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test logging_runtime -q`  
Expected: FAIL with missing `AppLoggingConfig`, `AppLogMode`, or `build_test_logging_runtime`.

**Step 3: Write minimal implementation**

创建 `src/app/logging/config.rs`：

```rust
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLogMode {
    ErrorOnly,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLoggingConfig {
    pub mode: AppLogMode,
}

impl AppLoggingConfig {
    pub fn new(mode: AppLogMode) -> Self {
        Self { mode }
    }

    pub fn from_env() -> Self {
        let mode = match env::var("MICA_TERM_LOG").ok().as_deref() {
            Some("debug") => AppLogMode::Debug,
            Some("trace") => AppLogMode::Debug,
            _ => AppLogMode::ErrorOnly,
        };

        Self::new(mode)
    }

    pub fn filter_directive(self) -> &'static str {
        match self.mode {
            AppLogMode::ErrorOnly => "error",
            AppLogMode::Debug => "debug",
        }
    }
}
```

创建 `src/app/logging/runtime.rs`：

```rust
use std::path::Path;

use anyhow::Result;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

use super::config::AppLoggingConfig;
use super::paths::LoggingPaths;

pub struct TestLoggingRuntime {
    pub dispatch: tracing::Dispatch,
    pub guard: WorkerGuard,
}

pub fn build_test_logging_runtime(
    paths: &LoggingPaths,
    config: &AppLoggingConfig,
) -> Result<TestLoggingRuntime> {
    let file_appender = tracing_appender::rolling::never(&paths.logs_dir, "system-error.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(EnvFilter::new(config.filter_directive()))
        .with_writer(writer)
        .finish();

    Ok(TestLoggingRuntime {
        dispatch: tracing::Dispatch::new(subscriber),
        guard,
    })
}
```

在 `src/app/logging/mod.rs` 导出：

```rust
pub mod cleanup;
pub mod config;
pub mod panic;
pub mod paths;
pub mod runtime;
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test logging_runtime -q`  
Expected: PASS，默认模式只看到 `ERROR`，`MICA_TERM_LOG=debug` 对应模式保留 debug event。

**Step 5: Commit**

```bash
git add src/app/logging/mod.rs src/app/logging/config.rs src/app/logging/runtime.rs tests/logging_runtime.rs
git commit -m "feat: add tracing runtime for system logging"
```

## Task 3: 实现 `14 天 + 64MB` cleanup 策略

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/logging/mod.rs`
- Create: `src/app/logging/cleanup.rs`
- Test: `tests/logging_cleanup.rs`

**Step 1: Write the failing test**

创建 `tests/logging_cleanup.rs`：

```rust
use std::fs;
use std::time::{Duration, SystemTime};

use filetime::{FileTime, set_file_mtime};
use mica_term::app::logging::cleanup::{CleanupPolicy, cleanup_logging_dirs};

#[test]
fn cleanup_removes_files_older_than_max_age() {
    let temp_root = std::env::temp_dir().join("mica-term").join("tests").join("logging-cleanup-age");
    let logs_dir = temp_root.join("logs");
    let crash_dir = temp_root.join("crash");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&logs_dir).unwrap();
    fs::create_dir_all(&crash_dir).unwrap();

    let old_file = logs_dir.join("old.log");
    fs::write(&old_file, "old").unwrap();
    let old_time = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 30));
    set_file_mtime(&old_file, old_time).unwrap();

    cleanup_logging_dirs(
        &logs_dir,
        &crash_dir,
        CleanupPolicy {
            max_age: Duration::from_secs(60 * 60 * 24 * 14),
            max_total_bytes: 1024 * 1024,
        },
    )
    .unwrap();

    assert!(!old_file.exists());
}

#[test]
fn cleanup_trims_oldest_files_when_total_size_exceeds_limit() {
    let temp_root = std::env::temp_dir().join("mica-term").join("tests").join("logging-cleanup-size");
    let logs_dir = temp_root.join("logs");
    let crash_dir = temp_root.join("crash");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&logs_dir).unwrap();
    fs::create_dir_all(&crash_dir).unwrap();

    let oldest = logs_dir.join("a.log");
    let newer = logs_dir.join("b.log");
    fs::write(&oldest, vec![b'a'; 32]).unwrap();
    fs::write(&newer, vec![b'b'; 32]).unwrap();
    set_file_mtime(
        &oldest,
        FileTime::from_system_time(SystemTime::now() - Duration::from_secs(120)),
    )
    .unwrap();

    cleanup_logging_dirs(
        &logs_dir,
        &crash_dir,
        CleanupPolicy {
            max_age: Duration::from_secs(60 * 60 * 24 * 14),
            max_total_bytes: 40,
        },
    )
    .unwrap();

    assert!(!oldest.exists());
    assert!(newer.exists());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test logging_cleanup -q`  
Expected: FAIL with missing `cleanup_logging_dirs` or `CleanupPolicy`.

**Step 3: Write minimal implementation**

在 `Cargo.toml` 的 `dev-dependencies` 中确保存在：

```toml
filetime = "0.2"
```

创建 `src/app/logging/cleanup.rs`：

```rust
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupPolicy {
    pub max_age: Duration,
    pub max_total_bytes: u64,
}

pub fn cleanup_logging_dirs(logs_dir: &Path, crash_dir: &Path, policy: CleanupPolicy) -> Result<()> {
    let cutoff = SystemTime::now() - policy.max_age;
    let mut entries = collect_entries(logs_dir)?;
    entries.extend(collect_entries(crash_dir)?);

    for (path, modified, _) in &entries {
        if *modified < cutoff {
            let _ = fs::remove_file(path);
        }
    }

    let mut entries = collect_entries(logs_dir)?;
    entries.extend(collect_entries(crash_dir)?);
    entries.sort_by_key(|(_, modified, _)| *modified);

    let mut total_size: u64 = entries.iter().map(|(_, _, len)| *len).sum();
    for (path, _, len) in entries {
        if total_size <= policy.max_total_bytes {
            break;
        }
        let _ = fs::remove_file(path);
        total_size = total_size.saturating_sub(len);
    }

    Ok(())
}

fn collect_entries(dir: &Path) -> Result<Vec<(PathBuf, SystemTime, u64)>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            out.push((entry.path(), metadata.modified()?, metadata.len()));
        }
    }
    Ok(out)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test logging_cleanup -q`  
Expected: PASS，过期文件被删，超额时最旧文件先被淘汰。

**Step 5: Commit**

```bash
git add Cargo.toml src/app/logging/mod.rs src/app/logging/cleanup.rs tests/logging_cleanup.rs
git commit -m "feat: add logging retention cleanup"
```

## Task 4: 新增 panic/fatal 记录与子进程 smoke

**Files:**
- Modify: `src/app/logging/mod.rs`
- Create: `src/app/logging/panic.rs`
- Test: `tests/panic_logging.rs`

**Step 1: Write the failing test**

创建 `tests/panic_logging.rs`：

```rust
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use mica_term::app::logging::panic::install_panic_hook;

#[test]
fn panic_hook_writes_crash_file_for_child_process() {
    if std::env::var("MICA_TERM_PANIC_CHILD").ok().as_deref() == Some("1") {
        let crash_dir = PathBuf::from(std::env::var("MICA_TERM_CRASH_DIR").unwrap());
        install_panic_hook(crash_dir).unwrap();
        panic!("panic hook smoke");
    }

    let temp_root = std::env::temp_dir().join("mica-term").join("tests").join("panic-hook");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("panic_hook_writes_crash_file_for_child_process")
        .env("MICA_TERM_PANIC_CHILD", "1")
        .env("MICA_TERM_CRASH_DIR", temp_root.join("crash"))
        .status()
        .unwrap();

    assert!(!status.success());

    let crash_file = fs::read_dir(temp_root.join("crash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let content = fs::read_to_string(crash_file).unwrap();
    assert!(content.contains("panic hook smoke"));
    assert!(content.contains("thread="));
    assert!(content.contains("backtrace="));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test panic_logging -q`  
Expected: FAIL with missing `install_panic_hook`.

**Step 3: Write minimal implementation**

创建 `src/app/logging/panic.rs`：

```rust
use std::backtrace::Backtrace;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

pub fn install_panic_hook(crash_dir: PathBuf) -> Result<()> {
    fs::create_dir_all(&crash_dir)?;
    let previous = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let _ = write_panic_record(&crash_dir, info);
        previous(info);
    }));

    Ok(())
}

pub fn write_fatal_record(crash_dir: &Path, phase: &str, error_text: &str) -> Result<PathBuf> {
    fs::create_dir_all(crash_dir)?;
    let file_path = crash_dir.join(format!("fatal-{}.log", unix_millis()));
    let backtrace = Backtrace::force_capture();
    let body = format!(
        "phase={phase}\nerror={error_text}\nbacktrace={backtrace}\n"
    );
    fs::write(&file_path, body)?;
    Ok(file_path)
}

fn write_panic_record(crash_dir: &Path, info: &std::panic::PanicHookInfo<'_>) -> Result<PathBuf> {
    fs::create_dir_all(crash_dir)?;
    let file_path = crash_dir.join(format!("panic-{}.log", unix_millis()));
    let location = info
        .location()
        .map(|loc| format!("{}:{}", loc.file(), loc.line()))
        .unwrap_or_else(|| "unknown".into());
    let payload = if let Some(text) = info.payload().downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = info.payload().downcast_ref::<String>() {
        text.clone()
    } else {
        "non-string panic payload".into()
    };
    let thread = std::thread::current().name().unwrap_or("unnamed");
    let backtrace = Backtrace::force_capture();
    let body = format!(
        "message={payload}\nlocation={location}\nthread={thread}\nbacktrace={backtrace}\n"
    );
    fs::write(&file_path, body)?;
    Ok(file_path)
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test panic_logging -q`  
Expected: PASS，子进程 panic 后在 `crash/` 中生成 `panic-*.log`，内容包含 message、thread 和 backtrace。

**Step 5: Commit**

```bash
git add src/app/logging/mod.rs src/app/logging/panic.rs tests/panic_logging.rs
git commit -m "feat: add panic and fatal crash logging"
```

## Task 5: 接入启动链，迁移现有 `eprintln!` 到统一 tracing

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/window_effects.rs`
- Modify: `src/app/logging/paths.rs`
- Modify: `src/app/logging/runtime.rs`
- Test: `tests/bootstrap_logging_contract_smoke.sh`
- Regression: `tests/bootstrap_smoke.rs`
- Regression: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing smoke contract**

创建 `tests/bootstrap_logging_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

grep -F 'try_init_global_logging' "$ROOT_DIR/src/main.rs" >/dev/null
grep -F 'install_panic_hook' "$ROOT_DIR/src/main.rs" >/dev/null
grep -F 'write_fatal_record' "$ROOT_DIR/src/main.rs" >/dev/null
grep -F 'ProjectDirs::from("dev", "MicaTerm", "MicaTerm")' "$ROOT_DIR/src/app/logging/paths.rs" >/dev/null
grep -F 'tracing::error!' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
grep -F 'target: "config.preferences"' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
grep -F 'target: "app.window"' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
! rg -n 'eprintln!' "$ROOT_DIR/src/app/bootstrap.rs" >/dev/null
```

**Step 2: Run smoke to verify it fails**

Run: `bash tests/bootstrap_logging_contract_smoke.sh`  
Expected: FAIL because `main.rs` 尚未初始化 system logging，`bootstrap.rs` 仍有 `eprintln!`。

**Step 3: Write minimal implementation**

在 `src/app/logging/paths.rs` 中补应用级 helper：

```rust
use anyhow::{Context, Result};
use directories::ProjectDirs;

pub fn resolve_logging_paths_for_app() -> Result<LoggingPaths> {
    let project_dirs = ProjectDirs::from("dev", "MicaTerm", "MicaTerm")
        .context("project directories are unavailable")?;

    let executable_dir = std::env::current_exe()?
        .parent()
        .context("executable directory is unavailable")?
        .to_path_buf();

    let env_log_dir = std::env::var_os("MICA_TERM_LOG_DIR").map(PathBuf::from);
    let standard_local_data_dir = project_dirs.data_local_dir().join("MicaTerm");

    resolve_logging_paths(&LoggingPathInputs {
        env_log_dir,
        executable_dir,
        standard_local_data_dir,
        portable_marker_name: ".mica-term-portable",
    })
}
```

在 `src/app/logging/runtime.rs` 中补全全局初始化：

```rust
use anyhow::Result;
use tracing_subscriber::EnvFilter;

use super::config::AppLoggingConfig;
use super::paths::{LoggingPaths, resolve_logging_paths_for_app};

pub struct AppLoggingRuntime {
    pub paths: LoggingPaths,
    pub guard: WorkerGuard,
}

pub fn try_init_global_logging() -> Result<AppLoggingRuntime> {
    let paths = resolve_logging_paths_for_app()?;
    let config = AppLoggingConfig::from_env();
    let file_appender = tracing_appender::rolling::daily(&paths.logs_dir, "system-error.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(EnvFilter::new(config.filter_directive()))
        .with_writer(writer)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(AppLoggingRuntime { paths, guard })
}
```

在 `src/main.rs` 中前置 logging：

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    let logging = match mica_term::app::logging::runtime::try_init_global_logging() {
        Ok(runtime) => {
            let _ = mica_term::app::logging::panic::install_panic_hook(runtime.paths.crash_dir.clone());
            Some(runtime)
        }
        Err(err) => {
            eprintln!("failed to initialize system logging: {err}");
            None
        }
    };

    if let Err(err) = mica_term::app::bootstrap::run() {
        if let Some(runtime) = &logging {
            let _ = mica_term::app::logging::panic::write_fatal_record(
                &runtime.paths.crash_dir,
                "bootstrap.run",
                &err.to_string(),
            );
        } else {
            eprintln!("fatal bootstrap error: {err}");
        }
        return Err(err);
    }

    Ok(())
}
```

在 `src/app/bootstrap.rs` 中迁移当前 `eprintln!` 触点：

```rust
fn load_ui_preferences(store: &Option<Rc<UiPreferencesStore>>) -> UiPreferences {
    match store {
        Some(store) => match store.load_or_default() {
            Ok(prefs) => prefs,
            Err(err) => {
                tracing::error!(
                    target: "config.preferences",
                    error = %err,
                    "failed to load ui preferences"
                );
                UiPreferences::default()
            }
        },
        None => UiPreferences::default(),
    }
}
```

```rust
fn save_ui_preferences(store: &Option<Rc<UiPreferencesStore>>, state: &ShellViewModel) {
    if let Some(store) = store
        && let Err(err) = store.save(&UiPreferences::from(state))
    {
        tracing::error!(
            target: "config.preferences",
            error = %err,
            "failed to save ui preferences"
        );
    }
}
```

```rust
pub fn bind_top_status_bar(window: &AppWindow) {
    let store = match UiPreferencesStore::for_app() {
        Ok(store) => Some(store),
        Err(err) => {
            tracing::error!(
                target: "config.preferences",
                error = %err,
                "failed to resolve ui preferences store"
            );
            None
        }
    };

    bind_top_status_bar_with_store(window, store);
}
```

并在 `sync_theme_and_window_effects(...)` 中记录原生窗口效果失败：

```rust
let report = effects.apply_to_app_window(window, &request);
if matches!(report.backdrop_status, crate::app::window_effects::BackdropApplyStatus::Failed) {
    tracing::error!(
        target: "app.window",
        ?request.theme,
        ?request.backdrop,
        "failed to apply native window appearance"
    );
}
```

**Step 4: Run verification to prove the wiring works**

Run: `bash tests/bootstrap_logging_contract_smoke.sh`  
Expected: PASS.

Run: `cargo test --test bootstrap_smoke --test top_status_bar_smoke -q`  
Expected: PASS, 顶栏绑定与窗口效果同步不回归。

Run: `cargo check -q`  
Expected: PASS.

**Step 5: Commit**

```bash
git add src/main.rs src/app/bootstrap.rs src/app/window_effects.rs src/app/logging/paths.rs src/app/logging/runtime.rs tests/bootstrap_logging_contract_smoke.sh
git commit -m "feat: wire system logging into startup"
```

## Final Verification

按顺序执行：

1. `cargo fmt`
2. `cargo fmt --check`
3. `cargo check -q`
4. `cargo test --test logging_paths --test logging_runtime --test logging_cleanup --test panic_logging -q`
5. `cargo test --test bootstrap_smoke --test top_status_bar_smoke -q`
6. `bash tests/bootstrap_logging_contract_smoke.sh`
7. `cargo test --tests -q`

记录到 `verification.md` 的最小结论应包括：

- 默认模式下 `ERROR` 落盘、`DEBUG` 被过滤
- portable/env/standard 路径优先级验证通过
- panic 子进程 smoke 生成 `crash/` 文件
- cleanup 满足 `14 天 + 64MB`
- 当前 `bootstrap`/`top_status_bar` 绑定行为无回归

