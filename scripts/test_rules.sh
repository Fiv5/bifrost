#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RULES_DIR="${SCRIPT_DIR}/rules"
TEST_DATA_DIR="${PROJECT_DIR}/.bifrost-test"

source "$SCRIPT_DIR/test_utils/assert.sh"
source "$SCRIPT_DIR/test_utils/http_client.sh"

PROXY_PORT="${PROXY_PORT:-8080}"
PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY="http://${PROXY_HOST}:${PROXY_PORT}"

ECHO_HTTP_PORT="${ECHO_HTTP_PORT:-3000}"
ECHO_HTTPS_PORT="${ECHO_HTTPS_PORT:-3443}"
ECHO_WS_PORT="${ECHO_WS_PORT:-3020}"
ECHO_WSS_PORT="${ECHO_WSS_PORT:-3021}"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

header() { echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"; echo -e "${CYAN}  $1${NC}"; echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"; }
info() { echo -e "${BLUE}ℹ${NC} $1"; }
warn() { echo -e "${YELLOW}⚠${NC} $1"; }

PROXY_PID=""
RULE_FILE=""

usage() {
    echo "用法: $0 [选项] <规则文件>"
    echo ""
    echo "选项:"
    echo "  -h, --help         显示帮助信息"
    echo "  -p, --port PORT    指定代理端口 (默认: 8080)"
    echo "  -l, --list         列出所有可用的规则文件"
    echo "  --no-build         跳过编译步骤"
    echo "  --keep-proxy       测试完成后保持代理运行"
    echo ""
    echo "环境变量:"
    echo "  ECHO_HTTP_PORT     HTTP Echo 服务器端口 (默认: 3000)"
    echo "  ECHO_HTTPS_PORT    HTTPS Echo 服务器端口 (默认: 3443)"
    echo "  ECHO_WS_PORT       WebSocket Echo 服务器端口 (默认: 3020)"
    echo ""
    echo "示例:"
    echo "  $0 rules/forwarding/http_to_http.txt"
    echo "  $0 -p 9090 rules/request_modify/headers.txt"
    echo "  $0 --list"
    exit 0
}

list_rules() {
    header "可用的规则文件"

    find "$RULES_DIR" -name "*.txt" -type f 2>/dev/null | sort | while read -r rule_file; do
        local rel_path="${rule_file#$RULES_DIR/}"
        local desc=$(grep -m1 '^#' "$rule_file" 2>/dev/null | sed 's/^# *//' || echo "无描述")
        printf "  ${CYAN}%-40s${NC} %s\n" "$rel_path" "$desc"
    done
    exit 0
}

cleanup() {
    if [[ "$KEEP_PROXY" != "true" ]]; then
        if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
            info "正在停止代理服务器 (PID: $PROXY_PID)..."
            kill "$PROXY_PID" 2>/dev/null || true
            wait "$PROXY_PID" 2>/dev/null || true
        fi
    fi

    "$SCRIPT_DIR/mock_servers/start_servers.sh" stop 2>/dev/null || true
}

trap cleanup EXIT

check_dependencies() {
    header "检查依赖"

    if ! command -v curl &> /dev/null; then
        echo -e "${RED}✗${NC} curl 未安装"
        exit 1
    fi
    echo -e "${GREEN}✓${NC} curl 已安装"

    if ! command -v python3 &> /dev/null; then
        echo -e "${RED}✗${NC} python3 未安装"
        exit 1
    fi
    echo -e "${GREEN}✓${NC} python3 已安装"

    if ! command -v jq &> /dev/null; then
        echo -e "${YELLOW}⚠${NC} jq 未安装 (JSON 断言将被跳过)"
    else
        echo -e "${GREEN}✓${NC} jq 已安装"
    fi
}

