#!/bin/bash
# HTTP 客户端封装 - 简化测试请求发送

_HTTP_CLIENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PROXY_HOST=${PROXY_HOST:-127.0.0.1}
PROXY_PORT=${PROXY_PORT:-8080}
TIMEOUT=${TIMEOUT:-10}

_temp_headers_file=""
_temp_body_file=""

_cleanup_temp() {
    [ -f "$_temp_headers_file" ] && rm -f "$_temp_headers_file"
    [ -f "$_temp_body_file" ] && rm -f "$_temp_body_file"
}

http_request() {
    local url=$1
    local method=${2:-GET}
    local data=$3
    local extra_headers=$4

    _temp_headers_file=$(mktemp)
    _temp_body_file=$(mktemp)

    local curl_args=(
        -s
        -X "$method"
        --max-time "$TIMEOUT"
        -D "$_temp_headers_file"
        -o "$_temp_body_file"
        -w '%{http_code}'
    )

    if [ -n "$TEST_ID" ]; then
        curl_args+=(-H "X-Test-ID: $TEST_ID")
    fi

    if [ -n "$PROXY_HOST" ] && [ -n "$PROXY_PORT" ]; then
        curl_args+=(--proxy "http://${PROXY_HOST}:${PROXY_PORT}")
    fi

    if [ -n "$data" ]; then
        curl_args+=(-d "$data")
    fi

    if [ -n "$extra_headers" ]; then
        while IFS= read -r header; do
            [ -n "$header" ] && curl_args+=(-H "$header")
        done <<< "$extra_headers"
    fi

    curl_args+=("$url")

    HTTP_STATUS=$(curl "${curl_args[@]}")
    HTTP_HEADERS=$(cat "$_temp_headers_file")
    HTTP_BODY=$(cat "$_temp_body_file")

    _cleanup_temp
}

http_get() {
    local url=$1
    local extra_headers=$2
    http_request "$url" "GET" "" "$extra_headers"
}

http_post() {
    local url=$1
    local data=$2
    local extra_headers=$3
    http_request "$url" "POST" "$data" "$extra_headers"
}

http_put() {
    local url=$1
    local data=$2
    local extra_headers=$3
    http_request "$url" "PUT" "$data" "$extra_headers"
}

http_delete() {
    local url=$1
    local extra_headers=$2
    http_request "$url" "DELETE" "" "$extra_headers"
}

http_request_no_proxy() {
    local url=$1
    local method=${2:-GET}
    local data=$3
    local extra_headers=$4

    _temp_headers_file=$(mktemp)
    _temp_body_file=$(mktemp)

    local curl_args=(
        -s
        -X "$method"
        --max-time "$TIMEOUT"
        -D "$_temp_headers_file"
        -o "$_temp_body_file"
        -w '%{http_code}'
        --noproxy '*'
    )

    if [ -n "$TEST_ID" ]; then
        curl_args+=(-H "X-Test-ID: $TEST_ID")
    fi

    if [ -n "$data" ]; then
        curl_args+=(-d "$data")
    fi

    if [ -n "$extra_headers" ]; then
        while IFS= read -r header; do
            [ -n "$header" ] && curl_args+=(-H "$header")
        done <<< "$extra_headers"
    fi

    curl_args+=("$url")

    HTTP_STATUS=$(curl "${curl_args[@]}")
    HTTP_HEADERS=$(cat "$_temp_headers_file")
    HTTP_BODY=$(cat "$_temp_body_file")

    _cleanup_temp
}

https_request() {
    local url=$1
    local method=${2:-GET}
    local data=$3
    local extra_headers=$4

    _temp_headers_file=$(mktemp)
    _temp_body_file=$(mktemp)

    local curl_args=(
        -s
        -k  # 允许自签名证书
        -X "$method"
        --max-time "$TIMEOUT"
        -D "$_temp_headers_file"
        -o "$_temp_body_file"
        -w '%{http_code}'
    )

    if [ -n "$TEST_ID" ]; then
        curl_args+=(-H "X-Test-ID: $TEST_ID")
    fi

    if [ -n "$PROXY_HOST" ] && [ -n "$PROXY_PORT" ]; then
        curl_args+=(--proxy "http://${PROXY_HOST}:${PROXY_PORT}")
    fi

    if [ -n "$data" ]; then
        curl_args+=(-d "$data")
    fi

    if [ -n "$extra_headers" ]; then
        while IFS= read -r header; do
            [ -n "$header" ] && curl_args+=(-H "$header")
        done <<< "$extra_headers"
    fi

    curl_args+=("$url")

    HTTP_STATUS=$(curl "${curl_args[@]}")
    HTTP_HEADERS=$(cat "$_temp_headers_file")
    HTTP_BODY=$(cat "$_temp_body_file")

    _cleanup_temp
}

print_request_result() {
    echo "========================================="
    echo "HTTP Status: $HTTP_STATUS"
    echo "========================================="
    echo "Response Headers:"
    echo "$HTTP_HEADERS"
    echo "========================================="
    echo "Response Body:"
    echo "$HTTP_BODY"
    echo "========================================="
}

get_header() {
    local header_name=$1
    echo "$HTTP_HEADERS" | grep -i "^${header_name}:" | head -1 | sed "s/^${header_name}:[[:space:]]*//" | tr -d '\r'
}

get_json_field() {
    local jq_path=$1
    echo "$HTTP_BODY" | jq -r "$jq_path" 2>/dev/null
}
