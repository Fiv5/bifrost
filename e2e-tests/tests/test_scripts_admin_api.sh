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

TEST_SCRIPT_PREFIX="e2e_test_$$"

cleanup_test_scripts() {
    for t in request response decode; do
        delete_script "$t" "${TEST_SCRIPT_PREFIX}_basic" > /dev/null 2>&1
        delete_script "$t" "${TEST_SCRIPT_PREFIX}_update" > /dev/null 2>&1
        delete_script "$t" "${TEST_SCRIPT_PREFIX}_special" > /dev/null 2>&1
        delete_script "$t" "${TEST_SCRIPT_PREFIX}_desc" > /dev/null 2>&1
    done
}

# --- Test: List scripts API returns three categories ---
test_scripts_list_api() {
    local response
    response=$(list_scripts)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call scripts list API"
        return 1
    fi

    if ! assert_not_empty "$response" "Scripts list response should not be empty"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "request" "Response should have request field"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_has_field "$response" "response" "Response should have response field"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "decode" "Response should have decode field"; then
        return 1
    fi

    return 0
}

# --- Test: Create a request script ---
test_scripts_create_request() {
    delete_script "request" "${TEST_SCRIPT_PREFIX}_basic" > /dev/null 2>&1

    local content='log.info("e2e test request script");'
    local response
    response=$(create_script "request" "${TEST_SCRIPT_PREFIX}_basic" "$content")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call create script API"
        return 1
    fi

    local verify_response
    verify_response=$(get_script "request" "${TEST_SCRIPT_PREFIX}_basic")
    local name
    name=$(echo "$verify_response" | jq -r '.name')

    if ! assert_equals "${TEST_SCRIPT_PREFIX}_basic" "$name" "Script should be created"; then
        log_debug "Verify response: $verify_response"
        return 1
    fi

    local retrieved_content
    retrieved_content=$(echo "$verify_response" | jq -r '.content')
    if ! assert_equals "$content" "$retrieved_content" "Script content should match"; then
        return 1
    fi

    return 0
}

# --- Test: Create a response script ---
test_scripts_create_response() {
    delete_script "response" "${TEST_SCRIPT_PREFIX}_basic" > /dev/null 2>&1

    local content='log.info("e2e test response script");'
    local response
    response=$(create_script "response" "${TEST_SCRIPT_PREFIX}_basic" "$content")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call create response script API"
        return 1
    fi

    local verify_response
    verify_response=$(get_script "response" "${TEST_SCRIPT_PREFIX}_basic")
    local script_type
    script_type=$(echo "$verify_response" | jq -r '.script_type')

    if ! assert_equals "response" "$script_type" "Script type should be response"; then
        return 1
    fi

    return 0
}

# --- Test: Create a decode script ---
test_scripts_create_decode() {
    delete_script "decode" "${TEST_SCRIPT_PREFIX}_basic" > /dev/null 2>&1

    local content='ctx.output = { code: "0", data: request.body || "", msg: "" };'
    local response
    response=$(create_script "decode" "${TEST_SCRIPT_PREFIX}_basic" "$content")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call create decode script API"
        return 1
    fi

    local verify_response
    verify_response=$(get_script "decode" "${TEST_SCRIPT_PREFIX}_basic")
    local script_type
    script_type=$(echo "$verify_response" | jq -r '.script_type')

    if ! assert_equals "decode" "$script_type" "Script type should be decode"; then
        return 1
    fi

    return 0
}

# --- Test: Get script detail ---
test_scripts_get_api() {
    local response
    response=$(get_script "request" "${TEST_SCRIPT_PREFIX}_basic")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call get script API"
        return 1
    fi

    if ! assert_json_field "$response" ".name" "${TEST_SCRIPT_PREFIX}_basic" "Script name should match"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "content" "Response should have content field"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "script_type" "Response should have script_type field"; then
        return 1
    fi

    return 0
}

# --- Test: Update script content ---
test_scripts_update_api() {
    local new_content='log.info("updated e2e test script");'
    local response
    response=$(create_script "request" "${TEST_SCRIPT_PREFIX}_basic" "$new_content")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call update script API"
        return 1
    fi

    local verify_response
    verify_response=$(get_script "request" "${TEST_SCRIPT_PREFIX}_basic")
    local content
    content=$(echo "$verify_response" | jq -r '.content')

    if ! assert_equals "$new_content" "$content" "Script content should be updated"; then
        return 1
    fi

    return 0
}

# --- Test: Delete script ---
test_scripts_delete_api() {
    delete_script "request" "${TEST_SCRIPT_PREFIX}_basic" > /dev/null 2>&1
    create_script "request" "${TEST_SCRIPT_PREFIX}_basic" "// to delete" > /dev/null 2>&1

    local response
    response=$(delete_script "request" "${TEST_SCRIPT_PREFIX}_basic")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call delete script API"
        return 1
    fi

    local verify_response
    verify_response=$(get_script "request" "${TEST_SCRIPT_PREFIX}_basic")
    local error
    error=$(echo "$verify_response" | jq -r '.error // empty')

    if [[ -z "$error" ]]; then
        local name
        name=$(echo "$verify_response" | jq -r '.name // empty')
        if [[ "$name" == "${TEST_SCRIPT_PREFIX}_basic" ]]; then
            log_fail "Script should be deleted but still exists"
            return 1
        fi
    fi

    return 0
}