check_rule_file() {
    if [[ ! -f "$RULE_FILE" ]]; then
        echo -e "${RED}✗${NC} 规则文件不存在: $RULE_FILE"
        echo "请使用 --list 查看可用的规则文件"
        exit 1
    fi

    local rule_count=$(grep -v '^#' "$RULE_FILE" | grep -v '^[[:space:]]*$' | wc -l | tr -d ' ')
    if [[ "$rule_count" -eq 0 ]]; then
        echo -e "${RED}✗${NC} 规则文件为空或只包含注释"
        exit 1
    fi

    echo -e "${GREEN}✓${NC} 找到 $rule_count 条规则"
}

build_proxy() {
    if [[ "$SKIP_BUILD" == "true" ]]; then
        info "跳过编译步骤"
        return 0
    fi

    header "编译代理服务器"

    if [[ -f "${PROJECT_DIR}/target/release/bifrost" ]]; then
        local mod_time=$(stat -f %m "${PROJECT_DIR}/target/release/bifrost" 2>/dev/null || stat -c %Y "${PROJECT_DIR}/target/release/bifrost" 2>/dev/null)
        local now=$(date +%s)
        local age=$((now - mod_time))

        if [[ $age -lt 86400 ]]; then
            echo -e "${GREEN}✓${NC} 使用已编译的代理 (编译于 $((age / 60)) 分钟前)"
            return 0
        fi
    fi

    info "正在编译代理服务器..."
    cd "$PROJECT_DIR"
    cargo build --release --bin bifrost 2>&1 | tail -5
    echo -e "${GREEN}✓${NC} 代理服务器编译完成"
}

setup_data_dir() {
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
}

start_echo_servers() {
    header "启动 Echo 服务器"

    if curl -s "http://127.0.0.1:${ECHO_HTTP_PORT}/health" >/dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} HTTP Echo 服务器已在运行 (端口: ${ECHO_HTTP_PORT})"
        return 0
    fi

    "$SCRIPT_DIR/mock_servers/start_servers.sh" start-bg

    sleep 2

    if curl -s "http://127.0.0.1:${ECHO_HTTP_PORT}/health" >/dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} HTTP Echo 服务器已启动 (端口: ${ECHO_HTTP_PORT})"
    else
        echo -e "${RED}✗${NC} HTTP Echo 服务器启动失败"
        exit 1
    fi
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

    mkdir -p "${TEST_DATA_DIR}"
    export BIFROST_DATA_DIR="${TEST_DATA_DIR}"
    BIFROST_DATA_DIR="${TEST_DATA_DIR}" "${PROJECT_DIR}/target/release/bifrost" --port "${PROXY_PORT}" start --skip-cert-check --unsafe-ssl --rules-file "${RULE_FILE}" &
    PROXY_PID=$!

    local max_wait=10
    local waited=0
    while [[ $waited -lt $max_wait ]]; do
        if curl -s --proxy "$PROXY" --connect-timeout 1 http://example.com >/dev/null 2>&1; then
            echo -e "${GREEN}✓${NC} 代理服务器已启动 (PID: $PROXY_PID)"
            echo -e "${GREEN}✓${NC} 规则已从文件加载: ${RULE_FILE}"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    echo -e "${RED}✗${NC} 代理服务器启动超时"
    exit 1
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

test_http_to_http_forward() {
    local pattern="$1"
    local target="$2"
    local test_url="http://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】HTTP→HTTP 转发${NC}"
    echo "    请求: $test_url"
    echo "    目标: $target"

    http_get "$test_url"

    assert_status_2xx "$HTTP_STATUS" "代理应成功转发请求"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        assert_json_field_exists ".request.method" "$HTTP_BODY" "Echo 服务器应返回请求信息"
        assert_json_field ".server.protocol" "http" "$HTTP_BODY" "后端应通过 HTTP 接收请求"
    fi
}

