#!/bin/bash
# 断言工具库 - 用于端到端测试验证

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 测试统计
TOTAL_ASSERTIONS=0
PASSED_ASSERTIONS=0
FAILED_ASSERTIONS=0

# 输出辅助函数
_log_pass() {
    echo -e "${GREEN}✓${NC} $1"
    ((TOTAL_ASSERTIONS++))
    ((PASSED_ASSERTIONS++))
}

_log_fail() {
    echo -e "${RED}✗${NC} $1"
    echo -e "  ${RED}Expected:${NC} $2"
    echo -e "  ${RED}Actual:${NC}   $3"
    ((TOTAL_ASSERTIONS++))
    ((FAILED_ASSERTIONS++))
}

_log_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

_log_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# ==============================================================================
# HTTP 状态码断言
# ==============================================================================

assert_status() {
    local expected=$1
    local actual=$2
    local message=${3:-"HTTP status code should be $expected"}

    if [ "$expected" == "$actual" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "$expected" "$actual"
        return 1
    fi
}

assert_status_2xx() {
    local actual=$1
    local message=${2:-"HTTP status should be 2xx (success)"}

    if [[ "$actual" =~ ^2[0-9]{2}$ ]]; then
        _log_pass "$message (got $actual)"
        return 0
    else
        _log_fail "$message" "2xx" "$actual"
        return 1
    fi
}

assert_status_3xx() {
    local actual=$1
    local message=${2:-"HTTP status should be 3xx (redirect)"}

    if [[ "$actual" =~ ^3[0-9]{2}$ ]]; then
        _log_pass "$message (got $actual)"
        return 0
    else
        _log_fail "$message" "3xx" "$actual"
        return 1
    fi
}

assert_status_4xx() {
    local actual=$1
    local message=${2:-"HTTP status should be 4xx (client error)"}

    if [[ "$actual" =~ ^4[0-9]{2}$ ]]; then
        _log_pass "$message (got $actual)"
        return 0
    else
        _log_fail "$message" "4xx" "$actual"
        return 1
    fi
}

assert_status_5xx() {
    local actual=$1
    local message=${2:-"HTTP status should be 5xx (server error)"}

    if [[ "$actual" =~ ^5[0-9]{2}$ ]]; then
        _log_pass "$message (got $actual)"
        return 0
    else
        _log_fail "$message" "5xx" "$actual"
        return 1
    fi
}

# ==============================================================================
# 响应头断言
# ==============================================================================

assert_header_exists() {
    local header_name=$1
    local headers=$2
    local message=${3:-"Response header '$header_name' should exist"}

    if echo "$headers" | grep -qi "^${header_name}:"; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "Header '$header_name' present" "Header not found"
        return 1
    fi
}

assert_header_not_exists() {
    local header_name=$1
    local headers=$2
    local message=${3:-"Response header '$header_name' should NOT exist"}

    if echo "$headers" | grep -qi "^${header_name}:"; then
        local actual_value
        actual_value=$(echo "$headers" | grep -i "^${header_name}:" | head -1)
        _log_fail "$message" "Header '$header_name' absent" "$actual_value"
        return 1
    else
        _log_pass "$message"
        return 0
    fi
}

assert_header_value() {
    local header_name=$1
    local expected_value=$2
    local headers=$3
    local message=${4:-"Header '$header_name' should be '$expected_value'"}

    local actual_value
    actual_value=$(echo "$headers" | grep -i "^${header_name}:" | head -1 | cut -d':' -f2- | sed 's/^[[:space:]]*//' | tr -d '\r')

    if [ "$actual_value" == "$expected_value" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "$expected_value" "$actual_value"
        return 1
    fi
}

assert_header_contains() {
    local header_name=$1
    local expected_substring=$2
    local headers=$3
    local message=${4:-"Header '$header_name' should contain '$expected_substring'"}

    local actual_value
    actual_value=$(echo "$headers" | grep -i "^${header_name}:" | head -1 | cut -d':' -f2- | sed 's/^[[:space:]]*//' | tr -d '\r')

    if [[ "$actual_value" == *"$expected_substring"* ]]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "Contains '$expected_substring'" "$actual_value"
        return 1
    fi
}

# ==============================================================================
# 响应体断言
# ==============================================================================

assert_body_equals() {
    local expected=$1
    local actual=$2
    local message=${3:-"Response body should match"}

    if [ "$expected" == "$actual" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "${expected:0:100}..." "${actual:0:100}..."
        return 1
    fi
}

assert_body_contains() {
    local expected_substring=$1
    local body=$2
    local message=${3:-"Response body should contain '$expected_substring'"}

    if [[ "$body" == *"$expected_substring"* ]]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "Contains '$expected_substring'" "${body:0:200}..."
        return 1
    fi
}

assert_body_not_contains() {
    local unexpected_substring=$1
    local body=$2
    local message=${3:-"Response body should NOT contain '$unexpected_substring'"}

    if [[ "$body" == *"$unexpected_substring"* ]]; then
        _log_fail "$message" "Not contains '$unexpected_substring'" "Found in body"
        return 1
    else
        _log_pass "$message"
        return 0
    fi
}

assert_body_matches() {
    local pattern=$1
    local body=$2
    local message=${3:-"Response body should match pattern '$pattern'"}

    if echo "$body" | grep -qE "$pattern"; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "Match pattern '$pattern'" "${body:0:200}..."
        return 1
    fi
}

# ==============================================================================
# JSON 断言 (需要 jq)
# ==============================================================================

assert_json_field() {
    local jq_path=$1
    local expected_value=$2
    local json_body=$3
    local message=${4:-"JSON field '$jq_path' should be '$expected_value'"}

    if ! command -v jq &> /dev/null; then
        _log_warning "jq not installed, skipping JSON assertion"
        return 2
    fi

    local actual_value
    actual_value=$(echo "$json_body" | jq -r "$jq_path" 2>/dev/null)

    if [ "$actual_value" == "$expected_value" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "$expected_value" "$actual_value"
        return 1
    fi
}

assert_json_field_exists() {
    local jq_path=$1
    local json_body=$2
    local message=${3:-"JSON field '$jq_path' should exist"}

    if ! command -v jq &> /dev/null; then
        _log_warning "jq not installed, skipping JSON assertion"
        return 2
    fi

    local actual_value
    actual_value=$(echo "$json_body" | jq -r "$jq_path" 2>/dev/null)

    if [ "$actual_value" != "null" ] && [ -n "$actual_value" ]; then
        _log_pass "$message (value: $actual_value)"
        return 0
    else
        _log_fail "$message" "Field exists" "null or missing"
        return 1
    fi
}

# ==============================================================================
# 后端请求验证 (从 Echo Server 响应解析)
# ==============================================================================

assert_backend_received_header() {
    local header_name=$1
    local expected_value=$2
    local echo_response=$3
    local message=${4:-"Backend should receive header '$header_name' = '$expected_value'"}

    if ! command -v jq &> /dev/null; then
        _log_warning "jq not installed, skipping backend assertion"
        return 2
    fi

    local actual_value
    actual_value=$(echo "$echo_response" | jq -r ".request.headers[\"$header_name\"]" 2>/dev/null)

    if [ "$actual_value" == "$expected_value" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "$expected_value" "$actual_value"
        return 1
    fi
}

assert_backend_received_method() {
    local expected_method=$1
    local echo_response=$2
    local message=${3:-"Backend should receive method '$expected_method'"}

    if ! command -v jq &> /dev/null; then
        _log_warning "jq not installed, skipping backend assertion"
        return 2
    fi

    local actual_method
    actual_method=$(echo "$echo_response" | jq -r ".request.method" 2>/dev/null)

    if [ "$actual_method" == "$expected_method" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "$expected_method" "$actual_method"
        return 1
    fi
}

assert_backend_received_path() {
    local expected_path=$1
    local echo_response=$2
    local message=${3:-"Backend should receive path '$expected_path'"}

    if ! command -v jq &> /dev/null; then
        _log_warning "jq not installed, skipping backend assertion"
        return 2
    fi

    local actual_path
    actual_path=$(echo "$echo_response" | jq -r ".request.path" 2>/dev/null)

    if [ "$actual_path" == "$expected_path" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "$expected_path" "$actual_path"
        return 1
    fi
}

assert_backend_protocol() {
    local expected_protocol=$1
    local echo_response=$2
    local message=${3:-"Backend should use protocol '$expected_protocol'"}

    if ! command -v jq &> /dev/null; then
        _log_warning "jq not installed, skipping backend assertion"
        return 2
    fi

    local actual_protocol
    actual_protocol=$(echo "$echo_response" | jq -r ".server.protocol" 2>/dev/null)

    if [ "$actual_protocol" == "$expected_protocol" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "$expected_protocol" "$actual_protocol"
        return 1
    fi
}

# ==============================================================================
# 通用断言
# ==============================================================================

assert_equals() {
    local expected=$1
    local actual=$2
    local message=${3:-"Values should be equal"}

    if [ "$expected" == "$actual" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "$expected" "$actual"
        return 1
    fi
}

assert_not_equals() {
    local unexpected=$1
    local actual=$2
    local message=${3:-"Values should NOT be equal"}

    if [ "$unexpected" != "$actual" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "Not equal to '$unexpected'" "$actual"
        return 1
    fi
}

assert_not_empty() {
    local value=$1
    local message=${2:-"Value should not be empty"}

    if [ -n "$value" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "Non-empty value" "(empty)"
        return 1
    fi
}

assert_empty() {
    local value=$1
    local message=${2:-"Value should be empty"}

    if [ -z "$value" ]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "(empty)" "$value"
        return 1
    fi
}

# ==============================================================================
# 测试报告
# ==============================================================================

print_test_summary() {
    echo ""
    echo "========================================"
    echo "Test Summary"
    echo "========================================"
    echo -e "Total:  ${TOTAL_ASSERTIONS}"
    echo -e "Passed: ${GREEN}${PASSED_ASSERTIONS}${NC}"
    echo -e "Failed: ${RED}${FAILED_ASSERTIONS}${NC}"
    echo "========================================"

    if [ "$FAILED_ASSERTIONS" -gt 0 ]; then
        return 1
    fi
    return 0
}

reset_test_stats() {
    TOTAL_ASSERTIONS=0
    PASSED_ASSERTIONS=0
    FAILED_ASSERTIONS=0
}

get_failed_count() {
    echo "$FAILED_ASSERTIONS"
}

get_passed_count() {
    echo "$PASSED_ASSERTIONS"
}

get_total_count() {
    echo "$TOTAL_ASSERTIONS"
}
