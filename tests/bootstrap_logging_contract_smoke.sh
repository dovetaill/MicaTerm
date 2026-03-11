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