test_https_to_http_forward() {
    local pattern="$1"
    local target="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】HTTPS→HTTP 转发 (TLS 终止)${NC}"
    echo "    请求: $test_url"
    echo "    目标: $target"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "代理应成功转发 HTTPS 请求"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        assert_json_field ".server.protocol" "http" "$HTTP_BODY" "后端应通过 HTTP 接收请求 (TLS 已终止)"
    fi
}

test_http_to_https_forward() {
    local pattern="$1"
    local target="$2"
    local test_url="http://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】HTTP→HTTPS 转发 (TLS 建立)${NC}"
    echo "    请求: $test_url"
    echo "    目标: $target"

    http_get "$test_url"

    assert_status_2xx "$HTTP_STATUS" "代理应成功转发请求到 HTTPS 后端"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        assert_json_field ".server.protocol" "https" "$HTTP_BODY" "后端应通过 HTTPS 接收请求"
        assert_json_field_exists ".server.tls" "$HTTP_BODY" "HTTPS 连接应有 TLS 信息"
    fi
}

test_redirect_rule() {
    local pattern="$1"
    local target="$2"
    local test_url="https://${pattern}/"

    echo ""
    echo -e "  ${CYAN}【测试】重定向规则${NC}"
    echo "    请求: $test_url"
    echo "    目标: $target"

    _temp_headers_file=$(mktemp)
    _temp_body_file=$(mktemp)

    HTTP_STATUS=$(curl -s -w '%{http_code}' \
        --proxy "$PROXY" \
        -k \
        -D "$_temp_headers_file" \
        -o "$_temp_body_file" \
        --max-time 10 \
        "$test_url" 2>/dev/null) || HTTP_STATUS="000"

    HTTP_HEADERS=$(cat "$_temp_headers_file")
    HTTP_BODY=$(cat "$_temp_body_file")
    rm -f "$_temp_headers_file" "$_temp_body_file"

    assert_status_3xx "$HTTP_STATUS" "重定向应返回 3xx 状态码"
    assert_header_exists "Location" "$HTTP_HEADERS" "重定向应包含 Location 头"

    if [[ -n "$target" ]]; then
        assert_header_contains "Location" "$target" "$HTTP_HEADERS" "Location 应指向目标地址"
    fi
}

test_req_headers_add() {
    local pattern="$1"
    local header_name="$2"
    local header_value="$3"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】添加请求头${NC}"
    echo "    请求: $test_url"
    echo "    期望添加: $header_name: $header_value"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        local header_key_lower=$(echo "$header_name" | tr '[:upper:]' '[:lower:]')
        local actual_value=$(echo "$HTTP_BODY" | jq -r ".request.headers[\"$header_name\"] // .request.headers[\"$header_key_lower\"]" 2>/dev/null)

        assert_equals "$header_value" "$actual_value" "后端应收到添加的请求头 $header_name=$header_value"
    fi
}

test_req_headers_delete() {
    local pattern="$1"
    local header_name="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】删除请求头${NC}"
    echo "    请求: $test_url"
    echo "    期望删除: $header_name"

    https_request "$test_url" "GET" "" "X-Custom-Test: should-be-deleted"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        local header_key_lower=$(echo "$header_name" | tr '[:upper:]' '[:lower:]')
        local actual_value=$(echo "$HTTP_BODY" | jq -r ".request.headers[\"$header_name\"] // .request.headers[\"$header_key_lower\"] // \"null\"" 2>/dev/null)

        assert_equals "null" "$actual_value" "后端不应收到被删除的请求头 $header_name"
    fi
}

test_res_headers_add() {
    local pattern="$1"
    local header_name="$2"
    local header_value="$3"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】添加响应头${NC}"
    echo "    请求: $test_url"
    echo "    期望添加: $header_name: $header_value"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"
    assert_header_exists "$header_name" "$HTTP_HEADERS" "响应应包含添加的头 $header_name"

    if [[ -n "$header_value" ]]; then
        assert_header_value "$header_name" "$header_value" "$HTTP_HEADERS" "响应头值应正确"
    fi
}

