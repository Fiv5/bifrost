#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test_utils/sse_client.sh"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"

PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${PROXY_PORT:-9900}"
ADMIN_HOST="${ADMIN_HOST:-127.0.0.1}"
ADMIN_PORT="${ADMIN_PORT:-9900}"
SSE_HOST="${SSE_HOST:-127.0.0.1}"
SSE_PORT="${SSE_PORT:-8767}"
ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
export ADMIN_PATH_PREFIX

SSE_PROXY="http://${PROXY_HOST}:${PROXY_PORT}"
SSE_TARGET="http://${SSE_HOST}:${SSE_PORT}"
export SSE_PROXY

TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

log_info() { echo "[INFO] $*"; }
log_pass() { echo "[PASS] $*"; }
log_fail() { echo "[FAIL] $*"; }
log_debug() { [[ "${DEBUG:-0}" == "1" ]] && echo "[DEBUG] $*"; }

BIFROST_DATA_DIR=""
BIFROST_PID=""
SSE_SERVER_PID=""

cleanup() {
    log_info "Cleaning up..."

    if [[ -n "$BIFROST_PID" ]] && kill -0 "$BIFROST_PID" 2>/dev/null; then
        kill "$BIFROST_PID" 2>/dev/null || true
        wait "$BIFROST_PID" 2>/dev/null || true
    fi

    if [[ -n "$SSE_SERVER_PID" ]] && kill -0 "$SSE_SERVER_PID" 2>/dev/null; then
        kill "$SSE_SERVER_PID" 2>/dev/null || true
        wait "$SSE_SERVER_PID" 2>/dev/null || true
    fi

    if [[ -n "$BIFROST_DATA_DIR" && -d "$BIFROST_DATA_DIR" ]]; then
        rm -rf "$BIFROST_DATA_DIR"
    fi

    sse_cleanup_all 2>/dev/null || true
}

trap cleanup EXIT

start_sse_server() {
    log_info "Starting SSE echo server on port $SSE_PORT..."
    python3 "$SCRIPT_DIR/../mock_servers/sse_echo_server.py" --port "$SSE_PORT" > /dev/null 2>&1 &
    SSE_SERVER_PID=$!

    sleep 1
    if ! kill -0 "$SSE_SERVER_PID" 2>/dev/null; then
        log_fail "Failed to start SSE server"
        return 1
    fi

    if ! curl -s "${SSE_TARGET}/health" >/dev/null 2>&1; then
        log_fail "SSE server health check failed"
        return 1
    fi
    return 0
}

start_bifrost() {
    log_info "Building bifrost binary..."
    (cd "$SCRIPT_DIR/../.." && cargo build --bin bifrost > /dev/null 2>&1) || {
        log_fail "Failed to build bifrost"
        return 1
    }

    log_info "Starting Bifrost proxy on port $PROXY_PORT..."
    BIFROST_DATA_DIR="$(mktemp -d)"
    export BIFROST_DATA_DIR

    (cd "$SCRIPT_DIR/../.." && BIFROST_DATA_DIR="$BIFROST_DATA_DIR" cargo run --bin bifrost -- -p "$PROXY_PORT" start --skip-cert-check --unsafe-ssl > /dev/null 2>&1) &
    BIFROST_PID=$!

    local max_wait=60
    local waited=0
    while [[ $waited -lt $max_wait ]]; do
        if curl -sf "http://${ADMIN_HOST}:${ADMIN_PORT}${ADMIN_PATH_PREFIX}/api/system" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    log_fail "Failed to start Bifrost proxy"
    return 1
}

assert_equals() {
    local expected="$1"
    local actual="$2"
    local msg="${3:-Values should be equal}"

    if [[ "$expected" == "$actual" ]]; then
        return 0
    else
        log_fail "$msg: expected '$expected', got '$actual'"
        return 1
    fi
}

assert_contains() {
    local haystack="$1"
    local needle="$2"
    local msg="${3:-Should contain substring}"

    if [[ "$haystack" == *"$needle"* ]]; then
        return 0
    else
        log_fail "$msg: '$needle' not found in '$haystack'"
        return 1
    fi
}

assert_greater_than() {
    local actual="$1"
    local threshold="$2"
    local msg="${3:-Value should be greater than threshold}"

    if [[ "$actual" -gt "$threshold" ]]; then
        return 0
    else
        log_fail "$msg: $actual is not greater than $threshold"
        return 1
    fi
}

run_test() {
    local test_name="$1"
    local test_func="$2"

    TESTS_RUN=$((TESTS_RUN + 1))
    log_info "Running test: $test_name"

    if $test_func; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        log_pass "$test_name"
        return 0
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        log_fail "$test_name"
        return 1
    fi
}

test_sse_basic_events() {
    local events
    local event_count

    events=$(sse_fetch_all "$SSE_TARGET" "/sse?count=3" 5)
    if [[ $? -ne 0 ]]; then
        log_fail "Failed to fetch SSE events"
        return 1
    fi

    event_count=$(echo "$events" | grep -c "^data:" 2>/dev/null | tr -d '[:space:]' || echo "0")

    if ! assert_greater_than "$event_count" 0 "Should receive at least one SSE event"; then
        log_debug "Events received: $events"
        return 1
    fi

    if ! assert_contains "$events" "data:" "Events should contain data fields"; then
        return 1
    fi

    return 0
}

