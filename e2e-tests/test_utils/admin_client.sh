#!/bin/bash
# Bifrost Admin API 客户端工具

ADMIN_HOST="${ADMIN_HOST:-127.0.0.1}"
ADMIN_PORT="${ADMIN_PORT:-9900}"
ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
ADMIN_BASE_URL="${ADMIN_BASE_URL:-http://${ADMIN_HOST}:${ADMIN_PORT}${ADMIN_PATH_PREFIX}}"

admin_get() {
    local path="$1"
    curl -s "${ADMIN_BASE_URL}${path}"
}

admin_post() {
    local path="$1"
    local data="$2"
    curl -s -X POST -H "Content-Type: application/json" -d "$data" "${ADMIN_BASE_URL}${path}"
}

admin_delete() {
    local path="$1"
    curl -s -X DELETE "${ADMIN_BASE_URL}${path}"
}

get_traffic_list() {
    local arg1="$1"
    local arg2="$2"
    local arg3="${3:-100}"

    if [[ -n "$arg2" ]]; then
        local host="$arg1"
        local port="$arg2"
        local limit="$arg3"
        curl -s "http://${host}:${port}${ADMIN_PATH_PREFIX}/api/traffic?limit=${limit}"
    else
        local limit="${arg1:-100}"
        admin_get "/api/traffic?limit=${limit}"
    fi
}

get_traffic_detail() {
    local id="$1"
    admin_get "/api/traffic/${id}"
}

get_traffic_by_url() {
    local url_pattern="$1"
    local limit="${2:-10}"

    get_traffic_list "$limit" | jq -r ".records[] | select(.url | contains(\"$url_pattern\"))"
}

find_traffic_id_by_url() {
    local host="$1"
    local port="$2"
    local url_pattern="$3"
    local limit="${4:-50}"

    if [[ -z "$url_pattern" ]]; then
        url_pattern="$host"
        limit="${port:-50}"
        get_traffic_list "$limit" | jq -r ".records[] | select(.url | contains(\"$url_pattern\")) | .id" | head -1
    else
        curl -s "http://${host}:${port}${ADMIN_PATH_PREFIX}/api/traffic?limit=${limit}" | jq -r ".records[] | select(.url | contains(\"$url_pattern\")) | .id" | head -1
    fi
}

get_frames() {
    local arg1="$1"
    local arg2="$2"
    local arg3="$3"
    local arg4="${4:-0}"
    local arg5="${5:-100}"

    if [[ -n "$arg3" ]]; then
        local host="$arg1"
        local port="$arg2"
        local traffic_id="$arg3"
        local after="$arg4"
        local limit="$arg5"
        curl -s "http://${host}:${port}${ADMIN_PATH_PREFIX}/api/traffic/${traffic_id}/frames?after=${after}&limit=${limit}"
    else
        local traffic_id="$arg1"
        local after="${arg2:-0}"
        local limit="${arg3:-100}"
        admin_get "/api/traffic/${traffic_id}/frames?after=${after}&limit=${limit}"
    fi
}

get_frame_detail() {
    local traffic_id="$1"
    local frame_id="$2"

    admin_get "/api/traffic/${traffic_id}/frames/${frame_id}"
}

get_frame_count() {
    local traffic_id="$1"

    get_frames "$traffic_id" | jq -r '.frames | length'
}

wait_for_frames() {
    local traffic_id="$1"
    local expected_count="$2"
    local timeout="${3:-10}"

    local waited=0
    while [[ $waited -lt $((timeout * 10)) ]]; do
        local count=$(get_frame_count "$traffic_id")
        if [[ "$count" -ge "$expected_count" ]]; then
            return 0
        fi
        sleep 0.1
        waited=$((waited + 1))
    done

    echo "Timeout waiting for $expected_count frames (got $(get_frame_count "$traffic_id"))" >&2
    return 1
}

subscribe_frames() {
    local traffic_id="$1"
    local timeout="${2:-10}"

    timeout "$timeout" curl -sN "${ADMIN_BASE_URL}/api/traffic/${traffic_id}/frames/stream" 2>/dev/null
}

subscribe_frames_bg() {
    local traffic_id="$1"
    local output_file="$2"

    curl -sN "${ADMIN_BASE_URL}/api/traffic/${traffic_id}/frames/stream" > "$output_file" 2>/dev/null &
    echo $!
}

list_websocket_connections() {
    admin_get "/api/websocket/connections"
}

start_monitoring() {
    local traffic_id="$1"
    admin_post "/api/traffic/${traffic_id}/monitor" '{"action": "start"}'
}

stop_monitoring() {
    local traffic_id="$1"
    admin_post "/api/traffic/${traffic_id}/monitor" '{"action": "stop"}'
}

get_metrics() {
    admin_get "/api/metrics"
}

check_health() {
    admin_get "/api/system/status"
}

wait_for_admin() {
    local timeout="${1:-30}"
    local waited=0

    while [[ $waited -lt $timeout ]]; do
        if curl -s "${ADMIN_BASE_URL}/api/system/status" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    echo "Timeout waiting for admin server at ${ADMIN_BASE_URL}" >&2
    return 1
}

is_websocket_traffic() {
    local traffic_id="$1"
    get_traffic_detail "$traffic_id" | jq -r '.is_websocket'
}

is_sse_traffic() {
    local traffic_id="$1"
    get_traffic_detail "$traffic_id" | jq -r '.is_sse'
}