test_status_code() {
    local pattern="$1"
    local expected_status="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】状态码修改${NC}"
    echo "    请求: $test_url"
    echo "    期望状态码: $expected_status"

    https_request "$test_url"

    assert_status "$expected_status" "$HTTP_STATUS" "响应状态码应被修改为 $expected_status"
}

test_method_change() {
    local pattern="$1"
    local expected_method="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】请求方法修改${NC}"
    echo "    请求: GET $test_url"
    echo "    期望后端收到: $expected_method"

    https_request "$test_url" "GET"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        assert_backend_received_method "$expected_method" "$HTTP_BODY" "后端应收到 $expected_method 方法"
    fi
}

test_ua_change() {
    local pattern="$1"
    local expected_ua="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】User-Agent 修改${NC}"
    echo "    请求: $test_url"
    echo "    期望 UA: $expected_ua"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        local actual_ua=$(echo "$HTTP_BODY" | jq -r '.request.headers["User-Agent"] // .request.headers["user-agent"]' 2>/dev/null)
        assert_body_contains "$expected_ua" "$actual_ua" "User-Agent 应被修改为包含 $expected_ua"
    fi
}

test_referer_change() {
    local pattern="$1"
    local expected_referer="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】Referer 修改${NC}"
    echo "    请求: $test_url"
    echo "    期望 Referer: $expected_referer"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        local actual_referer=$(echo "$HTTP_BODY" | jq -r '.request.headers["Referer"] // .request.headers["referer"]' 2>/dev/null)
        assert_equals "$expected_referer" "$actual_referer" "Referer 应被修改为 $expected_referer"
    fi
}

test_referer_removed() {
    local pattern="$1"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】Referer 删除${NC}"
    echo "    请求: $test_url (带 Referer 头)"
    echo "    期望: Referer 头被删除"

    https_request "$test_url" "GET" "" "Referer: https://original.example.com"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        local actual_referer=$(echo "$HTTP_BODY" | jq -r '.request.headers["Referer"] // .request.headers["referer"]' 2>/dev/null)
        if [[ "$actual_referer" == "null" ]] || [[ -z "$actual_referer" ]]; then
            echo -e "  ${GREEN}✓${NC} Referer 已被正确删除"
            TESTS_PASSED=$((TESTS_PASSED + 1))
            TESTS_TOTAL=$((TESTS_TOTAL + 1))
        else
            echo -e "  ${RED}✗${NC} Referer 应该被删除"
            echo "    实际值: $actual_referer"
            TESTS_FAILED=$((TESTS_FAILED + 1))
            TESTS_TOTAL=$((TESTS_TOTAL + 1))
        fi
    fi
}

test_delay() {
    local pattern="$1"
    local delay_ms="$2"
    local delay_type="$3"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】${delay_type}延迟${NC}"
    echo "    请求: $test_url"
    echo "    期望延迟: ${delay_ms}ms"

    local start_time=$(python3 -c "import time; print(int(time.time() * 1000))")
    https_request "$test_url"
    local end_time=$(python3 -c "import time; print(int(time.time() * 1000))")

    local elapsed=$((end_time - start_time))
    local min_expected=$((delay_ms - 100))

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if [[ $elapsed -ge $min_expected ]]; then
        _log_pass "延迟生效: 实际 ${elapsed}ms >= 预期 ${min_expected}ms"
    else
        _log_fail "延迟可能未生效" ">= ${min_expected}ms" "${elapsed}ms"
    fi
}

test_cors() {
    local pattern="$1"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】CORS 支持${NC}"
    echo "    请求: $test_url"

    _temp_headers_file=$(mktemp)
    _temp_body_file=$(mktemp)

    HTTP_STATUS=$(curl -s -w '%{http_code}' \
        --proxy "$PROXY" \
        -k \
        -H "Origin: https://example.com" \
        -D "$_temp_headers_file" \
        -o "$_temp_body_file" \
        --max-time 10 \
        "$test_url" 2>/dev/null) || HTTP_STATUS="000"

    HTTP_HEADERS=$(cat "$_temp_headers_file")
    rm -f "$_temp_headers_file" "$_temp_body_file"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"
    assert_header_exists "Access-Control-Allow-Origin" "$HTTP_HEADERS" "响应应包含 CORS 头"
}

