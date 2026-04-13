#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"

ADMIN_HOST="${ADMIN_HOST:-127.0.0.1}"
ADMIN_PORT="${ADMIN_PORT:-}"
if [[ -z "${ADMIN_PORT}" ]]; then
    ADMIN_PORT="$(allocate_free_port)"
fi
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

assert_json_has_field() {
    local json="$1"
    local field="$2"
    local msg="${3:-JSON should have field}"

    local has_field
    has_field=$(echo "$json" | jq "has(\"$field\")")

    if [[ "$has_field" == "true" ]]; then
        return 0
    else
        log_fail "$msg: field '$field' not found"
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

test_system_info_api() {
    local response
    response=$(get_system_info)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call system info API"
        return 1
    fi

    if ! assert_not_empty "$response" "System info response should not be empty"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "version" "Response should have version field"; then
        log_debug "Response: $response"
        return 1
    fi

    return 0
}

test_system_info_structure() {
    local response
    response=$(get_system_info)

    local version
    version=$(echo "$response" | jq -r '.version')
    if [[ -z "$version" || "$version" == "null" ]]; then
        log_fail "Version should not be empty"
        return 1
    fi

    local port
    port=$(echo "$response" | jq -r '.port // empty')
    if [[ -n "$port" ]]; then
        if ! [[ "$port" =~ ^[0-9]+$ ]]; then
            log_fail "Port should be a number: $port"
            return 1
        fi
    fi

    return 0
}

test_system_overview_api() {
    local response
    response=$(get_system_overview)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call system overview API"
        return 1
    fi

    if ! assert_not_empty "$response" "System overview response should not be empty"; then
        return 1
    fi

    return 0
}

test_system_overview_structure() {
    local response
    response=$(get_system_overview)

    local response_type
    response_type=$(echo "$response" | jq 'type')
    if [[ "$response_type" != "\"object\"" ]]; then
        log_fail "System overview response should be an object"
        return 1
    fi

    return 0
}

test_metrics_api() {
    local response
    response=$(get_metrics)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call metrics API"
        return 1
    fi

    if ! assert_not_empty "$response" "Metrics response should not be empty"; then
        return 1
    fi

    return 0
}

test_metrics_structure() {
    local response
    response=$(get_metrics)

    local response_type
    response_type=$(echo "$response" | jq 'type')
    if [[ "$response_type" != "\"object\"" ]]; then
        log_fail "Metrics response should be an object"
        return 1
    fi

    local timestamp
    timestamp=$(echo "$response" | jq -r '.timestamp // empty')
    if [[ -n "$timestamp" ]]; then
        if ! [[ "$timestamp" =~ ^[0-9]+$ ]]; then
            log_fail "Timestamp should be a number: $timestamp"
            return 1
        fi
    fi

    return 0
}

test_metrics_history_api() {
    local response
    response=$(get_metrics_history 10)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call metrics history API"
        return 1
    fi

    if ! assert_not_empty "$response" "Metrics history response should not be empty"; then
        return 1
    fi

    return 0
}

test_metrics_history_structure() {
    local response
    response=$(get_metrics_history 10)

    local response_type
    response_type=$(echo "$response" | jq 'type')
    if [[ "$response_type" != "\"array\"" ]]; then
        log_fail "Metrics history response should be an array"
        return 1
    fi

    local count
    count=$(echo "$response" | jq 'length')
    if [[ "$count" -gt 10 ]]; then
        log_fail "Metrics history should respect limit: got $count, expected <= 10"
        return 1
    fi

    return 0
}

test_metrics_history_limit() {
    local response5
    response5=$(get_metrics_history 5)

    local count5
    count5=$(echo "$response5" | jq 'length')

    local response20
    response20=$(get_metrics_history 20)

    local count20
    count20=$(echo "$response20" | jq 'length')

    if [[ "$count5" -gt 5 ]]; then
        log_fail "Metrics history with limit 5 returned $count5 items"
        return 1
    fi

    if [[ "$count20" -gt 20 ]]; then
        log_fail "Metrics history with limit 20 returned $count20 items"
        return 1
    fi

    return 0
}

test_system_info_fields() {
    local response
    response=$(get_system_info)

    local fields=("version")
    for field in "${fields[@]}"; do
        local value
        value=$(echo "$response" | jq -r ".$field // empty")
        if [[ -z "$value" ]]; then
            log_debug "Field '$field' is empty or missing"
        fi
    done

    return 0
}

print_summary() {
    echo ""
    echo "======================================"
    echo "System/Metrics Admin API Test Summary"
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
    trap admin_cleanup_bifrost EXIT

    if ! admin_ensure_bifrost; then
        log_fail "Admin server is not reachable and failed to start"
        exit 1
    fi

    log_info "Starting System/Metrics Admin API Tests"
    log_info "Admin: $ADMIN_HOST:$ADMIN_PORT"
    echo ""

    run_test "System Info API" test_system_info_api
    run_test "System Info Structure" test_system_info_structure
    run_test "System Overview API" test_system_overview_api
    run_test "System Overview Structure" test_system_overview_structure
    run_test "Metrics API" test_metrics_api
    run_test "Metrics Structure" test_metrics_structure
    run_test "Metrics History API" test_metrics_history_api
    run_test "Metrics History Structure" test_metrics_history_structure
    run_test "Metrics History Limit" test_metrics_history_limit
    run_test "System Info Fields" test_system_info_fields

    print_summary
    exit $?
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
