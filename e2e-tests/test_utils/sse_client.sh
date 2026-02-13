#!/bin/bash
# SSE 客户端测试工具

SSE_TEMP_DIR="${SSE_TEMP_DIR:-/tmp/bifrost_sse_test}"
SSE_TIMEOUT="${SSE_TIMEOUT:-30}"
SSE_PROXY="${SSE_PROXY:-}"

sse_timeout_cmd() {
    if command -v gtimeout >/dev/null 2>&1; then
        gtimeout "$@"
    elif command -v timeout >/dev/null 2>&1; then
        timeout "$@"
    else
        local timeout_val="$1"
        shift
        perl -e '
            use POSIX ":sys_wait_h";
            my $timeout = $ARGV[0];
            shift @ARGV;
            my $pid = fork();
            if ($pid == 0) {
                exec(@ARGV);
                exit(1);
            }
            eval {
                local $SIG{ALRM} = sub { die "timeout\n" };
                alarm($timeout);
                waitpid($pid, 0);
                alarm(0);
            };
            if ($@ eq "timeout\n") {
                kill("TERM", $pid);
                waitpid($pid, 0);
                exit(124);
            }
            exit($? >> 8);
        ' "$timeout_val" "$@"
    fi
}

sse_ensure_dir() {
    mkdir -p "$SSE_TEMP_DIR"
}

sse_connect() {
    local url="$1"
    local conn_id="${2:-$(date +%s%N)}"
    local output_file="$SSE_TEMP_DIR/sse_${conn_id}.out"
    local pid_file="$SSE_TEMP_DIR/sse_${conn_id}.pid"
    local events_file="$SSE_TEMP_DIR/sse_${conn_id}.events"
    local last_event_id="${3:-}"

    sse_ensure_dir

    rm -f "$output_file" "$pid_file" "$events_file"
    touch "$output_file" "$events_file"

    local curl_args=("-sN")
    if [[ -n "$last_event_id" ]]; then
        curl_args+=("-H" "Last-Event-ID: $last_event_id")
    fi
    if [[ -n "$SSE_PROXY" ]]; then
        curl_args+=("-x" "$SSE_PROXY")
    fi
    curl_args+=("$url")

    (curl "${curl_args[@]}" 2>/dev/null | while IFS= read -r line; do
        echo "$line" >> "$output_file"

        if [[ "$line" == data:* ]]; then
            echo "${line#data: }" >> "$events_file"
        fi
    done) &

    local curl_pid=$!
    echo "$curl_pid" > "$pid_file"

    sleep 0.3

    if ! kill -0 "$curl_pid" 2>/dev/null; then
        if [[ ! -s "$output_file" ]]; then
            echo "Error: Failed to connect to $url" >&2
            return 1
        fi
    fi

    echo "$conn_id"
}

sse_get_raw() {
    local conn_id="$1"
    local output_file="$SSE_TEMP_DIR/sse_${conn_id}.out"

    if [[ -f "$output_file" ]]; then
        cat "$output_file"
    fi
}

sse_get_events() {
    local conn_id="$1"
    local events_file="$SSE_TEMP_DIR/sse_${conn_id}.events"

    if [[ -f "$events_file" ]]; then
        cat "$events_file"
    fi
}

sse_get_event_count() {
    local conn_id="$1"
    local events_file="$SSE_TEMP_DIR/sse_${conn_id}.events"

    if [[ -f "$events_file" ]]; then
        wc -l < "$events_file" | tr -d ' '
    else
        echo "0"
    fi
}

sse_wait_events() {
    local conn_id="$1"
    local count="$2"
    local timeout="${3:-$SSE_TIMEOUT}"
    local events_file="$SSE_TEMP_DIR/sse_${conn_id}.events"

    local waited=0
    while [[ $waited -lt $((timeout * 10)) ]]; do
        local current_count=$(sse_get_event_count "$conn_id")
        if [[ $current_count -ge $count ]]; then
            cat "$events_file"
            return 0
        fi
        sleep 0.1
        waited=$((waited + 1))
    done

    echo "Timeout waiting for $count events (got $(sse_get_event_count "$conn_id"))" >&2
    cat "$events_file" 2>/dev/null
    return 1
}

sse_disconnect() {
    local conn_id="$1"
    local pid_file="$SSE_TEMP_DIR/sse_${conn_id}.pid"
    local output_file="$SSE_TEMP_DIR/sse_${conn_id}.out"
    local events_file="$SSE_TEMP_DIR/sse_${conn_id}.events"

    if [[ -f "$pid_file" ]]; then
        local pid=$(cat "$pid_file")
        kill "$pid" 2>/dev/null
        rm -f "$pid_file"
    fi

    rm -f "$output_file" "$events_file"
}

sse_cleanup_all() {
    pkill -f "curl.*text/event-stream" 2>/dev/null
    rm -rf "$SSE_TEMP_DIR"
}

sse_fetch_all() {
    local base_url="$1"
    local path="$2"
    local timeout="${3:-$SSE_TIMEOUT}"

    local url="$base_url"
    if [[ -n "$path" ]]; then
        url="${base_url}${path}"
    fi

    if [[ -n "$SSE_PROXY" ]]; then
        curl --max-time "$timeout" -sN -x "$SSE_PROXY" "$url" 2>/dev/null
    else
        curl --max-time "$timeout" -sN "$url" 2>/dev/null
    fi
}

sse_fetch_events() {
    local base_url="$1"
    local path="$2"
    local timeout="${3:-$SSE_TIMEOUT}"

    local url="$base_url"
    if [[ -n "$path" ]]; then
        url="${base_url}${path}"
    fi

    if [[ -n "$SSE_PROXY" ]]; then
        curl --max-time "$timeout" -sN -x "$SSE_PROXY" "$url" 2>/dev/null | grep "^data:" | sed 's/^data: //'
    else
        curl --max-time "$timeout" -sN "$url" 2>/dev/null | grep "^data:" | sed 's/^data: //'
    fi
}

sse_count_events() {
    local url="$1"
    local timeout="${2:-$SSE_TIMEOUT}"

    if [[ -n "$SSE_PROXY" ]]; then
        curl --max-time "$timeout" -sN -x "$SSE_PROXY" "$url" 2>/dev/null | grep -c "^data:" | tr -d '[:space:]'
    else
        curl --max-time "$timeout" -sN "$url" 2>/dev/null | grep -c "^data:" | tr -d '[:space:]'
    fi
}

sse_parse_event() {
    local raw_event="$1"
    local field="$2"

    echo "$raw_event" | grep "^${field}:" | head -1 | sed "s/^${field}: //"
}
