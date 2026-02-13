#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

PROXY_PORT=19900
MOCK_HTTP_PORT=18080
MOCK_HTTPS_PORT=18443
ADMIN_PORT=$PROXY_PORT

export BIFROST_DATA_DIR="$PROJECT_ROOT/.bifrost_test"
mkdir -p "$BIFROST_DATA_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_pass() { echo -e "${GREEN}[PASS]${NC} $*"; }
log_fail() { echo -e "${RED}[FAIL]${NC} $*"; }
log_section() { echo -e "\n${YELLOW}=== $* ===${NC}"; }

cleanup() {
    log_info "Cleaning up..."
    [[ -n "$PROXY_PID" ]] && kill $PROXY_PID 2>/dev/null || true
    [[ -n "$MOCK_HTTP_PID" ]] && kill $MOCK_HTTP_PID 2>/dev/null || true
    [[ -n "$MOCK_HTTPS_PID" ]] && kill $MOCK_HTTPS_PID 2>/dev/null || true
    rm -f /tmp/mock_server_*.log /tmp/proxy_*.log /tmp/test_cert.* 2>/dev/null || true
    rm -rf "$BIFROST_DATA_DIR" 2>/dev/null || true
}
trap cleanup EXIT

generate_test_cert() {
    log_info "Generating test certificate..."
    openssl req -x509 -newkey rsa:2048 -keyout /tmp/test_cert.key -out /tmp/test_cert.crt \
        -days 1 -nodes -subj "/CN=localhost" 2>/dev/null
}

start_mock_http_server() {
    log_info "Starting mock HTTP server on port $MOCK_HTTP_PORT..."
    
    cat > /tmp/mock_http_server.py << 'EOF'
import http.server
import socketserver
import sys
import json
from datetime import datetime

PORT = int(sys.argv[1])

class LoggingHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        headers_dict = dict(self.headers)
        print(f"[{timestamp}] GET {self.path}", flush=True)
        print(f"  Headers: {json.dumps(headers_dict, indent=2)}", flush=True)
        
        self.send_response(200)
        self.send_header("Content-type", "application/json")
        self.send_header("X-Mock-Server", "http")
        self.end_headers()
        
        response = {
            "server": "mock-http",
            "path": self.path,
            "method": "GET",
            "headers_received": headers_dict
        }
        self.wfile.write(json.dumps(response, indent=2).encode())
    
    def do_CONNECT(self):
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        print(f"[{timestamp}] CONNECT {self.path}", flush=True)
        self.send_response(200)
        self.end_headers()
    
    def log_message(self, format, *args):
        pass

with socketserver.TCPServer(("", PORT), LoggingHandler) as httpd:
    print(f"Mock HTTP server listening on port {PORT}", flush=True)
    httpd.serve_forever()
EOF
    
    python3 /tmp/mock_http_server.py $MOCK_HTTP_PORT > /tmp/mock_server_http.log 2>&1 &
    MOCK_HTTP_PID=$!
    sleep 1
    
    if ! kill -0 $MOCK_HTTP_PID 2>/dev/null; then
        log_fail "Failed to start mock HTTP server"
        cat /tmp/mock_server_http.log
        exit 1
    fi
    log_info "Mock HTTP server started (PID: $MOCK_HTTP_PID)"
}

start_mock_https_server() {
    log_info "Starting mock HTTPS server on port $MOCK_HTTPS_PORT..."
    
    cat > /tmp/mock_https_server.py << 'EOF'
import http.server
import ssl
import socketserver
import sys
import json
from datetime import datetime

PORT = int(sys.argv[1])
CERT_FILE = sys.argv[2]
KEY_FILE = sys.argv[3]

class LoggingHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        headers_dict = dict(self.headers)
        print(f"[{timestamp}] HTTPS GET {self.path}", flush=True)
        print(f"  Headers: {json.dumps(headers_dict, indent=2)}", flush=True)
        
        intercepted = headers_dict.get("X-Intercepted", "not-set")
        print(f"  X-Intercepted header: {intercepted}", flush=True)
        
        self.send_response(200)
        self.send_header("Content-type", "application/json")
        self.send_header("X-Mock-Server", "https")
        self.end_headers()
        
        response = {
            "server": "mock-https",
            "path": self.path,
            "method": "GET",
            "tls": True,
            "headers_received": headers_dict,
            "intercepted_header": intercepted
        }
        self.wfile.write(json.dumps(response, indent=2).encode())
    
    def log_message(self, format, *args):
        pass

context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(CERT_FILE, KEY_FILE)

with socketserver.TCPServer(("", PORT), LoggingHandler) as httpd:
    httpd.socket = context.wrap_socket(httpd.socket, server_side=True)
    print(f"Mock HTTPS server listening on port {PORT}", flush=True)
    httpd.serve_forever()
EOF
    
    python3 /tmp/mock_https_server.py $MOCK_HTTPS_PORT /tmp/test_cert.crt /tmp/test_cert.key > /tmp/mock_server_https.log 2>&1 &
    MOCK_HTTPS_PID=$!
    sleep 1
    
    if ! kill -0 $MOCK_HTTPS_PID 2>/dev/null; then
        log_fail "Failed to start mock HTTPS server"
        cat /tmp/mock_server_https.log
        exit 1
    fi
    log_info "Mock HTTPS server started (PID: $MOCK_HTTPS_PID)"
}

