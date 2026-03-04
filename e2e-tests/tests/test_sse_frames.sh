#!/bin/bash

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
    sse_fetch_all "$SSE_TARGET" "/sse/json" 5 > /dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/sse/json")

    if [[ -z "$traffic_id" ]]; then
        log_fail "No SSE traffic found"
        return 1
    fi

    local frames
    frames=$(get_frames "$ADMIN_HOST" "$ADMIN_PORT" "$traffic_id")

    local has_content=false
    local frame_data

    for i in $(seq 0 9); do
        frame_data=$(echo "$frames" | jq -r ".frames[$i].payload_preview // empty")
        if [[ -n "$frame_data" && "$frame_data" != "null" ]]; then
            has_content=true
            break
        fi
    done

    if [[ "$has_content" != "true" ]]; then
        log_fail "SSE frames should contain data"
        log_debug "Frames: $frames"
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
