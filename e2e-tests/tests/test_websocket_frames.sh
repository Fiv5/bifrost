#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${PROXY_PORT:-}"
WS_HOST="${WS_HOST:-127.0.0.1}"
WS_PORT="${WS_PORT:-}"
ADMIN_HOST="${ADMIN_HOST:-$PROXY_HOST}"
ADMIN_PORT="${ADMIN_PORT:-}"
ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
export ADMIN_PATH_PREFIX

if [[ -z "$PROXY_PORT" ]]; then
    PROXY_PORT=$((19000 + ($$ % 1000)))
fi
if [[ -z "$WS_PORT" ]]; then
    WS_PORT=$((20000 + ($$ % 1000)))
fi
if [[ -z "$ADMIN_PORT" ]]; then
    ADMIN_PORT="$PROXY_PORT"
fi

WS_HOST_HEADER="${WS_HOST}:${WS_PORT}"

source "$SCRIPT_DIR/../test_utils/assert.sh"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"

TESTS_PASSED=0
TESTS_FAILED=0

BIFROST_DATA_DIR=""
BIFROST_PID=""
WS_SERVER_PID=""

cleanup() {
    if [[ -n "$BIFROST_PID" ]] && kill -0 "$BIFROST_PID" 2>/dev/null; then
        kill "$BIFROST_PID" 2>/dev/null || true
        wait "$BIFROST_PID" 2>/dev/null || true
    fi

    if [[ -n "$WS_SERVER_PID" ]] && kill -0 "$WS_SERVER_PID" 2>/dev/null; then
        kill "$WS_SERVER_PID" 2>/dev/null || true
        wait "$WS_SERVER_PID" 2>/dev/null || true
    fi

    if [[ -n "$BIFROST_DATA_DIR" && -d "$BIFROST_DATA_DIR" ]]; then
        rm -rf "$BIFROST_DATA_DIR"
    fi
}

trap cleanup EXIT

log_test() {
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🧪 TEST: $1"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
}

