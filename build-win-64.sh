#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Mica Term (GPUI) — Windows x86_64 Cross-Build & HTTP Package Publisher
#
# Cross-compiles `mica-term-gpui.exe` for `x86_64-pc-windows-msvc` via
# `cargo-xwin`, packages a portable `.zip` bundle (with font license & portable
# marker), and atomically publishes it to `/var/www/html` for HTTP download.
# ==============================================================================

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

TARGET="${TARGET:-x86_64-pc-windows-msvc}"
PROFILE="${PROFILE:-release}"
OUTPUT_DIR="${OUTPUT_DIR:-/var/www/html}"
PORTABLE="${PORTABLE:-1}"
PACKAGE_NAME="${PACKAGE_NAME:-mica-term-gpui}"
BIN_NAME="${BIN_NAME:-mica-term-gpui}"
SKIP_PUBLISH=0

usage() {
    cat <<EOF
Usage: $(basename "$0") [options]

Options:
  -o, --output-dir <DIR>   HTTP download directory (default: ${OUTPUT_DIR})
      --profile <PROFILE>  Cargo build profile: release or debug (default: ${PROFILE})
      --target <TRIPLE>    Windows Rust target triple (default: ${TARGET})
      --no-portable        Do not create .mica-term-portable marker inside zip
      --skip-publish       Build and zip under target/dist only; skip copying to OUTPUT_DIR
  -h, --help               Show this help message

Environment overrides:
  OUTPUT_DIR, PROFILE, TARGET, PORTABLE (1|0), PACKAGE_NAME, CARGO_BUILD_JOBS
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -o|--output-dir)
            OUTPUT_DIR="${2:?missing value for $1}"
            shift 2
            ;;
        --profile)
            PROFILE="${2:?missing value for $1}"
            shift 2
            ;;
        --target)
            TARGET="${2:?missing value for $1}"
            shift 2
            ;;
        --no-portable)
            PORTABLE=0
            shift
            ;;
        --skip-publish)
            SKIP_PUBLISH=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "[ERROR] Unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

log() {
    printf '\033[1;36m==>\033[0m %s\n' "$*"
}

err() {
    printf '\033[1;31m[ERROR]\033[0m %s\n' "$*" >&2
    exit 1
}

# 1. Ensure cargo & LLVM/Clang cross tools are on PATH
if [[ -d "${HOME}/.cargo/bin" ]]; then
    export PATH="${HOME}/.cargo/bin:${PATH}"
fi

# Prefer installed LLVM bin directory (e.g. /usr/lib/llvm-19/bin) when unversioned clang-cl is absent
if ! command -v clang-cl >/dev/null 2>&1 || ! command -v llvm-lib >/dev/null 2>&1; then
    for llvm_dir in /usr/lib/llvm-20/bin /usr/lib/llvm-19/bin /usr/lib/llvm-18/bin /usr/lib/llvm-17/bin /usr/lib/llvm-16/bin; do
        if [[ -x "${llvm_dir}/clang-cl" ]]; then
            export PATH="${llvm_dir}:${PATH}"
            break
        fi
    done
fi

# Fallback shim directory if only versioned clang-19 / lld-link-19 wrappers exist in /usr/bin
if ! command -v clang-cl >/dev/null 2>&1 || ! command -v lld-link >/dev/null 2>&1 || ! command -v llvm-lib >/dev/null 2>&1; then
    TOOL_SHIM_DIR="${ROOT_DIR}/target/cargo-xwin-tools"
    mkdir -p "${TOOL_SHIM_DIR}"
    for ver in 20 19 18 17 16; do
        [[ -x "/usr/bin/clang-${ver}" && ! -e "${TOOL_SHIM_DIR}/clang" ]] && ln -s "/usr/bin/clang-${ver}" "${TOOL_SHIM_DIR}/clang"
        [[ -x "/usr/bin/clang++-${ver}" && ! -e "${TOOL_SHIM_DIR}/clang++" ]] && ln -s "/usr/bin/clang++-${ver}" "${TOOL_SHIM_DIR}/clang++"
        [[ -x "/usr/bin/clang-cl-${ver}" && ! -e "${TOOL_SHIM_DIR}/clang-cl" ]] && ln -s "/usr/bin/clang-cl-${ver}" "${TOOL_SHIM_DIR}/clang-cl"
        [[ -x "/usr/bin/lld-link-${ver}" && ! -e "${TOOL_SHIM_DIR}/lld-link" ]] && ln -s "/usr/bin/lld-link-${ver}" "${TOOL_SHIM_DIR}/lld-link"
        [[ -x "/usr/bin/llvm-lib-${ver}" && ! -e "${TOOL_SHIM_DIR}/llvm-lib" ]] && ln -s "/usr/bin/llvm-lib-${ver}" "${TOOL_SHIM_DIR}/llvm-lib"
        [[ -x "/usr/bin/llvm-rc-${ver}" && ! -e "${TOOL_SHIM_DIR}/llvm-rc" ]] && ln -s "/usr/bin/llvm-rc-${ver}" "${TOOL_SHIM_DIR}/llvm-rc"
    done
    export PATH="${TOOL_SHIM_DIR}:${PATH}"
