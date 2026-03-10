#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

source "${PROJECT_DIR}/e2e-tests/test_utils/assert.sh"

PROXY_PORT="${PROXY_PORT:-18889}"
BIFROST_BIN="${PROJECT_DIR}/target/debug/bifrost"
TEST_DATA_DIR=""
TEST_HOME=""
PROXY_PID=""

cleanup() {
    if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
        kill "$PROXY_PID" 2>/dev/null || true
        wait "$PROXY_PID" 2>/dev/null || true
    fi
    if [[ -n "$TEST_DATA_DIR" ]] && [[ -d "$TEST_DATA_DIR" ]]; then
        rm -rf "$TEST_DATA_DIR"
    fi
    if [[ -n "$TEST_HOME" ]] && [[ -d "$TEST_HOME" ]]; then
        rm -rf "$TEST_HOME"
    fi
}
trap cleanup EXIT

build_bifrost() {
    if [[ -f "$BIFROST_BIN" ]] && [[ "${SKIP_BUILD:-false}" == "true" ]]; then
        return 0
    fi
    (cd "$PROJECT_DIR" && cargo build --bin bifrost) || return 1
}

start_proxy() {
    export BIFROST_DATA_DIR="${TEST_DATA_DIR}"
    "$BIFROST_BIN" -p "${PROXY_PORT}" start \
        --skip-cert-check --unsafe-ssl \
        --cli-proxy \
        --cli-proxy-no-proxy "localhost,127.0.0.1,::1,*.local" \
        > "${TEST_DATA_DIR}/proxy.log" 2>&1 &
    PROXY_PID=$!
    sleep 2
    if ! kill -0 "$PROXY_PID" 2>/dev/null; then
        _log_fail "proxy started" "running process" "not running"
        cat "${TEST_DATA_DIR}/proxy.log" || true
        return 1
    fi
}

stop_proxy() {
    export BIFROST_DATA_DIR="${TEST_DATA_DIR}"
    "$BIFROST_BIN" stop >/dev/null 2>&1 || true
    sleep 1
}

assert_marker_present() {
    local file="$1"
    local waited=0
    while [[ ! -f "$file" && "$waited" -lt 10 ]]; do
        sleep 0.2
        waited=$((waited + 1))
    done
    if [[ ! -f "$file" ]]; then
        _log_fail "marker file exists" "$file" "(not found)"
        cat "${TEST_DATA_DIR}/proxy.log" || true
        return 1
    fi
    local content
    content="$(cat "$file")"
    assert_body_contains "# >>> Bifrost proxy start >>>" "$content" "marker begin exists"
    assert_body_contains "# <<< Bifrost proxy end <<<" "$content" "marker end exists"
}

assert_marker_removed() {
    local file="$1"
    if [[ ! -f "$file" ]]; then
        _log_pass "marker removed (file deleted)"
        return 0
    fi
    local content
    content="$(cat "$file")"
    if [[ "$content" == *"# >>> Bifrost proxy start >>>"* ]] || [[ "$content" == *"# <<< Bifrost proxy end <<<"* ]]; then
        _log_fail "marker removed" "markers absent" "markers still present"
    else
        _log_pass "marker removed"
    fi
}

main() {
    build_bifrost || { echo "编译失败"; exit 1; }

    TEST_DATA_DIR="$(mktemp -d)"
    TEST_HOME="$(mktemp -d)"
    export HOME="$TEST_HOME"
    export SHELL="/bin/zsh"

    start_proxy
    if [[ "${PROXY_PID}" == "" ]]; then
        print_test_summary || exit 1
        return 1
    fi

    assert_marker_present "${TEST_HOME}/.zshrc"
    assert_marker_present "${TEST_HOME}/.zprofile"

    stop_proxy

    assert_marker_removed "${TEST_HOME}/.zshrc"
    assert_marker_removed "${TEST_HOME}/.zprofile"

    print_test_summary || exit 1
}

SKIP_BUILD="${SKIP_BUILD:-false}"
main "$@"
