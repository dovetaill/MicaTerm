#!/usr/bin/env bash
# Validates the desktop packaging wrapper interface and required commands.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/build-desktop.sh"

if [[ ! -f "$SCRIPT_PATH" ]]; then
  echo "missing build script: $SCRIPT_PATH" >&2
  exit 1
fi

bash -n "$SCRIPT_PATH"

HELP_OUTPUT="$("$SCRIPT_PATH" --help)"

grep -F "x86_64-unknown-linux-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "aarch64-unknown-linux-gnu" <<<"$HELP_OUTPUT" >/dev/null
grep -F "x86_64-apple-darwin" <<<"$HELP_OUTPUT" >/dev/null
grep -F "aarch64-apple-darwin" <<<"$HELP_OUTPUT" >/dev/null
grep -F "nasm" <<<"$HELP_OUTPUT" >/dev/null
grep -F "cargo-xwin" <<<"$HELP_OUTPUT" >/dev/null
grep -F "clang" <<<"$HELP_OUTPUT" >/dev/null
grep -F "lld-link-19" <<<"$HELP_OUTPUT" >/dev/null
grep -F "llvm-ar-19" <<<"$HELP_OUTPUT" >/dev/null
grep -F "llvm-lib-19" <<<"$HELP_OUTPUT" >/dev/null
grep -F "aarch64-pc-windows-msvc" <<<"$HELP_OUTPUT" >/dev/null
grep -F "dist/<app>-<target>-<profile><package flavor suffix>.tar.gz" <<<"$HELP_OUTPUT" >/dev/null
grep -F "dist/<app>-<target>-<profile><package flavor suffix>.zip" <<<"$HELP_OUTPUT" >/dev/null
grep -F "MICA_TERM_PACKAGE_PORTABLE=1" <<<"$HELP_OUTPUT" >/dev/null

grep -F 'require_cmd nasm' "$SCRIPT_PATH" >/dev/null
grep -F 'choose_clang_command' "$SCRIPT_PATH" >/dev/null
grep -F 'clang-19 clang' "$SCRIPT_PATH" >/dev/null
grep -F 'choose_lld_link_command' "$SCRIPT_PATH" >/dev/null
grep -F 'lld-link-19 lld-link' "$SCRIPT_PATH" >/dev/null
grep -F 'choose_llvm_ar_command' "$SCRIPT_PATH" >/dev/null
grep -F 'llvm-ar-19 llvm-ar' "$SCRIPT_PATH" >/dev/null
grep -F 'choose_llvm_lib_command' "$SCRIPT_PATH" >/dev/null
grep -F 'llvm-lib-19 llvm-lib' "$SCRIPT_PATH" >/dev/null
grep -F 'choose_llvm_rc_command' "$SCRIPT_PATH" >/dev/null
grep -F 'llvm-rc-19 llvm-rc' "$SCRIPT_PATH" >/dev/null
grep -F 'setup_linux_windows_msvc_tool_shims' "$SCRIPT_PATH" >/dev/null
grep -F 'WINDOWS_MSVC_TOOL_SHIM_DIR' "$SCRIPT_PATH" >/dev/null
grep -F 'MICA_TERM_WINDOWS_MSVC_STRIP_ICU_IMPORTS' "$SCRIPT_PATH" >/dev/null
grep -F 'cargo-xwin-patched-registry' "$SCRIPT_PATH" >/dev/null
grep -F 'setup_linux_windows_msvc_library_shims' "$SCRIPT_PATH" >/dev/null
grep -F 'WINDOWS_MSVC_LIB_SHIM_DIR' "$SCRIPT_PATH" >/dev/null
grep -F 'Advapi32.lib' "$SCRIPT_PATH" >/dev/null
grep -F 'cargo xwin build' "$SCRIPT_PATH" >/dev/null
grep -F 'MICA_TERM_PACKAGE_PORTABLE' "$SCRIPT_PATH" >/dev/null
grep -F '.mica-term-portable' "$SCRIPT_PATH" >/dev/null
