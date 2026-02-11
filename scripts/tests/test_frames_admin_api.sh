#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test_utils/ws_client.sh"
source "$SCRIPT_DIR/../test_utils/sse_client.sh"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"

PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${PROXY_PORT:-8899}"
ADMIN_HOST="${ADMIN_HOST:-127.0.0.1}"
ADMIN_PORT="${ADMIN_PORT:-8899}"
WS_HOST="${WS_HOST:-127.0.0.1}"
WS_PORT="${WS_PORT:-8766}"
SSE_HOST="${SSE_HOST:-127.0.0.1}"
SSE_PORT="${SSE_PORT:-8767}"
ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
export ADMIN_PATH_PREFIX

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

assert_not_empty() {
    local value="$1"
    local msg="${2:-Value should not be empty}"

    if [[ -n "$value" && "$value" != "null" ]]; then
        return 0
    else
        log_fail "$msg: value is empty or null"
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

assert_json_field() {
    local json="$1"
    local field="$2"
    local expected="$3"
    local msg="${4:-JSON field should match}"

    local actual
    actual=$(echo "$json" | jq -r "$field")

    if [[ "$actual" == "$expected" ]]; then
        return 0
    else
        log_fail "$msg: field '$field' expected '$expected', got '$actual'"
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

test_traffic_list_api() {
    local response
    response=$(curl -s "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/traffic?limit=10")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call traffic list API"
        return 1
    fi

    if ! assert_not_empty "$response" "Traffic list response should not be empty"; then
        return 1
    fi

    local has_records
    has_records=$(echo "$response" | jq 'has("records")')

    if ! assert_equals "true" "$has_records" "Response should have records field"; then
        log_debug "Response: $response"
        return 1
    fi

    return 0
}

test_traffic_list_pagination() {
    local response1
    local response2

    response1=$(curl -s "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/traffic?limit=5&offset=0")
    response2=$(curl -s "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/traffic?limit=5&offset=5")

    local count1
    local count2
    count1=$(echo "$response1" | jq '.records | length')
    count2=$(echo "$response2" | jq '.records | length')

    if [[ "$count1" -le 5 ]] && [[ "$count2" -le 5 ]]; then
        return 0
    else
        log_fail "Pagination not working correctly"
        return 1
    fi
}

test_frames_api_structure() {
    ws_send_recv "ws://$PROXY_HOST:$PROXY_PORT/ws" "test_admin_api" > /dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws")

    if [[ -z "$traffic_id" ]]; then
        log_fail "No WebSocket traffic found"
        return 1
    fi

    local response
    response=$(curl -s "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/traffic/$traffic_id/frames")

    if ! assert_not_empty "$response" "Frames response should not be empty"; then
        return 1
    fi

    local has_frames
    has_frames=$(echo "$response" | jq 'has("frames")')

    if ! assert_equals "true" "$has_frames" "Response should have frames field"; then
        log_debug "Response: $response"
        return 1
    fi

    local has_total
    has_total=$(echo "$response" | jq 'has("total")')

    if ! assert_equals "true" "$has_total" "Response should have total field"; then
        return 1
    fi

    return 0
}

test_frames_api_frame_fields() {
    ws_send_recv "ws://$PROXY_HOST:$PROXY_PORT/ws" "test_frame_fields" > /dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws")

    if [[ -z "$traffic_id" ]]; then
        log_fail "No WebSocket traffic found"
        return 1
    fi

    local frames
    frames=$(get_frames "$ADMIN_HOST" "$ADMIN_PORT" "$traffic_id")

    local frame_count
    frame_count=$(echo "$frames" | jq '.frames | length')

    if [[ "$frame_count" -eq 0 ]]; then
        log_fail "No frames found"
        return 1
    fi

    local first_frame
    first_frame=$(echo "$frames" | jq '.frames[0]')

    local has_id has_direction has_timestamp has_frame_type
    has_id=$(echo "$first_frame" | jq 'has("id")')
    has_direction=$(echo "$first_frame" | jq 'has("direction")')
    has_timestamp=$(echo "$first_frame" | jq 'has("timestamp")')
    has_frame_type=$(echo "$first_frame" | jq 'has("frame_type")')

    if ! assert_equals "true" "$has_id" "Frame should have id field"; then
        return 1
    fi

    if ! assert_equals "true" "$has_direction" "Frame should have direction field"; then
        return 1
    fi

    if ! assert_equals "true" "$has_timestamp" "Frame should have timestamp field"; then
        return 1
    fi

    if ! assert_equals "true" "$has_frame_type" "Frame should have frame_type field"; then
        return 1
    fi

    return 0
}

test_frames_api_pagination() {
    for i in $(seq 1 10); do
        ws_send_recv "ws://$PROXY_HOST:$PROXY_PORT/ws" "message_$i" > /dev/null 2>&1
    done

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws")

    if [[ -z "$traffic_id" ]]; then
        log_fail "No WebSocket traffic found"
        return 1
    fi

    local response1
    response1=$(curl -s "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/traffic/$traffic_id/frames?limit=5")

    local count1
    count1=$(echo "$response1" | jq '.frames | length')

    if [[ "$count1" -gt 5 ]]; then
        log_fail "Pagination limit not respected: got $count1 frames"
        return 1
    fi

    return 0
}

test_frames_api_invalid_traffic_id() {
    local response
    response=$(curl -s -w "\n%{http_code}" "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/traffic/invalid_id_12345/frames")

    local http_code
    http_code=$(echo "$response" | tail -1)

    if [[ "$http_code" == "404" ]] || [[ "$http_code" == "400" ]]; then
        return 0
    fi

    local body
    body=$(echo "$response" | head -n -1)
    local frames_count
    frames_count=$(echo "$body" | jq '.frames | length // 0')

    if [[ "$frames_count" -eq 0 ]]; then
        return 0
    fi

    log_fail "Expected error for invalid traffic ID, got HTTP $http_code"
    log_debug "Response: $response"
    return 1
}

test_websocket_connections_list() {
    local response
    response=$(curl -s "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/websocket/connections")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call WebSocket connections API"
        return 1
    fi

    if ! assert_not_empty "$response" "Connections response should not be empty"; then
        return 1
    fi

    local has_connections
    has_connections=$(echo "$response" | jq 'has("connections")')

    if ! assert_equals "true" "$has_connections" "Response should have connections field"; then
        log_debug "Response: $response"
        return 1
    fi

    return 0
}

test_traffic_record_ws_fields() {
    ws_send_recv "ws://$PROXY_HOST:$PROXY_PORT/ws" "test_ws_fields" > /dev/null 2>&1

    sleep 1

    local traffic_list
    traffic_list=$(get_traffic_list "$ADMIN_HOST" "$ADMIN_PORT" 10)

    local ws_record
    ws_record=$(echo "$traffic_list" | jq '.records[] | select(.url | contains("/ws")) | select(.is_websocket == true)' | head -1)

    if [[ -z "$ws_record" || "$ws_record" == "null" ]]; then
        log_fail "No WebSocket traffic record found"
        return 1
    fi

    local is_ws
    is_ws=$(echo "$ws_record" | jq -r '.is_websocket')

    if ! assert_equals "true" "$is_ws" "Traffic should be marked as WebSocket"; then
        return 1
    fi

    return 0
}

test_traffic_record_sse_fields() {
    sse_fetch_all "http://$PROXY_HOST:$PROXY_PORT" "/sse?count=2" 5 > /dev/null 2>&1

    sleep 1

    local traffic_list
    traffic_list=$(get_traffic_list "$ADMIN_HOST" "$ADMIN_PORT" 10)

    local sse_record
    sse_record=$(echo "$traffic_list" | jq '.records[] | select(.url | contains("/sse")) | select(.is_sse == true)' | head -1)

    if [[ -z "$sse_record" || "$sse_record" == "null" ]]; then
        log_fail "No SSE traffic record found"
        log_debug "Traffic list: $traffic_list"
        return 1
    fi

    local is_sse
    is_sse=$(echo "$sse_record" | jq -r '.is_sse')

    if ! assert_equals "true" "$is_sse" "Traffic should be marked as SSE"; then
        return 1
    fi

    return 0
}

test_frame_direction_values() {
    ws_send_recv "ws://$PROXY_HOST:$PROXY_PORT/ws" "test_direction" > /dev/null 2>&1

    sleep 1

    local traffic_id
    traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws")

    if [[ -z "$traffic_id" ]]; then
        log_fail "No WebSocket traffic found"
        return 1
    fi

    local frames
    frames=$(get_frames "$ADMIN_HOST" "$ADMIN_PORT" "$traffic_id")

    local directions
    directions=$(echo "$frames" | jq -r '.frames[].direction' | sort -u)

    local valid=true
    while IFS= read -r dir; do
        if [[ "$dir" != "ClientToServer" && "$dir" != "ServerToClient" ]]; then
            log_fail "Invalid direction: $dir"
            valid=false
        fi
    done <<< "$directions"

    if [[ "$valid" != "true" ]]; then
        return 1
    fi

    return 0
}

test_frame_type_values() {
    ws_send_recv "ws://$PROXY_HOST:$PROXY_PORT/ws" "test_type" > /dev/null 2>&1
    sse_fetch_all "http://$PROXY_HOST:$PROXY_PORT" "/sse?count=1" 5 > /dev/null 2>&1

    sleep 1

    local ws_traffic_id
    ws_traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/ws")

    if [[ -n "$ws_traffic_id" ]]; then
        local ws_frames
        ws_frames=$(get_frames "$ADMIN_HOST" "$ADMIN_PORT" "$ws_traffic_id")

        local ws_types
        ws_types=$(echo "$ws_frames" | jq -r '.frames[].frame_type' | sort -u)

        while IFS= read -r type; do
            case "$type" in
                Text|Binary|Ping|Pong|Close)
                    ;;
                *)
                    log_fail "Invalid WebSocket frame type: $type"
                    return 1
                    ;;
            esac
        done <<< "$ws_types"
    fi

    local sse_traffic_id
    sse_traffic_id=$(find_traffic_id_by_url "$ADMIN_HOST" "$ADMIN_PORT" "/sse")

    if [[ -n "$sse_traffic_id" ]]; then
        local sse_frames
        sse_frames=$(get_frames "$ADMIN_HOST" "$ADMIN_PORT" "$sse_traffic_id")

        local sse_type
        sse_type=$(echo "$sse_frames" | jq -r '.frames[0].frame_type')

        if [[ "$sse_type" != "Sse" ]]; then
            log_fail "SSE frame should have type 'Sse', got '$sse_type'"
            return 1
        fi
    fi

    return 0
}