test_sse_custom_events() {
    local events

    events=$(sse_fetch_all "$SSE_TARGET" "/sse/custom?count=2&interval=0.1" 5)
    if [[ $? -ne 0 ]]; then
        log_fail "Failed to fetch custom SSE events"
        return 1
    fi

    local event_count
    event_count=$(echo "$events" | grep -c "^event:" 2>/dev/null | tr -d '[:space:]' || echo "0")

    if ! assert_greater_than "$event_count" 0 "Should receive custom events with event type"; then
        log_debug "Events received: $events"
        return 1
    fi

    return 0
}

test_sse_multiline_data() {
    local events

    events=$(sse_fetch_all "$SSE_TARGET" "/sse/multiline" 5)
    if [[ $? -ne 0 ]]; then
        log_fail "Failed to fetch multiline SSE events"
        return 1
    fi

    local data_lines
    data_lines=$(echo "$events" | grep -c "^data:" 2>/dev/null | tr -d '[:space:]' || echo "0")

    if ! assert_greater_than "$data_lines" 1 "Multiline events should have multiple data lines"; then
        log_debug "Events received: $events"
        return 1
    fi

    return 0
}

test_sse_json_events() {
    local events

    events=$(sse_fetch_all "$SSE_TARGET" "/sse/json" 5)
    if [[ $? -ne 0 ]]; then
        log_fail "Failed to fetch JSON SSE events"
        return 1
    fi

    if ! assert_contains "$events" "{" "JSON events should contain JSON data"; then
        log_debug "Events received: $events"
        return 1
    fi

    if ! assert_contains "$events" "}" "JSON events should contain complete JSON"; then
        return 1
    fi

    return 0
}

test_sse_frames_capture() {
    sse_fetch_all "$SSE_TARGET" "/sse?count=3" 5 > /dev/null 2>&1

    sleep 1

    local traffic_list
    traffic_list=$(get_traffic_list "$ADMIN_HOST" "$ADMIN_PORT" 20)
    if [[ $? -ne 0 ]]; then
        log_fail "Failed to get traffic list"
        return 1
    fi

    local traffic_id
    traffic_id=$(echo "$traffic_list" | jq -r '.records[] | select((.url // .p // "") | contains("/sse")) | .id' | head -1)

    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        log_fail "No SSE traffic found in traffic list"
        log_debug "Traffic list: $traffic_list"
        return 1
    fi

    local record
    record=$(get_traffic_detail "$traffic_id")
    if [[ $? -ne 0 || -z "$record" ]]; then
        log_fail "Failed to get traffic detail for $traffic_id"
        return 1
    fi

    local is_sse
    is_sse=$(echo "$record" | jq -r '.is_sse // false')

    if ! assert_equals "true" "$is_sse" "Traffic should be marked as SSE"; then
        log_debug "Record: $record"
        return 1
    fi

    return 0
}

test_sse_frames_api() {
    sse_fetch_all "$SSE_TARGET" "/sse/custom?count=5&interval=0.05" 5 > /dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/sse/custom")

    if [[ -z "$traffic_id" ]]; then
        log_fail "No SSE traffic found"
        return 1
    fi

    local frames
    frames=$(get_frames "$ADMIN_HOST" "$ADMIN_PORT" "$traffic_id")
    if [[ $? -ne 0 ]]; then
        log_fail "Failed to get frames for traffic $traffic_id"
        return 1
    fi

    local frame_count
    frame_count=$(echo "$frames" | jq -r '.frames | length')

    if ! assert_greater_than "$frame_count" 0 "Should have captured SSE frames"; then
        log_debug "Frames response: $frames"
        return 1
    fi

    local first_frame_type
    first_frame_type=$(echo "$frames" | jq -r '.frames[0].frame_type')

    if ! assert_equals "sse" "$first_frame_type" "Frame type should be sse"; then
        return 1
    fi

    return 0
}

