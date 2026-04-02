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

ORIGINAL_WHITELIST_RESPONSE=""

save_state() {
    ORIGINAL_WHITELIST_RESPONSE=$(admin_get "/api/whitelist")
}

restore_state() {
    set_userpass_config '{"enabled":false,"accounts":[],"loopback_requires_auth":false}' >/dev/null 2>&1

    if [[ -n "$ORIGINAL_WHITELIST_RESPONSE" ]]; then
        local mode
        mode=$(echo "$ORIGINAL_WHITELIST_RESPONSE" | jq -r '.mode // "local_only"')
        set_whitelist_mode "$mode" >/dev/null 2>&1
    fi
}

enable_userpass() {
    local loopback_requires_auth="${1:-false}"
    set_userpass_config "{
        \"enabled\": true,
        \"accounts\": [
            {\"username\": \"testuser\", \"password\": \"testpass123\", \"enabled\": true}
        ],
        \"loopback_requires_auth\": $loopback_requires_auth
    }"
}

disable_userpass() {
    set_userpass_config '{"enabled": false, "accounts": [], "loopback_requires_auth": false}'
}

proxy_http_status() {
    local proxy_user_arg=""
    if [[ -n "${1:-}" ]]; then
        proxy_user_arg="--proxy-user $1"
    fi
    curl -s -o /dev/null -w '%{http_code}' \
        --proxy "http://127.0.0.1:${ADMIN_PORT}" \
        $proxy_user_arg \
        --max-time 10 \
        "http://httpbin.org/get" 2>/dev/null
}

test_userpass_config_api() {
    enable_userpass false >/dev/null 2>&1
    sleep 0.3

    local response
    response=$(admin_get "/api/whitelist")
    local userpass_enabled
    userpass_enabled=$(echo "$response" | jq -r '.userpass.enabled')
    if [[ "$userpass_enabled" != "true" ]]; then
        log_fail "Userpass should be enabled after setting, got: $userpass_enabled"
        return 1
    fi

    local loopback_requires_auth
    loopback_requires_auth=$(echo "$response" | jq -r '.userpass.loopback_requires_auth')
    if [[ "$loopback_requires_auth" != "false" ]]; then
        log_fail "loopback_requires_auth should be false, got: $loopback_requires_auth"
        return 1
    fi

    local account_count
    account_count=$(echo "$response" | jq '.userpass.accounts | length')
    if [[ "$account_count" -lt 1 ]]; then
        log_fail "Should have at least 1 account, got: $account_count"
        return 1
    fi

    local username
    username=$(echo "$response" | jq -r '.userpass.accounts[0].username')
    if [[ "$username" != "testuser" ]]; then
        log_fail "Username should be 'testuser', got: $username"
        return 1
    fi

    local has_password
    has_password=$(echo "$response" | jq -r '.userpass.accounts[0].has_password')
    if [[ "$has_password" != "true" ]]; then
        log_fail "has_password should be true, got: $has_password"
        return 1
    fi

    enable_userpass true >/dev/null 2>&1
    sleep 0.3

    response=$(admin_get "/api/whitelist")
    loopback_requires_auth=$(echo "$response" | jq -r '.userpass.loopback_requires_auth')
    if [[ "$loopback_requires_auth" != "true" ]]; then
        log_fail "loopback_requires_auth should be true after update, got: $loopback_requires_auth"
        return 1
    fi

    disable_userpass >/dev/null 2>&1
    sleep 0.3

    response=$(admin_get "/api/whitelist")
    userpass_enabled=$(echo "$response" | jq -r '.userpass.enabled')
    if [[ "$userpass_enabled" != "false" ]]; then
        log_fail "Userpass should be disabled after clearing, got: $userpass_enabled"
        return 1
    fi

    return 0
}

test_loopback_no_auth_default() {
    enable_userpass false >/dev/null 2>&1
    sleep 0.5

    local http_status
    http_status=$(proxy_http_status)

    log_debug "HTTP proxy without auth (loopback_requires_auth=false): $http_status"

    if [[ "$http_status" == "407" ]]; then
        log_fail "Loopback HTTP proxy should NOT require auth when loopback_requires_auth=false (got 407)"
        return 1
    fi

    log_info "Status: $http_status (not 407, OK)"
    return 0
}

test_loopback_with_auth_also_works() {
    enable_userpass false >/dev/null 2>&1
    sleep 0.5

    local http_status
    http_status=$(proxy_http_status "testuser:testpass123")

    log_debug "HTTP proxy with valid auth: $http_status"

    if [[ "$http_status" == "407" ]]; then
        log_fail "HTTP proxy with valid credentials should NOT return 407"
        return 1
    fi

    log_info "Status: $http_status (not 407, OK)"
    return 0
}

test_loopback_requires_auth_on_returns_407_without_creds() {
    enable_userpass true >/dev/null 2>&1
    sleep 0.5

    local http_status
    http_status=$(proxy_http_status)

    log_debug "HTTP proxy without auth (loopback_requires_auth=true): $http_status"

    if [[ "$http_status" != "407" ]]; then
        log_fail "Loopback should require auth when loopback_requires_auth=true (expected 407, got $http_status)"
        return 1
    fi

    log_info "Status: $http_status (407 as expected)"
    return 0
}

test_loopback_requires_auth_on_passes_with_valid_creds() {
    enable_userpass true >/dev/null 2>&1
    sleep 0.5

    local http_status
    http_status=$(proxy_http_status "testuser:testpass123")

    log_debug "HTTP proxy with valid auth (loopback_requires_auth=true): $http_status"

    if [[ "$http_status" == "407" ]]; then
        log_fail "Loopback with valid credentials should NOT return 407 even with loopback_requires_auth=true"
        return 1
    fi

    log_info "Status: $http_status (not 407, OK)"
    return 0
}

