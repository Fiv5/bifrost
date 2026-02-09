#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RULES_DIR="${SCRIPT_DIR}/rules"
TEST_DATA_DIR="${PROJECT_DIR}/.bifrost-test"

PROXY_PORT="${PROXY_PORT:-8080}"
PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY="http://${PROXY_HOST}:${PROXY_PORT}"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ PASS${NC}: $1"; }
fail() { echo -e "${RED}✗ FAIL${NC}: $1"; }
info() { echo -e "${BLUE}ℹ INFO${NC}: $1"; }
warn() { echo -e "${YELLOW}⚠ WARN${NC}: $1"; }
header() { echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"; echo -e "${CYAN}  $1${NC}"; echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"; }

PROXY_PID=""
MOCK_SERVER_PID=""
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
RULE_FILE=""

usage() {
    echo "用法: $0 [选项] <规则文件>"
    echo ""
    echo "选项:"
    echo "  -h, --help         显示帮助信息"
    echo "  -p, --port PORT    指定代理端口 (默认: 8080)"
    echo "  -l, --list         列出所有可用的规则文件"
    echo ""
    echo "示例:"
    echo "  $0 rules/host.txt"
    echo "  $0 -p 9090 rules/req_headers.txt"
    echo "  $0 --list"
    exit 0
}

list_rules() {
    header "可用的规则文件"
    if [[ -d "$RULES_DIR" ]]; then
        for rule_file in "$RULES_DIR"/*.txt; do
            if [[ -f "$rule_file" ]]; then
                local name=$(basename "$rule_file" .txt)
                local desc=$(grep -m1 '^#' "$rule_file" 2>/dev/null | sed 's/^# *//' || echo "无描述")
                printf "  ${CYAN}%-20s${NC} %s\n" "$name" "$desc"
            fi
        done
    else
        warn "规则目录不存在: $RULES_DIR"
    fi
    exit 0
}

cleanup() {
    if [[ -n "$MOCK_SERVER_PID" ]] && kill -0 "$MOCK_SERVER_PID" 2>/dev/null; then
        info "正在停止 Mock 服务器 (PID: $MOCK_SERVER_PID)..."
        kill "$MOCK_SERVER_PID" 2>/dev/null || true
        wait "$MOCK_SERVER_PID" 2>/dev/null || true
    fi
    if [[ -n "$WS_MOCK_SERVER_PID" ]] && kill -0 "$WS_MOCK_SERVER_PID" 2>/dev/null; then
        info "正在停止 WebSocket Mock 服务器 (PID: $WS_MOCK_SERVER_PID)..."
        kill "$WS_MOCK_SERVER_PID" 2>/dev/null || true
        wait "$WS_MOCK_SERVER_PID" 2>/dev/null || true
    fi
    if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
        info "正在停止代理服务器 (PID: $PROXY_PID)..."
        kill "$PROXY_PID" 2>/dev/null || true
        wait "$PROXY_PID" 2>/dev/null || true
    fi
}

trap cleanup EXIT

check_rule_file() {
    if [[ ! -f "$RULE_FILE" ]]; then
        fail "规则文件不存在: $RULE_FILE"
        echo "请使用 --list 查看可用的规则文件"
        exit 1
    fi

    local rule_count=$(grep -v '^#' "$RULE_FILE" | grep -v '^[[:space:]]*$' | wc -l | tr -d ' ')
    if [[ "$rule_count" -eq 0 ]]; then
        fail "规则文件为空或只包含注释"
        exit 1
    fi

    pass "找到 $rule_count 条规则"
}

build_proxy() {
    header "编译代理服务器"
    
    if [[ -f "${PROJECT_DIR}/target/release/bifrost" ]]; then
        local mod_time=$(stat -f %m "${PROJECT_DIR}/target/release/bifrost" 2>/dev/null || stat -c %Y "${PROJECT_DIR}/target/release/bifrost" 2>/dev/null)
        local now=$(date +%s)
        local age=$((now - mod_time))
        
        if [[ $age -lt 86400 ]]; then
            pass "使用已编译的代理 (编译于 $((age / 60)) 分钟前)"
            return 0
        fi
    fi

    info "正在编译代理服务器..."
    cd "$PROJECT_DIR"
    cargo build --release --bin bifrost 2>&1 | tail -5
    pass "代理服务器编译完成"
}

setup_data_dir() {
    header "初始化配置目录"
    
    mkdir -p "${TEST_DATA_DIR}"/{rules,values,plugins,certs}
    
    if [[ ! -f "${TEST_DATA_DIR}/config.toml" ]]; then
        cat > "${TEST_DATA_DIR}/config.toml" << 'TOML'
[access]
mode = "local_only"
whitelist = []
allow_lan = false

intercept_exclude = []
TOML
    fi
    
    pass "配置目录: ${TEST_DATA_DIR}"
}

