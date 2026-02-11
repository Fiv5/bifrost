#!/bin/bash
# Bifrost Admin API 客户端工具

ADMIN_HOST="${ADMIN_HOST:-127.0.0.1}"
ADMIN_PORT="${ADMIN_PORT:-8899}"
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