fi

command -v cargo >/dev/null 2>&1 || err "cargo not found on PATH"
command -v cargo-xwin >/dev/null 2>&1 || err "cargo-xwin not found (install via: cargo install cargo-xwin)"
command -v clang-cl >/dev/null 2>&1 || err "clang-cl not found (install clang / llvm)"
command -v lld-link >/dev/null 2>&1 || err "lld-link not found (install lld / llvm)"

if command -v rustup >/dev/null 2>&1; then
    if ! rustup target list --installed | grep -qx "${TARGET}"; then
        log "Installing Rust target ${TARGET}..."
        rustup target add "${TARGET}"
    fi
fi

# 2. Prepare writable XWIN_CACHE_DIR reusing cached Windows CRT/SDK without dangling tool symlinks
DEFAULT_XWIN_CACHE="${XWIN_CACHE_DIR:-${HOME}/.cache/cargo-xwin}"
if [[ -d "${DEFAULT_XWIN_CACHE}/xwin" ]]; then
    if [[ ! -w "${DEFAULT_XWIN_CACHE}" ]] || { [[ -L "${DEFAULT_XWIN_CACHE}/clang-cl" ]] && [[ ! -e "${DEFAULT_XWIN_CACHE}/clang-cl" ]]; }; then
        LOCAL_XWIN_CACHE="${ROOT_DIR}/target/xwin-cache"
        mkdir -p "${LOCAL_XWIN_CACHE}"
        ln -sfn "${DEFAULT_XWIN_CACHE}/xwin" "${LOCAL_XWIN_CACHE}/xwin"
        export XWIN_CACHE_DIR="${LOCAL_XWIN_CACHE}"
    else
        export XWIN_CACHE_DIR="${DEFAULT_XWIN_CACHE}"
    fi
fi

# 3. Extract package metadata from Cargo.toml & Git
VERSION="$(sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n 1)"
[[ -n "${VERSION}" ]] || err "Failed to parse version from Cargo.toml"

GIT_COMMIT="$(git -C "${ROOT_DIR}" rev-parse HEAD 2>/dev/null || echo "unknown")"
GIT_SHORT="$(git -C "${ROOT_DIR}" rev-parse --short=12 HEAD 2>/dev/null || echo "unknown")"
if git -C "${ROOT_DIR}" status --porcelain --untracked-files=no 2>/dev/null | grep -q .; then
    GIT_DIRTY=" (modified)"
else
    GIT_DIRTY=""
fi

CARGO_PROFILE_ARGS=()
TARGET_SUBDIR="${PROFILE}"
if [[ "${PROFILE}" == "release" ]]; then
    CARGO_PROFILE_ARGS+=(--release)
elif [[ "${PROFILE}" == "debug" || "${PROFILE}" == "dev" ]]; then
    PROFILE="debug"
    TARGET_SUBDIR="debug"
else
    CARGO_PROFILE_ARGS+=(--profile "${PROFILE}")
fi

if [[ -n "${CARGO_BUILD_JOBS:-}" ]]; then
    CARGO_PROFILE_ARGS+=(--jobs "${CARGO_BUILD_JOBS}")
fi

# Disable AWS-LC JENT_FORCE_FIPS CPU jitter entropy timer on Windows so RAND_bytes uses
# native Windows CSPRNG (ProcessPrng / BCryptGenRandom) and never calls abort() on VMs / Win11 VBS.
export AWS_LC_SYS_NO_JITTER_ENTROPY="${AWS_LC_SYS_NO_JITTER_ENTROPY:-1}"

log "Building ${PACKAGE_NAME} v${VERSION} (${GIT_SHORT}${GIT_DIRTY}) for ${TARGET} [${PROFILE}]..."
cargo xwin build \
    --target "${TARGET}" \
    --bin "${BIN_NAME}" \
    "${CARGO_PROFILE_ARGS[@]}"

EXE_PATH="${ROOT_DIR}/target/${TARGET}/${TARGET_SUBDIR}/${BIN_NAME}.exe"
[[ -f "${EXE_PATH}" ]] || err "Expected Windows binary not found: ${EXE_PATH}"

# 4. Stage distribution folder & create .zip archive
BUNDLE_STEM="${PACKAGE_NAME}-${TARGET}-${PROFILE}"
VERSIONED_ZIP_NAME="${PACKAGE_NAME}-${VERSION}-${TARGET}-${PROFILE}.zip"
LATEST_ZIP_NAME="${BUNDLE_STEM}.zip"

DIST_ROOT="${ROOT_DIR}/target/dist"
STAGE_DIR="${DIST_ROOT}/${BUNDLE_STEM}"
DIST_ZIP="${DIST_ROOT}/${VERSIONED_ZIP_NAME}"

rm -rf "${STAGE_DIR}" "${DIST_ZIP}"
mkdir -p "${STAGE_DIR}"

cp -f "${EXE_PATH}" "${STAGE_DIR}/${BIN_NAME}.exe"

