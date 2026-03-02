#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

PROXY_PORT="${PROXY_PORT:-18888}"
MOCK_HTTP_PORT="${MOCK_HTTP_PORT:-13000}"
ADMIN_PORT="$PROXY_PORT"
ADMIN_BASE_URL="http://127.0.0.1:${ADMIN_PORT}/_bifrost"

source "$SCRIPT_DIR/../test_utils/assert.sh"
source "$SCRIPT_DIR/../test_utils/http_client.sh"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"

BIFROST_PID=""
MOCK_HTTP_PID=""
passed=0
failed=0

cleanup() {
    echo ""
    echo "Cleaning up..."
    
    if [ -n "$BIFROST_PID" ] && kill -0 "$BIFROST_PID" 2>/dev/null; then
        echo "  Stopping Bifrost proxy (PID: $BIFROST_PID)..."
        kill "$BIFROST_PID" 2>/dev/null || true
        wait "$BIFROST_PID" 2>/dev/null || true
    fi
    
    if [ -n "$MOCK_HTTP_PID" ] && kill -0 "$MOCK_HTTP_PID" 2>/dev/null; then
        echo "  Stopping Mock HTTP server (PID: $MOCK_HTTP_PID)..."
        kill "$MOCK_HTTP_PID" 2>/dev/null || true
        wait "$MOCK_HTTP_PID" 2>/dev/null || true
    fi
    
    echo "Cleanup complete."
}

trap cleanup EXIT

start_mock_server() {
    echo "Starting Mock HTTP Echo Server on port $MOCK_HTTP_PORT..."
    python3 "$SCRIPT_DIR/../mock_servers/http_echo_server.py" "$MOCK_HTTP_PORT" > /dev/null 2>&1 &
    MOCK_HTTP_PID=$!
    
    local timeout=10
    local waited=0
    while [ $waited -lt $timeout ]; do
        if curl -s "http://127.0.0.1:${MOCK_HTTP_PORT}/health" >/dev/null 2>&1; then
            echo "  Mock HTTP server started (PID: $MOCK_HTTP_PID)"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    
    if ! kill -0 "$MOCK_HTTP_PID" 2>/dev/null; then
        echo "Failed to start Mock HTTP server"
        exit 1
    fi
    echo "  Mock HTTP server started (PID: $MOCK_HTTP_PID)"
}

start_bifrost() {
    echo "Starting Bifrost proxy on port $PROXY_PORT..."
    cd "$ROOT_DIR"
    
    if [ -x "./target/release/bifrost" ]; then
        BIFROST_DATA_DIR="./.bifrost-e2e-test" ./target/release/bifrost start -p "$PROXY_PORT" --unsafe-ssl --skip-cert-check > /tmp/bifrost_e2e.log 2>&1 &
    else
        echo "Release binary not found, building..."
        BIFROST_DATA_DIR="./.bifrost-e2e-test" cargo run --release --bin bifrost -- start -p "$PROXY_PORT" --unsafe-ssl --skip-cert-check > /tmp/bifrost_e2e.log 2>&1 &
    fi
    BIFROST_PID=$!
    
    local timeout=120
    local waited=0
    while [ $waited -lt $timeout ]; do
        if curl -s "http://127.0.0.1:${PROXY_PORT}/_bifrost/api/system" >/dev/null 2>&1; then
            echo "  Bifrost proxy started (PID: $BIFROST_PID)"
            return 0
        fi
        sleep 2
        waited=$((waited + 2))
    done
    
    echo "Failed to start Bifrost proxy within ${timeout}s"
    echo "Last log:"
    tail -20 /tmp/bifrost_e2e.log
    exit 1
}

replay_request() {
    local url="$1"
    local method="${2:-GET}"
    local headers_json="${3:-[]}"
    local body="$4"
    local rule_config="$5"
    local timeout="${6:-15000}"
    
    if [ -z "$rule_config" ]; then
        rule_config='{"mode":"none"}'
    fi
    
    local body_field=""
    if [ -n "$body" ]; then
        body_field=",\"body\":\"$body\""
    fi
    
    local request_json
    request_json="{\"request\":{\"method\":\"$method\",\"url\":\"$url\",\"headers\":$headers_json$body_field},\"rule_config\":$rule_config,\"timeout_ms\":$timeout}"
    
    if [ "${DEBUG:-}" = "1" ]; then
        echo "[DEBUG] Request JSON: $request_json" >&2
    fi
    
    local response
    response=$(curl -s -X POST "${ADMIN_BASE_URL}/api/replay/execute" \
        -H "Content-Type: application/json" \
        -d "$request_json")
    
    if [ "${DEBUG:-}" = "1" ]; then
        echo "[DEBUG] Response: $response" >&2
    fi
    
    printf '%s' "$response"
}

