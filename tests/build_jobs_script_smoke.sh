#!/usr/bin/env bash
# Verifies BUILD_JOBS help text, pass-through behavior, default compatibility, and validation errors.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_SOURCE="$ROOT_DIR/build-desktop.sh"
WINDOWS_WRAPPER="$ROOT_DIR/build-win-x64.sh"

TMP_ROOT="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_ROOT"
}
trap cleanup EXIT

PROJECT_DIR="$TMP_ROOT/project"
FAKE_BIN="$TMP_ROOT/bin"
mkdir -p \
  "$PROJECT_DIR/assets/icons/windows" \
  "$PROJECT_DIR/assets/fonts/JetBrainsMapleMono" \
  "$PROJECT_DIR/assets/fonts/SarasaTermSCNerd" \
  "$FAKE_BIN"

cp "$SCRIPT_SOURCE" "$PROJECT_DIR/build-desktop.sh"
cp "$WINDOWS_WRAPPER" "$PROJECT_DIR/build-win-x64.sh"

cat <<'TOML' > "$PROJECT_DIR/Cargo.toml"
[package]
name = "mica-term"
version = "0.1.0"
edition = "2021"
TOML

printf '# stub readme\n' > "$PROJECT_DIR/readme.md"
printf 'icon\n' > "$PROJECT_DIR/assets/icons/windows/mica-term.ico"
printf 'jetbrains maple mono license\n' > "$PROJECT_DIR/assets/fonts/JetBrainsMapleMono/LICENSE.txt"
printf 'sarasa term license\n' > "$PROJECT_DIR/assets/fonts/SarasaTermSCNerd/LICENSE.txt"

cat <<'EOF_CARGO' > "$FAKE_BIN/cargo"
#!/usr/bin/env bash
set -euo pipefail

log_path="${FAKE_CARGO_LOG:?missing FAKE_CARGO_LOG}"
printf '%s\n' "$*" >> "$log_path"

if [[ "${1:-}" == "xwin" && "${2:-}" == "--version" ]]; then
  echo 'cargo-xwin-xwin 0.21.4'
  exit 0
fi

if [[ "${1:-}" == "xwin" && "${2:-}" == "build" ]]; then
  shift 2
  target=''
  profile='debug'
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --target)
        target="$2"
        shift 2
        ;;
      --release)
        profile='release'
        shift
        ;;
      *)
        shift
        ;;
    esac
  done
  [[ -n "$target" ]] || {
    echo 'missing --target for fake cargo xwin build' >&2
    exit 1
  }
  echo 'Compiling dependency-crate v0.1.0'
  echo 'Building [=====================> ] 1131/1133: mica-term'
  mkdir -p "$FAKE_PROJECT_DIR/target/$target/$profile"
  printf 'stub exe\n' > "$FAKE_PROJECT_DIR/target/$target/$profile/mica-term.exe"
  exit 0
fi

if [[ "${1:-}" == "build" ]]; then
  shift
  target=''
  profile='debug'
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --target)
        target="$2"
        shift 2
        ;;
      --release)
        profile='release'
        shift
        ;;
      *)
        shift
        ;;
    esac
  done
  [[ -n "$target" ]] || {
    echo 'missing --target for fake cargo build' >&2
    exit 1
  }
  echo 'Compiling dependency-crate v0.1.0'
  echo 'Compiling mica-term v0.1.0'
  mkdir -p "$FAKE_PROJECT_DIR/target/$target/$profile"
  if [[ "$target" == *windows* ]]; then
    printf 'stub exe\n' > "$FAKE_PROJECT_DIR/target/$target/$profile/mica-term.exe"
  else
    printf 'stub bin\n' > "$FAKE_PROJECT_DIR/target/$target/$profile/mica-term"
  fi
  exit 0
fi

echo "unexpected cargo invocation: $*" >&2
exit 1
EOF_CARGO

cat <<'EOF_RUSTUP' > "$FAKE_BIN/rustup"
#!/usr/bin/env bash
set -euo pipefail

if [[ "$*" == 'target list --installed' ]]; then
  cat <<'EOF_TARGETS'
x86_64-unknown-linux-gnu
x86_64-pc-windows-msvc
EOF_TARGETS
  exit 0
fi

echo "unexpected rustup invocation: $*" >&2
exit 1
EOF_RUSTUP

cat <<'EOF_ZIP' > "$FAKE_BIN/zip"
#!/usr/bin/env bash
set -euo pipefail

archive=''
for arg in "$@"; do
  if [[ "$arg" != -* ]]; then
    archive="$arg"
    break
  fi
done

[[ -n "$archive" ]] || {
  echo 'missing archive path for fake zip' >&2
  exit 1
}

touch "$PWD/$archive"
EOF_ZIP

cat <<'EOF_TAR' > "$FAKE_BIN/tar"
#!/usr/bin/env bash
set -euo pipefail