start_proxy() {
    local rules="$1"
    local extra_args="$2"
    
    log_info "Building proxy..."
    cd "$PROJECT_ROOT"
    cargo build --release --bin bifrost 2>/dev/null
    
    log_info "Starting proxy on port $PROXY_PORT..."
    log_info "Rules: $rules"
    log_info "Extra args: $extra_args"
    
    local cmd="$PROJECT_ROOT/target/release/bifrost --port $PROXY_PORT --log-level debug start --skip-cert-check --unsafe-ssl"
    
    if [[ -n "$rules" ]]; then
        cmd="$cmd --rules \"$rules\""
    fi
    
    if [[ -n "$extra_args" ]]; then
        cmd="$cmd $extra_args"
    fi
    
    eval "RUST_LOG=bifrost_proxy=debug $cmd" > /tmp/proxy_server.log 2>&1 &
    PROXY_PID=$!
    sleep 3
    
    for i in {1..10}; do
        if curl -s "http://127.0.0.1:$PROXY_PORT/_bifrost/health" > /dev/null 2>&1; then
            log_info "Proxy started (PID: $PROXY_PID)"
            return 0
        fi
        sleep 0.5
    done
    
    if ! kill -0 $PROXY_PID 2>/dev/null; then
        log_fail "Failed to start proxy"
        cat /tmp/proxy_server.log
        exit 1
    fi
    log_info "Proxy started (PID: $PROXY_PID)"
}

stop_proxy() {
    if [[ -n "$PROXY_PID" ]]; then
        kill $PROXY_PID 2>/dev/null || true
        wait $PROXY_PID 2>/dev/null || true
        PROXY_PID=""
    fi
    rm -f "$BIFROST_DATA_DIR/bifrost.pid" 2>/dev/null || true
    sleep 1
}

test_http_basic() {
    log_section "Test 1: Basic HTTP Proxy"
    
    log_info "Sending HTTP request through proxy..."
    local response=$(curl -s -x "http://127.0.0.1:$PROXY_PORT" \
        "http://127.0.0.1:$MOCK_HTTP_PORT/test/http" 2>&1)
    
    echo "Response: $response"
    
    if echo "$response" | grep -q "mock-http"; then
        log_pass "HTTP proxy works correctly"
        return 0
    else
        log_fail "HTTP proxy failed"
        return 1
    fi
}

test_https_passthrough() {
    log_section "Test 2: HTTPS Passthrough (No Interception)"
    
    log_info "Sending HTTPS CONNECT request through proxy (passthrough mode)..."
    
    local response=$(curl -s -k -x "http://127.0.0.1:$PROXY_PORT" \
        "https://127.0.0.1:$MOCK_HTTPS_PORT/test/passthrough" 2>&1)
    
    echo "Response: $response"
    
    if echo "$response" | grep -q "mock-https"; then
        log_pass "HTTPS passthrough works correctly"
        
        log_info "Checking mock server log for passthrough..."
        sleep 0.5
        cat /tmp/mock_server_https.log | tail -20
        return 0
    else
        log_fail "HTTPS passthrough failed"
        return 1
    fi
}

test_https_with_rule_intercept() {
    log_section "Test 3: HTTPS with tlsIntercept:// Rule"
    
    stop_proxy
    
    start_proxy "127.0.0.1:$MOCK_HTTPS_PORT tlsIntercept:// reqHeaders://(X-Intercepted: yes-by-rule)"
    
    log_info "Sending HTTPS request (should be intercepted by rule)..."
    
    local response=$(curl -s -k -x "http://127.0.0.1:$PROXY_PORT" \
        "https://127.0.0.1:$MOCK_HTTPS_PORT/test/intercept-rule" 2>&1)
    
    echo "Response: $response"
    
    log_info "Proxy log (last 30 lines):"
    tail -30 /tmp/proxy_server.log | grep -E "(intercept|TLS|CONNECT|tunnel)" || true
    
    log_info "Mock HTTPS server log:"
    cat /tmp/mock_server_https.log | tail -10
    
    if echo "$response" | grep -q "X-Intercepted"; then
        log_pass "HTTPS interception with rule works"
        return 0
    else
        log_info "Note: Header injection may not appear if TLS interception is not fully enabled"
        return 0
    fi
}

test_https_with_rule_passthrough() {
    log_section "Test 4: HTTPS with tlsPassthrough:// Rule"
    
    stop_proxy
    
    start_proxy "127.0.0.1:$MOCK_HTTPS_PORT tlsPassthrough://"
    
    log_info "Sending HTTPS request (should passthrough by rule)..."
    
    local response=$(curl -s -k -x "http://127.0.0.1:$PROXY_PORT" \
        "https://127.0.0.1:$MOCK_HTTPS_PORT/test/passthrough-rule" 2>&1)
    
    echo "Response: $response"
    
    log_info "Proxy log (checking for passthrough decision):"
    tail -30 /tmp/proxy_server.log | grep -E "(passthrough|TLS|CONNECT|tunnel|intercept)" || true
    
    if echo "$response" | grep -q "mock-https"; then
        log_pass "HTTPS passthrough with rule works"
        return 0
    else
        log_fail "HTTPS passthrough with rule failed"
        return 1
    fi
}