test_reqHeaders_rule() {
    echo ""
    echo "=== Test: reqHeaders Rule ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-headers"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 reqHeaders://X-Custom-Header=custom-value-123"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local received_header
    received_header=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.headers["x-custom-header"] // empty')
    
    if [ "$status" = "200" ] && [ "$received_header" = "custom-value-123" ]; then
        _log_pass "reqHeaders rule applied: X-Custom-Header=custom-value-123"
        ((passed++))
    else
        _log_fail "reqHeaders rule not applied" "custom-value-123" "$received_header"
        ((failed++))
    fi
    
    local applied_rules
    applied_rules=$(printf '%s' "$response" | jq -r '.data.applied_rules | length // 0')
    if [ "$applied_rules" -gt 0 ]; then
        _log_pass "applied_rules returned: count=$applied_rules"
        ((passed++))
    else
        _log_fail "applied_rules not returned" ">0" "$applied_rules"
        ((failed++))
    fi
}

test_host_rule() {
    echo ""
    echo "=== Test: Host Rule ==="
    
    local url="http://fake-host.local:${MOCK_HTTP_PORT}/test-host"
    local rule_config='{"mode":"custom","custom_rules":"fake-host.local host://127.0.0.1:'"${MOCK_HTTP_PORT}"'"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local server_port
    server_port=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .server.port // empty')
    
    if [ "$status" = "200" ] && [ "$server_port" = "$MOCK_HTTP_PORT" ]; then
        _log_pass "Host rule applied: redirected to 127.0.0.1:${MOCK_HTTP_PORT}"
        ((passed++))
    else
        _log_fail "Host rule not applied" "port=$MOCK_HTTP_PORT" "status=$status, port=$server_port"
        ((failed++))
    fi
}

test_method_rule() {
    echo ""
    echo "=== Test: Method Rule ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-method"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 method://POST"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local received_method
    received_method=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.method // empty')
    
    if [ "$status" = "200" ] && [ "$received_method" = "POST" ]; then
        _log_pass "Method rule applied: GET -> POST"
        ((passed++))
    else
        _log_fail "Method rule not applied" "POST" "$received_method"
        ((failed++))
    fi
}

test_ua_rule() {
    echo ""
    echo "=== Test: User-Agent Rule ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-ua"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 ua://CustomUA/1.0-test"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local received_ua
    received_ua=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.headers["user-agent"] // empty')
    
    if [ "$status" = "200" ] && [ "$received_ua" = "CustomUA/1.0-test" ]; then
        _log_pass "UA rule applied: User-Agent=CustomUA/1.0-test"
        ((passed++))
    else
        _log_fail "UA rule not applied" "CustomUA/1.0-test" "$received_ua"
        ((failed++))
    fi
}

test_referer_rule() {
    echo ""
    echo "=== Test: Referer Rule ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-referer"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 referer://https://example.com/page"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local received_referer
    received_referer=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.headers["referer"] // empty')
    
    if [ "$status" = "200" ] && [ "$received_referer" = "https://example.com/page" ]; then
        _log_pass "Referer rule applied: Referer=https://example.com/page"
        ((passed++))
    else
        _log_fail "Referer rule not applied" "https://example.com/page" "$received_referer"
        ((failed++))
    fi
}

test_urlParams_rule() {
    echo ""
    echo "=== Test: urlParams Rule ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-params?existing=value"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 urlParams://added_param=new_value"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local existing_param
    existing_param=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.query_params.existing[0] // empty')
    
    local added_param
    added_param=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.query_params.added_param[0] // empty')
    
    if [ "$status" = "200" ] && [ "$existing_param" = "value" ] && [ "$added_param" = "new_value" ]; then
        _log_pass "urlParams rule applied: added_param=new_value (existing param preserved)"
        ((passed++))
    else
        _log_fail "urlParams rule not applied" "existing=value, added_param=new_value" "existing=$existing_param, added_param=$added_param"
        ((failed++))
    fi
}

test_reqCookies_rule() {
    echo ""
    echo "=== Test: reqCookies Rule ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-cookies"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 reqCookies://session_id=abc123"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local received_cookie
    received_cookie=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.cookies.session_id // empty')
    
    if [ "$status" = "200" ] && [ "$received_cookie" = "abc123" ]; then
        _log_pass "reqCookies rule applied: session_id=abc123"
        ((passed++))
    else
        _log_fail "reqCookies rule not applied" "abc123" "$received_cookie"
        ((failed++))
    fi
}

