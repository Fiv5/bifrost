#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"
source "$SCRIPT_DIR/../test_utils/assert.sh"

ADMIN_HOST="${ADMIN_HOST:-127.0.0.1}"
ADMIN_PORT="${ADMIN_PORT:-9900}"
ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
export ADMIN_PATH_PREFIX

cleanup() {
    admin_cleanup_bifrost
}

trap cleanup EXIT

if ! admin_ensure_bifrost; then
    echo "Failed to start or reach Bifrost admin API" >&2
    exit 1
fi

response=$(get_server_config)

assert_json_field ".timeout_secs" "30" "$response" "default timeout should be 30 seconds"
assert_json_field ".http1_max_header_size" "65536" "$response" "default HTTP/1 header limit should be 64 KiB"
assert_json_field ".http2_max_header_list_size" "262144" "$response" "default HTTP/2 header limit should be 256 KiB"
assert_json_field ".websocket_handshake_max_header_size" "65536" "$response" "default WebSocket handshake limit should be 64 KiB"

updated=$(update_server_config '{
  "timeout_secs": 45,
  "http1_max_header_size": 131072,
  "http2_max_header_list_size": 524288,
  "websocket_handshake_max_header_size": 98304
}')

assert_json_field ".timeout_secs" "45" "$updated" "updated timeout should be returned"
assert_json_field ".http1_max_header_size" "131072" "$updated" "updated HTTP/1 header limit should be returned"
assert_json_field ".http2_max_header_list_size" "524288" "$updated" "updated HTTP/2 header limit should be returned"
assert_json_field ".websocket_handshake_max_header_size" "98304" "$updated" "updated WebSocket handshake limit should be returned"

persisted=$(get_server_config)

assert_json_field ".timeout_secs" "45" "$persisted" "updated timeout should persist"
assert_json_field ".http1_max_header_size" "131072" "$persisted" "updated HTTP/1 header limit should persist"
assert_json_field ".http2_max_header_list_size" "524288" "$persisted" "updated HTTP/2 header limit should persist"
assert_json_field ".websocket_handshake_max_header_size" "98304" "$persisted" "updated WebSocket handshake limit should persist"

invalid=$(update_server_config '{"http2_max_header_list_size": 4294967296}')

assert_json_field ".error" "http2_max_header_list_size must be <= 4294967295" "$invalid" "oversized HTTP/2 header limit should be rejected"

echo ""
echo "Results: $PASSED_ASSERTIONS passed, $FAILED_ASSERTIONS failed"
if [ "$FAILED_ASSERTIONS" -gt 0 ]; then
    exit 1
fi
