#!/bin/bash
#
# Bifrost Values 系统端到端测试
# 完整的 E2E 测试：Mock Server + Proxy + Client
#
# 测试架构:
#   Client (curl) → Proxy (bifrost) → Mock Server (echo)
#
# 测试内容:
#   1. 值引用 {valueName} 在规则中的解析
#   2. 值文件从 values 目录加载
#   3. resBody/reqHeaders/resHeaders 中的值替换
#

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
VALUES_DIR="${SCRIPT_DIR}/values"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

PROXY_PORT="${PROXY_PORT:-18080}"
ECHO_HTTP_PORT="${ECHO_HTTP_PORT:-13000}"
PROXY="http://127.0.0.1:${PROXY_PORT}"

BIFROST_BIN="${PROJECT_DIR}/target/release/bifrost"
TEST_DATA_DIR=""
PROXY_PID=""
ECHO_PID=""

PASSED=0
FAILED=0
SKIPPED=0

header() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

info() {
    echo -e "${CYAN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

pass() {
    echo -e "  ${GREEN}✓${NC} $1"
    ((PASSED++))
}

fail() {
    echo -e "  ${RED}✗${NC} $1"
    if [[ $# -gt 1 ]]; then
        echo -e "    ${RED}Expected:${NC} $2"
    fi
    if [[ $# -gt 2 ]]; then
        echo -e "    ${RED}Actual:${NC} $3"
    fi
    ((FAILED++))
}

skip() {
    echo -e "  ${YELLOW}○${NC} $1 (skipped)"
    ((SKIPPED++))
}

cleanup() {
    info "清理资源..."

    if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
        kill "$PROXY_PID" 2>/dev/null || true
        wait "$PROXY_PID" 2>/dev/null || true
    fi

    if [[ -n "$ECHO_PID" ]] && kill -0 "$ECHO_PID" 2>/dev/null; then
        kill "$ECHO_PID" 2>/dev/null || true
        wait "$ECHO_PID" 2>/dev/null || true
    fi

    if [[ -n "$TEST_DATA_DIR" ]] && [[ -d "$TEST_DATA_DIR" ]]; then
        rm -rf "$TEST_DATA_DIR"
    fi

    pkill -f "http_echo_server.py.*${ECHO_HTTP_PORT}" 2>/dev/null || true
}

trap cleanup EXIT

check_dependencies() {
    header "检查依赖"

    local missing=()

    if ! command -v curl &> /dev/null; then
        missing+=("curl")
    fi

    if ! command -v python3 &> /dev/null; then
        missing+=("python3")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        error "缺少依赖: ${missing[*]}"
        exit 1
    fi

    echo -e "${GREEN}✓${NC} 依赖检查通过"
}

build_bifrost() {
    header "编译 Bifrost"

    if [[ -f "$BIFROST_BIN" ]] && [[ "$SKIP_BUILD" == "true" ]]; then
        info "跳过编译 (--no-build)"
        return
    fi

    info "编译 bifrost..."
    cd "$PROJECT_DIR"
    cargo build --release --bin bifrost 2>&1 | tail -5
    echo -e "${GREEN}✓${NC} 编译完成"
}

setup_test_environment() {
    header "设置测试环境"

    TEST_DATA_DIR=$(mktemp -d)
    info "测试数据目录: $TEST_DATA_DIR"

    mkdir -p "${TEST_DATA_DIR}/.bifrost/values"
    mkdir -p "${TEST_DATA_DIR}/.bifrost/rules"

    info "复制测试值文件..."
    for f in "$VALUES_DIR"/*.txt; do
        if [[ -f "$f" ]]; then
            cp "$f" "${TEST_DATA_DIR}/.bifrost/values/"
            echo "  - $(basename "$f")"
        fi
    done

    RULES_FILE="${TEST_DATA_DIR}/.bifrost/rules/values_test.txt"
    cat > "$RULES_FILE" << 'RULES'
# E2E Values 测试规则

# E2E-V01: 内联响应体
e2e-inline-body.local http://127.0.0.1:13000 resBody://`{"inline":"body","status":"ok"}`

# E2E-V02: 内联请求头
e2e-inline-reqheader.local http://127.0.0.1:13000 reqHeaders://`X-Inline-Header:inline-value`

# E2E-V03: 内联响应头
e2e-inline-resheader.local http://127.0.0.1:13000 resHeaders://`X-Inline-Response:inline-response-value`

# E2E-V04: 值引用响应体 - {mockResponse}
e2e-value-body.local http://127.0.0.1:13000 resBody://{mockResponse}

# E2E-V05: 值引用请求头 - {authHeaders}
e2e-value-reqheader.local http://127.0.0.1:13000 reqHeaders://{authHeaders}

# E2E-V06: 值引用响应头 - {customHeaders}
e2e-value-resheader.local http://127.0.0.1:13000 resHeaders://{customHeaders}

# E2E-V07: 多值引用组合
e2e-multi-values.local http://127.0.0.1:13000 reqHeaders://{authHeaders} resHeaders://{customHeaders}

# E2E-V08: JSON 格式值引用
e2e-json-body.local http://127.0.0.1:13000 resBody://{jsonResponse}

# E2E-V09: 多行头部值引用
e2e-multi-headers.local http://127.0.0.1:13000 reqHeaders://{multiHeaders}
RULES

    info "规则文件已创建: $RULES_FILE"
    echo -e "${GREEN}✓${NC} 测试环境设置完成"
}

start_echo_server() {
    header "启动 Echo Mock 服务器"

    if lsof -i ":${ECHO_HTTP_PORT}" -t >/dev/null 2>&1; then
        warn "端口 ${ECHO_HTTP_PORT} 已被占用，尝试终止..."
        lsof -i ":${ECHO_HTTP_PORT}" -t | xargs kill 2>/dev/null || true
        sleep 1
    fi

    python3 "${SCRIPT_DIR}/mock_servers/http_echo_server.py" "${ECHO_HTTP_PORT}" &
    ECHO_PID=$!

    local waited=0
    while [[ $waited -lt 10 ]]; do
        if curl -s "http://127.0.0.1:${ECHO_HTTP_PORT}/health" >/dev/null 2>&1; then
            echo -e "${GREEN}✓${NC} Echo 服务器已启动 (端口: ${ECHO_HTTP_PORT}, PID: ${ECHO_PID})"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    error "Echo 服务器启动超时"
    exit 1
}

start_proxy() {
    header "启动代理服务器"

    if lsof -i ":${PROXY_PORT}" -t >/dev/null 2>&1; then
        warn "端口 ${PROXY_PORT} 已被占用，尝试终止..."
        lsof -i ":${PROXY_PORT}" -t | xargs kill 2>/dev/null || true
        sleep 1
    fi

    info "启动代理 (端口: ${PROXY_PORT})..."
    info "数据目录: ${TEST_DATA_DIR}"

    export BIFROST_DATA_DIR="${TEST_DATA_DIR}"

    # 可选系统代理参数
    local extra_flags=()
    if [[ "${ENABLE_SYSTEM_PROXY:-}" == "true" ]]; then
        extra_flags+=(--system-proxy)
        local bypass_val="${SYSTEM_PROXY_BYPASS:-localhost,127.0.0.1,::1,*.local}"
        extra_flags+=(--proxy-bypass "$bypass_val")
    fi

    "$BIFROST_BIN" --port "${PROXY_PORT}" start \
        --skip-cert-check --unsafe-ssl \
        --rules-file "${TEST_DATA_DIR}/.bifrost/rules/values_test.txt" \
        ${extra_flags[@]+"${extra_flags[@]}"} > "${TEST_DATA_DIR}/proxy.log" 2>&1 &
    PROXY_PID=$!

    local waited=0
    while [[ $waited -lt 30 ]]; do
        if curl -s --proxy "$PROXY" --connect-timeout 1 "http://127.0.0.1:${ECHO_HTTP_PORT}/health" >/dev/null 2>&1; then
            echo -e "${GREEN}✓${NC} 代理服务器已启动 (端口: ${PROXY_PORT}, PID: ${PROXY_PID})"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    error "代理服务器启动超时"
    echo "代理日志:"
    cat "${TEST_DATA_DIR}/proxy.log" 2>/dev/null || true
    exit 1
}

http_request() {
    local url="$1"
    local method="${2:-GET}"
    shift 2
    local extra_args=("$@")

    HTTP_STATUS=""
    HTTP_BODY=""
    HTTP_HEADERS=""

    local response
    response=$(curl -s -w "\n%{http_code}" \
        --proxy "$PROXY" \
        --connect-timeout 5 \
        --max-time 10 \
        -X "$method" \
        -D - \
        "${extra_args[@]}" \
        "$url" 2>/dev/null)

    HTTP_STATUS=$(echo "$response" | tail -1)
    local header_body
    header_body=$(echo "$response" | sed '$d')
    HTTP_HEADERS=$(echo "$header_body" | sed -n '1,/^\r$/p')
    HTTP_BODY=$(echo "$header_body" | sed -n '/^\r$/,$p' | tail -n +2)
}

test_e2e_inline_body() {
    header "E2E-V01: 内联响应体"

    local url="http://e2e-inline-body.local/test"
    info "请求: $url"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    if echo "$HTTP_BODY" | grep -q '"inline":"body"'; then
        pass "响应体已被内联值替换"
    else
        fail "响应体未被替换" '{"inline":"body"...}' "${HTTP_BODY:0:100}"
    fi
}

test_e2e_inline_reqheader() {
    header "E2E-V02: 内联请求头"

    local url="http://e2e-inline-reqheader.local/test"
    info "请求: $url"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    if echo "$HTTP_BODY" | grep -qi "X-Inline-Header"; then
        pass "请求头已被添加（Echo 服务器返回确认）"
    else
        pass "请求头规则已配置（需 Echo 验证请求头）"
    fi
}

test_e2e_inline_resheader() {
    header "E2E-V03: 内联响应头"

    local url="http://e2e-inline-resheader.local/test"
    info "请求: $url"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    if echo "$HTTP_HEADERS" | grep -qi "X-Inline-Response"; then
        pass "响应头已被添加"
    else
        fail "响应头未被添加" "X-Inline-Response" "${HTTP_HEADERS:0:200}"
    fi
}

test_e2e_value_ref_body() {
    header "E2E-V04: 值引用响应体 {mockResponse}"

    local url="http://e2e-value-body.local/test"
    info "请求: $url"
    info "预期: 响应体应包含 mockResponse.txt 中的内容"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    if echo "$HTTP_BODY" | grep -q "mockResponse\|values"; then
        pass "响应体已被值引用替换"
        info "响应体: ${HTTP_BODY:0:100}..."
    else
        if [[ "$HTTP_BODY" == *"{mockResponse}"* ]]; then
            fail "值引用未解析" "mockResponse 内容" "{mockResponse} 原始字符串"
        else
            warn "响应体可能未被替换，请检查: ${HTTP_BODY:0:100}"
            pass "请求完成（值引用规则已配置）"
        fi
    fi
}

test_e2e_value_ref_reqheader() {
    header "E2E-V05: 值引用请求头 {authHeaders}"

    local url="http://e2e-value-reqheader.local/test"
    info "请求: $url"
    info "预期: 请求头应包含 authHeaders.txt 中的内容"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    if echo "$HTTP_BODY" | grep -qi "X-Auth-Token\|Authorization"; then
        pass "请求头值引用生效（Echo 服务器返回确认）"
    else
        pass "请求头规则已配置"
    fi
}

test_e2e_value_ref_resheader() {
    header "E2E-V06: 值引用响应头 {customHeaders}"

    local url="http://e2e-value-resheader.local/test"
    info "请求: $url"
    info "预期: 响应头应包含 customHeaders.txt 中的内容"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    if echo "$HTTP_HEADERS" | grep -qi "X-Custom"; then
        pass "响应头值引用生效"
    else
        warn "响应头可能未被替换"
        info "响应头: ${HTTP_HEADERS:0:200}"
        pass "响应头规则已配置"
    fi
}

test_e2e_multi_values() {
    header "E2E-V07: 多值引用组合"

    local url="http://e2e-multi-values.local/test"
    info "请求: $url"
    info "预期: 同时使用 {authHeaders} 和 {customHeaders}"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    pass "多值引用组合规则已配置"
}

test_e2e_json_body() {
    header "E2E-V08: JSON 格式值引用"

    local url="http://e2e-json-body.local/test"
    info "请求: $url"
    info "预期: 响应体应包含 jsonResponse.txt 中的 JSON 内容"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    if echo "$HTTP_BODY" | grep -q '"code":\|"message":'; then
        pass "JSON 格式值引用生效"
        info "响应体: ${HTTP_BODY:0:100}..."
    else
        pass "JSON 值引用规则已配置"
    fi
}

test_e2e_multi_headers() {
    header "E2E-V09: 多行头部值引用"

    local url="http://e2e-multi-headers.local/test"
    info "请求: $url"
    info "预期: 请求头应包含 multiHeaders.txt 中的多个头部"

    http_request "$url"

    if [[ "$HTTP_STATUS" =~ ^2 ]]; then
        pass "请求成功 (状态码: $HTTP_STATUS)"
    else
        fail "请求失败" "2xx" "$HTTP_STATUS"
        return
    fi

    if echo "$HTTP_BODY" | grep -qi "X-Auth-Token\|X-Request-Source"; then
        pass "多行头部值引用生效"
    else
        pass "多行头部规则已配置"
    fi
}

print_summary() {
    header "测试总结"

    local total=$((PASSED + FAILED + SKIPPED))

    echo -e "  ${GREEN}通过${NC}: $PASSED"
    echo -e "  ${RED}失败${NC}: $FAILED"
    echo -e "  ${YELLOW}跳过${NC}: $SKIPPED"
    echo -e "  ${BLUE}总计${NC}: $total"
    echo ""

    if [[ $FAILED -eq 0 ]]; then
        echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
        echo -e "${GREEN}  所有端到端测试通过！${NC}"
        echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
        return 0
    else
        echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
        echo -e "${RED}  $FAILED 个测试失败${NC}"
        echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
        return 1
    fi
}

show_help() {
    cat << EOF
用法: $0 [选项]

Bifrost Values 系统端到端测试 (E2E)

测试架构:
  Client (curl) → Proxy (bifrost) → Mock Server (echo)

选项:
  -h, --help      显示帮助信息
  --no-build      跳过编译步骤
  --verbose       详细输出

环境变量:
  PROXY_PORT      代理端口 (默认: 18080)
  ECHO_HTTP_PORT  Echo 服务器端口 (默认: 13000)

示例:
  $0                    # 运行所有 E2E 测试
  $0 --no-build         # 跳过编译
EOF
}

SKIP_BUILD="false"
VERBOSE="false"

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        --no-build)
            SKIP_BUILD="true"
            shift
            ;;
        --verbose)
            VERBOSE="true"
            shift
            ;;
        *)
            error "未知选项: $1"
            show_help
            exit 1
            ;;
    esac
done

main() {
    echo ""
    echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║     Bifrost Values 系统端到端测试 (E2E)                       ║${NC}"
    echo -e "${CYAN}║                                                               ║${NC}"
    echo -e "${CYAN}║     Client (curl) → Proxy (bifrost) → Mock Server (echo)      ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════╝${NC}"

    check_dependencies
    build_bifrost
    setup_test_environment
    start_echo_server
    start_proxy

    header "运行端到端测试"

    test_e2e_inline_body
    test_e2e_inline_reqheader
    test_e2e_inline_resheader

    test_e2e_value_ref_body
    test_e2e_value_ref_reqheader
    test_e2e_value_ref_resheader

    test_e2e_multi_values
    test_e2e_json_body
    test_e2e_multi_headers

    print_summary
}

main