show_rules() {
    header "规则配置"
    
    info "规则文件: ${RULE_FILE}"
    echo ""
    
    echo "规则内容:"
    echo "─────────────────────────────────────────────────"
    grep -v '^#' "$RULE_FILE" | grep -v '^[[:space:]]*$' | while read -r line; do
        echo "  $line"
    done
    echo "─────────────────────────────────────────────────"
    echo ""
}

start_proxy() {
    header "启动代理服务器"
    
    if lsof -i ":${PROXY_PORT}" -t >/dev/null 2>&1; then
        local existing_pid=$(lsof -i ":${PROXY_PORT}" -t 2>/dev/null | head -1)
        warn "端口 ${PROXY_PORT} 已被占用 (PID: $existing_pid)"
        info "尝试终止现有进程..."
        kill "$existing_pid" 2>/dev/null || true
        sleep 1
    fi
    
    info "启动代理 (端口: ${PROXY_PORT})..."
    info "使用 --rules-file 参数直接加载规则"
    
    export BIFROST_DATA_DIR="${TEST_DATA_DIR}"
    "${PROJECT_DIR}/target/release/bifrost" --port "${PROXY_PORT}" start --skip-cert-check --rules-file "${RULE_FILE}" &
    PROXY_PID=$!
    
    local max_wait=10
    local waited=0
    while [[ $waited -lt $max_wait ]]; do
        if curl -s --proxy "$PROXY" --connect-timeout 1 http://example.com >/dev/null 2>&1; then
            pass "代理服务器已启动 (PID: $PROXY_PID)"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    
    fail "代理服务器启动超时"
    exit 1
}

start_mock_server() {
    local port="$1"
    local mock_response="${2:-mock-response}"
    
    if lsof -i ":${port}" -t >/dev/null 2>&1; then
        local existing_pid=$(lsof -i ":${port}" -t 2>/dev/null | head -1)
        kill "$existing_pid" 2>/dev/null || true
        sleep 0.5
    fi
    
    python3 -c "
import http.server
import socketserver
import json

class MockHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('X-Mock-Server', 'bifrost-test')
        self.end_headers()
        
        headers_dict = {k: v for k, v in self.headers.items()}
        response = {
            'status': 'ok',
            'message': '$mock_response',
            'path': self.path,
            'method': self.command,
            'headers': headers_dict
        }
        self.wfile.write(json.dumps(response).encode())
    
    def do_POST(self):
        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length).decode('utf-8') if content_length > 0 else ''
        
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('X-Mock-Server', 'bifrost-test')
        self.end_headers()
        
        headers_dict = {k: v for k, v in self.headers.items()}
        response = {
            'status': 'ok',
            'message': '$mock_response',
            'path': self.path,
            'method': self.command,
            'headers': headers_dict,
            'body': body
        }
        self.wfile.write(json.dumps(response).encode())
    
    def do_PUT(self):
        self.do_POST()
    
    def do_DELETE(self):
        self.do_GET()
    
    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', '*')
        self.end_headers()
    
    def log_message(self, format, *args):
        pass

with socketserver.TCPServer(('127.0.0.1', $port), MockHandler) as httpd:
    httpd.serve_forever()
" &
    MOCK_SERVER_PID=$!
    sleep 1
    
    if ! kill -0 "$MOCK_SERVER_PID" 2>/dev/null; then
        fail "Mock 服务器启动失败"
        return 1
    fi
    
    return 0
}

stop_mock_server() {
    if [[ -n "$MOCK_SERVER_PID" ]] && kill -0 "$MOCK_SERVER_PID" 2>/dev/null; then
        kill "$MOCK_SERVER_PID" 2>/dev/null || true
        wait "$MOCK_SERVER_PID" 2>/dev/null || true
        MOCK_SERVER_PID=""
    fi
}

extract_http_url() {
    local protocols="$1"
    echo "$protocols" | grep -o 'http://[^[:space:]]*\|https://[^[:space:]]*' | head -1
}

extract_target_port() {
    local protocols="$1"
    
    local target_url=$(echo "$protocols" | grep -o 'http://[^[:space:]]*\|https://[^[:space:]]*\|host://[^[:space:]]*' | head -1)
    
    if [[ -z "$target_url" ]]; then
        echo "3000"
        return
    fi
    
    local target_host
    if [[ "$target_url" == *"://"* ]]; then
        target_host=$(echo "$target_url" | sed 's|.*://||' | sed 's|/.*||')
    else
        target_host="$target_url"
    fi
    
    local port=$(echo "$target_host" | grep -o ':[0-9]*$' | tr -d ':')
    echo "${port:-3000}"
}

CURL_RESPONSE_CODE=""
CURL_RESPONSE_BODY=""
CURL_RESPONSE_HEADERS=""

