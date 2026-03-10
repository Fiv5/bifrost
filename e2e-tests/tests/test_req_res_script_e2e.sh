#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_DIR="$(cd "$E2E_DIR/.." && pwd)"

source "$E2E_DIR/test_utils/assert.sh"
source "$E2E_DIR/test_utils/http_client.sh"

# Admin API is served on the same port as proxy (path prefix /_bifrost)
export ADMIN_HOST="${ADMIN_HOST:-$PROXY_HOST}"
export ADMIN_PORT="${ADMIN_PORT:-$PROXY_PORT}"
export ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
source "$E2E_DIR/test_utils/admin_client.sh"

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
    mkdir -p "$TEST_DATA_DIR/scripts/decode"

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

    cat > "$TEST_DATA_DIR/scripts/decode/decode_script.js" <<'EOF'
log.info("decode phase", ctx.phase);

if (ctx.phase === "request") {
  ctx.output = {
    code: "0",
    data: "decoded-req::" + (request.body || ""),
    msg: "",
  };
} else {
  ctx.output = {
    code: "0",
    data:
      "decoded-res::" + (response.body || "") + "::req-path::" + response.request.path,
    msg: "",
  };
}
EOF
}

start_proxy() {
    mkdir -p "$TEST_DATA_DIR"

    local rules_file="$E2E_DIR/rules/request_modify/req_res_script.txt"
    if [[ ! -f "$rules_file" ]]; then
        exit 1
    fi

    echo "[e2e] Building bifrost (release)..." >>"$PROXY_LOG_FILE"
    (cd "$PROJECT_DIR" && cargo build --release --bin bifrost) >>"$PROXY_LOG_FILE" 2>&1

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

    wait_for_admin 30
}

wait_for_traffic_id() {
    local url_pattern="$1"
    local timeout_seconds="${2:-10}"

    local waited=0
    while [[ $waited -lt $timeout_seconds ]]; do
        local id
        id=$(admin_get "/api/traffic?limit=50" | jq -r ".records[] | select((.url // .p // \"\") | contains(\"$url_pattern\")) | .id" | head -1)
        if [[ -n "$id" && "$id" != "null" ]]; then
            echo "$id"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    echo "" 
    return 1
}

get_request_body_text() {
    local id="$1"
    admin_get "/api/traffic/${id}/request-body" | jq -r '.data // ""'
}

get_response_body_text() {
    local id="$1"
    admin_get "/api/traffic/${id}/response-body" | jq -r '.data // ""'
}

test_decode_script_bodies() {
    local id
    id=$(wait_for_traffic_id "/echo" 15)
    if [[ -z "$id" ]]; then
        _log_fail "decode should record traffic" "traffic id" "not found"
        return 1
    fi

    local req_body
    req_body=$(get_request_body_text "$id")
    assert_body_contains "decoded-req::" "$req_body" "decode should store decoded request body"
    assert_body_contains "decoded-req::body-from-reqscript" "$req_body" "decode should see final (post-reqScript) request body"

    local res_id
    res_id=$(wait_for_traffic_id "/res-body" 15)
    if [[ -z "$res_id" ]]; then
        _log_fail "decode should record response traffic" "traffic id" "not found"
        return 1
    fi

    local res_body
    res_body=$(get_response_body_text "$res_id")
    assert_body_contains "decoded-res::" "$res_body" "decode should store decoded response body"
    assert_body_contains "::req-path::/res-body" "$res_body" "decode response phase should carry request snapshot"
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
    test_decode_script_bodies

    echo "========================================"
    echo "Total:  $TOTAL_ASSERTIONS"
    echo "Passed: $PASSED_ASSERTIONS"
    echo "Failed: $FAILED_ASSERTIONS"
    echo "========================================"
    [ "$FAILED_ASSERTIONS" -eq 0 ]
}

main "$@"
