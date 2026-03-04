#!/bin/bash
# WebSocket Frames 端到端测试

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

source "$SCRIPT_DIR/../test_utils/assert.sh"
source "$SCRIPT_DIR/../test_utils/ws_client.sh"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"

PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${PROXY_PORT:-9900}"
WS_HOST="${WS_HOST:-127.0.0.1}"
WS_PORT="${WS_PORT:-8766}"
WS_SERVER="ws://${WS_HOST}:${WS_PORT}"
WS_PROXY_URL="ws://${PROXY_HOST}:${PROXY_PORT}"
ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
export ADMIN_PATH_PREFIX

TESTS_PASSED=0
TESTS_FAILED=0
TESTS_SKIPPED=0

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

skip() {
    echo "⏭️ SKIP: $1"
    TESTS_SKIPPED=$((TESTS_SKIPPED + 1))
}

check_deps() {
    if ! command -v websocat &> /dev/null; then
        echo "Error: websocat is required. Install with: brew install websocat"
        exit 1
    fi
    if ! command -v jq &> /dev/null; then
        echo "Error: jq is required. Install with: brew install jq"
        exit 1
    fi
}

test_ws_text_frame_forwarding() {
    log_test "WebSocket text frame forwarding"

    local test_msg='{"test": "hello world"}'
    local conn_id
    conn_id=$(ws_connect "$WS_SERVER/ws" "" 2>/dev/null) || true
    if [[ -z "$conn_id" ]]; then
        fail "Failed to establish WebSocket connection"
        return 1
    fi

    ws_send "$conn_id" "$test_msg" 2>/dev/null || true

    local messages
    messages=$(ws_wait_messages "$conn_id" 2 5 2>/dev/null) || true
    ws_close "$conn_id" 2>/dev/null || true

    local echo_line
    echo_line=$(echo "$messages" | tail -n 1 | tr -d '\r')

    if [[ -z "$echo_line" ]]; then
        fail "No response received"
        return 1
    fi

    if echo "$echo_line" | jq -e '.type == "echo"' >/dev/null 2>&1; then
        pass "Text frame forwarded and echoed correctly"
        return 0
    else
        fail "Unexpected response: $echo_line"
        return 1
    fi
}

test_ws_broadcast_mode() {
    log_test "WebSocket broadcast mode"

    local response
    response=$(ws_connect_recv_all "$WS_SERVER/ws/broadcast" 10 2>/dev/null)

    local broadcast_count
    broadcast_count=$(echo "$response" | grep -c '"type":"broadcast"' || true)

    if [[ "$broadcast_count" -ge 5 ]]; then
        pass "Received $broadcast_count broadcast messages"
        return 0
    else
        fail "Expected 5 broadcast messages, got $broadcast_count"
        echo "Response: $response"
        return 1
    fi
}

test_ws_frames_capture() {
    log_test "WebSocket frames capture in traffic monitor"

    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    local response
    response=$(ws_connect_recv_all "$WS_SERVER/ws/broadcast" 10 2>/dev/null)

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "ws/broadcast" 20)

    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        fail "No WebSocket traffic recorded"
        return 1
    fi

    local is_ws
    is_ws=$(is_websocket_traffic "$traffic_id")

    if [[ "$is_ws" != "true" ]]; then
        fail "Traffic not marked as WebSocket (is_websocket=$is_ws)"
        return 1
    fi

    local frame_count
    frame_count=$(get_frame_count "$traffic_id")

    if [[ "$frame_count" -ge 5 ]]; then
        pass "Captured $frame_count frames (expected >= 5)"
        return 0
    else
        fail "Expected >= 5 frames, got $frame_count"
        return 1
    fi
}