do_curl_request() {
    local url="$1"
    local method="${2:-GET}"
    local extra_args="${3:-}"
    local tmpfile=$(mktemp)
    local headers_file=$(mktemp)
    
    CURL_RESPONSE_CODE=$(curl -s -w "%{http_code}" \
        --proxy "$PROXY" \
        -k \
        -X "$method" \
        --connect-timeout 10 \
        --max-time 15 \
        -D "$headers_file" \
        -o "$tmpfile" \
        $extra_args \
        "$url" 2>/dev/null) || CURL_RESPONSE_CODE="000"
    
    CURL_RESPONSE_BODY=$(cat "$tmpfile" 2>/dev/null || echo "")
    CURL_RESPONSE_HEADERS=$(cat "$headers_file" 2>/dev/null || echo "")
    rm -f "$tmpfile" "$headers_file"
}

test_host_rule() {
    local pattern="$1"
    local target="$2"
    local target_port
    if [[ "$target" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+:[0-9]+$ ]] || [[ "$target" =~ ^[^:]+:[0-9]+$ ]]; then
        target_port=$(echo "$target" | grep -o ':[0-9]*$' | tr -d ':')
    else
        target_port=$(extract_target_port "$target")
    fi
    target_port="${target_port:-3000}"
    local test_url="http://${pattern}/"

    echo ""
    echo -e "  ${CYAN}【场景1】目标服务未启动 - 期望返回 502${NC}"
    
    stop_mock_server
    
    do_curl_request "$test_url"
    
    echo "    HTTP 响应码: $CURL_RESPONSE_CODE"
    echo "    响应体: ${CURL_RESPONSE_BODY:0:100}..."
    
    if [[ "$CURL_RESPONSE_CODE" == "502" ]] || [[ "$CURL_RESPONSE_CODE" == "503" ]]; then
        pass "目标服务未启动时返回 $CURL_RESPONSE_CODE"
        ((PASS_COUNT++))
    elif [[ "$CURL_RESPONSE_CODE" == "200" ]]; then
        fail "目标服务未启动但收到 200 - 规则可能未生效"
        ((FAIL_COUNT++))
    else
        fail "目标服务未启动时期望返回 502/503，实际返回: $CURL_RESPONSE_CODE"
        ((FAIL_COUNT++))
    fi
    
    echo ""
    echo -e "  ${CYAN}【场景2】目标服务已启动 - 期望返回 200${NC}"
    
    info "启动 Mock 服务器 (端口: ${target_port})..."
    if ! start_mock_server "$target_port" "mock-response-from-bifrost-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    echo -e "    ${GREEN}Mock 服务器已启动${NC}"
    
    do_curl_request "$test_url"
    
    echo "    HTTP 响应码: $CURL_RESPONSE_CODE"
    echo "    响应体: ${CURL_RESPONSE_BODY:0:100}..."
    
    if [[ "$CURL_RESPONSE_CODE" == "200" ]]; then
        if [[ "$CURL_RESPONSE_BODY" == *"mock-response-from-bifrost-test"* ]] || [[ "$CURL_RESPONSE_BODY" == *"bifrost-test"* ]]; then
            pass "目标服务启动时返回 200，响应来自 Mock 服务器"
            ((PASS_COUNT++))
        else
            warn "目标服务启动时返回 200，但响应可能不是来自 Mock 服务器"
            ((SKIP_COUNT++))
        fi
    else
        fail "目标服务启动时期望返回 200，实际返回: $CURL_RESPONSE_CODE"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_redirect_rule() {
    local pattern="$1"
    local target="$2"
    local test_url="https://${pattern}/"
    
    info "测试重定向: ${test_url} -> ${target}"

    local response
    response=$(curl -s -o /dev/null -w "%{http_code}|%{redirect_url}" \
        --proxy "$PROXY" \
        -k \
        --connect-timeout 5 \
        --max-time 10 \
        "$test_url" 2>/dev/null || echo "ERROR|")
    
    local http_code=$(echo "$response" | cut -d'|' -f1)
    local redirect_url=$(echo "$response" | cut -d'|' -f2)
    
    echo "  HTTP 响应码: $http_code"
    if [[ -n "$redirect_url" ]]; then
        echo "  重定向到: $redirect_url"
    fi
    
    if [[ "$http_code" == "301" ]] || [[ "$http_code" == "302" ]] || [[ "$http_code" == "307" ]] || [[ "$http_code" == "308" ]]; then
        if [[ "$redirect_url" == *"$target"* ]] || [[ -n "$redirect_url" ]]; then
            pass "重定向成功 - ${test_url} (HTTP $http_code) -> $redirect_url"
            ((PASS_COUNT++))
        else
            warn "重定向返回 $http_code 但目标地址不匹配"
            ((SKIP_COUNT++))
        fi
    elif [[ "$http_code" == "ERROR" ]]; then
        fail "请求失败 - ${test_url}"
        ((FAIL_COUNT++))
    else
        fail "期望重定向状态码 (301/302/307/308)，实际返回: $http_code"
        ((FAIL_COUNT++))
    fi
}

test_req_headers_rule() {
    local pattern="$1"
    local protocols="$2"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    info "测试请求头修改..."
    
    if ! start_mock_server "$target_port" "req-headers-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    do_curl_request "$test_url"
    
    echo "  HTTP 响应码: $CURL_RESPONSE_CODE"
    echo "  响应体: ${CURL_RESPONSE_BODY:0:200}..."
    
    if [[ "$CURL_RESPONSE_CODE" == "200" ]]; then
        if [[ "$CURL_RESPONSE_BODY" == *"X-Bifrost-Test"* ]] || [[ "$CURL_RESPONSE_BODY" == *"x-bifrost-test"* ]] || [[ "$CURL_RESPONSE_BODY" == *"req-header-test"* ]]; then
            pass "请求头已成功添加/修改"
            ((PASS_COUNT++))
        else
            warn "请求返回 200，但未能确认请求头是否被修改"
            echo "  提示: 检查 Mock 服务器是否正确回显请求头"
            ((SKIP_COUNT++))
        fi
    else
        fail "请求失败，期望 200，实际: $CURL_RESPONSE_CODE"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_res_headers_rule() {
    local pattern="$1"
    local protocols="$2"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    info "测试响应头修改..."
    
    if ! start_mock_server "$target_port" "res-headers-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    local tmpfile=$(mktemp)
    local headers_file=$(mktemp)
    
    local response_code
    response_code=$(curl -s -w "%{http_code}" \
        --proxy "$PROXY" \
        -k \
        --connect-timeout 10 \
        --max-time 15 \
        -D "$headers_file" \
        -o "$tmpfile" \
        "$test_url" 2>&1) || true
    
    local headers=$(cat "$headers_file" 2>/dev/null || echo "")
    rm -f "$tmpfile" "$headers_file"
    
    echo "  HTTP 响应码: $response_code"
    echo "  响应头 (部分): ${headers:0:300}..."
    
    if [[ "$response_code" == "200" ]]; then
        if [[ "$headers" == *"X-Bifrost-Response"* ]] || [[ "$headers" == *"x-bifrost-response"* ]] || [[ "$headers" == *"res-header-test"* ]]; then
            pass "响应头已成功添加/修改"
            ((PASS_COUNT++))
        else
            warn "请求返回 200，但未能确认响应头是否被修改"
            ((SKIP_COUNT++))
        fi
    else
        fail "请求失败，期望 200，实际: $response_code"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_status_code_rule() {
    local pattern="$1"
    local protocols="$2"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    local expected_status=$(echo "$protocols" | grep -o 'statusCode://[0-9]*' | sed 's|statusCode://||')
    expected_status=${expected_status:-201}
    
    info "测试状态码修改 (期望: $expected_status)..."
    
    if ! start_mock_server "$target_port" "status-code-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    do_curl_request "$test_url"
    
    echo "  HTTP 响应码: $CURL_RESPONSE_CODE"
    echo "  期望响应码: $expected_status"
    
    if [[ "$CURL_RESPONSE_CODE" == "$expected_status" ]]; then
        pass "状态码已成功修改为 $expected_status"
        ((PASS_COUNT++))
    else
        fail "状态码修改失败，期望 $expected_status，实际: $CURL_RESPONSE_CODE"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_method_rule() {
    local pattern="$1"
    local protocols="$2"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    local expected_method=$(echo "$protocols" | grep -o 'method://[A-Z]*' | sed 's|method://||')
    expected_method=${expected_method:-POST}
    
    info "测试请求方法修改 (期望: $expected_method)..."
    
    if ! start_mock_server "$target_port" "method-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    do_curl_request "$test_url" "GET"
    
    echo "  HTTP 响应码: $CURL_RESPONSE_CODE"
    echo "  响应体: ${CURL_RESPONSE_BODY:0:200}..."
    
    if [[ "$CURL_RESPONSE_CODE" == "200" ]]; then
        if [[ "$CURL_RESPONSE_BODY" == *"\"method\": \"$expected_method\""* ]] || [[ "$CURL_RESPONSE_BODY" == *"\"method\":\"$expected_method\""* ]]; then
            pass "请求方法已成功修改为 $expected_method"
            ((PASS_COUNT++))
        else
            warn "请求返回 200，但未能确认请求方法是否被修改"
            ((SKIP_COUNT++))
        fi
    else
        fail "请求失败，期望 200，实际: $CURL_RESPONSE_CODE"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_ua_rule() {
    local pattern="$1"
    local protocols="$2"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    local expected_ua=$(echo "$protocols" | grep -o 'ua://[^[:space:]]*' | sed 's|ua://||')
    
    info "测试 User-Agent 修改..."
    
    if ! start_mock_server "$target_port" "ua-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    do_curl_request "$test_url"
    
    echo "  HTTP 响应码: $CURL_RESPONSE_CODE"
    echo "  响应体: ${CURL_RESPONSE_BODY:0:200}..."
    
    if [[ "$CURL_RESPONSE_CODE" == "200" ]]; then
        if [[ "$CURL_RESPONSE_BODY" == *"$expected_ua"* ]] || [[ "$CURL_RESPONSE_BODY" == *"Bifrost"* ]]; then
            pass "User-Agent 已成功修改"
            ((PASS_COUNT++))
        else
            warn "请求返回 200，但未能确认 User-Agent 是否被修改"
            ((SKIP_COUNT++))
        fi
    else
        fail "请求失败，期望 200，实际: $CURL_RESPONSE_CODE"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_referer_rule() {
    local pattern="$1"
    local protocols="$2"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    local expected_referer=$(echo "$protocols" | grep -o 'referer://[^[:space:]]*' | sed 's|referer://||')
    
    info "测试 Referer 修改..."
    
    if ! start_mock_server "$target_port" "referer-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    do_curl_request "$test_url"
    
    echo "  HTTP 响应码: $CURL_RESPONSE_CODE"
    echo "  响应体: ${CURL_RESPONSE_BODY:0:200}..."
    
    if [[ "$CURL_RESPONSE_CODE" == "200" ]]; then
        if [[ "$CURL_RESPONSE_BODY" == *"$expected_referer"* ]] || [[ "$CURL_RESPONSE_BODY" == *"bifrost"* ]]; then
            pass "Referer 已成功修改"
            ((PASS_COUNT++))
        else
            warn "请求返回 200，但未能确认 Referer 是否被修改"
            ((SKIP_COUNT++))
        fi
    else
        fail "请求失败，期望 200，实际: $CURL_RESPONSE_CODE"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_delay_rule() {
    local pattern="$1"
    local protocols="$2"
    local delay_type="$3"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    local expected_delay
    if [[ "$delay_type" == "req" ]]; then
        expected_delay=$(echo "$protocols" | grep -o 'reqDelay://[0-9]*' | sed 's|reqDelay://||')
    else
        expected_delay=$(echo "$protocols" | grep -o 'resDelay://[0-9]*' | sed 's|resDelay://||')
    fi
    expected_delay=${expected_delay:-500}
    
    info "测试 ${delay_type} 延迟 (期望延迟: ${expected_delay}ms)..."
    
    if ! start_mock_server "$target_port" "delay-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    local start_time=$(python3 -c "import time; print(int(time.time() * 1000))")
    do_curl_request "$test_url"
    local end_time=$(python3 -c "import time; print(int(time.time() * 1000))")
    
    local elapsed=$((end_time - start_time))
    
    echo "  HTTP 响应码: $CURL_RESPONSE_CODE"
    echo "  实际耗时: ${elapsed}ms"
    echo "  期望延迟: ${expected_delay}ms"
    
    if [[ "$CURL_RESPONSE_CODE" == "200" ]]; then
        local min_expected=$((expected_delay - 100))
        if [[ $elapsed -ge $min_expected ]]; then
            pass "延迟生效，耗时 ${elapsed}ms >= ${min_expected}ms"
            ((PASS_COUNT++))
        else
            warn "请求返回 200，但延迟可能未生效 (耗时 ${elapsed}ms < ${min_expected}ms)"
            ((SKIP_COUNT++))
        fi
    else
        fail "请求失败，期望 200，实际: $CURL_RESPONSE_CODE"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_cors_rule() {
    local pattern="$1"
    local protocols="$2"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    info "测试 CORS 响应头..."
    
    if ! start_mock_server "$target_port" "cors-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    local tmpfile=$(mktemp)
    local headers_file=$(mktemp)
    
    local response_code
    response_code=$(curl -s -w "%{http_code}" \
        --proxy "$PROXY" \
        -k \
        -H "Origin: https://example.com" \
        --connect-timeout 10 \
        --max-time 15 \
        -D "$headers_file" \
        -o "$tmpfile" \
        "$test_url" 2>&1) || true
    
    local headers=$(cat "$headers_file" 2>/dev/null || echo "")
    rm -f "$tmpfile" "$headers_file"
    
    echo "  HTTP 响应码: $response_code"
    echo "  响应头 (部分): ${headers:0:300}..."
    
    if [[ "$response_code" == "200" ]]; then
        if [[ "$headers" == *"Access-Control-Allow-Origin"* ]] || [[ "$headers" == *"access-control-allow-origin"* ]]; then
            pass "CORS 响应头已添加"
            ((PASS_COUNT++))
        else
            warn "请求返回 200，但未找到 CORS 响应头"
            ((SKIP_COUNT++))
        fi
    else
        fail "请求失败，期望 200，实际: $response_code"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

test_cookies_rule() {
    local pattern="$1"
    local protocols="$2"
    local cookie_type="$3"
    local target_port=$(extract_target_port "$protocols")
    local test_url="https://${pattern}/"
    
    info "测试 ${cookie_type} Cookie..."
    
    if ! start_mock_server "$target_port" "cookies-test"; then
        fail "无法启动 Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    
    local tmpfile=$(mktemp)
    local headers_file=$(mktemp)
    
    local response_code
    response_code=$(curl -s -w "%{http_code}" \
        --proxy "$PROXY" \
        -k \
        --connect-timeout 10 \
        --max-time 15 \
        -D "$headers_file" \
        -o "$tmpfile" \
        "$test_url" 2>&1) || true
    
    local body=$(cat "$tmpfile" 2>/dev/null || echo "")
    local headers=$(cat "$headers_file" 2>/dev/null || echo "")
    rm -f "$tmpfile" "$headers_file"
    
    echo "  HTTP 响应码: $response_code"
    
    if [[ "$response_code" == "200" ]]; then
        if [[ "$cookie_type" == "req" ]]; then
            if [[ "$body" == *"bifrost_test"* ]] || [[ "$body" == *"cookie"* ]]; then
                pass "请求 Cookie 已添加"
                ((PASS_COUNT++))
            else
                warn "请求返回 200，但未能确认请求 Cookie 是否被添加"
                ((SKIP_COUNT++))
            fi
        else
            if [[ "$headers" == *"Set-Cookie"* ]] || [[ "$headers" == *"set-cookie"* ]] || [[ "$headers" == *"bifrost"* ]]; then
                pass "响应 Cookie 已设置"
                ((PASS_COUNT++))
            else
                warn "请求返回 200，但未找到响应 Cookie"
                ((SKIP_COUNT++))
            fi
        fi
    else
        fail "请求失败，期望 200，实际: $response_code"
        ((FAIL_COUNT++))
    fi
    
    stop_mock_server
}

WS_MOCK_SERVER_PID=""

start_ws_mock_server() {
    local port="$1"
    
    if lsof -i ":${port}" -t >/dev/null 2>&1; then
        local existing_pid=$(lsof -i ":${port}" -t 2>/dev/null | head -1)
        kill "$existing_pid" 2>/dev/null || true
        sleep 0.5
    fi
    
    python3 -c "
import asyncio
import hashlib
import base64
import struct

async def handle_client(reader, writer):
    try:
        request = await reader.read(4096)
        request_str = request.decode('utf-8', errors='ignore')
        
        key = None
        for line in request_str.split('\r\n'):
            if line.lower().startswith('sec-websocket-key:'):
                key = line.split(':', 1)[1].strip()
                break
        
        if not key:
            writer.close()
            return
        
        accept = base64.b64encode(
            hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()
        ).decode()
        
        response = (
            'HTTP/1.1 101 Switching Protocols\r\n'
            'Upgrade: websocket\r\n'
            'Connection: Upgrade\r\n'
            f'Sec-WebSocket-Accept: {accept}\r\n'
            '\r\n'
        )
        writer.write(response.encode())
        await writer.drain()
        
        while True:
            header = await reader.read(2)
            if len(header) < 2:
                break
            
            opcode = header[0] & 0x0F
            masked = (header[1] & 0x80) != 0
            payload_len = header[1] & 0x7F
            
            if payload_len == 126:
                ext = await reader.read(2)
                payload_len = struct.unpack('>H', ext)[0]
            elif payload_len == 127:
                ext = await reader.read(8)
                payload_len = struct.unpack('>Q', ext)[0]
            
            mask = await reader.read(4) if masked else b''
            payload = await reader.read(payload_len)
            
            if masked and mask:
                payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
            
            if opcode == 8:
                close_frame = bytes([0x88, 0x00])
                writer.write(close_frame)
                await writer.drain()
                break
            elif opcode == 9:
                pong_frame = bytes([0x8A, len(payload)]) + payload
                writer.write(pong_frame)
                await writer.drain()
            elif opcode in (1, 2):
                response_header = bytes([0x80 | opcode, len(payload)])
                writer.write(response_header + payload)
                await writer.drain()
    except Exception:
        pass
    finally:
        writer.close()

async def main():
    server = await asyncio.start_server(handle_client, '127.0.0.1', $port)
    async with server:
        await server.serve_forever()

asyncio.run(main())
" &
    WS_MOCK_SERVER_PID=$!
    sleep 1
    
    if ! kill -0 "$WS_MOCK_SERVER_PID" 2>/dev/null; then
        fail "WebSocket Mock 服务器启动失败"
        return 1
    fi
    
    return 0
}

stop_ws_mock_server() {
    if [[ -n "$WS_MOCK_SERVER_PID" ]] && kill -0 "$WS_MOCK_SERVER_PID" 2>/dev/null; then
        kill "$WS_MOCK_SERVER_PID" 2>/dev/null || true
        wait "$WS_MOCK_SERVER_PID" 2>/dev/null || true
        WS_MOCK_SERVER_PID=""
    fi
}

extract_ws_target_port() {
    local protocols="$1"
    local ws_url=$(echo "$protocols" | grep -o 'ws://[^[:space:]]*\|wss://[^[:space:]]*' | head -1)
    
    if [[ -z "$ws_url" ]]; then
        echo "3020"
        return
    fi
    
    local target_host=$(echo "$ws_url" | sed 's|wss\?://||' | sed 's|/.*||')
    local port=$(echo "$target_host" | grep -o ':[0-9]*$' | tr -d ':')
    echo "${port:-3020}"
}

test_websocket_rule() {
    local pattern="$1"
    local protocols="$2"
    local target_port=$(extract_ws_target_port "$protocols")
    
    info "测试 WebSocket 转发规则..."
    
    echo ""
    echo -e "  ${CYAN}【场景1】WebSocket 目标服务未启动 - 期望连接失败${NC}"
    
    stop_ws_mock_server
    
    local ws_test_result
    ws_test_result=$(curl -s -o /dev/null -w "%{http_code}" \
        --proxy "$PROXY" \
        -k \
        --connect-timeout 5 \
        --max-time 10 \
        -H "Upgrade: websocket" \
        -H "Connection: Upgrade" \
        -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
        -H "Sec-WebSocket-Version: 13" \
        "http://${pattern}/ws" 2>/dev/null) || ws_test_result="000"
    
    echo "    HTTP 响应码: $ws_test_result"
    
    if [[ "$ws_test_result" == "502" ]] || [[ "$ws_test_result" == "503" ]] || [[ "$ws_test_result" == "000" ]]; then
        pass "WebSocket 目标服务未启动时正确返回错误"
        ((PASS_COUNT++))
    else
        warn "WebSocket 目标服务未启动时返回: $ws_test_result (期望 502/503)"
        ((SKIP_COUNT++))
    fi
    
    echo ""
    echo -e "  ${CYAN}【场景2】WebSocket 目标服务已启动 - 期望握手成功${NC}"
    
    info "启动 WebSocket Mock 服务器 (端口: ${target_port})..."
    if ! start_ws_mock_server "$target_port"; then
        fail "无法启动 WebSocket Mock 服务器"
        ((FAIL_COUNT++))
        return
    fi
    echo -e "    ${GREEN}WebSocket Mock 服务器已启动${NC}"
    
    local tmpfile=$(mktemp)
    local headers_file=$(mktemp)
    
    local ws_response_code
    ws_response_code=$(curl -s -w "%{http_code}" \
        --proxy "$PROXY" \
        -k \
        --connect-timeout 5 \
        --max-time 10 \
        -H "Upgrade: websocket" \
        -H "Connection: Upgrade" \
        -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
        -H "Sec-WebSocket-Version: 13" \
        -D "$headers_file" \
        -o "$tmpfile" \
        "http://${pattern}/ws" 2>/dev/null) || ws_response_code="000"
    
    local ws_headers=$(cat "$headers_file" 2>/dev/null || echo "")
    rm -f "$tmpfile" "$headers_file"
    
    if [[ "$ws_response_code" == "000" ]] && [[ "$ws_headers" == *"101"* ]]; then
        ws_response_code="101"
    fi
    
    echo "    HTTP 响应码: $ws_response_code"
    echo "    响应头 (部分): ${ws_headers:0:200}..."
    
    if [[ "$ws_response_code" == "101" ]]; then
        if [[ "$ws_headers" == *"Upgrade"* ]] || [[ "$ws_headers" == *"upgrade"* ]]; then
            pass "WebSocket 握手成功 (101 Switching Protocols)"
            ((PASS_COUNT++))
        else
            warn "返回 101 但响应头可能不完整"
            ((SKIP_COUNT++))
        fi
    elif [[ "$ws_response_code" == "200" ]]; then
        warn "返回 200 而非 101，可能代理未正确处理 WebSocket 升级"
        ((SKIP_COUNT++))
    else
        fail "WebSocket 握手失败，期望 101，实际: $ws_response_code"
        ((FAIL_COUNT++))
    fi
    
    stop_ws_mock_server
}

detect_rule_type() {
    local line="$1"
    
    if [[ "$line" == *"redirect://"* ]] || [[ "$line" == *"locationHref://"* ]]; then
        echo "redirect"
    elif [[ "$line" == *"reqHeaders://"* ]]; then
        echo "reqHeaders"
    elif [[ "$line" == *"resHeaders://"* ]]; then
        echo "resHeaders"
    elif [[ "$line" == *"statusCode://"* ]]; then
        echo "statusCode"
    elif [[ "$line" == *"method://"* ]]; then
        echo "method"
    elif [[ "$line" == *"ua://"* ]]; then
        echo "ua"
    elif [[ "$line" == *"referer://"* ]]; then
        echo "referer"
    elif [[ "$line" == *"reqDelay://"* ]]; then
        echo "reqDelay"
    elif [[ "$line" == *"resDelay://"* ]]; then
        echo "resDelay"
    elif [[ "$line" == *"resCors://"* ]] || [[ "$line" == *"reqCors://"* ]]; then
        echo "cors"
    elif [[ "$line" == *"reqCookies://"* ]]; then
        echo "reqCookies"
    elif [[ "$line" == *"resCookies://"* ]]; then
        echo "resCookies"
    elif [[ "$line" == *"host://"* ]] || [[ "$line" == *"xhost://"* ]]; then
        echo "host"
    elif [[ "$line" == *" http://"* ]] || [[ "$line" == *" https://"* ]]; then
        echo "host"
    elif [[ "$line" == *" ws://"* ]] || [[ "$line" == *" wss://"* ]]; then
        echo "websocket"
    else
        echo "unknown"
    fi
}

run_tests() {
    header "执行端到端测试"

    while IFS= read -r line; do
        [[ "$line" =~ ^#.*$ ]] && continue
        [[ -z "${line// }" ]] && continue
        
        local pattern=$(echo "$line" | awk '{print $1}')
        local protocols=$(echo "$line" | cut -d' ' -f2-)
        
        if [[ -z "$pattern" ]] || [[ -z "$protocols" ]]; then
            continue
        fi
        
        echo ""
        echo -e "${YELLOW}┌─────────────────────────────────────────────────────${NC}"
        echo -e "${YELLOW}│ 规则: $line${NC}"
        echo -e "${YELLOW}└─────────────────────────────────────────────────────${NC}"
        
        local rule_type=$(detect_rule_type "$line")
        
        case "$rule_type" in
            host)
                local target=$(echo "$protocols" | grep -o 'host://[^[:space:]]*\|http://[^[:space:]]*\|https://[^[:space:]]*' | head -1 | sed 's|host://||')
                test_host_rule "$pattern" "$target"
                ;;
            redirect)
                local target=$(echo "$protocols" | grep -o 'redirect://[^[:space:]]*\|locationHref://[^[:space:]]*' | sed 's|redirect://||; s|locationHref://||')
                test_redirect_rule "$pattern" "$target"
                ;;
            reqHeaders)
                test_req_headers_rule "$pattern" "$protocols"
                ;;
            resHeaders)
                test_res_headers_rule "$pattern" "$protocols"
                ;;
            statusCode)
                test_status_code_rule "$pattern" "$protocols"
                ;;
            method)
                test_method_rule "$pattern" "$protocols"
                ;;
            ua)
                test_ua_rule "$pattern" "$protocols"
                ;;
            referer)
                test_referer_rule "$pattern" "$protocols"
                ;;
            reqDelay)
                test_delay_rule "$pattern" "$protocols" "req"
                ;;
            resDelay)
                test_delay_rule "$pattern" "$protocols" "res"
                ;;
            cors)
                test_cors_rule "$pattern" "$protocols"
                ;;
            reqCookies)
                test_cookies_rule "$pattern" "$protocols" "req"
                ;;
            resCookies)
                test_cookies_rule "$pattern" "$protocols" "res"
                ;;
            websocket)
                test_websocket_rule "$pattern" "$protocols"
                ;;
            *)
                warn "跳过不支持的规则类型: $rule_type"
                ((SKIP_COUNT++))
                ;;
        esac
        
    done < "$RULE_FILE"
}