test_sse_frame_content() {
    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    sse_fetch_all "$SSE_TARGET" "/sse/custom?count=20&interval=0.05" 5 > /dev/null 2>&1
    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/sse/custom")
    if [[ -z "$traffic_id" || "$traffic_id" == "null" ]]; then
        log_fail "No SSE traffic found"
        return 1
    fi

    local frames
    frames=$(get_frames "$ADMIN_HOST" "$ADMIN_PORT" "$traffic_id")
    local frame_count
    frame_count=$(echo "$frames" | jq -r '.frames | length')
    if [[ "${frame_count:-0}" -le 0 ]]; then
        log_fail "No SSE frames recorded"
        return 1
    fi

    local has_payload
    has_payload=$(echo "$frames" | jq -r '[.frames[] | select((.payload_size // 0) > 0)] | length')
    if [[ "${has_payload:-0}" -le 0 ]]; then
        log_fail "SSE frames should have payload_size"
        return 1
    fi

    local first_frame_id
    first_frame_id=$(echo "$frames" | jq -r '.frames[0].frame_id // 0')
    if [[ "${first_frame_id:-0}" -le 0 ]]; then
        log_fail "SSE frame_id should be available"
        return 1
    fi
    local frame_detail
    frame_detail=$(get_frame_detail "$traffic_id" "$first_frame_id")
    local full_payload
    full_payload=$(echo "$frame_detail" | jq -r '.full_payload // ""')
    if [[ -z "$full_payload" ]]; then
        log_debug "Frame detail: $frame_detail"
        log_fail "SSE frame detail should include full_payload"
        return 1
    fi
    local record
    record=$(get_traffic_detail "$traffic_id")
    local response_size
    response_size=$(echo "$record" | jq -r '.response_size // 0')
    if [[ "${response_size:-0}" -le 0 ]]; then
        log_debug "Record: $record"
        log_fail "SSE response_size should be persisted"
        return 1
    fi
    local socket_bytes
    socket_bytes=$(echo "$record" | jq -r '(.socket_status.send_bytes // 0) + (.socket_status.receive_bytes // 0)')
    if [[ "${socket_bytes:-0}" -le 0 ]]; then
        log_fail "SSE socket_status bytes should be persisted"
        return 1
    fi

    return 0
}

test_sse_frame_direction() {
    sse_fetch_all "$SSE_TARGET" "/sse?count=2" 5 > /dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/sse")

    if [[ -z "$traffic_id" ]]; then
        log_fail "No SSE traffic found"
        return 1
    fi

    local frames
    frames=$(get_frames "$ADMIN_HOST" "$ADMIN_PORT" "$traffic_id")

    local first_direction
    first_direction=$(echo "$frames" | jq -r '.frames[0].direction')

    if ! assert_equals "receive" "$first_direction" "SSE frames should be server-to-client (receive)"; then
        log_debug "Frames: $frames"
        return 1
    fi

    return 0
}

test_sse_traffic_identification() {
    sse_fetch_all "$SSE_TARGET" "/sse?count=1" 5 > /dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/sse")

    if [[ -z "$traffic_id" ]]; then
        log_fail "No SSE traffic found"
        return 1
    fi

    if ! is_sse_traffic "$ADMIN_HOST" "$ADMIN_PORT" "$traffic_id"; then
        log_fail "Traffic should be identified as SSE"
        return 1
    fi

    return 0
}

test_sse_vs_websocket_distinction() {
    sse_fetch_all "$SSE_TARGET" "/sse?count=1" 5 > /dev/null 2>&1

    sleep 1

    local traffic_list
    traffic_list=$(get_traffic_list "$ADMIN_HOST" "$ADMIN_PORT" 50)

    local sse_records
    local ws_records

    sse_records=$(echo "$traffic_list" | jq '[.records[] | select(.is_sse == true)] | length')
    ws_records=$(echo "$traffic_list" | jq '[.records[] | select(.is_websocket == true)] | length')

    local overlap
    overlap=$(echo "$traffic_list" | jq '[.records[] | select(.is_sse == true and .is_websocket == true)] | length')

    if ! assert_equals "0" "$overlap" "No traffic should be both SSE and WebSocket"; then
        log_debug "Traffic list: $traffic_list"
        return 1
    fi

    return 0
}

print_summary() {
    echo ""
    echo "======================================"
    echo "SSE Frames Test Summary"
    echo "======================================"
    echo "Tests Run:    $TESTS_RUN"
    echo "Tests Passed: $TESTS_PASSED"
    echo "Tests Failed: $TESTS_FAILED"
    echo "======================================"

    if [[ $TESTS_FAILED -eq 0 ]]; then
        echo "All tests passed!"
        return 0
    else
        echo "Some tests failed!"
        return 1
    fi
}

main() {
    log_info "Starting SSE Frames Tests"
    log_info "Proxy: $PROXY_HOST:$PROXY_PORT"
    log_info "Admin: $ADMIN_HOST:$ADMIN_PORT"
    log_info "SSE Server: $SSE_HOST:$SSE_PORT"
    echo ""

    start_sse_server
    start_bifrost

    clear_traffic >/dev/null 2>&1 || true
    sleep 0.5

    run_test "SSE Basic Events" test_sse_basic_events
    run_test "SSE Custom Events" test_sse_custom_events
    run_test "SSE Multiline Data" test_sse_multiline_data
    run_test "SSE JSON Events" test_sse_json_events
    run_test "SSE Frames Capture" test_sse_frames_capture
    run_test "SSE Frames API" test_sse_frames_api
    run_test "SSE Frame Content" test_sse_frame_content
    run_test "SSE Frame Direction" test_sse_frame_direction
    run_test "SSE Traffic Identification" test_sse_traffic_identification
    run_test "SSE vs WebSocket Distinction" test_sse_vs_websocket_distinction

    print_summary
    exit $?
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