test_concurrent_api_calls() {
    for i in $(seq 1 5); do
        curl -s "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/traffic?limit=10" > /dev/null &
    done

    wait

    local response
    response=$(curl -s "http://$ADMIN_HOST:$ADMIN_PORT${ADMIN_PATH_PREFIX}/api/traffic?limit=10")

    if ! assert_not_empty "$response" "API should respond after concurrent calls"; then
        return 1
    fi

    return 0
}

print_summary() {
    echo ""
    echo "======================================"
    echo "Admin API Test Summary"
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
    log_info "Starting Admin API Tests"
    log_info "Proxy: $PROXY_HOST:$PROXY_PORT"
    log_info "Admin: $ADMIN_HOST:$ADMIN_PORT"
    echo ""

    run_test "Traffic List API" test_traffic_list_api
    run_test "Traffic List Pagination" test_traffic_list_pagination
    run_test "Frames API Structure" test_frames_api_structure
    run_test "Frames API Frame Fields" test_frames_api_frame_fields
    run_test "Frames API Pagination" test_frames_api_pagination
    run_test "Frames API Invalid Traffic ID" test_frames_api_invalid_traffic_id
    run_test "WebSocket Connections List" test_websocket_connections_list
    run_test "Traffic Record WS Fields" test_traffic_record_ws_fields
    run_test "Traffic Record SSE Fields" test_traffic_record_sse_fields
    run_test "Frame Direction Values" test_frame_direction_values
    run_test "Frame Type Values" test_frame_type_values
    run_test "Concurrent API Calls" test_concurrent_api_calls

    print_summary
    exit $?
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