test_req_cookies() {
    local pattern="$1"
    local cookie_name="$2"
    local cookie_value="$3"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】添加请求 Cookie${NC}"
    echo "    请求: $test_url"
    echo "    期望 Cookie: $cookie_name=$cookie_value"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        local actual_cookie=$(echo "$HTTP_BODY" | jq -r ".request.cookies[\"$cookie_name\"]" 2>/dev/null)
        assert_equals "$cookie_value" "$actual_cookie" "后端应收到 Cookie $cookie_name=$cookie_value"
    fi
}

test_res_cookies() {
    local pattern="$1"
    local cookie_name="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】设置响应 Cookie${NC}"
    echo "    请求: $test_url"
    echo "    期望 Set-Cookie 包含: $cookie_name"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"
    assert_header_exists "Set-Cookie" "$HTTP_HEADERS" "响应应包含 Set-Cookie 头"
    assert_header_contains "Set-Cookie" "$cookie_name" "$HTTP_HEADERS" "Set-Cookie 应包含 $cookie_name"
}

test_websocket_forward() {
    local pattern="$1"
    local target="$2"
    local test_url="http://${pattern}/ws"

    echo ""
    echo -e "  ${CYAN}【测试】WebSocket 转发${NC}"
    echo "    请求: $test_url"
    echo "    目标: $target"

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
        "$test_url" 2>/dev/null) || ws_response_code="000"

    local ws_headers=$(cat "$headers_file" 2>/dev/null || echo "")
    rm -f "$tmpfile" "$headers_file"

    if [[ "$ws_response_code" == "000" ]] && [[ "$ws_headers" == *"101"* ]]; then
        ws_response_code="101"
    fi

    assert_status "101" "$ws_response_code" "WebSocket 握手应返回 101"
    assert_header_contains "Upgrade" "websocket" "$ws_headers" "响应应包含 Upgrade: websocket"
}

test_template_file() {
    local pattern="$1"
    local file_path="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】模板文件响应 (tpl)${NC}"
    echo "    请求: $test_url"
    echo "    模板文件: $file_path"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if [[ -n "$HTTP_BODY" ]]; then
        if echo "$HTTP_BODY" | grep -q '"template"'; then
            _log_pass "响应包含模板内容"
        else
            _log_fail "响应应该来自模板文件" "包含 template" "$HTTP_BODY"
        fi

        if echo "$HTTP_BODY" | grep -q '\${'; then
            _log_fail "模板变量应该被替换" "不包含 \${" "包含未替换的变量"
        else
            _log_pass "模板变量已被替换"
        fi
    fi
}

test_raw_file() {
    local pattern="$1"
    local file_path="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】原始文件响应 (rawfile)${NC}"
    echo "    请求: $test_url"
    echo "    原始文件: $file_path"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if [[ -n "$HTTP_BODY" ]]; then
        if echo "$HTTP_BODY" | grep -q 'raw file content'; then
            _log_pass "响应包含原始文件内容"
        else
            _log_fail "响应应该来自原始文件" "包含 raw file content" "$HTTP_BODY"
        fi

        if echo "$HTTP_BODY" | grep -q '\${url}'; then
            _log_pass "模板变量未被替换 (正确行为)"
        else
            _log_fail "rawfile 不应替换模板变量" "包含 \${url}" "变量被替换了"
        fi
    fi
}

test_mock_file() {
    local pattern="$1"
    local file_path="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】文件响应 (file)${NC}"
    echo "    请求: $test_url"
    echo "    文件: $file_path"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if [[ -n "$HTTP_BODY" ]]; then
        _log_pass "响应包含文件内容"
    else
        _log_fail "响应应该包含文件内容" "非空内容" "空响应"
    fi
}