archive=''
while [[ $# -gt 0 ]]; do
  case "$1" in
    -czf)
      archive="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

[[ -n "$archive" ]] || {
  echo 'missing archive path for fake tar' >&2
  exit 1
}

touch "$archive"
EOF_TAR

cat <<'EOF_GZIP' > "$FAKE_BIN/gzip"
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF_GZIP

cat <<'EOF_UNAME' > "$FAKE_BIN/uname"
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "-s" ]]; then
  echo "${FAKE_UNAME:-Linux}"
  exit 0
fi

/usr/bin/uname "$@"
EOF_UNAME

cat <<'EOF_NPROC' > "$FAKE_BIN/nproc"
#!/usr/bin/env bash
set -euo pipefail
echo "${FAKE_NPROC:-12}"
EOF_NPROC

cat <<'EOF_CLANG' > "$FAKE_BIN/clang"
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF_CLANG

cat <<'EOF_LLVM_LIB' > "$FAKE_BIN/llvm-lib"
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF_LLVM_LIB

cat <<'EOF_LLVM_RC' > "$FAKE_BIN/llvm-rc"
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF_LLVM_RC

cat <<'EOF_LLVM_AR' > "$FAKE_BIN/llvm-ar"
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF_LLVM_AR

cat <<'EOF_LLD' > "$FAKE_BIN/lld-link"
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF_LLD