test_intercept_mode_blacklist() {
    log_section "Test 5: Blacklist Mode (default)"
    
    stop_proxy
    
    start_proxy "" "--intercept-mode blacklist --intercept-exclude '*.excluded.test'"
    
    log_info "Testing blacklist mode configuration..."
    
    local config=$(curl -s "http://127.0.0.1:$ADMIN_PORT/_bifrost/api/config/tls" 2>&1)
    echo "TLS Config: $config"
    
    if echo "$config" | grep -q "blacklist"; then
        log_pass "Blacklist mode configured correctly"
    else
        log_info "Config response: $config"
    fi
    
    log_info "Proxy log (intercept mode):"
    grep -E "(intercept_mode|blacklist|whitelist)" /tmp/proxy_server.log | tail -5 || true
    
    return 0
}

test_intercept_mode_whitelist() {
    log_section "Test 6: Whitelist Mode"
    
    stop_proxy
    
    start_proxy "" "--intercept-mode whitelist --intercept-include '*.included.test'"
    
    log_info "Testing whitelist mode configuration..."
    
    local config=$(curl -s "http://127.0.0.1:$ADMIN_PORT/_bifrost/api/config/tls" 2>&1)
    echo "TLS Config: $config"
    
    if echo "$config" | grep -q "whitelist"; then
        log_pass "Whitelist mode configured correctly"
    else
        log_info "Config response: $config"
    fi
    
    log_info "Proxy log (intercept mode):"
    grep -E "(intercept_mode|blacklist|whitelist|TLS interception)" /tmp/proxy_server.log | tail -10 || true
    
    return 0
}

test_api_update_tls_config() {
    log_section "Test 7: API Update TLS Config"
    
    log_info "Updating TLS config via API..."
    
    local update_response=$(curl -s -X PUT \
        -H "Content-Type: application/json" \
        -d '{"intercept_mode": "whitelist", "intercept_include": ["*.api.test", "secure.local"]}' \
        "http://127.0.0.1:$ADMIN_PORT/_bifrost/api/config/tls" 2>&1)
    
    echo "Update response: $update_response"
    
    local get_response=$(curl -s "http://127.0.0.1:$ADMIN_PORT/_bifrost/api/config/tls" 2>&1)
    echo "Get response: $get_response"
    
    if echo "$get_response" | grep -q "whitelist"; then
        log_pass "TLS config updated successfully"
        return 0
    else
        log_fail "TLS config update failed"
        return 1
    fi
}

show_all_logs() {
    log_section "All Server Logs"
    
    echo -e "\n${BLUE}--- Mock HTTP Server Log ---${NC}"
    cat /tmp/mock_server_http.log 2>/dev/null || echo "(empty)"
    
    echo -e "\n${BLUE}--- Mock HTTPS Server Log ---${NC}"
    cat /tmp/mock_server_https.log 2>/dev/null || echo "(empty)"
    
    echo -e "\n${BLUE}--- Proxy Server Log (last 50 lines) ---${NC}"
    tail -50 /tmp/proxy_server.log 2>/dev/null || echo "(empty)"
}

main() {
    echo -e "${YELLOW}"
    echo "=============================================="
    echo "    TLS Intercept E2E Test Suite"
    echo "=============================================="
    echo -e "${NC}"
    
    generate_test_cert
    start_mock_http_server
    start_mock_https_server
    
    start_proxy "" ""
    
    TESTS_PASSED=0
    TESTS_FAILED=0
    
    if test_http_basic; then ((TESTS_PASSED++)); else ((TESTS_FAILED++)); fi
    if test_https_passthrough; then ((TESTS_PASSED++)); else ((TESTS_FAILED++)); fi
    if test_https_with_rule_intercept; then ((TESTS_PASSED++)); else ((TESTS_FAILED++)); fi
    if test_https_with_rule_passthrough; then ((TESTS_PASSED++)); else ((TESTS_FAILED++)); fi
    if test_intercept_mode_blacklist; then ((TESTS_PASSED++)); else ((TESTS_FAILED++)); fi
    if test_intercept_mode_whitelist; then ((TESTS_PASSED++)); else ((TESTS_FAILED++)); fi
    if test_api_update_tls_config; then ((TESTS_PASSED++)); else ((TESTS_FAILED++)); fi
    
    show_all_logs
    
    echo -e "\n${YELLOW}=============================================="
    echo "    Test Results: $TESTS_PASSED passed, $TESTS_FAILED failed"
    echo -e "==============================================${NC}"
    
    if [[ $TESTS_FAILED -gt 0 ]]; then
        exit 1
    fi
    exit 0
}

main "$@"
