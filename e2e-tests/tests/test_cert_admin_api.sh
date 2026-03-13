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

assert_contains() {
    local haystack="$1"
    local needle="$2"
    local msg="${3:-String should contain substring}"

    if [[ "$haystack" == *"$needle"* ]]; then
        return 0
    else
        log_fail "$msg: '$needle' not found"
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

test_cert_info_api() {
    local response
    response=$(get_cert_info)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call cert info API"
        return 1
    fi

    if ! assert_not_empty "$response" "Cert info response should not be empty"; then
        return 1
    fi

    if ! assert_json_has_field "$response" "available" "Response should have available field"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_has_field "$response" "status" "Response should have status field"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_has_field "$response" "installed" "Response should have installed field"; then
        log_debug "Response: $response"
        return 1
    fi

    if ! assert_json_has_field "$response" "trusted" "Response should have trusted field"; then
        log_debug "Response: $response"
        return 1
    fi

    return 0
}

test_cert_info_structure() {
    local response
    response=$(get_cert_info)

    local available
    available=$(echo "$response" | jq -r '.available')
    local status
    status=$(echo "$response" | jq -r '.status')
    local installed
    installed=$(echo "$response" | jq -r '.installed')
    local trusted
    trusted=$(echo "$response" | jq -r '.trusted')

    if [[ "$available" != "true" && "$available" != "false" ]]; then
        log_fail "Invalid available value: $available"
        return 1
    fi

    case "$status" in
        not_installed|installed_not_trusted|installed_and_trusted|unknown) ;;
        *)
            log_fail "Invalid status value: $status"
            return 1
            ;;
    esac

    if [[ "$installed" != "true" && "$installed" != "false" ]]; then
        log_fail "Invalid installed value: $installed"
        return 1
    fi

    if [[ "$trusted" != "true" && "$trusted" != "false" ]]; then
        log_fail "Invalid trusted value: $trusted"
        return 1
    fi

    case "$status" in
        not_installed)
            assert_equals "false" "$installed" "not_installed should report installed=false" || return 1
            assert_equals "false" "$trusted" "not_installed should report trusted=false" || return 1
            ;;
        installed_not_trusted)
            assert_equals "true" "$installed" "installed_not_trusted should report installed=true" || return 1
            assert_equals "false" "$trusted" "installed_not_trusted should report trusted=false" || return 1
            ;;
        installed_and_trusted)
            assert_equals "true" "$installed" "installed_and_trusted should report installed=true" || return 1
            assert_equals "true" "$trusted" "installed_and_trusted should report trusted=true" || return 1
            ;;
        unknown)
            assert_equals "false" "$trusted" "unknown should never claim trusted=true" || return 1
            ;;
    esac

    if [[ "$available" == "true" ]]; then
        if ! assert_json_has_field "$response" "local_ips" "Response should have local_ips when available"; then
            return 1
        fi

        if ! assert_json_has_field "$response" "download_urls" "Response should have download_urls when available"; then
            return 1
        fi

        local local_ips_type
        local_ips_type=$(echo "$response" | jq '.local_ips | type')
        if [[ "$local_ips_type" != "\"array\"" ]]; then
            log_fail "local_ips should be an array"
            return 1
        fi

        local download_urls_type
        download_urls_type=$(echo "$response" | jq '.download_urls | type')
        if [[ "$download_urls_type" != "\"array\"" ]]; then
            log_fail "download_urls should be an array"
            return 1
        fi
    fi

    return 0
}

test_cert_download_api() {
    local response
    response=$(download_cert)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call cert download API"
        return 1
    fi

    if [[ -z "$response" ]]; then
        log_debug "Cert download returned empty response (cert may not be available)"
        return 0
    fi

    if [[ "$response" == *"-----BEGIN CERTIFICATE-----"* ]]; then
        return 0
    fi

    local error
    error=$(echo "$response" | jq -r '.error // empty' 2>/dev/null)
    if [[ -n "$error" ]]; then
        log_debug "Cert not available: $error"
        return 0
    fi

    return 0
}

