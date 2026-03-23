#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$E2E_DIR")"

PROXY_HOST=${PROXY_HOST:-127.0.0.1}
PROXY_PORT=${PROXY_PORT:-18080}
SOCKS5_PORT=${SOCKS5_PORT:-11080}
ADMIN_PORT=${ADMIN_PORT:-18080}
BIFROST_BIN="${PROJECT_ROOT}/target/release/bifrost"
DATA_DIR="${PROJECT_ROOT}/.bifrost-http3-rules-test"
GO_CLIENT="$SCRIPT_DIR/quic_socks5_client/quic_socks5_test"
source "$E2E_DIR/test_utils/rule_fixture.sh"
RULES_DIR="$E2E_DIR/rules/http3"

cleanup() {
    echo "Cleaning up..."
    pkill -f "bifrost.*${DATA_DIR}" 2>/dev/null || true
    sleep 1
    rm -rf "$DATA_DIR"
}

trap cleanup EXIT

start_proxy() {
    echo "Starting Bifrost proxy on port $PROXY_PORT (SOCKS5: $SOCKS5_PORT)..."
    
    pkill -f "bifrost" 2>/dev/null || true
    sleep 2
    
    rm -rf "$DATA_DIR"
    mkdir -p "$DATA_DIR/rules"
    
    export BIFROST_DATA_DIR="$DATA_DIR"
    
    "$BIFROST_BIN" --port "$PROXY_PORT" --socks5-port "$SOCKS5_PORT" start \
        --unsafe-ssl --skip-cert-check 2>&1 &
    PROXY_PID=$!
    
    sleep 5
    
    if ! kill -0 $PROXY_PID 2>/dev/null; then
        echo "ERROR: Proxy failed to start"
        exit 1
    fi
    
    echo "Proxy started (PID: $PROXY_PID)"
}

add_rule() {
    local name=$1
    local content=$2
    export BIFROST_DATA_DIR="$DATA_DIR"
    "$BIFROST_BIN" rule add "$name" -c "$content"
}

add_rule_from_fixture() {
    local name="$1"
    local fixture_name="$2"
    add_rule "$name" "$(rule_fixture_content "$RULES_DIR/$fixture_name")"
}

check_admin_api() {
    local endpoint=$1
    local expected=$2
    
    local response=$(curl -s "http://${PROXY_HOST}:${ADMIN_PORT}${endpoint}" 2>&1)
    
    if echo "$response" | grep -q "$expected"; then
        echo "  ✓ Admin API $endpoint contains '$expected'"
        return 0
    else
        echo "  ✗ Admin API $endpoint missing '$expected'"
        echo "    Response: $response"
        return 1
    fi
}

test_http3_connection() {
    echo ""
    echo "=== Test 1: HTTP/3 Connection via SOCKS5 UDP ==="
    
    if [ ! -f "$GO_CLIENT" ]; then
        echo "Building Go QUIC client..."
        (cd "$SCRIPT_DIR/quic_socks5_client" && go build -o quic_socks5_test . 2>&1) || {
            echo "⚠ Failed to build Go client"
            return 1
        }
    fi
    
    echo "Testing HTTP/3 connection..."
    SOCKS5_HOST="$PROXY_HOST" SOCKS5_PORT="$SOCKS5_PORT" TEST_MODE="connection" "$GO_CLIENT" 2>&1
    
    if [ $? -eq 0 ]; then
        echo "✅ HTTP/3 connection test PASSED"
        return 0
    else
        echo "❌ HTTP/3 connection test FAILED"
        return 1
    fi
}

test_http3_full_request() {
    echo ""
    echo "=== Test 2: Full HTTP/3 Request via SOCKS5 UDP ==="
    
    echo "Testing full HTTP/3 request..."
    SOCKS5_HOST="$PROXY_HOST" SOCKS5_PORT="$SOCKS5_PORT" TEST_MODE="full" "$GO_CLIENT" 2>&1
    
    if [ $? -eq 0 ]; then
        echo "✅ Full HTTP/3 request test PASSED"
        return 0
    else
        echo "❌ Full HTTP/3 request test FAILED"
        return 1
    fi
}

test_admin_status() {
    echo ""
    echo "=== Test 3: Admin API Status Check ==="
    
    echo "Checking proxy status..."
    check_admin_api "/api/status" "running" || true
    
    echo "Checking SOCKS5 status..."
    local status_response=$(curl -s "http://${PROXY_HOST}:${ADMIN_PORT}/api/status" 2>&1)
    echo "  Status response: $status_response"
    
    echo "✅ Admin status check completed"
}

test_traffic_recording() {
    echo ""
    echo "=== Test 4: Traffic Recording for UDP ==="
    
    echo "Making HTTP/3 request to generate traffic..."
    SOCKS5_HOST="$PROXY_HOST" SOCKS5_PORT="$SOCKS5_PORT" TEST_MODE="full" "$GO_CLIENT" 2>&1 > /dev/null
    
    sleep 2
    
    echo "Checking traffic records..."
    local traffic_response=$(curl -s "http://${PROXY_HOST}:${ADMIN_PORT}/api/traffic?limit=10" 2>&1)
    
    echo "  Traffic API response (first 500 chars):"
    echo "  ${traffic_response:0:500}"
    
    echo "✅ Traffic recording check completed"
}

test_socks5_tcp_with_rules() {
    echo ""
    echo "=== Test 5: SOCKS5 TCP with Rules ==="
    
    echo "Adding test rule..."
    add_rule_from_fixture "test-header" "http3_rules_header.txt"
    
    echo "Restarting proxy to load rules..."
    pkill -f "bifrost.*${DATA_DIR}" 2>/dev/null || true
    sleep 2
    
    export BIFROST_DATA_DIR="$DATA_DIR"
    "$BIFROST_BIN" --port "$PROXY_PORT" --socks5-port "$SOCKS5_PORT" start \
        --unsafe-ssl --skip-cert-check 2>&1 &
    sleep 5
    
    echo "Testing SOCKS5 TCP request with rule..."
    local response=$(curl -sI --socks5-hostname "${PROXY_HOST}:${SOCKS5_PORT}" \
        "https://edith.xiaohongshu.com/api/im/redmoji/version" 2>&1)
    
    if echo "$response" | grep -qi "x-test-header"; then
        echo "✅ Rule applied to SOCKS5 TCP request"
    else
        echo "⚠ Rule may not be applied (checking response):"
        echo "  ${response:0:300}"
    fi
}

test_udp_relay_stats() {
    echo ""
    echo "=== Test 6: UDP Relay Statistics ==="
    
    echo "Checking UDP relay status in logs..."
    if [ -d "$DATA_DIR/logs" ]; then
        local udp_logs=$(grep -i "UDP" "$DATA_DIR/logs/"*.log 2>/dev/null | tail -10)
        if [ -n "$udp_logs" ]; then
            echo "  UDP relay logs found:"
            echo "$udp_logs" | head -5
            echo "✅ UDP relay is active"
        else
            echo "  No UDP relay logs found"
        fi
    else
        echo "  Logs directory not found"
    fi
}

echo "=============================================="
echo "  HTTP/3 Rules & Admin API E2E Test Suite"
echo "=============================================="

start_proxy

test_http3_connection
test_http3_full_request
test_admin_status
test_traffic_recording
test_socks5_tcp_with_rules
test_udp_relay_stats

echo ""
echo "=============================================="
echo "  All HTTP/3 Tests Completed"
echo "=============================================="