# --- Test: Get nonexistent script returns error ---
test_scripts_get_nonexistent() {
    local response
    response=$(get_script "request" "nonexistent_script_12345")

    local error
    error=$(echo "$response" | jq -r '.error // empty')

    if [[ -z "$error" ]]; then
        local name
        name=$(echo "$response" | jq -r '.name // empty')
        if [[ "$name" == "nonexistent_script_12345" ]]; then
            log_fail "Should return error for nonexistent script"
            return 1
        fi
    fi

    return 0
}

# --- Test: Created script appears in list ---
test_scripts_list_contains_created() {
    delete_script "request" "${TEST_SCRIPT_PREFIX}_basic" > /dev/null 2>&1
    create_script "request" "${TEST_SCRIPT_PREFIX}_basic" "// list test" > /dev/null 2>&1

    local response
    response=$(list_scripts)

    local found
    found=$(echo "$response" | jq -r ".request[] | select(.name == \"${TEST_SCRIPT_PREFIX}_basic\") | .name")

    if ! assert_equals "${TEST_SCRIPT_PREFIX}_basic" "$found" "Created script should appear in request list"; then
        log_debug "Scripts list: $response"
        delete_script "request" "${TEST_SCRIPT_PREFIX}_basic" > /dev/null 2>&1
        return 1
    fi

    delete_script "request" "${TEST_SCRIPT_PREFIX}_basic" > /dev/null 2>&1
    return 0
}

# --- Test: Script with description ---
test_scripts_with_description() {
    delete_script "request" "${TEST_SCRIPT_PREFIX}_desc" > /dev/null 2>&1

    local content='log.info("desc test");'
    local description="E2E test script with description"
    local response
    response=$(create_script "request" "${TEST_SCRIPT_PREFIX}_desc" "$content" "$description")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to create script with description"
        return 1
    fi

    local verify_response
    verify_response=$(get_script "request" "${TEST_SCRIPT_PREFIX}_desc")
    local retrieved_content
    retrieved_content=$(echo "$verify_response" | jq -r '.content')

    if ! assert_equals "$content" "$retrieved_content" "Script content should match"; then
        delete_script "request" "${TEST_SCRIPT_PREFIX}_desc" > /dev/null 2>&1
        return 1
    fi

    delete_script "request" "${TEST_SCRIPT_PREFIX}_desc" > /dev/null 2>&1
    return 0
}

# --- Test: Builtin decode scripts (utf8, default) are listed ---
test_scripts_builtin_decode() {
    local response
    response=$(list_scripts)

    local utf8_found
    utf8_found=$(echo "$response" | jq -r '.decode[] | select(.name == "utf8") | .name')

    if ! assert_equals "utf8" "$utf8_found" "Builtin utf8 decode script should be listed"; then
        log_debug "Decode scripts: $(echo "$response" | jq '.decode')"
        return 1
    fi

    local default_found
    default_found=$(echo "$response" | jq -r '.decode[] | select(.name == "default") | .name')

    if ! assert_equals "default" "$default_found" "Builtin default decode script should be listed"; then
        return 1
    fi

    return 0
}

# --- Test: Delete nonexistent script returns error ---
test_scripts_delete_nonexistent() {
    local response
    response=$(delete_script "request" "nonexistent_delete_12345")

    local error
    error=$(echo "$response" | jq -r '.error // empty')

    if [[ -z "$error" ]]; then
        log_fail "Deleting nonexistent script should return error"
        return 1
    fi

    return 0
}

print_summary() {
    echo ""
    echo "======================================"
    echo "Scripts Admin API Test Summary"
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

    log_info "Starting Scripts Admin API Tests"
    log_info "Admin: $ADMIN_HOST:$ADMIN_PORT"
    echo ""

    cleanup_test_scripts

    run_test "Scripts List API" test_scripts_list_api
    run_test "Scripts Create Request" test_scripts_create_request
    run_test "Scripts Create Response" test_scripts_create_response
    run_test "Scripts Create Decode" test_scripts_create_decode
    run_test "Scripts Get API" test_scripts_get_api
    run_test "Scripts Update API" test_scripts_update_api
    run_test "Scripts Delete API" test_scripts_delete_api
    run_test "Scripts Get Nonexistent" test_scripts_get_nonexistent
    run_test "Scripts List Contains Created" test_scripts_list_contains_created
    run_test "Scripts With Description" test_scripts_with_description
    run_test "Scripts Builtin Decode" test_scripts_builtin_decode
    run_test "Scripts Delete Nonexistent" test_scripts_delete_nonexistent

    cleanup_test_scripts

    print_summary
    exit $?
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