chmod +x "$FAKE_BIN"/*

mkdir -p "$TMP_ROOT/home/.cache/cargo-xwin/xwin/sdk/lib/um/x86_64"
printf 'stub advapi32\n' > "$TMP_ROOT/home/.cache/cargo-xwin/xwin/sdk/lib/um/x86_64/advapi32.lib"

COMMON_ENV=(
  "PATH=$FAKE_BIN:/usr/bin:/bin"
  "FAKE_PROJECT_DIR=$PROJECT_DIR"
  "HOME=$TMP_ROOT/home"
)

HELP_OUTPUT="$("$ROOT_DIR/build-desktop.sh" --help)"
grep -F "BUILD_JOBS" <<<"$HELP_OUTPUT" >/dev/null

WINDOWS_HELP_OUTPUT="$("$ROOT_DIR/build-win-x64.sh" --help)"
grep -F "BUILD_JOBS" <<<"$WINDOWS_HELP_OUTPUT" >/dev/null
grep -F "x86_64-pc-windows-msvc" <<<"$WINDOWS_HELP_OUTPUT" >/dev/null
grep -F "auto-detects parallel jobs" <<<"$WINDOWS_HELP_OUTPUT" >/dev/null

linux_log="$TMP_ROOT/linux.log"
linux_out="$TMP_ROOT/linux.out"
: > "$linux_log"
env "${COMMON_ENV[@]}" \
  FAKE_CARGO_LOG="$linux_log" \
  TARGET=x86_64-unknown-linux-gnu \
  PROFILE=debug \
  DIST_DIR="$PROJECT_DIR/out-linux" \
  BUILD_JOBS=32 \
  bash "$PROJECT_DIR/build-desktop.sh" >"$linux_out"

grep -F 'build jobs: BUILD_JOBS=32 -> --jobs 32' "$linux_out" >/dev/null
grep -F 'build driver: cargo build' "$linux_out" >/dev/null
grep -F 'phase 1/3: parallel dependency compilation' "$linux_out" >/dev/null
grep -F 'phase 2/3: final crate compile + link' "$linux_out" >/dev/null
grep -F 'phase 3/3: package staging and archive' "$linux_out" >/dev/null
grep -F -- 'build --target x86_64-unknown-linux-gnu --locked --jobs 32' "$linux_log" >/dev/null

linux_default_log="$TMP_ROOT/linux-default.log"
linux_default_out="$TMP_ROOT/linux-default.out"
: > "$linux_default_log"
env "${COMMON_ENV[@]}" \
  FAKE_CARGO_LOG="$linux_default_log" \
  TARGET=x86_64-unknown-linux-gnu \
  PROFILE=debug \
  DIST_DIR="$PROJECT_DIR/out-linux-default" \
  bash "$PROJECT_DIR/build-desktop.sh" >"$linux_default_out"

grep -F 'build jobs: default' "$linux_default_out" >/dev/null
if grep -F -- '--jobs' "$linux_default_log" >/dev/null; then
  echo 'BUILD_JOBS 未设置时不应向 cargo 追加 --jobs' >&2
  exit 1
fi

windows_log="$TMP_ROOT/windows.log"
windows_out="$TMP_ROOT/windows.out"
: > "$windows_log"
env "${COMMON_ENV[@]}" \
  FAKE_CARGO_LOG="$windows_log" \
  FAKE_UNAME=Linux \
  TARGET=x86_64-pc-windows-msvc \
  PROFILE=debug \
  MICA_TERM_PACKAGE_RENDERER=skia \
  DIST_DIR="$PROJECT_DIR/out-win" \
  BUILD_JOBS=32 \
  bash "$PROJECT_DIR/build-desktop.sh" >"$windows_out"

grep -F 'build driver: cargo xwin build' "$windows_out" >/dev/null
grep -F 'build jobs: BUILD_JOBS=32 -> --jobs 32' "$windows_out" >/dev/null
grep -F 'phase 1/3: parallel dependency compilation' "$windows_out" >/dev/null
grep -F 'phase 2/3: final crate compile + link' "$windows_out" >/dev/null
grep -F 'phase 3/3: package staging and archive' "$windows_out" >/dev/null
grep -F -- 'xwin build --target x86_64-pc-windows-msvc --locked --jobs 32' "$windows_log" >/dev/null

for invalid_jobs in 0 -1 abc; do
  invalid_log="$TMP_ROOT/invalid-$invalid_jobs.out"
  if env "${COMMON_ENV[@]}" \
    FAKE_CARGO_LOG="$TMP_ROOT/should-not-run.log" \
    TARGET=x86_64-unknown-linux-gnu \
    PROFILE=debug \
    DIST_DIR="$PROJECT_DIR/out-invalid-$invalid_jobs" \
    BUILD_JOBS="$invalid_jobs" \
    bash "$PROJECT_DIR/build-desktop.sh" >"$invalid_log" 2>&1; then
    echo "BUILD_JOBS=$invalid_jobs 应当失败" >&2
    exit 1
  fi
  grep -F "error: BUILD_JOBS must be a positive integer, got '$invalid_jobs'" "$invalid_log" >/dev/null
done

wrapper_target_log="$TMP_ROOT/wrapper-target.log"
wrapper_target_out="$TMP_ROOT/wrapper-target.out"
publish_dir="$TMP_ROOT/publish"
publish_archive="$publish_dir/mica-term-x86_64-pc-windows-msvc-release-skia.zip"
: > "$wrapper_target_log"
mkdir -p "$publish_dir"
printf 'stale symlink target\n' > "$TMP_ROOT/stale-archive.zip"
ln -s "$TMP_ROOT/stale-archive.zip" "$publish_archive"
env "${COMMON_ENV[@]}" \
  FAKE_CARGO_LOG="$wrapper_target_log" \
  FAKE_UNAME=Linux \
  DIST_DIR="$PROJECT_DIR/out-wrapper" \
  BUILD_JOBS=32 \
  PUBLISH_DIR="$publish_dir" \
  bash "$PROJECT_DIR/build-win-x64.sh" >"$wrapper_target_out"

grep -F -- 'xwin build --release --target x86_64-pc-windows-msvc --locked --no-default-features --features slint-renderer-skia,terminal-native-renderer --jobs 32' "$wrapper_target_log" >/dev/null
grep -F 'phase 1/3: parallel dependency compilation' "$wrapper_target_out" >/dev/null
grep -F 'phase 2/3: final crate compile + link' "$wrapper_target_out" >/dev/null
grep -F 'phase 3/3: package staging and archive' "$wrapper_target_out" >/dev/null
[[ -f "$PROJECT_DIR/out-wrapper/mica-term-x86_64-pc-windows-msvc-release-skia.zip" ]]
[[ -f "$publish_archive" ]]
[[ ! -L "$publish_archive" ]]
cmp -s \
  "$PROJECT_DIR/out-wrapper/mica-term-x86_64-pc-windows-msvc-release-skia.zip" \
  "$publish_archive"
grep -F 'Published archive:' "$wrapper_target_out" >/dev/null
grep -F "$publish_archive" "$wrapper_target_out" >/dev/null

wrapper_auto_log="$TMP_ROOT/wrapper-auto.log"
wrapper_auto_out="$TMP_ROOT/wrapper-auto.out"
wrapper_auto_publish_dir="$TMP_ROOT/publish-auto"
: > "$wrapper_auto_log"
mkdir -p "$wrapper_auto_publish_dir"
env "${COMMON_ENV[@]}" \
  FAKE_CARGO_LOG="$wrapper_auto_log" \
  FAKE_UNAME=Linux \
  FAKE_NPROC=12 \
  DIST_DIR="$PROJECT_DIR/out-wrapper-auto" \
  PUBLISH_DIR="$wrapper_auto_publish_dir" \
  bash "$PROJECT_DIR/build-win-x64.sh" >"$wrapper_auto_out"

grep -F 'build jobs: auto-detected 12 via nproc -> --jobs 12' "$wrapper_auto_out" >/dev/null
grep -F -- 'xwin build --release --target x86_64-pc-windows-msvc --locked --no-default-features --features slint-renderer-skia,terminal-native-renderer --jobs 12' "$wrapper_auto_log" >/dev/null