test_res_body() {
    local pattern="$1"
    local expected_body="$2"
    local test_url="https://${pattern}/test"

    echo ""
    echo -e "  ${CYAN}【测试】响应体替换 (resBody)${NC}"
    echo "    请求: $test_url"
    echo "    期望内容: ${expected_body:0:50}..."

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if [[ -n "$HTTP_BODY" ]]; then
        if [[ "$HTTP_BODY" == *"$expected_body"* ]] || [[ -n "$expected_body" && "$HTTP_BODY" != *"request"* ]]; then
            _log_pass "响应体已被替换"
        else
            _log_fail "响应体应被替换" "不包含 echo 响应" "包含原始响应"
        fi
    else
        _log_fail "响应体应该非空" "非空" "空响应"
    fi
}

detect_rule_type() {
    local line="$1"

    if [[ "$line" == *"redirect://"* ]] || [[ "$line" == *"locationHref://"* ]]; then
        echo "redirect"
    elif [[ "$line" == *"resBody://"* ]]; then
        echo "resBody"
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
    elif [[ "$line" == *"tpl://"* ]]; then
        echo "tpl"
    elif [[ "$line" == *"rawfile://"* ]]; then
        echo "rawfile"
    elif [[ "$line" == *"file://"* ]]; then
        echo "file"
    elif [[ "$line" == *" ws://"* ]]; then
        echo "websocket"
    elif [[ "$line" == *" wss://"* ]]; then
        echo "websocket_secure"
    elif [[ "$line" == *" https://"* ]]; then
        if [[ "$line" == "http://"* ]] || [[ "$line" != "https://"* ]]; then
            echo "http_to_https"
        else
            echo "https_forward"
        fi
    elif [[ "$line" == *" http://"* ]]; then
        if [[ "$line" == "https://"* ]]; then
            echo "https_to_http"
        else
            echo "http_forward"
        fi
    elif [[ "$line" == *"host://"* ]] || [[ "$line" == *"xhost://"* ]]; then
        echo "host"
    elif [[ "$line" =~ [[:space:]][0-9]+\.[0-9]+\.[0-9]+\.[0-9]+:[0-9]+ ]] || [[ "$line" =~ [[:space:]]localhost:[0-9]+ ]] || [[ "$line" =~ [[:space:]]127\.0\.0\.1:[0-9]+ ]]; then
        echo "ip_forward"
    else
        echo "unknown"
    fi
}