test_reqBody_rule() {
    echo ""
    echo "=== Test: reqBody Rule ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-body"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 reqBody://{replaced_body_content}"}'
    
    local response
    response=$(replay_request "$url" "POST" '[["Content-Type", "text/plain"]]' "original_body" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local received_body
    received_body=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.body // empty')
    
    if [ "$status" = "200" ] && [ "$received_body" = "replaced_body_content" ]; then
        _log_pass "reqBody rule applied: body replaced"
        ((passed++))
    else
        _log_fail "reqBody rule not applied" "replaced_body_content" "$received_body"
        ((failed++))
    fi
}

test_delete_header_rule() {
    echo ""
    echo "=== Test: Delete Header Rule ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-delete-header"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 delete://X-Remove-Me"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"], ["X-Remove-Me", "should-be-removed"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local removed_header
    removed_header=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.headers["x-remove-me"] // "null"')
    
    if [ "$status" = "200" ] && [ "$removed_header" = "null" ]; then
        _log_pass "Delete header rule applied: X-Remove-Me removed"
        ((passed++))
    else
        _log_fail "Delete header rule not applied" "null (removed)" "$removed_header"
        ((failed++))
    fi
}

test_multiple_rules() {
    echo ""
    echo "=== Test: Multiple Rules Combined ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-multi"
    local rule_config='{"mode":"custom","custom_rules":"127.0.0.1 reqHeaders://X-Multi-1=value1\n127.0.0.1 reqHeaders://X-Multi-2=value2\n127.0.0.1 ua://MultiTestUA/2.0"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local header1
    header1=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.headers["x-multi-1"] // empty')
    
    local header2
    header2=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.headers["x-multi-2"] // empty')
    
    local ua
    ua=$(printf '%s' "$response" | jq -r '.data.body | fromjson | .request.headers["user-agent"] // empty')
    
    local applied_count
    applied_count=$(printf '%s' "$response" | jq -r '.data.applied_rules | length // 0')
    
    if [ "$status" = "200" ] && [ "$header1" = "value1" ] && [ "$header2" = "value2" ] && [ "$ua" = "MultiTestUA/2.0" ]; then
        _log_pass "Multiple rules applied: 3 rules, all effective"
        ((passed++))
    else
        _log_fail "Multiple rules not fully applied" "X-Multi-1=value1, X-Multi-2=value2, UA=MultiTestUA/2.0" "h1=$header1, h2=$header2, ua=$ua"
        ((failed++))
    fi
    
    if [ "$applied_count" = "3" ]; then
        _log_pass "Applied rules count correct: $applied_count"
        ((passed++))
    else
        _log_fail "Applied rules count incorrect" "3" "$applied_count"
        ((failed++))
    fi
}

test_no_rules_mode() {
    echo ""
    echo "=== Test: No Rules Mode ==="
    
    local url="http://127.0.0.1:${MOCK_HTTP_PORT}/test-no-rules"
    local rule_config='{"mode":"none"}'
    
    local response
    response=$(replay_request "$url" "GET" '[["Accept", "application/json"]]' "" "$rule_config")
    
    local status
    status=$(printf '%s' "$response" | jq -r '.data.status // empty')
    
    local applied_count
    applied_count=$(printf '%s' "$response" | jq -r '.data.applied_rules | length // 0')
    
    if [ "$status" = "200" ] && [ "$applied_count" = "0" ]; then
        _log_pass "No rules mode: request sent without rules"
        ((passed++))
    else
        _log_fail "No rules mode failed" "applied_rules=0" "applied_rules=$applied_count"
        ((failed++))
    fi
}

test_sse_replay_with_rules() {
    echo ""
    echo "=== Test: SSE Replay with Rules ==="
    
    local response
    response=$(curl -s --max-time 5 -X POST "http://127.0.0.1:${PROXY_PORT}/_bifrost/api/replay/execute/stream" \
        -H "Content-Type: application/json" \
        -d "{
            \"url\": \"http://127.0.0.1:${MOCK_HTTP_PORT}/test-sse\",
            \"method\": \"POST\",
            \"headers\": [[\"Accept\", \"text/event-stream\"]],
            \"body\": \"test-sse-body\",
            \"rule_config\": {
                \"mode\": \"custom\",
                \"custom_rules\": \"127.0.0.1 reqHeaders://X-SSE-Test=sse-rule-applied\"
            }
        }" 2>&1 || true)
    
    if printf '%s' "$response" | grep -q "connection"; then
        local applied_rules_count
        applied_rules_count=$(printf '%s' "$response" | grep -o '"applied_rules":\[' | head -1 || echo "")
        
        if [ -n "$applied_rules_count" ]; then
            _log_pass "SSE Replay: connection event with applied_rules returned"
            ((passed++))
        else
            _log_fail "SSE Replay: connection event missing applied_rules" "applied_rules in response" "not found"
            ((failed++))
        fi
    else
        _log_fail "SSE Replay: no connection event" "connection event" "not found"
        ((failed++))
    fi
}

main() {
    echo "=========================================="
    echo "  Replay Rules E2E Tests"
    echo "=========================================="
    echo ""
    echo "Configuration:"
    echo "  PROXY_PORT: $PROXY_PORT"
    echo "  MOCK_HTTP_PORT: $MOCK_HTTP_PORT"
    echo ""
    
    start_mock_server
    start_bifrost
    
    sleep 2
    
    test_reqHeaders_rule
    test_host_rule
    test_method_rule
    test_ua_rule
    test_referer_rule
    test_urlParams_rule
    test_reqCookies_rule
    test_reqBody_rule
    test_delete_header_rule
    test_multiple_rules
    test_no_rules_mode
    test_sse_replay_with_rules
    
    echo ""
    echo "=========================================="
    echo "  Test Results"
    echo "=========================================="
    echo "  Passed: $passed"
    echo "  Failed: $failed"
    echo "=========================================="
    
    if [ "$failed" -gt 0 ]; then
        exit 1
    fi
    exit 0
}

main "$@"
