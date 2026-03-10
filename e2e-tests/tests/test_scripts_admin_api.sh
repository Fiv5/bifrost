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

test_scripts_list_api() {
    local response
    response=$(admin_get "/api/scripts")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call scripts list API"
        return 1
    fi

    if ! assert_not_empty "$response" "Scripts list response should not be empty"; then
        return 1
    fi

    local is_object
    is_object=$(echo "$response" | jq 'type == "object"')
    if ! assert_equals "true" "$is_object" "Response should be an object"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_has_field "$response" "request" "Response should have request"; then
        return 1
    fi
    if ! assert_json_has_field "$response" "response" "Response should have response"; then
        return 1
    fi
    if ! assert_json_has_field "$response" "decode" "Response should have decode"; then
        return 1
    fi

    return 0
}

test_decode_script_test_api_returns_output() {
    local payload
    payload=$(
        cat <<'JSON'
{
  "type": "decode",
  "content": "log.info(\"decode phase:\", ctx.phase);\\nctx.output = { code: \"0\", data: request.body || \"\", msg: \"\" };",
  "mock_request": {
    "url": "https://example.com/",
    "method": "POST",
    "headers": { "Content-Type": "text/plain" },
    "body": "hello"
  }
}
JSON
    )

    local response
    response=$(admin_post "/api/scripts/test" "$payload")

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call scripts test API"
        return 1
    fi

    if ! assert_json_field "$response" ".success" "true" "Test should succeed"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_field "$response" ".script_type" "decode" "script_type should be decode"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_has_field "$response" "decode_output" "Response should contain decode_output"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_field "$response" ".decode_output.code" "0" "decode_output.code should be 0"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_field "$response" ".decode_output.data" "hello" "decode_output.data should be request body"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_has_field "$response" "logs" "Response should contain logs"; then
        log_debug "Response: $response"
        return 1
    fi

    local log_count
    log_count=$(echo "$response" | jq -r ".logs | length")
    if [[ "$log_count" -lt 1 ]]; then
        log_fail "logs should not be empty"
        log_debug "Response: $response"
        return 1
    fi

    return 0
}

main() {
    admin_ensure_bifrost || exit 1
    trap admin_cleanup_bifrost EXIT

    run_test "Scripts list API" test_scripts_list_api
    run_test "Decode test API returns output" test_decode_script_test_api_returns_output

    echo "========================================"
    echo "Test Summary"
    echo "========================================"
    echo "Total:  $TESTS_RUN"
    echo "Passed: $TESTS_PASSED"
    echo "Failed: $TESTS_FAILED"
    echo "========================================"

    [[ "$TESTS_FAILED" -eq 0 ]]
}

main "$@"

