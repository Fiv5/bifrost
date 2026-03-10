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

ADMIN_HOST_HEADER="${ADMIN_HOST}:${ADMIN_PORT}"
WS_BASE_URL="ws://${WS_HOST}:${WS_PORT}"

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
    (cd "$ROOT_DIR" && SKIP_FRONTEND_BUILD=1 cargo build --bin bifrost)

    BIFROST_DATA_DIR="$(mktemp -d)"
    export BIFROST_DATA_DIR

    local log_file="$BIFROST_DATA_DIR/proxy.log"
    (cd "$ROOT_DIR" && SKIP_FRONTEND_BUILD=1 BIFROST_DATA_DIR="$BIFROST_DATA_DIR" cargo run --bin bifrost -- -p "$PROXY_PORT" start --skip-cert-check --unsafe-ssl > "$log_file" 2>&1) &
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

ws_replay_generate_echo_traffic() {
    local path_suffix="${1:-/ws}"
    local messages="${2:-3}"
    local url="${WS_BASE_URL}${path_suffix}"
    local encoded
    encoded="$(python3 -c "import urllib.parse; print(urllib.parse.quote('''$url''', safe=''))")"
    python3 "$SCRIPT_DIR/../test_utils/ws_stress_client.py" \
        --proxy-host "$ADMIN_HOST" \
        --proxy-port "$ADMIN_PORT" \
        --host-header "$ADMIN_HOST_HEADER" \
        --path "${ADMIN_PATH_PREFIX}/api/replay/execute/ws?url=${encoded}" \
        --messages "$messages" \
        --timeout 15.0
}

test_ws_replay_echo_forwarding() {
    log_test "Replay WebSocket echo forwarding"
    if ws_replay_generate_echo_traffic "/ws" 2; then
        pass "Echo frames forwarded via replay"
        return 0
    fi
    fail "Failed to forward echo frames via replay"
    return 1
}

test_ws_replay_frames_capture() {
    log_test "Replay WebSocket frames capture"

    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    ws_replay_generate_echo_traffic "/ws" 3 >/dev/null 2>&1 || true
    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws" 20)
    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        fail "No WebSocket replay traffic recorded"
        return 1
    fi

    local record
    record=$(get_traffic_detail "$traffic_id")
    local is_replay is_ws status protocol
    is_replay=$(echo "$record" | jq -r '.is_replay // false')
    is_ws=$(echo "$record" | jq -r '.is_websocket // false')
    status=$(echo "$record" | jq -r '.status // 0')
    protocol=$(echo "$record" | jq -r '.protocol // ""')

    if [[ "$is_replay" != "true" ]]; then
        fail "Traffic not marked as replay"
        return 1
    fi
    if [[ "$is_ws" != "true" ]]; then
        fail "Traffic not marked as WebSocket"
        return 1
    fi
    if [[ "${status:-0}" -ne 101 ]]; then
        fail "Replay WebSocket status should be 101, got ${status}"
        return 1
    fi
    if [[ "$protocol" != "ws" && "$protocol" != "wss" ]]; then
        fail "Replay WebSocket protocol should be ws/wss, got ${protocol}"
        return 1
    fi

    local frames_response
    frames_response=$(get_frames "$traffic_id")
    local frame_count
    frame_count=$(echo "$frames_response" | jq -r '.frames | length')
    if [[ "$frame_count" -lt 1 ]]; then
        fail "Expected frames, got ${frame_count}"
        return 1
    fi

    local send_count receive_count
    send_count=$(echo "$frames_response" | jq '[.frames[] | select(.direction == "send")] | length')
    receive_count=$(echo "$frames_response" | jq '[.frames[] | select(.direction == "receive")] | length')
    if [[ "$send_count" -lt 1 || "$receive_count" -lt 1 ]]; then
        fail "Expected send and receive frames, got send=${send_count}, receive=${receive_count}"
        return 1
    fi

    local response_size
    response_size=$(echo "$record" | jq -r '.response_size // 0')
    if [[ "${response_size:-0}" -le 0 ]]; then
        fail "Replay WebSocket response_size should be persisted"
        return 1
    fi
    local socket_bytes
    socket_bytes=$(echo "$record" | jq -r '(.socket_status.send_bytes // 0) + (.socket_status.receive_bytes // 0)')
    if [[ "${socket_bytes:-0}" -le 0 ]]; then
        fail "Replay WebSocket socket_status bytes should be persisted"
        return 1
    fi

    pass "Captured ${frame_count} frames via replay"
    return 0
}

test_ws_replay_ping_pong_capture() {
    log_test "Replay WebSocket ping/pong forwarding and capture"

    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    ws_replay_generate_echo_traffic "/ws/ping" 3 >/dev/null 2>&1 || true
    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws/ping" 20)
    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        fail "No traffic ID found"
        return 1
    fi

    local types
    types=$(get_frame_types "$traffic_id")
    if ! echo "$types" | grep -q "ping"; then
        fail "Expected Ping frame type"
        return 1
    fi
    if ! echo "$types" | grep -q "pong"; then
        fail "Expected Pong frame type"
        return 1
    fi

    pass "Observed ping/pong frames via replay"
    return 0
}

main() {
    check_deps
    start_ws_server
    start_bifrost

    test_ws_replay_echo_forwarding || true
    test_ws_replay_frames_capture || true
    test_ws_replay_ping_pong_capture || true

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Replay WebSocket E2E Results: PASSED=$TESTS_PASSED FAILED=$TESTS_FAILED"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    if [[ $TESTS_FAILED -gt 0 ]]; then
        exit 1
    fi
}

main "$@"