test_cert_download_format() {
    local response
    response=$(download_cert)

    if [[ -z "$response" ]]; then
        log_debug "Cert download returned empty response"
        return 0
    fi

    if [[ "$response" == *"-----BEGIN CERTIFICATE-----"* ]]; then
        if ! assert_contains "$response" "-----END CERTIFICATE-----" "Cert should have END marker"; then
            return 1
        fi
        return 0
    fi

    return 0
}

test_cert_download_absolute_form() {
    local response
    response=$(download_cert_absolute_form)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call cert download API with absolute-form request target"
        return 1
    fi

    if [[ -z "$response" ]]; then
        log_debug "Absolute-form cert download returned empty response"
        return 0
    fi

    if [[ "$response" == *"-----BEGIN CERTIFICATE-----"* ]]; then
        if ! assert_contains "$response" "-----END CERTIFICATE-----" "Absolute-form cert should have END marker"; then
            return 1
        fi
        return 0
    fi

    local error
    error=$(echo "$response" | jq -r '.error // empty' 2>/dev/null)
    if [[ -n "$error" ]]; then
        log_fail "Absolute-form cert download should not return API error: $error"
        return 1
    fi

    log_fail "Absolute-form cert download returned unexpected response"
    return 1
}

test_cert_qrcode_api() {
    local response
    response=$(get_cert_qrcode)

    if [[ $? -ne 0 ]]; then
        log_fail "Failed to call cert QR code API"
        return 1
    fi

    if [[ -z "$response" ]]; then
        log_debug "QR code returned empty response (cert may not be available)"
        return 0
    fi

    if [[ "$response" == *"<svg"* ]] || [[ "$response" == *"<?xml"* ]]; then
        return 0
    fi

    local error
    error=$(echo "$response" | jq -r '.error // empty' 2>/dev/null)
    if [[ -n "$error" ]]; then
        log_debug "QR code not available: $error"
        return 0
    fi

    return 0
}

test_cert_qrcode_format() {
    local response
    response=$(get_cert_qrcode)

    if [[ -z "$response" ]]; then
        log_debug "QR code returned empty response"
        return 0
    fi

    if [[ "$response" == *"<svg"* ]]; then
        if ! assert_contains "$response" "</svg>" "SVG should have closing tag"; then
            return 1
        fi
        return 0
    fi

    return 0
}

test_cert_info_urls_format() {
    local response
    response=$(get_cert_info)

    local available
    available=$(echo "$response" | jq -r '.available')

    if [[ "$available" != "true" ]]; then
        log_debug "Cert not available, skipping URL format test"
        return 0
    fi

    local download_urls
    download_urls=$(echo "$response" | jq -r '.download_urls[]?' 2>/dev/null)

    if [[ -n "$download_urls" ]]; then
        while IFS= read -r url; do
            if [[ ! "$url" =~ ^https?:// ]]; then
                log_fail "Invalid download URL format: $url"
                return 1
            fi
        done <<< "$download_urls"
    fi

    return 0
}

print_summary() {
    echo ""
    echo "======================================"
    echo "Cert Admin API Test Summary"
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

    log_info "Starting Cert Admin API Tests"
    log_info "Admin: $ADMIN_HOST:$ADMIN_PORT"
    echo ""

    run_test "Cert Info API" test_cert_info_api
    run_test "Cert Info Structure" test_cert_info_structure
    run_test "Cert Download API" test_cert_download_api
    run_test "Cert Download Format" test_cert_download_format
    run_test "Cert Download Absolute Form" test_cert_download_absolute_form
    run_test "Cert QR Code API" test_cert_qrcode_api
    run_test "Cert QR Code Format" test_cert_qrcode_format
    run_test "Cert Info URLs Format" test_cert_info_urls_format

    print_summary
    exit $?
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