test_ws_frames_api() {
    log_test "WebSocket frames API"

    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    ws_connect_recv_all "$WS_SERVER/ws/broadcast" 10 >/dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "ws/broadcast" 20)

    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        skip "No traffic ID found"
        return 0
    fi

    local frames_response
    frames_response=$(get_frames "$traffic_id")

    if ! echo "$frames_response" | jq -e '.frames' >/dev/null 2>&1; then
        fail "Invalid frames response: $frames_response"
        return 1
    fi

    local first_frame_id
    first_frame_id=$(echo "$frames_response" | jq -r '.frames[0].frame_id')

    if [[ -z "$first_frame_id" || "$first_frame_id" == "null" ]]; then
        fail "No frame_id in response"
        return 1
    fi

    local frame_types
    frame_types=$(echo "$frames_response" | jq -r '.frames[].frame_type' | sort | uniq)

    if echo "$frame_types" | grep -q "text"; then
        pass "Frames API returned valid data with text frames"
        return 0
    else
        fail "No text frames found in response"
        return 1
    fi
}

test_ws_frame_directions() {
    log_test "WebSocket frame directions"

    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    local test_msg='{"ping": "test"}'
    ws_send_recv "$WS_SERVER/ws" "$test_msg" 5 >/dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "/ws" 20)

    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        skip "No traffic ID found"
        return 0
    fi

    local frames_response
    frames_response=$(get_frames "$traffic_id")

    local send_count receive_count
    send_count=$(echo "$frames_response" | jq '[.frames[] | select(.direction == "send")] | length')
    receive_count=$(echo "$frames_response" | jq '[.frames[] | select(.direction == "receive")] | length')

    if [[ "$send_count" -ge 1 && "$receive_count" -ge 1 ]]; then
        pass "Found send ($send_count) and receive ($receive_count) frames"
        return 0
    else
        fail "Expected both send and receive frames (send=$send_count, receive=$receive_count)"
        return 1
    fi
}

test_ws_connection_list() {
    log_test "WebSocket connection list API"

    local connections
    connections=$(list_websocket_connections)

    if echo "$connections" | jq -e 'type == "array" or type == "object"' >/dev/null 2>&1; then
        pass "Connection list API returned valid JSON"
        return 0
    else
        fail "Invalid connection list response: $connections"
        return 1
    fi
}

run_all_tests() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║            WebSocket Frames E2E Test Suite                   ║"
    echo "╠══════════════════════════════════════════════════════════════╣"
    echo "║  Proxy: ${PROXY_HOST}:${PROXY_PORT}                                        ║"
    echo "║  WS Server: ${WS_SERVER}                              ║"
    echo "║  Admin: ${ADMIN_BASE_URL}                           ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""

    check_deps

    if ! nc -z "$PROXY_HOST" "$PROXY_PORT" 2>/dev/null; then
        echo "❌ Proxy server not running at ${PROXY_HOST}:${PROXY_PORT}"
        exit 1
    fi

    if ! nc -z "127.0.0.1" "$WS_PORT" 2>/dev/null; then
        echo "❌ WebSocket server not running at 127.0.0.1:${WS_PORT}"
        exit 1
    fi

    test_ws_text_frame_forwarding || true
    test_ws_broadcast_mode || true
    test_ws_frames_capture || true
    test_ws_frames_api || true
    test_ws_frame_directions || true
    test_ws_connection_list || true

    ws_cleanup_all 2>/dev/null || true

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📊 TEST SUMMARY"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  Passed:  $TESTS_PASSED"
    echo "  Failed:  $TESTS_FAILED"
    echo "  Skipped: $TESTS_SKIPPED"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    if [[ $TESTS_FAILED -gt 0 ]]; then
        exit 1
    fi
}

case "${1:-all}" in
    all)
        run_all_tests
        ;;
    text)
        test_ws_text_frame_forwarding
        ;;
    broadcast)
        test_ws_broadcast_mode
        ;;
    capture)
        test_ws_frames_capture
        ;;
    api)
        test_ws_frames_api
        ;;
    directions)
        test_ws_frame_directions
        ;;
    connections)
        test_ws_connection_list
        ;;
    *)
        echo "Usage: $0 {all|text|broadcast|capture|api|directions|connections}"
        exit 1
        ;;
esac
