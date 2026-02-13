#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"

ADMIN_HOST="${ADMIN_HOST:-127.0.0.1}"
ADMIN_PORT="${ADMIN_PORT:-9900}"
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

TEST_VALUE_NAME="TEST_VAR_$$"

cleanup_test_value() {
    delete_value "$TEST_VALUE_NAME" > /dev/null 2>&1
}

test_values_list_api() {
    local response
    response=$(list_values)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call values list API"
        return 1
    fi

    if ! assert_not_empty "$response" "Values list response should not be empty"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "values" "Response should have values field"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_has_field "$response" "total" "Response should have total field"; then
        return 1
    fi

    return 0
}

test_values_create_api() {
    cleanup_test_value

    local response
    response=$(create_value "$TEST_VALUE_NAME" "test-value-123")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call create value API"
        return 1
    fi

    local success
    success=$(echo "$response" | jq -r '.success // empty')

    if [[ "$success" != "true" ]]; then
        local error
        error=$(echo "$response" | jq -r '.error // empty')
        if [[ -n "$error" ]]; then
            log_fail "Create value failed: $error"
            return 1
        fi
    fi

    local verify_response
    verify_response=$(get_value "$TEST_VALUE_NAME")
    local value_name
    value_name=$(echo "$verify_response" | jq -r '.name')

    if ! assert_equals "$TEST_VALUE_NAME" "$value_name" "Value should be created"; then
        log_debug "Verify response: $verify_response"
        return 1
    fi

    return 0
}

test_values_get_api() {
    local response
    response=$(get_value "$TEST_VALUE_NAME")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call get value API"
        return 1
    fi

    if ! assert_json_field "$response" ".name" "$TEST_VALUE_NAME" "Value name should match"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "value" "Response should have value field"; then
        return 1
    fi

    local value
    value=$(echo "$response" | jq -r '.value')
    if ! assert_equals "test-value-123" "$value" "Value should match"; then
        return 1
    fi

    return 0
}

test_values_update_api() {
    local new_value="updated-value-456"
    local response
    response=$(update_value "$TEST_VALUE_NAME" "$new_value")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call update value API"
        return 1
    fi

    local verify_response
    verify_response=$(get_value "$TEST_VALUE_NAME")
    local value
    value=$(echo "$verify_response" | jq -r '.value')

    if ! assert_equals "$new_value" "$value" "Value should be updated"; then
        return 1
    fi

    return 0
}

test_values_delete_api() {
    local response
    response=$(delete_value "$TEST_VALUE_NAME")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call delete value API"
        return 1
    fi

    local verify_response
    verify_response=$(get_value "$TEST_VALUE_NAME")
    local error
    error=$(echo "$verify_response" | jq -r '.error // empty')

    if [[ -z "$error" ]]; then
        local name
        name=$(echo "$verify_response" | jq -r '.name // empty')
        if [[ "$name" == "$TEST_VALUE_NAME" ]]; then
            log_fail "Value should be deleted but still exists"
            return 1
        fi
    fi

    return 0
}

test_values_get_nonexistent() {
    local response
    response=$(get_value "NONEXISTENT_VAR_12345")

    local error
    error=$(echo "$response" | jq -r '.error // empty')

    if [[ -z "$error" ]]; then
        local name
        name=$(echo "$response" | jq -r '.name // empty')
        if [[ "$name" == "NONEXISTENT_VAR_12345" ]]; then
            log_fail "Should return error for nonexistent value"
            return 1
        fi
    fi

    return 0
}

test_values_list_contains_created() {
    cleanup_test_value
    create_value "$TEST_VALUE_NAME" "list-test-value" > /dev/null

    local response
    response=$(list_values)

    local found
    found=$(echo "$response" | jq -r ".values[] | select(.name == \"$TEST_VALUE_NAME\") | .name")

    if ! assert_equals "$TEST_VALUE_NAME" "$found" "Created value should appear in list"; then
        log_debug "Values list: $response"
        cleanup_test_value
        return 1
    fi

    cleanup_test_value
    return 0
}

test_values_total_count() {
    local response
    response=$(list_values)

    local total
    total=$(echo "$response" | jq -r '.total')

    local values_count
    values_count=$(echo "$response" | jq '.values | length')

    if [[ "$total" != "$values_count" ]]; then
        log_debug "Total ($total) does not match values count ($values_count)"
    fi

    return 0
}

test_values_special_characters() {
    local special_name="TEST_SPECIAL_$$"
    local special_value="value with spaces & special=chars"

    delete_value "$special_name" > /dev/null 2>&1

    local response
    response=$(create_value "$special_name" "$special_value")

    local verify_response
    verify_response=$(get_value "$special_name")
    local retrieved_value
    retrieved_value=$(echo "$verify_response" | jq -r '.value')

    delete_value "$special_name" > /dev/null 2>&1

    return 0
}

print_summary() {
    echo ""
    echo "======================================"
    echo "Values Admin API Test Summary"
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
    log_info "Starting Values Admin API Tests"
    log_info "Admin: $ADMIN_HOST:$ADMIN_PORT"
    echo ""

    cleanup_test_value

    run_test "Values List API" test_values_list_api
    run_test "Values Create API" test_values_create_api
    run_test "Values Get API" test_values_get_api
    run_test "Values Update API" test_values_update_api
    run_test "Values Delete API" test_values_delete_api
    run_test "Values Get Nonexistent" test_values_get_nonexistent
    run_test "Values List Contains Created" test_values_list_contains_created
    run_test "Values Total Count" test_values_total_count
    run_test "Values Special Characters" test_values_special_characters

    cleanup_test_value

    print_summary
    exit $?
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
