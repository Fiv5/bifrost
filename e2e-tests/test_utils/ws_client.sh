#!/bin/bash
# WebSocket 客户端测试工具
# 依赖: websocat (brew install websocat)

WS_TEMP_DIR="${WS_TEMP_DIR:-/tmp/bifrost_ws_test}"
WS_TIMEOUT="${WS_TIMEOUT:-10}"

ws_ensure_deps() {
    if ! command -v websocat &> /dev/null; then
        echo "Error: websocat is required. Install with: brew install websocat" >&2
        return 1
    fi
    mkdir -p "$WS_TEMP_DIR"
}

ws_connect() {
    local url="$1"
    local conn_id="${2:-$(date +%s%N)}"
    local output_file="$WS_TEMP_DIR/ws_${conn_id}.out"
    local pid_file="$WS_TEMP_DIR/ws_${conn_id}.pid"
    local input_fifo="$WS_TEMP_DIR/ws_${conn_id}.in"
    
    ws_ensure_deps || return 1
    
    rm -f "$output_file" "$pid_file" "$input_fifo"
    mkfifo "$input_fifo"
    touch "$output_file"
    
    (cat "$input_fifo" | websocat -t "$url" >> "$output_file" 2>&1) &
    local ws_pid=$!
    echo "$ws_pid" > "$pid_file"
    
    sleep 0.5
    
    if ! kill -0 "$ws_pid" 2>/dev/null; then
        echo "Error: Failed to connect to $url" >&2
        cat "$output_file" >&2
        return 1
    fi
    
    echo "$conn_id"
}

ws_send() {
    local conn_id="$1"
    local message="$2"
    local input_fifo="$WS_TEMP_DIR/ws_${conn_id}.in"
    
    if [[ ! -p "$input_fifo" ]]; then
        echo "Error: Connection $conn_id not found" >&2
        return 1
    fi
    
    echo "$message" > "$input_fifo"
}

ws_recv() {
    local conn_id="$1"
    local timeout="${2:-$WS_TIMEOUT}"
    local output_file="$WS_TEMP_DIR/ws_${conn_id}.out"
    
    if [[ ! -f "$output_file" ]]; then
        echo "Error: Connection $conn_id not found" >&2
        return 1
    fi
    
    local start_lines=$(wc -l < "$output_file")
    local waited=0
    
    while [[ $waited -lt $timeout ]]; do
        local current_lines=$(wc -l < "$output_file")
        if [[ $current_lines -gt $start_lines ]]; then
            tail -n +$((start_lines + 1)) "$output_file"
            return 0
        fi
        sleep 0.1
        waited=$((waited + 1))
    done
    
    return 1
}

ws_get_all_messages() {
    local conn_id="$1"
    local output_file="$WS_TEMP_DIR/ws_${conn_id}.out"
    
    if [[ -f "$output_file" ]]; then
        cat "$output_file"
    fi
}

ws_wait_messages() {
    local conn_id="$1"
    local count="$2"
    local timeout="${3:-$WS_TIMEOUT}"
    local output_file="$WS_TEMP_DIR/ws_${conn_id}.out"
    
    local waited=0
    while [[ $waited -lt $((timeout * 10)) ]]; do
        local current_count=$(wc -l < "$output_file" | tr -d ' ')
        if [[ $current_count -ge $count ]]; then
            cat "$output_file"
            return 0
        fi
        sleep 0.1
        waited=$((waited + 1))
    done
    
    echo "Timeout waiting for $count messages (got $(wc -l < "$output_file"))" >&2
    cat "$output_file"
    return 1
}

ws_close() {
    local conn_id="$1"
    local pid_file="$WS_TEMP_DIR/ws_${conn_id}.pid"
    local input_fifo="$WS_TEMP_DIR/ws_${conn_id}.in"
    local output_file="$WS_TEMP_DIR/ws_${conn_id}.out"
    
    if [[ -f "$pid_file" ]]; then
        local pid=$(cat "$pid_file")
        kill "$pid" 2>/dev/null
        rm -f "$pid_file"
    fi
    
    rm -f "$input_fifo" "$output_file"
}

ws_cleanup_all() {
    rm -rf "$WS_TEMP_DIR"
}

ws_send_recv() {
    local url="$1"
    local message="$2"
    local timeout="${3:-5}"
    
    ws_ensure_deps || return 1
    
    echo "$message" | timeout "$timeout" websocat -t -1 "$url" 2>/dev/null
}

ws_connect_recv_all() {
    local url="$1"
    local timeout="${2:-10}"
    
    ws_ensure_deps || return 1
    
    timeout "$timeout" websocat -t "$url" 2>/dev/null
}