extract_target() {
    local protocols="$1"
    local target=$(echo "$protocols" | grep -o 'http://[^[:space:]]*\|https://[^[:space:]]*\|ws://[^[:space:]]*\|wss://[^[:space:]]*\|host://[^[:space:]]*' | head -1 | sed 's|host://||')
    if [[ -z "$target" ]]; then
        target=$(echo "$protocols" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+:[0-9]+(/[^[:space:]]*)?' | head -1)
    fi
    echo "$target"
}

extract_value() {
    local protocols="$1"
    local prefix="$2"
    echo "$protocols" | grep -o "${prefix}://[^[:space:]]*" | sed "s|${prefix}://||"
}

extract_header_from_value() {
    local value="$1"
    value="${value#\`}"
    value="${value%\`}"
    value="${value#(}"
    value="${value%)}"

    local header_name=""
    local header_value=""

    if [[ "$value" == *":"* ]]; then
        header_name=$(echo "$value" | cut -d':' -f1 | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        header_value=$(echo "$value" | cut -d':' -f2- | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
    fi

    echo "$header_name|$header_value"
}

test_res_headers_template() {
    local pattern="$1"
    local header_info="$2"
    local test_url="https://${pattern}/test"

    local header_name=$(echo "$header_info" | cut -d'|' -f1)
    local header_template=$(echo "$header_info" | cut -d'|' -f2)

    echo ""
    echo -e "  ${CYAN}【测试】添加响应头 (模板变量)${NC}"
    echo "    请求: $test_url"
    echo "    期望添加头: $header_name"
    echo "    模板: $header_template"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"
    assert_header_exists "$header_name" "$HTTP_HEADERS" "响应应包含添加的头 $header_name"

    if [[ "$header_template" == *'${'* ]]; then
        local actual_value=$(echo "$HTTP_HEADERS" | grep -i "^${header_name}:" | head -1 | cut -d':' -f2- | sed 's/^[[:space:]]*//')
        if [[ -n "$actual_value" ]] && [[ "$actual_value" != *'${'* ]]; then
            _log_pass "模板变量已替换: $actual_value"
        else
            _log_fail "模板变量应该被替换" "不包含 \${}" "${actual_value:-空值}"
        fi
    fi
}

test_req_headers_template() {
    local pattern="$1"
    local header_info="$2"
    local test_url="https://${pattern}/test"

    local header_name=$(echo "$header_info" | cut -d'|' -f1)
    local header_template=$(echo "$header_info" | cut -d'|' -f2)

    echo ""
    echo -e "  ${CYAN}【测试】添加请求头 (模板变量)${NC}"
    echo "    请求: $test_url"
    echo "    期望添加头: $header_name"
    echo "    模板: $header_template"

    https_request "$test_url"

    assert_status_2xx "$HTTP_STATUS" "请求应成功"

    if command -v jq &> /dev/null && [[ -n "$HTTP_BODY" ]]; then
        local header_key_lower=$(echo "$header_name" | tr '[:upper:]' '[:lower:]')
        local actual_value=$(echo "$HTTP_BODY" | jq -r ".request.headers[\"$header_name\"] // .request.headers[\"$header_key_lower\"]" 2>/dev/null)

        if [[ -n "$actual_value" ]] && [[ "$actual_value" != "null" ]]; then
            if [[ "$header_template" == *'${'* ]]; then
                if [[ "$actual_value" != *'${'* ]]; then
                    _log_pass "请求头已添加且模板变量已替换: $actual_value"
                else
                    _log_fail "模板变量应该被替换" "不包含 \${}" "$actual_value"
                fi
            else
                _log_pass "后端收到添加的请求头: $header_name=$actual_value"
            fi
        else
            _log_fail "后端应收到添加的请求头" "$header_name" "未找到"
        fi
    fi
}

run_tests() {
    header "执行端到端测试"

    local rules=()
    while IFS= read -r line; do
        [[ "$line" =~ ^#.*$ ]] && continue
        [[ -z "${line// }" ]] && continue
        rules+=("$line")
    done < "$RULE_FILE"

    set +e
    for line in "${rules[@]}"; do
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
        local target=$(extract_target "$protocols")

        case "$rule_type" in
            http_forward|host|ip_forward)
                test_http_to_http_forward "$pattern" "$target"
                ;;
            https_to_http)
                test_https_to_http_forward "$pattern" "$target"
                ;;
            http_to_https)
                test_http_to_https_forward "$pattern" "$target"
                ;;
            redirect)
                local redirect_target=$(extract_value "$protocols" "redirect")
                [[ -z "$redirect_target" ]] && redirect_target=$(extract_value "$protocols" "locationHref")
                test_redirect_rule "$pattern" "$redirect_target"
                ;;
            resBody)
                local res_body_raw=$(extract_value "$protocols" "resBody")
                test_res_body "$pattern" "$res_body_raw"
                ;;
            reqHeaders)
                local req_header_raw=$(extract_value "$protocols" "reqHeaders")
                local req_header_info=$(extract_header_from_value "$req_header_raw")
                local req_header_name=$(echo "$req_header_info" | cut -d'|' -f1)
                local req_header_value=$(echo "$req_header_info" | cut -d'|' -f2)
                if [[ -n "$req_header_name" ]]; then
                    if [[ "$req_header_value" == *'${'* ]] || [[ "$req_header_raw" == *'`'* ]]; then
                        test_req_headers_template "$pattern" "$req_header_info"
                    else
                        test_req_headers_add "$pattern" "$req_header_name" "$req_header_value"
                    fi
                else
                    test_req_headers_add "$pattern" "X-Test-Header" "test-value"
                fi
                ;;
            resHeaders)
                local res_header_raw=$(extract_value "$protocols" "resHeaders")
                local res_header_info=$(extract_header_from_value "$res_header_raw")
                local res_header_name=$(echo "$res_header_info" | cut -d'|' -f1)
                local res_header_value=$(echo "$res_header_info" | cut -d'|' -f2)
                if [[ -n "$res_header_name" ]]; then
                    if [[ "$res_header_value" == *'${'* ]] || [[ "$res_header_raw" == *'`'* ]]; then
                        test_res_headers_template "$pattern" "$res_header_info"
                    else
                        test_res_headers_add "$pattern" "$res_header_name" "$res_header_value"
                    fi
                else
                    test_res_headers_add "$pattern" "X-Test-Response" "test-value"
                fi
                ;;
            statusCode)
                local status=$(extract_value "$protocols" "statusCode")
                test_status_code "$pattern" "${status:-201}"
                ;;
            method)
                local method=$(extract_value "$protocols" "method")
                test_method_change "$pattern" "${method:-POST}"
                ;;
            ua)
                local ua=$(extract_value "$protocols" "ua")
                test_ua_change "$pattern" "${ua:-Bifrost}"
                ;;
            referer)
                local referer=$(extract_value "$protocols" "referer")
                if [[ -z "$referer" ]]; then
                    test_referer_removed "$pattern"
                else
                    test_referer_change "$pattern" "$referer"
                fi
                ;;
            reqDelay)
                local delay=$(extract_value "$protocols" "reqDelay")
                test_delay "$pattern" "${delay:-500}" "请求"
                ;;
            resDelay)
                local delay=$(extract_value "$protocols" "resDelay")
                test_delay "$pattern" "${delay:-500}" "响应"
                ;;
            cors)
                test_cors "$pattern"
                ;;
            reqCookies)
                test_req_cookies "$pattern" "test_cookie" "test_value"
                ;;
            resCookies)
                test_res_cookies "$pattern" "bifrost"
                ;;
            websocket|websocket_secure)
                test_websocket_forward "$pattern" "$target"
                ;;
            tpl)
                local file_path=$(extract_value "$protocols" "tpl")
                test_template_file "$pattern" "$file_path"
                ;;
            rawfile)
                local file_path=$(extract_value "$protocols" "rawfile")
                test_raw_file "$pattern" "$file_path"
                ;;
            file)
                local file_path=$(extract_value "$protocols" "file")
                test_mock_file "$pattern" "$file_path"
                ;;
            *)
                warn "跳过不支持的规则类型: $rule_type (规则: $line)"
                ;;
        esac
    done
    set -e
}

SKIP_BUILD="false"
KEEP_PROXY="false"

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
            --no-build)
                SKIP_BUILD="true"
                shift
                ;;
            --keep-proxy)
                KEEP_PROXY="true"
                shift
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
        echo -e "${RED}✗${NC} 请指定规则文件"
        echo ""
        usage
    fi
}

main() {
    parse_args "$@"

    header "Bifrost 规则端到端测试 v2"
    echo "代理端口: $PROXY_PORT"
    echo "规则文件: $RULE_FILE"
    echo "项目目录: $PROJECT_DIR"
    echo ""

    reset_test_stats

    check_dependencies
    check_rule_file
    build_proxy
    setup_data_dir
    start_echo_servers
    show_rules
    start_proxy
    run_tests

    print_test_summary
    exit $?
}

main "$@"