if [[ "${PORTABLE}" == "1" ]]; then
    : > "${STAGE_DIR}/.mica-term-portable"
fi

if [[ -f "${ROOT_DIR}/README.md" ]]; then
    cp -f "${ROOT_DIR}/README.md" "${STAGE_DIR}/README.md"
fi

for font_dir in SarasaTermSCNerd JetBrainsMapleMono; do
    [[ -f "${ROOT_DIR}/assets/fonts/${font_dir}/LICENSE.txt" ]] || continue
    mkdir -p "${STAGE_DIR}/licenses/fonts/${font_dir}"
    cp -f "${ROOT_DIR}/assets/fonts/${font_dir}/LICENSE.txt" \
        "${STAGE_DIR}/licenses/fonts/${font_dir}/LICENSE.txt"
done

log "Creating archive ${DIST_ZIP}..."
(
    cd "${DIST_ROOT}"
    if command -v zip >/dev/null 2>&1; then
        zip -r -9 -q "${VERSIONED_ZIP_NAME}" "${BUNDLE_STEM}"
    elif command -v 7z >/dev/null 2>&1; then
        7z a -tzip -mx=9 -bso0 "${VERSIONED_ZIP_NAME}" "${BUNDLE_STEM}"
    else
        err "Neither 'zip' nor '7z' is installed to create .zip archive"
    fi
)

SHA256_HASH="$(sha256sum "${DIST_ZIP}" | awk '{print $1}')"
EXE_SIZE_HUMAN="$(du -h "${EXE_PATH}" | awk '{print $1}')"
ZIP_SIZE_HUMAN="$(du -h "${DIST_ZIP}" | awk '{print $1}')"
BUILT_AT_UTC="$(date -u '+%Y-%m-%d %H:%M:%S UTC')"

VERSIONED_SHA_FILE="${DIST_ZIP}.sha256"
LATEST_SHA_FILE="${DIST_ROOT}/${LATEST_ZIP_NAME}.sha256"
BUILD_INFO_FILE="${DIST_ROOT}/${LATEST_ZIP_NAME}.build-info.txt"

printf '%s  %s\n' "${SHA256_HASH}" "${VERSIONED_ZIP_NAME}" > "${VERSIONED_SHA_FILE}"
printf '%s  %s\n' "${SHA256_HASH}" "${LATEST_ZIP_NAME}" > "${LATEST_SHA_FILE}"

cat > "${BUILD_INFO_FILE}" <<EOF
Package: ${PACKAGE_NAME}
Version: ${VERSION}
Commit: ${GIT_COMMIT}${GIT_DIRTY}
Target: ${TARGET}
Profile: ${PROFILE}
Portable: ${PORTABLE}
BuiltAt: ${BUILT_AT_UTC}
SHA256: ${SHA256_HASH}
EOF

# 5. Atomically publish artifacts into OUTPUT_DIR (/var/www/html)
atomic_publish() {
    local src="$1"
    local dest="$2"
    local tmp="${dest}.tmp.$$"
    cp -f "${src}" "${tmp}"
    chmod 0644 "${tmp}"
    mv -f "${tmp}" "${dest}"
}

if [[ "${SKIP_PUBLISH}" == "0" ]]; then
    log "Publishing HTTP download packages to ${OUTPUT_DIR}..."
    mkdir -p "${OUTPUT_DIR}"
    atomic_publish "${DIST_ZIP}" "${OUTPUT_DIR}/${VERSIONED_ZIP_NAME}"
    atomic_publish "${DIST_ZIP}" "${OUTPUT_DIR}/${LATEST_ZIP_NAME}"
    atomic_publish "${VERSIONED_SHA_FILE}" "${OUTPUT_DIR}/${VERSIONED_ZIP_NAME}.sha256"
    atomic_publish "${LATEST_SHA_FILE}" "${OUTPUT_DIR}/${LATEST_ZIP_NAME}.sha256"
    atomic_publish "${BUILD_INFO_FILE}" "${OUTPUT_DIR}/${LATEST_ZIP_NAME}.build-info.txt"
fi

printf '\n\033[1;32m✔ Windows x64 build complete!\033[0m\n'
printf '  Executable : %s (%s)\n' "${EXE_PATH}" "${EXE_SIZE_HUMAN}"
printf '  Archive    : %s (%s)\n' "${DIST_ZIP}" "${ZIP_SIZE_HUMAN}"
printf '  SHA256     : %s\n' "${SHA256_HASH}"
if [[ "${SKIP_PUBLISH}" == "0" ]]; then
    printf '  Published  :\n'
    printf '    - %s/%s\n' "${OUTPUT_DIR}" "${VERSIONED_ZIP_NAME}"
    printf '    - %s/%s\n' "${OUTPUT_DIR}" "${LATEST_ZIP_NAME}"
    printf '    - %s/%s.sha256\n' "${OUTPUT_DIR}" "${LATEST_ZIP_NAME}"
    printf '    - %s/%s.build-info.txt\n' "${OUTPUT_DIR}" "${LATEST_ZIP_NAME}"
fi
