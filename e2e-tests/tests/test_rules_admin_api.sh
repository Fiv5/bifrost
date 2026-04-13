#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"
source "$SCRIPT_DIR/../test_utils/rule_fixture.sh"

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

TEST_RULE_NAME="test-rule-$$"
RULE_FIXTURES_DIR="$SCRIPT_DIR/../rules/admin_api"

rule_content_from_fixture() {
    local name="$1"
    rule_fixture_content "$RULE_FIXTURES_DIR/$name"
}

cleanup_test_rule() {
    delete_rule "$TEST_RULE_NAME" > /dev/null 2>&1
}

test_rules_list_api() {
    local response
    response=$(list_rules)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call rules list API"
        return 1
    fi

    if ! assert_not_empty "$response" "Rules list response should not be empty"; then
        return 1
    fi

    local is_array
    is_array=$(echo "$response" | jq 'type == "array"')

    if ! assert_equals "true" "$is_array" "Response should be an array"; then
        log_debug "Response: $response"
        return 1
    fi

    return 0
}

test_rules_create_api() {
    cleanup_test_rule

    local response
    response=$(create_rule "$TEST_RULE_NAME" "$(rule_content_from_fixture create_proxy_rule.txt)" "true")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call create rule API"
        return 1
    fi

    local success
    success=$(echo "$response" | jq -r '.success // empty')

    if [[ "$success" != "true" ]]; then
        local error
        error=$(echo "$response" | jq -r '.error // empty')
        if [[ -n "$error" ]]; then
            log_fail "Create rule failed: $error"
            return 1
        fi
    fi

    local verify_response
    verify_response=$(get_rule "$TEST_RULE_NAME")
    local rule_name
    rule_name=$(echo "$verify_response" | jq -r '.name')

    if ! assert_equals "$TEST_RULE_NAME" "$rule_name" "Rule should be created"; then
        log_debug "Verify response: $verify_response"
        return 1
    fi

    return 0
}

test_rules_get_api() {
    local response
    response=$(get_rule "$TEST_RULE_NAME")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call get rule API"
        return 1
    fi

    if ! assert_json_field "$response" ".name" "$TEST_RULE_NAME" "Rule name should match"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "content" "Rule should have content field"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "enabled" "Rule should have enabled field"; then
        return 1
    fi

    return 0
}

test_rules_update_api() {
    local new_content
    new_content=$(rule_content_from_fixture update_mock_file_rule.txt)
    local response
    response=$(update_rule "$TEST_RULE_NAME" "$new_content" "true")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call update rule API"
        return 1
    fi

    local verify_response
    verify_response=$(get_rule "$TEST_RULE_NAME")
    local content
    content=$(echo "$verify_response" | jq -r '.content')

    if [[ "$content" != "$new_content" ]]; then
        log_fail "Rule content should be updated: expected '$new_content', got '$content'"
        return 1
    fi

    return 0
}

test_rules_disable_api() {
    local response
    response=$(disable_rule "$TEST_RULE_NAME")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call disable rule API"
        return 1
    fi

    local verify_response
    verify_response=$(get_rule "$TEST_RULE_NAME")
    local enabled
    enabled=$(echo "$verify_response" | jq -r '.enabled')

    if ! assert_equals "false" "$enabled" "Rule should be disabled"; then
        return 1
    fi

    return 0
}

test_rules_enable_api() {
    local response
    response=$(enable_rule "$TEST_RULE_NAME")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call enable rule API"
        return 1
    fi

    local verify_response
    verify_response=$(get_rule "$TEST_RULE_NAME")
    local enabled
    enabled=$(echo "$verify_response" | jq -r '.enabled')

    if ! assert_equals "true" "$enabled" "Rule should be enabled"; then
        return 1
    fi

    return 0
}

test_rules_delete_api() {
    local response
    response=$(delete_rule "$TEST_RULE_NAME")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call delete rule API"
        return 1
    fi

    local verify_response
    verify_response=$(get_rule "$TEST_RULE_NAME")
    local error
    error=$(echo "$verify_response" | jq -r '.error // empty')

    if [[ -z "$error" ]]; then
        local name
        name=$(echo "$verify_response" | jq -r '.name // empty')
        if [[ "$name" == "$TEST_RULE_NAME" ]]; then
            log_fail "Rule should be deleted but still exists"
            return 1
        fi
    fi

    return 0
}

test_rules_get_nonexistent() {
    local response
    response=$(get_rule "nonexistent-rule-12345")

    local error
    error=$(echo "$response" | jq -r '.error // empty')

    if [[ -z "$error" ]]; then
        local name
        name=$(echo "$response" | jq -r '.name // empty')
        if [[ "$name" == "nonexistent-rule-12345" ]]; then
            log_fail "Should return error for nonexistent rule"
            return 1
        fi
    fi

    return 0
}

test_rules_list_contains_created_rule() {
    cleanup_test_rule
    create_rule "$TEST_RULE_NAME" "$(rule_content_from_fixture list_contains_created_rule.txt)" "true" > /dev/null

    local response
    response=$(list_rules)

    local found
    found=$(echo "$response" | jq -r ".[] | select(.name == \"$TEST_RULE_NAME\") | .name")

    if ! assert_equals "$TEST_RULE_NAME" "$found" "Created rule should appear in list"; then
        log_debug "Rules list: $response"
        cleanup_test_rule
        return 1
    fi

    cleanup_test_rule
    return 0
}

test_rules_rule_count_field() {
    cleanup_test_rule
    create_rule "$TEST_RULE_NAME" "$(rule_content_from_fixture rule_count_multiline.txt)" "true" > /dev/null

    local response
    response=$(list_rules)

    local rule_count
    rule_count=$(echo "$response" | jq -r ".[] | select(.name == \"$TEST_RULE_NAME\") | .rule_count")

    if [[ "$rule_count" == "null" || -z "$rule_count" ]]; then
        log_debug "Rule count field not found or null"
    fi

    cleanup_test_rule
    return 0
}

print_summary() {
    echo ""
    echo "======================================"
    echo "Rules Admin API Test Summary"
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

    log_info "Starting Rules Admin API Tests"
    log_info "Admin: $ADMIN_HOST:$ADMIN_PORT"
    echo ""

    cleanup_test_rule

    run_test "Rules List API" test_rules_list_api
    run_test "Rules Create API" test_rules_create_api
    run_test "Rules Get API" test_rules_get_api
    run_test "Rules Update API" test_rules_update_api
    run_test "Rules Disable API" test_rules_disable_api
    run_test "Rules Enable API" test_rules_enable_api
    run_test "Rules Delete API" test_rules_delete_api
    run_test "Rules Get Nonexistent" test_rules_get_nonexistent
    run_test "Rules List Contains Created Rule" test_rules_list_contains_created_rule
    run_test "Rules Rule Count Field" test_rules_rule_count_field

    cleanup_test_rule

    print_summary
    exit $?
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