get_frame_types() {
    local traffic_id="$1"
    get_frames "$traffic_id" | jq -r '.frames[].frame_type' | sort | uniq -c
}

get_frame_directions() {
    local traffic_id="$1"
    get_frames "$traffic_id" | jq -r '.frames[].direction' | sort | uniq -c
}

clear_traffic() {
    admin_delete "/api/traffic"
}

admin_put() {
    local path="$1"
    local data="$2"
    curl -s -X PUT -H "Content-Type: application/json" -d "$data" "${ADMIN_BASE_URL}${path}"
}

admin_delete_with_body() {
    local path="$1"
    local data="$2"
    curl -s -X DELETE -H "Content-Type: application/json" -d "$data" "${ADMIN_BASE_URL}${path}"
}

list_rules() {
    admin_get "/api/rules"
}

get_rule() {
    local name="$1"
    admin_get "/api/rules/${name}"
}

create_rule() {
    local name="$1"
    local content="$2"
    local enabled="${3:-true}"
    admin_post "/api/rules" "{\"name\":\"${name}\",\"content\":\"${content}\",\"enabled\":${enabled}}"
}

update_rule() {
    local name="$1"
    local content="$2"
    local enabled="${3:-true}"
    admin_put "/api/rules/${name}" "{\"content\":\"${content}\",\"enabled\":${enabled}}"
}

delete_rule() {
    local name="$1"
    admin_delete "/api/rules/${name}"
}

enable_rule() {
    local name="$1"
    admin_put "/api/rules/${name}/enable" "{}"
}

disable_rule() {
    local name="$1"
    admin_put "/api/rules/${name}/disable" "{}"
}

list_values() {
    admin_get "/api/values"
}

get_value() {
    local name="$1"
    admin_get "/api/values/${name}"
}

create_value() {
    local name="$1"
    local value="$2"
    admin_post "/api/values" "{\"name\":\"${name}\",\"value\":\"${value}\"}"
}

update_value() {
    local name="$1"
    local value="$2"
    admin_put "/api/values/${name}" "{\"value\":\"${value}\"}"
}

delete_value() {
    local name="$1"
    admin_delete "/api/values/${name}"
}

get_whitelist() {
    admin_get "/api/whitelist"
}

add_whitelist() {
    local ip_or_cidr="$1"
    admin_post "/api/whitelist" "{\"ip_or_cidr\":\"${ip_or_cidr}\"}"
}

remove_whitelist() {
    local ip_or_cidr="$1"
    admin_delete_with_body "/api/whitelist" "{\"ip_or_cidr\":\"${ip_or_cidr}\"}"
}

get_whitelist_mode() {
    admin_get "/api/whitelist/mode"
}

set_whitelist_mode() {
    local mode="$1"
    admin_put "/api/whitelist/mode" "{\"mode\":\"${mode}\"}"
}

get_allow_lan() {
    admin_get "/api/whitelist/allow-lan"
}

set_allow_lan() {
    local allow="$1"
    admin_put "/api/whitelist/allow-lan" "{\"allow_lan\":${allow}}"
}

add_temporary_whitelist() {
    local ip="$1"
    admin_post "/api/whitelist/temporary" "{\"ip\":\"${ip}\"}"
}

remove_temporary_whitelist() {
    local ip="$1"
    admin_delete_with_body "/api/whitelist/temporary" "{\"ip\":\"${ip}\"}"
}

get_pending_authorizations() {
    admin_get "/api/whitelist/pending"
}

approve_authorization() {
    local ip="$1"
    admin_post "/api/whitelist/pending/approve" "{\"ip\":\"${ip}\"}"
}

reject_authorization() {
    local ip="$1"
    admin_post "/api/whitelist/pending/reject" "{\"ip\":\"${ip}\"}"
}

clear_pending_authorizations() {
    admin_delete "/api/whitelist/pending"
}

get_cert_info() {
    admin_get "/api/cert/info"
}

download_cert() {
    curl -s "${ADMIN_BASE_URL%${ADMIN_PATH_PREFIX}}${ADMIN_PATH_PREFIX}/public/cert"
}

get_cert_qrcode() {
    curl -s "${ADMIN_BASE_URL%${ADMIN_PATH_PREFIX}}${ADMIN_PATH_PREFIX}/public/cert/qrcode"
}

get_system_proxy() {
    admin_get "/api/proxy/system"
}

set_system_proxy() {
    local enabled="$1"
    local bypass="${2:-}"
    if [[ -n "$bypass" ]]; then
        admin_put "/api/proxy/system" "{\"enabled\":${enabled},\"bypass\":\"${bypass}\"}"
    else
        admin_put "/api/proxy/system" "{\"enabled\":${enabled}}"
    fi
}

get_system_proxy_support() {
    admin_get "/api/proxy/system/support"
}

get_system_info() {
    admin_get "/api/system"
}

get_system_overview() {
    admin_get "/api/system/overview"
}

get_metrics_history() {
    local limit="${1:-100}"
    admin_get "/api/metrics/history?limit=${limit}"
}

get_tls_config() {
    admin_get "/api/config/tls"
}

update_tls_config() {
    local data="$1"
    admin_put "/api/config/tls" "$data"
}

set_unsafe_ssl() {
    local unsafe_ssl="$1"
    admin_put "/api/config/tls" "{\"unsafe_ssl\":${unsafe_ssl}}"
}

get_unsafe_ssl() {
    admin_get "/api/config/tls" | jq -r '.unsafe_ssl'
}