test_loopback_requires_auth_on_rejects_wrong_creds() {
    enable_userpass true >/dev/null 2>&1
    sleep 0.5

    local http_status
    http_status=$(proxy_http_status "testuser:wrongpassword")

    log_debug "HTTP proxy with wrong auth (loopback_requires_auth=true): $http_status"

    if [[ "$http_status" != "407" ]]; then
        log_fail "Wrong credentials should return 407 (got $http_status)"
        return 1
    fi

    log_info "Status: $http_status (407 as expected)"
    return 0
}

test_loopback_https_connect_no_auth_default() {
    enable_userpass false >/dev/null 2>&1
    sleep 0.5

    local http_status
    http_status=$(curl -s -o /dev/null -w '%{http_code}' \
        --proxy "http://127.0.0.1:${ADMIN_PORT}" \
        --max-time 10 \
        -k \
        "https://httpbin.org/get" 2>/dev/null)

    log_debug "HTTPS CONNECT without auth (loopback_requires_auth=false): $http_status"

    if [[ "$http_status" == "407" ]]; then
        log_fail "HTTPS CONNECT should NOT require auth when loopback_requires_auth=false (got 407)"
        return 1
    fi

    log_info "Status: $http_status (not 407, OK)"
    return 0
}

test_loopback_https_connect_requires_auth_on() {
    enable_userpass true >/dev/null 2>&1
    sleep 0.5

    local output
    output=$(curl -v -o /dev/null -w '%{http_code}' \
        --proxy "http://127.0.0.1:${ADMIN_PORT}" \
        --max-time 10 \
        -k \
        "https://httpbin.org/get" 2>&1)

    log_debug "HTTPS CONNECT without auth (loopback_requires_auth=true) output: $output"

    if echo "$output" | grep -q "407 Proxy Authentication Required"; then
        log_info "CONNECT correctly rejected with 407"
        return 0
    fi

    log_fail "HTTPS CONNECT should require auth when loopback_requires_auth=true (407 not found in output)"
    return 1
}

test_admin_api_still_works_with_userpass_enabled() {
    enable_userpass true >/dev/null 2>&1
    sleep 0.5

    local response
    response=$(admin_get "/api/system")

    if [[ -z "$response" || "$response" == "null" ]]; then
        log_fail "Admin API should still be accessible with userpass enabled"
        return 1
    fi

    local version
    version=$(echo "$response" | jq -r '.version // empty')
    if [[ -z "$version" ]]; then
        log_fail "Admin API returned invalid response: $response"
        return 1
    fi

    log_info "Admin API returned version: $version"
    return 0
}

test_toggle_loopback_requires_auth() {
    enable_userpass false >/dev/null 2>&1
    sleep 0.3

    local http_status
    http_status=$(proxy_http_status)
    if [[ "$http_status" == "407" ]]; then
        log_fail "Phase 1: loopback_requires_auth=false, should not get 407"
        return 1
    fi
    log_debug "Phase 1 (off): $http_status"

    enable_userpass true >/dev/null 2>&1
    sleep 0.3

    http_status=$(proxy_http_status)
    if [[ "$http_status" != "407" ]]; then
        log_fail "Phase 2: loopback_requires_auth=true, should get 407 (got $http_status)"
        return 1
    fi
    log_debug "Phase 2 (on): $http_status"

    http_status=$(proxy_http_status "testuser:testpass123")
    if [[ "$http_status" == "407" ]]; then
        log_fail "Phase 2b: valid creds should pass (got 407)"
        return 1
    fi
    log_debug "Phase 2b (on+creds): $http_status"

    enable_userpass false >/dev/null 2>&1
    sleep 0.3

    http_status=$(proxy_http_status)
    if [[ "$http_status" == "407" ]]; then
        log_fail "Phase 3: loopback_requires_auth=false again, should not get 407"
        return 1
    fi
    log_debug "Phase 3 (off again): $http_status"

    return 0
}

print_summary() {
    echo ""
    echo "======================================"
    echo "Userpass Loopback Auth E2E Test Summary"
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
    trap 'restore_state; admin_cleanup_bifrost' EXIT

    if ! admin_ensure_bifrost; then
        log_fail "Admin server is not reachable and failed to start"
        exit 1
    fi

    log_info "Starting Userpass Loopback Auth E2E Tests"
    log_info "Admin: $ADMIN_HOST:$ADMIN_PORT"
    echo ""

    save_state

    run_test "Userpass Config API (with loopback_requires_auth)" test_userpass_config_api
    run_test "Loopback No Auth Required (default)" test_loopback_no_auth_default
    run_test "Loopback With Auth Also Works" test_loopback_with_auth_also_works
    run_test "Loopback Requires Auth ON - Returns 407 Without Creds" test_loopback_requires_auth_on_returns_407_without_creds
    run_test "Loopback Requires Auth ON - Passes With Valid Creds" test_loopback_requires_auth_on_passes_with_valid_creds
    run_test "Loopback Requires Auth ON - Rejects Wrong Creds" test_loopback_requires_auth_on_rejects_wrong_creds
    run_test "Loopback HTTPS CONNECT No Auth (default)" test_loopback_https_connect_no_auth_default
    run_test "Loopback HTTPS CONNECT Requires Auth ON" test_loopback_https_connect_requires_auth_on
    run_test "Admin API Still Works With Userpass Enabled" test_admin_api_still_works_with_userpass_enabled
    run_test "Toggle loopback_requires_auth On/Off Cycle" test_toggle_loopback_requires_auth

    restore_state

    print_summary
    exit $?
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
