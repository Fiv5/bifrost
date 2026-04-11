#!/bin/bash

set -euo pipefail

# 避免环境中的代理变量干扰本地 curl（特别是 http_proxy/https_proxy）。
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY all_proxy ALL_PROXY no_proxy NO_PROXY

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/../test_utils/assert.sh"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"

HTTP_STATUS=""
HTTP_HEADERS=""
HTTP_BODY=""

http_request() {
    local url="$1"
    local method="${2:-GET}"
    local data="${3:-}"
    local extra_headers="${4:-}"
    local proxy="${5:-}"

    local headers_file
    local body_file
    headers_file="$(mktemp)"
    body_file="$(mktemp)"

    local curl_args=(
        -sS
        -X "$method"
        --max-time 15
        -D "$headers_file"
        -o "$body_file"
        -w '%{http_code}'
    )

    if [[ -n "$proxy" ]]; then
        curl_args+=(--proxy "$proxy")
    fi

    if [[ -n "$data" ]]; then
        curl_args+=(-H "Content-Type: application/json" --data-binary "$data")
    fi

    if [[ -n "$extra_headers" ]]; then
        while IFS= read -r header; do
            [[ -n "$header" ]] && curl_args+=(-H "$header")
        done <<< "$extra_headers"
    fi

    HTTP_STATUS="$(curl "${curl_args[@]}" "$url" 2>/dev/null)" || HTTP_STATUS="000"
    HTTP_HEADERS="$(cat "$headers_file" | tr -d '\r')"
    HTTP_BODY="$(cat "$body_file")"

    rm -f "$headers_file" "$body_file"
}

http_get() {
    local url="$1"
    local extra_headers="${2:-}"
    http_request "$url" "GET" "" "$extra_headers" ""
}

http_post_json() {
    local url="$1"
    local data="$2"
    local extra_headers="${3:-}"
    http_request "$url" "POST" "$data" "$extra_headers" ""
}

proxy_get() {
    local proxy_url="$1"
    local url="$2"
    http_request "$url" "GET" "" "" "$proxy_url"
}

ADMIN_PORT="${ADMIN_PORT:-18800}"
ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
export ADMIN_PATH_PREFIX

ADMIN_HOST="${ADMIN_HOST:-0.0.0.0}"
export ADMIN_HOST
export ADMIN_BASE_URL="${ADMIN_BASE_URL:-http://127.0.0.1:${ADMIN_PORT}${ADMIN_PATH_PREFIX}}"

BIFROST_DATA_DIR="${BIFROST_DATA_DIR:-$SCRIPT_DIR/../../.bifrost-e2e-remote-auth-$RANDOM}"
export BIFROST_DATA_DIR

MOCK_SERVERS="$SCRIPT_DIR/../mock_servers/start_servers.sh"

