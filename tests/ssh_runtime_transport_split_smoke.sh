#!/usr/bin/env bash
# Guards SSH runtime transport/auth/pump/SFTP backend helpers being extracted into dedicated modules.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME_ROOT="$ROOT_DIR/src/app/ssh/runtime.rs"
TRANSPORT_MODULE="$ROOT_DIR/src/app/ssh/runtime/transport.rs"
AUTH_MODULE="$ROOT_DIR/src/app/ssh/runtime/auth.rs"
PUMP_MODULE="$ROOT_DIR/src/app/ssh/runtime/pump.rs"
SFTP_BACKEND_MODULE="$ROOT_DIR/src/app/ssh/runtime/sftp_backend.rs"

[[ -f "$RUNTIME_ROOT" ]] || {
  echo "missing src/app/ssh/runtime.rs" >&2
  exit 1
}

for file in \
  "$TRANSPORT_MODULE" \
  "$AUTH_MODULE" \
  "$PUMP_MODULE" \
  "$SFTP_BACKEND_MODULE"
do
  [[ -f "$file" ]] || {
    echo "missing $file" >&2
    exit 1
  }
done

for module in transport auth pump sftp_backend; do
  grep -F "mod ${module};" "$RUNTIME_ROOT" >/dev/null
done

grep -F 'struct TransportChainGuard {' "$TRANSPORT_MODULE" >/dev/null
grep -F 'async fn connect_target_handle_for_profile(' "$TRANSPORT_MODULE" >/dev/null
grep -F 'async fn negotiate_socks5_proxy_tunnel(' "$TRANSPORT_MODULE" >/dev/null
grep -F 'async fn negotiate_http_connect_tunnel(' "$TRANSPORT_MODULE" >/dev/null

grep -F 'struct ConnectionProgressReporter {' "$AUTH_MODULE" >/dev/null
grep -F 'pub struct UnknownHostKeyError {' "$AUTH_MODULE" >/dev/null
grep -F 'struct RuntimeClientHandler {' "$AUTH_MODULE" >/dev/null
grep -F 'async fn authenticate_client(' "$AUTH_MODULE" >/dev/null
grep -F 'impl client::Handler for RuntimeClientHandler {' "$AUTH_MODULE" >/dev/null

grep -F 'async fn run_channel_pump(' "$PUMP_MODULE" >/dev/null
grep -F '_transport_chain_guard: TransportChainGuard,' "$PUMP_MODULE" >/dev/null
grep -F 'struct SurfaceDirtyNotifier {' "$PUMP_MODULE" >/dev/null
grep -F 'struct WorkingSetTrimScheduler {' "$PUMP_MODULE" >/dev/null

grep -F 'struct RusshSftpBackend {' "$SFTP_BACKEND_MODULE" >/dev/null
grep -F 'impl SftpBackend for RusshSftpBackend {' "$SFTP_BACKEND_MODULE" >/dev/null
grep -F 'fn remote_child_path(parent: &str, name: &str) -> String {' "$SFTP_BACKEND_MODULE" >/dev/null

for symbol in \
  'struct TransportChainGuard {' \
  'async fn connect_target_handle_for_profile(' \
  'async fn negotiate_socks5_proxy_tunnel(' \
  'async fn negotiate_http_connect_tunnel(' \
  'struct ConnectionProgressReporter {' \
  'pub struct UnknownHostKeyError {' \
  'struct RuntimeClientHandler {' \
  'async fn authenticate_client(' \
  'async fn run_channel_pump(' \
  'struct SurfaceDirtyNotifier {' \
  'struct WorkingSetTrimScheduler {' \
  'struct RusshSftpBackend {' \
  'fn remote_child_path(parent: &str, name: &str) -> String {'
do
  if grep -F "$symbol" "$RUNTIME_ROOT" >/dev/null; then
    echo "runtime.rs still owns moved helper: $symbol" >&2
    exit 1
  fi
done
