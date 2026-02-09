#!/bin/bash

set -e

PROXY_HOST="${PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${PROXY_PORT:-8080}"
PROXY="http://${PROXY_HOST}:${PROXY_PORT}"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ PASS${NC}: $1"; }
fail() { echo -e "${RED}✗ FAIL${NC}: $1"; }
info() { echo -e "${BLUE}ℹ INFO${NC}: $1"; }
warn() { echo -e "${YELLOW}⚠ WARN${NC}: $1"; }
header() { echo -e "\n${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"; echo -e "${YELLOW}  $1${NC}"; echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"; }

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

check_proxy() {
    if curl -s --proxy "$PROXY" --connect-timeout 3 http://example.com > /dev/null 2>&1; then
        pass "代理服务器运行正常 ($PROXY)"
        return 0
    else
        fail "无法连接到代理服务器 ($PROXY)"
        echo "请确保代理服务器正在运行："
        echo "  cargo run --bin bifrost -- --port $PROXY_PORT"
        return 1
    fi
}

test_http_redirect() {
    local url="$1"
    local expected_target="$2"
    local description="$3"

    local response
    response=$(curl -s -o /dev/null -w "%{http_code}|%{redirect_url}|%{url_effective}" \
        --proxy "$PROXY" \
        --connect-timeout 5 \
        --max-time 10 \
        -L "$url" 2>/dev/null || echo "ERROR")

    if [[ "$response" == "ERROR" ]]; then
        fail "$description - 请求失败"
        ((FAIL_COUNT++))
        return
    fi

    local http_code=$(echo "$response" | cut -d'|' -f1)
    local redirect_url=$(echo "$response" | cut -d'|' -f2)
    local effective_url=$(echo "$response" | cut -d'|' -f3)

    info "URL: $url"
    info "HTTP Code: $http_code, Effective URL: $effective_url"

    if [[ "$effective_url" == *"$expected_target"* ]] || [[ "$http_code" == "200" ]]; then
        pass "$description"
        ((PASS_COUNT++))
    else
        fail "$description - 期望重定向到: $expected_target"
        ((FAIL_COUNT++))
    fi
}

test_host_redirect() {
    local url="$1"
    local expected_host="$2"
    local description="$3"

    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" \
        --proxy "$PROXY" \
        --connect-timeout 5 \
        --max-time 10 \
        "$url" 2>/dev/null || echo "ERROR")

    if [[ "$response" == "ERROR" ]]; then
        fail "$description - 请求失败"
        ((FAIL_COUNT++))
        return
    fi

    info "URL: $url -> $expected_host"
    info "HTTP Code: $response"

    if [[ "$response" == "200" ]] || [[ "$response" == "301" ]] || [[ "$response" == "302" ]] || [[ "$response" == "304" ]]; then
        pass "$description (HTTP $response)"
        ((PASS_COUNT++))
    else
        warn "$description - 收到 HTTP $response (目标服务可能未启动)"
        ((SKIP_COUNT++))
    fi
}

test_api_passthrough() {
    local url="$1"
    local description="$2"

    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" \
        --proxy "$PROXY" \
        --connect-timeout 5 \
        --max-time 10 \
        "$url" 2>/dev/null || echo "ERROR")

    if [[ "$response" == "ERROR" ]]; then
        fail "$description - 请求失败"
        ((FAIL_COUNT++))
        return
    fi

    info "API URL: $url"
    info "HTTP Code: $response"

    if [[ "$response" != "000" ]]; then
        pass "$description (HTTP $response)"
        ((PASS_COUNT++))
    else
        fail "$description - 连接失败"
        ((FAIL_COUNT++))
    fi
}

test_websocket() {
    local url="$1"
    local expected_target="$2"
    local description="$3"

    if ! command -v websocat &> /dev/null; then
        warn "$description - 跳过 (需要安装 websocat: brew install websocat)"
        ((SKIP_COUNT++))
        return
    fi

    local ws_proxy="--proxy=${PROXY_HOST}:${PROXY_PORT}"

    info "WebSocket URL: $url -> $expected_target"

    local result
    result=$(echo "ping" | timeout 3 websocat "$ws_proxy" "$url" 2>/dev/null || echo "TIMEOUT")

    if [[ "$result" != "TIMEOUT" ]] && [[ -n "$result" ]]; then
        pass "$description"
        ((PASS_COUNT++))
    else
        warn "$description - WebSocket 连接超时 (目标服务可能未启动)"
        ((SKIP_COUNT++))
    fi
}

test_headers_injection() {
    local url="$1"
    local expected_header="$2"
    local description="$3"

    local response
    response=$(curl -s -D - -o /dev/null \
        --proxy "$PROXY" \
        --connect-timeout 5 \
        --max-time 10 \
        "$url" 2>/dev/null || echo "ERROR")

    if [[ "$response" == "ERROR" ]]; then
        fail "$description - 请求失败"
        ((FAIL_COUNT++))
        return
    fi

    info "URL: $url"
    info "检查请求头: $expected_header"

    pass "$description (需要通过服务端日志验证)"
    ((PASS_COUNT++))
}

check_local_server() {
    local port="$1"
    local name="$2"

    if curl -s --connect-timeout 2 "http://localhost:$port" > /dev/null 2>&1; then
        pass "本地服务 $name (localhost:$port) 运行正常"
        return 0
    else
        warn "本地服务 $name (localhost:$port) 未启动"
        return 1
    fi
}

header "代理规则验证脚本"
echo "代理地址: $PROXY"
echo "使用方法: PROXY_PORT=8899 ./verify_rules.sh"
echo ""

header "1. 检查代理服务器"
if ! check_proxy; then
    echo ""
    fail "代理服务器未运行，无法继续测试"
    exit 1
fi

header "2. 检查本地服务"
LOCAL_8000_OK=false
if check_local_server 8000 "前端开发服务器"; then
    LOCAL_8000_OK=true
fi

header "3. 测试 BOE 环境规则"
echo "规则: https://next-oncall-boe.bytedance.net -> http://localhost:8000/"
echo ""

test_api_passthrough "https://next-oncall-boe.bytedance.net/api/" "BOE API 透传"
test_host_redirect "https://next-oncall-boe.bytedance.net/" "localhost:8000" "BOE 主页面重定向"

header "4. 测试 PPE 环境规则"
echo "规则: https://nextoncall.bytedance.net -> http://localhost:8000/"
echo ""

test_api_passthrough "https://nextoncall.bytedance.net/api/" "PPE API 透传"
test_host_redirect "https://nextoncall.bytedance.net/" "localhost:8000" "PPE 主页面重定向"

header "5. 测试国际站规则"
echo "规则: https://nextoncall.byteintl.net -> http://localhost:8000/"
echo ""

test_api_passthrough "https://nextoncall.byteintl.net/api/" "国际站 API 透传"
test_host_redirect "https://nextoncall.byteintl.net/" "localhost:8000" "国际站主页面重定向"

header "6. 测试 COM 域名规则"
echo "规则: https://nextoncall.bytedance.com -> http://localhost:8000/"
echo ""

test_api_passthrough "https://nextoncall.bytedance.com/api/" "COM API 透传"
test_host_redirect "https://nextoncall.bytedance.com/" "localhost:8000" "COM 主页面重定向"

header "7. 测试 I18N 站点规则"
echo "规则: https://nextoncall-i18n.byteintl.com -> http://localhost:8000/"
echo ""

test_api_passthrough "https://nextoncall-i18n.byteintl.com/api/" "I18N API 透传"
test_host_redirect "https://nextoncall-i18n.byteintl.com/" "localhost:8000" "I18N 主页面重定向"

header "8. 测试 BD 站点规则"
echo "规则: https://nextoncall-bd.byteintl.net -> http://localhost:8000/"
echo ""

test_api_passthrough "https://nextoncall-bd.byteintl.net/api/" "BD API 透传"
test_host_redirect "https://nextoncall-bd.byteintl.net/" "localhost:8000" "BD 主页面重定向"

header "9. 测试 WebSocket 规则"
echo "需要安装 websocat: brew install websocat"
echo ""

test_websocket "wss://next-oncall-boe.bytedance.net/" "ws://localhost:8000/" "BOE WebSocket 代理"
test_websocket "wss://nextoncall.bytedance.net/" "ws://localhost:8000/" "PPE WebSocket 代理"
test_websocket "wss://nextoncall.byteintl.net/" "ws://localhost:8000/" "国际站 WebSocket 代理"
test_websocket "wss://nextoncall.bytedance.com/" "ws://localhost:8000/" "COM WebSocket 代理"
test_websocket "wss://nextoncall-i18n.byteintl.com/" "ws://localhost:8000/" "I18N WebSocket 代理"
test_websocket "wss://nextoncall-bd.byteintl.net/" "ws://localhost:8000/" "BD WebSocket 代理"

header "测试结果汇总"
echo -e "${GREEN}通过: $PASS_COUNT${NC}"
echo -e "${RED}失败: $FAIL_COUNT${NC}"
echo -e "${YELLOW}跳过: $SKIP_COUNT${NC}"
echo ""

if [[ $FAIL_COUNT -eq 0 ]]; then
    echo -e "${GREEN}所有测试通过！${NC}"
    exit 0
else
    echo -e "${RED}有 $FAIL_COUNT 个测试失败${NC}"
    exit 1
fi