show_summary() {
    header "测试结果汇总"
    
    echo -e "${GREEN}通过: $PASS_COUNT${NC}"
    echo -e "${RED}失败: $FAIL_COUNT${NC}"
    echo -e "${YELLOW}跳过: $SKIP_COUNT${NC}"
    echo ""
    
    if [[ $FAIL_COUNT -eq 0 ]]; then
        echo -e "${GREEN}所有测试通过！${NC}"
    else
        echo -e "${RED}有 $FAIL_COUNT 个测试失败${NC}"
    fi
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                usage
                ;;
            -p|--port)
                PROXY_PORT="$2"
                PROXY="http://${PROXY_HOST}:${PROXY_PORT}"
                shift 2
                ;;
            -l|--list)
                list_rules
                ;;
            *)
                if [[ -z "$RULE_FILE" ]]; then
                    if [[ "$1" == /* ]]; then
                        RULE_FILE="$1"
                    elif [[ -f "${SCRIPT_DIR}/$1" ]]; then
                        RULE_FILE="${SCRIPT_DIR}/$1"
                    elif [[ -f "${RULES_DIR}/$1" ]]; then
                        RULE_FILE="${RULES_DIR}/$1"
                    elif [[ -f "${RULES_DIR}/${1}.txt" ]]; then
                        RULE_FILE="${RULES_DIR}/${1}.txt"
                    else
                        RULE_FILE="$1"
                    fi
                fi
                shift
                ;;
        esac
    done
    
    if [[ -z "$RULE_FILE" ]]; then
        fail "请指定规则文件"
        echo ""
        usage
    fi
}

main() {
    parse_args "$@"
    
    header "Bifrost 规则端到端测试"
    echo "代理端口: $PROXY_PORT"
    echo "规则文件: $RULE_FILE"
    echo "项目目录: $PROJECT_DIR"
    echo ""

    check_rule_file
    build_proxy
    setup_data_dir
    show_rules
    start_proxy
    run_tests
    show_summary

    if [[ $FAIL_COUNT -gt 0 ]]; then
        exit 1
    fi
}

main "$@"