cleanup() {
    admin_cleanup_bifrost || true
    "$MOCK_SERVERS" stop >/dev/null 2>&1 || true
    rm -rf "$BIFROST_DATA_DIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

log() { echo "[remote-auth-e2e] $*"; }

require_cmd() {
    local cmd="$1"
    command -v "$cmd" >/dev/null 2>&1 || {
        echo "Missing required command: $cmd" >&2
        exit 1
    }
}

require_cmd curl
require_cmd jq
require_cmd cargo

log "Build bifrost (release)..."
(cd "$SCRIPT_DIR/../.." && cargo build --release --bin bifrost >/dev/null)

BIFROST_BIN="$SCRIPT_DIR/../../target/release/bifrost"
if [[ ! -x "$BIFROST_BIN" ]]; then
    echo "bifrost binary not found at $BIFROST_BIN" >&2
    exit 1
fi

log "Start mock servers..."
"$MOCK_SERVERS" start-bg >/dev/null

log "Start bifrost (admin+proxy)..."
export ADMIN_HOST
export ADMIN_PORT
admin_start_bifrost

ADMIN_BASE_URL_EFFECTIVE="${ADMIN_BASE_URL}"
ADMIN_URL_127="http://127.0.0.1:${ADMIN_PORT}${ADMIN_PATH_PREFIX}"

get_non_loopback_ip() {
    local ip
    ip=$(ip route get 1.1.1.1 2>/dev/null | awk '{for (i=1;i<=NF;i++) if ($i=="src") {print $(i+1); exit}}') || true
    if [[ -z "${ip:-}" ]]; then
        ip=$(hostname -I 2>/dev/null | awk '{print $1}') || true
    fi
    echo "${ip:-}"
}

NON_LOOPBACK_IP="$(get_non_loopback_ip)"

log "Case: remote access disabled -> local admin should work"
http_get "${ADMIN_URL_127}/api/auth/status"
assert_status "200" "$HTTP_STATUS" "本地访问 /api/auth/status 应返回 200"
assert_json_field ".remote_access_enabled" "false" "$HTTP_BODY"
assert_json_field ".auth_required" "false" "$HTTP_BODY"

if [[ -n "${NON_LOOPBACK_IP:-}" ]]; then
    log "Case: remote access disabled -> non-loopback admin should be rejected"
    http_get "http://${NON_LOOPBACK_IP}:${ADMIN_PORT}${ADMIN_PATH_PREFIX}/api/auth/status"
    assert_status "403" "$HTTP_STATUS" "非 loopback 访问管理端应返回 403"
else
    _log_warning "无法获取非 loopback IP，跳过非本地访问门禁断言"
fi

log "Ensure data plane forwarding works before enabling remote auth"
RULE_NAME="remote_auth_forward_${RANDOM}"
create_rule "$RULE_NAME" "example.com http://127.0.0.1:3000" "true" >/dev/null

proxy_get "http://127.0.0.1:${ADMIN_PORT}" "http://example.com/remote-auth-pre"
assert_status "200" "$HTTP_STATUS" "代理转发在鉴权开启前应正常工作"
assert_json_field ".server.port" "3000" "$HTTP_BODY"

log "Enable remote access and set admin password via CLI"
ADMIN_PASSWORD="test-pass-${RANDOM}-${RANDOM}"
printf '%s\n' "$ADMIN_PASSWORD" | "$BIFROST_BIN" admin passwd --username admin --password-stdin >/dev/null
"$BIFROST_BIN" admin remote enable >/dev/null

log "Case: protected API without token -> 401"
http_get "${ADMIN_URL_127}/api/system/status"
assert_status "401" "$HTTP_STATUS" "未携带 Token 访问受保护 API 应返回 401"

log "Login -> get token"
LOGIN_PAYLOAD=$(jq -cn --arg u "admin" --arg p "$ADMIN_PASSWORD" '{username:$u,password:$p}')
http_post_json "${ADMIN_URL_127}/api/auth/login" "$LOGIN_PAYLOAD"
assert_status "200" "$HTTP_STATUS" "登录接口应返回 200"
TOKEN=$(echo "$HTTP_BODY" | jq -r '.token')
if [[ -z "${TOKEN:-}" || "$TOKEN" == "null" ]]; then
    echo "Failed to get token from login response: $HTTP_BODY" >&2
    exit 1
fi

log "Case: protected API with token -> 200"
http_get "${ADMIN_URL_127}/api/system/status" "Authorization: Bearer ${TOKEN}"
assert_status "200" "$HTTP_STATUS" "携带有效 Token 访问受保护 API 应返回 200"

log "Revoke all sessions via CLI"
"$BIFROST_BIN" admin revoke-all >/dev/null

log "Case: old token after revoke-all -> 401"
http_get "${ADMIN_URL_127}/api/system/status" "Authorization: Bearer ${TOKEN}"
assert_status "401" "$HTTP_STATUS" "revoke-all 后旧 Token 应失效返回 401"

log "Ensure data plane forwarding still works after enabling/revoking admin auth"
proxy_get "http://127.0.0.1:${ADMIN_PORT}" "http://example.com/remote-auth-post"
assert_status "200" "$HTTP_STATUS" "代理转发不应受管理端鉴权影响"
assert_json_field ".server.port" "3000" "$HTTP_BODY"

delete_rule "$RULE_NAME" >/dev/null 2>&1 || true

log "All assertions: total=$TOTAL_ASSERTIONS passed=$PASSED_ASSERTIONS failed=$FAILED_ASSERTIONS"
if [[ "$FAILED_ASSERTIONS" -ne 0 ]]; then
    exit 1
fi