pass() {
    echo "✅ PASS: $1"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

fail() {
    echo "❌ FAIL: $1"
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

check_deps() {
    if ! command -v python3 &> /dev/null; then
        echo "Error: python3 is required"
        exit 1
    fi
    if ! command -v jq &> /dev/null; then
        echo "Error: jq is required. Install with: brew install jq"
        exit 1
    fi
}

start_ws_server() {
    python3 "$SCRIPT_DIR/../mock_servers/ws_echo_server.py" --port "$WS_PORT" > /dev/null 2>&1 &
    WS_SERVER_PID=$!
    sleep 1
    kill -0 "$WS_SERVER_PID" 2>/dev/null
}

start_bifrost() {
    (cd "$ROOT_DIR" && cargo build --bin bifrost > /dev/null 2>&1)

    BIFROST_DATA_DIR="$(mktemp -d)"
    export BIFROST_DATA_DIR

    local log_file="$BIFROST_DATA_DIR/proxy.log"
    (cd "$ROOT_DIR" && BIFROST_DATA_DIR="$BIFROST_DATA_DIR" cargo run --bin bifrost -- -p "$PROXY_PORT" start --skip-cert-check --unsafe-ssl > "$log_file" 2>&1) &
    BIFROST_PID=$!
    sleep 1
    if ! kill -0 "$BIFROST_PID" 2>/dev/null; then
        tail -n 120 "$log_file" || true
        return 1
    fi

    local max_wait=60
    local waited=0
    while [[ $waited -lt $max_wait ]]; do
        if curl -sf "http://${ADMIN_HOST}:${ADMIN_PORT}${ADMIN_PATH_PREFIX}/api/system" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    tail -n 120 "$log_file" || true
    return 1
}

ws_generate_echo_traffic() {
    local messages="${1:-3}"
    python3 "$SCRIPT_DIR/../test_utils/ws_stress_client.py" \
        --proxy-host "$PROXY_HOST" \
        --proxy-port "$PROXY_PORT" \
        --host-header "$WS_HOST_HEADER" \
        --path "/ws" \
        --messages "$messages" \
        --timeout 15.0
}

is_ws_record() {
    local traffic_id="$1"
    local record
    record=$(get_traffic_detail "$traffic_id")
    local is_ws
    is_ws=$(echo "$record" | jq -r '.is_websocket // false')
    if [[ "$is_ws" == "true" ]]; then
        return 0
    fi
    local flags
    flags=$(echo "$record" | jq -r '.flags // 0')
    [[ $(( (flags / 2) % 2 )) -eq 1 ]]
}

test_ws_text_frame_forwarding() {
    log_test "WebSocket text frame forwarding (via proxy)"
    if ws_generate_echo_traffic 2; then
        pass "Echo frames forwarded"
        return 0
    fi
    fail "Failed to forward echo frames"
    return 1
}

test_ws_frames_capture() {
    log_test "WebSocket frames capture"

    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    ws_generate_echo_traffic 3 >/dev/null 2>&1 || true
    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws" 20)
    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        fail "No WebSocket traffic recorded"
        return 1
    fi

    if ! is_ws_record "$traffic_id"; then
        fail "Traffic not marked as WebSocket"
        return 1
    fi

    local frames_response
    frames_response=$(get_frames "$traffic_id")
    local frame_count
    frame_count=$(echo "$frames_response" | jq -r '.frames | length')
    if [[ "$frame_count" -ge 1 ]]; then
        local has_preview
        has_preview=$(echo "$frames_response" | jq -r '[.frames[] | select((.payload_preview // "") | length > 0)] | length')
        if [[ "${has_preview:-0}" -le 0 ]]; then
            fail "Captured frames but payload_preview is empty"
            return 1
        fi
        local record
        record=$(get_traffic_detail "$traffic_id")
        local response_size
        response_size=$(echo "$record" | jq -r '.response_size // 0')
        if [[ "${response_size:-0}" -le 0 ]]; then
            fail "WebSocket response_size should be persisted"
            return 1
        fi
        local socket_bytes
        socket_bytes=$(echo "$record" | jq -r '(.socket_status.send_bytes // 0) + (.socket_status.receive_bytes // 0)')
        if [[ "${socket_bytes:-0}" -le 0 ]]; then
            fail "WebSocket socket_status bytes should be persisted"
            return 1
        fi
        pass "Captured $frame_count frames with payload_preview"
        return 0
    fi

    fail "Expected frames, got $frame_count"
    return 1
}

test_ws_frame_directions() {
    log_test "WebSocket frame directions"

    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    ws_generate_echo_traffic 1 >/dev/null 2>&1 || true
    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws" 20)
    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        fail "No traffic ID found"
        return 1
    fi

    local frames_response
    frames_response=$(get_frames "$traffic_id")

    local send_count receive_count
    send_count=$(echo "$frames_response" | jq '[.frames[] | select(.direction == "send")] | length')
    receive_count=$(echo "$frames_response" | jq '[.frames[] | select(.direction == "receive")] | length')

    if [[ "$send_count" -ge 1 && "$receive_count" -ge 1 ]]; then
        pass "Found send ($send_count) and receive ($receive_count) frames"
        return 0
    fi

    fail "Expected both send and receive frames (send=$send_count, receive=$receive_count)"
    return 1
}

test_ws_connection_list() {
    log_test "WebSocket connection list API"

    local connections
    connections=$(list_websocket_connections)
    if echo "$connections" | jq -e 'type == "array" or type == "object"' >/dev/null 2>&1; then
        pass "Connection list API returned valid JSON"
        return 0
    fi

    fail "Invalid connection list response: $connections"
    return 1
}

run_all_tests() {
    check_deps
    start_ws_server
    start_bifrost

    test_ws_text_frame_forwarding || true
    test_ws_frames_capture || true
    test_ws_frame_directions || true
    test_ws_connection_list || true

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📊 TEST SUMMARY"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  Passed:  $TESTS_PASSED"
    echo "  Failed:  $TESTS_FAILED"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    if [[ $TESTS_FAILED -gt 0 ]]; then
        exit 1
    fi
}

run_all_tests
