#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_DIR="$(cd "$E2E_DIR/.." && pwd)"

source "$E2E_DIR/test_utils/assert.sh"
source "$E2E_DIR/test_utils/http_client.sh"

PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${PROXY_PORT:-8080}"
ECHO_HTTP_PORT="${ECHO_HTTP_PORT:-3000}"
TEST_ID="${TEST_ID:-req_res_script}"
export TEST_ID

TEST_DATA_DIR="$PROJECT_DIR/.bifrost-test-req-res-script"
PROXY_LOG_FILE="$TEST_DATA_DIR/proxy.log"
MOCK_LOG_FILE="$TEST_DATA_DIR/mock.log"
PROXY_PID=""

cleanup() {
    if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
        kill "$PROXY_PID" 2>/dev/null || true
        wait "$PROXY_PID" 2>/dev/null || true
    fi

    "$E2E_DIR/mock_servers/start_servers.sh" stop 2>/dev/null || true
}

trap cleanup EXIT

assert_body_contains_ci() {
    local expected_substring=$1
    local body=$2
    local message=${3:-"Response body should contain '$expected_substring'"}
    local expected_lower
    local body_lower
    expected_lower=$(echo "$expected_substring" | tr '[:upper:]' '[:lower:]')
    body_lower=$(echo "$body" | tr '[:upper:]' '[:lower:]')

    if [[ "$body_lower" == *"$expected_lower"* ]]; then
        _log_pass "$message"
        return 0
    else
        _log_fail "$message" "Contains '$expected_substring'" "${body:0:200}..."
        return 1
    fi
}

start_mock_servers() {
    mkdir -p "$TEST_DATA_DIR"

    "$E2E_DIR/mock_servers/start_servers.sh" stop >/dev/null 2>&1 || true
    "$E2E_DIR/mock_servers/start_servers.sh" start > "$MOCK_LOG_FILE" 2>&1 &

    local count=0
    while ! curl -s "http://127.0.0.1:${ECHO_HTTP_PORT}/health" >/dev/null 2>&1; do
        count=$((count + 1))
        if [[ $count -ge 30 ]]; then
            cat "$MOCK_LOG_FILE"
            exit 1
        fi
        sleep 1
    done
}

write_scripts() {
    mkdir -p "$TEST_DATA_DIR/scripts/request"
    mkdir -p "$TEST_DATA_DIR/scripts/response"

    cat > "$TEST_DATA_DIR/scripts/request/req_script.js" <<'EOF'
request.headers["X-ReqScript"] = "enabled";
request.headers["X-ReqScript-Protocol"] = request.protocol;
if (request.method === "POST") {
  request.body = "body-from-reqscript";
}
EOF

    cat > "$TEST_DATA_DIR/scripts/response/res_script.js" <<'EOF'
response.headers["X-ResScript"] = "enabled";
if (response.request.path.indexOf("/res-body") >= 0 && response.body) {
  response.body = response.body + "::res-script";
}
EOF
}

start_proxy() {
    mkdir -p "$TEST_DATA_DIR"

    local rules_file="$E2E_DIR/rules/request_modify/req_res_script.txt"
    if [[ ! -f "$rules_file" ]]; then
        exit 1
    fi

    local bifrost_bin="$PROJECT_DIR/target/release/bifrost"
    if [[ ! -x "$bifrost_bin" ]]; then
        exit 1
    fi

    BIFROST_DATA_DIR="$TEST_DATA_DIR" \
    "$bifrost_bin" \
        -p "$PROXY_PORT" \
        start \
        --unsafe-ssl \
        --rules-file "$rules_file" \
        > "$PROXY_LOG_FILE" 2>&1 &

    PROXY_PID=$!

    local count=0
    while ! nc -z "$PROXY_HOST" "$PROXY_PORT" 2>/dev/null; do
        count=$((count + 1))
        if [[ $count -ge 60 ]]; then
            cat "$PROXY_LOG_FILE"
            exit 1
        fi
        sleep 1
    done
}

test_req_script() {
    local url="http://script-test.local/echo"
    http_post "$url" "origin-body"
    assert_status_2xx "$HTTP_STATUS" "reqScript should allow proxy request"

    assert_body_contains_ci "\\\"x-reqscript\\\": \\\"enabled\\\"" "$HTTP_BODY" "reqScript should inject request header"
    assert_body_contains_ci "\\\"x-reqscript-protocol\\\": \\\"http\\\"" "$HTTP_BODY" "reqScript should expose protocol"
    assert_body_contains "\\\"body\\\": \\\"body-from-reqscript\\\"" "$HTTP_BODY" "reqScript should update request body"

    assert_header_value "X-ResScript" "enabled" "$HTTP_HEADERS" "resScript should add response header"
}

test_res_script_body() {
    local url="http://script-test.local/res-body"
    http_get "$url"
    assert_status_2xx "$HTTP_STATUS" "resScript should allow proxy request"
    assert_body_contains "::res-script" "$HTTP_BODY" "resScript should append response body"
}

main() {
    start_mock_servers
    write_scripts
    start_proxy
    test_req_script
    test_res_script_body

    echo "========================================"
    echo "Total:  $TOTAL_ASSERTIONS"
    echo "Passed: $PASSED_ASSERTIONS"
    echo "Failed: $FAILED_ASSERTIONS"
    echo "========================================"
    [ "$FAILED_ASSERTIONS" -eq 0 ]
}

main "$@"
