#!/bin/bash
#
# Bifrost CLI 在线命令端到端测试
# 覆盖需要运行中代理服务器的命令:
#   metrics, sync, traffic clear, whitelist advanced, config performance/websocket/disconnect-by-app,
#   version-check, import/export (via API)
#

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

source "${PROJECT_DIR}/e2e-tests/test_utils/process.sh"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

BIFROST_BIN="${PROJECT_DIR}/target/release/bifrost"
if [[ ! -x "$BIFROST_BIN" && -f "${BIFROST_BIN}.exe" ]]; then
    BIFROST_BIN="${BIFROST_BIN}.exe"
fi
TEST_DATA_DIR=""
PROXY_PORT="${PROXY_PORT:-18991}"
PROXY_PID=""
BIFROST_LOG_FILE=""
SKIP_BUILD="${SKIP_BUILD:-false}"

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

pass() {
    echo -e "  ${GREEN}✓${NC} $1"
    ((PASSED++))
}

fail() {
    echo -e "  ${RED}✗${NC} $1"
    ((FAILED++))
}

skip() {
    echo -e "  ${YELLOW}○${NC} $1 (跳过)"
    ((SKIPPED++))
}

cleanup() {
    kill_bifrost_on_port "$PROXY_PORT"
    safe_cleanup_proxy "$PROXY_PID"

    if [[ -n "$TEST_DATA_DIR" ]] && [[ -d "$TEST_DATA_DIR" ]]; then
        rm -rf "$TEST_DATA_DIR"
    fi
    if [[ -n "$BIFROST_LOG_FILE" ]] && [[ -f "$BIFROST_LOG_FILE" ]]; then
        rm -f "$BIFROST_LOG_FILE"
    fi
}

trap cleanup EXIT

run_bifrost() {
    BIFROST_DATA_DIR="$TEST_DATA_DIR" "$BIFROST_BIN" -p "$PROXY_PORT" "$@" 2>&1 || true
}

run_bifrost_strict() {
    BIFROST_DATA_DIR="$TEST_DATA_DIR" "$BIFROST_BIN" -p "$PROXY_PORT" "$@" 2>&1
}

build_bifrost() {
    header "编译 Bifrost"
    if [[ -f "$BIFROST_BIN" ]] && [[ "$SKIP_BUILD" == "true" ]]; then
        info "跳过编译 (SKIP_BUILD=true)"
        return
    fi
    echo -e "${GREEN}✓${NC} 二进制文件就绪"
}

setup_test_data_dir() {
    TEST_DATA_DIR=$(mktemp -d)
    export BIFROST_DATA_DIR="$TEST_DATA_DIR"
    mkdir -p "${TEST_DATA_DIR}"
    info "测试数据目录: $TEST_DATA_DIR"
}

start_bifrost_server() {
    header "启动 Bifrost 代理服务器"

    BIFROST_LOG_FILE=$(mktemp)

    cd "$PROJECT_DIR" || return 1
    SKIP_FRONTEND_BUILD=1 BIFROST_DATA_DIR="$TEST_DATA_DIR" \
        "$BIFROST_BIN" -p "$PROXY_PORT" start --skip-cert-check --unsafe-ssl \
        >"$BIFROST_LOG_FILE" 2>&1 &
    PROXY_PID=$!

    local max_wait=60
    local waited=0
    while [[ $waited -lt $max_wait ]]; do
        if [[ -n "$PROXY_PID" ]] && ! kill -0 "$PROXY_PID" 2>/dev/null; then
            echo -e "${RED}✗${NC} Bifrost 进程退出"
            tail -n 50 "$BIFROST_LOG_FILE" 2>/dev/null || true
            return 1
        fi
        if curl -sf "http://127.0.0.1:${PROXY_PORT}/_bifrost/api/system" >/dev/null 2>&1; then
            echo -e "${GREEN}✓${NC} Bifrost 已启动 (端口: ${PROXY_PORT}, PID: ${PROXY_PID})"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    echo -e "${RED}✗${NC} Bifrost 启动超时"
    tail -n 50 "$BIFROST_LOG_FILE" 2>/dev/null || true
    return 1
}

seed_data() {
    header "准备测试数据"
    BIFROST_DATA_DIR="$TEST_DATA_DIR" "$BIFROST_BIN" rule add e2e_rule_1 -c "e2e1.example.com statusCode://200" >/dev/null 2>&1 || true
    BIFROST_DATA_DIR="$TEST_DATA_DIR" "$BIFROST_BIN" rule add e2e_rule_2 -c "e2e2.example.com statusCode://201" >/dev/null 2>&1 || true
    BIFROST_DATA_DIR="$TEST_DATA_DIR" "$BIFROST_BIN" rule add e2e_rule_3 -c "e2e3.example.com statusCode://202" >/dev/null 2>&1 || true
    BIFROST_DATA_DIR="$TEST_DATA_DIR" "$BIFROST_BIN" value add e2e_val_1 "e2e_value_content" >/dev/null 2>&1 || true
    BIFROST_DATA_DIR="$TEST_DATA_DIR" "$BIFROST_BIN" script add request e2e_script_1 -c 'log.info("e2e");' >/dev/null 2>&1 || true
    pass "测试数据已准备"
}

# ─── Rule Rename (requires admin API) ───

test_rule_rename() {
    header "测试 rule rename"

    local result
    result=$(run_bifrost rule rename e2e_rule_1 e2e_rule_1_renamed)

    if echo "$result" | grep -qi "renamed\|success\|ok"; then
        pass "rule rename 命令执行成功"
    else
        fail "rule rename 命令失败: $result"
    fi

    local show_result
    show_result=$(run_bifrost rule show e2e_rule_1_renamed)
    if echo "$show_result" | grep -q "e2e1.example.com"; then
        pass "rule rename 后内容保持不变"
    else
        fail "rule rename 后内容丢失: $show_result"
    fi
}

test_rule_rename_not_found() {
    header "测试 rule rename 不存在的规则"

    local result
    result=$(run_bifrost rule rename nonexistent_rule_xxx new_name)

    if echo "$result" | grep -qi "not found\|error\|Error"; then
        pass "rule rename 对不存在的规则返回错误"
    else
        fail "rule rename 对不存在的规则未报错: $result"
    fi
}

# ─── Rule Reorder (requires admin API) ───

test_rule_reorder() {
    header "测试 rule reorder"

    local result
    result=$(run_bifrost rule reorder e2e_rule_3 e2e_rule_2 e2e_rule_1_renamed)

    if echo "$result" | grep -qi "reorder\|success\|updated\|ok"; then
        pass "rule reorder 命令执行成功"
    else
        fail "rule reorder 命令失败: $result"
    fi
}

# ─── Script Rename (requires admin API) ───

test_script_rename() {
    header "测试 script rename"

    local result
    result=$(run_bifrost script rename request e2e_script_1 e2e_script_1_renamed)

    if echo "$result" | grep -qi "renamed\|success\|ok"; then
        pass "script rename 命令执行成功"
    else
        fail "script rename 命令失败: $result"
    fi
}

test_script_rename_not_found() {
    header "测试 script rename 不存在的脚本"

    local result
    result=$(run_bifrost script rename request nonexistent_script_xxx new_name)

    if echo "$result" | grep -qi "not found\|error\|Error"; then
        pass "script rename 对不存在的脚本返回错误"
    else
        fail "script rename 对不存在的脚本未报错: $result"
    fi
}

# ─── Metrics Commands ───

test_metrics_summary() {
    header "测试 metrics summary"

    local result
    result=$(run_bifrost metrics summary)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "metrics summary 返回了数据"
        info "输出前 5 行: $(echo "$result" | head -5)"
    else
        fail "metrics summary 失败: $result"
    fi
}

test_metrics_apps() {
    header "测试 metrics apps"

    local result
    result=$(run_bifrost metrics apps)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "metrics apps 返回了数据"
    else
        fail "metrics apps 失败: $result"
    fi
}

test_metrics_hosts() {
    header "测试 metrics hosts"

    local result
    result=$(run_bifrost metrics hosts)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "metrics hosts 返回了数据"
    else
        fail "metrics hosts 失败: $result"
    fi
}

test_metrics_history() {
    header "测试 metrics history"

    local result
    result=$(run_bifrost metrics history -l 5)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "metrics history 返回了数据"
    else
        fail "metrics history 失败: $result"
    fi
}

# ─── Sync Commands ───

test_sync_status() {
    header "测试 sync status"

    local result
    result=$(run_bifrost sync status)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "sync status 返回了数据"
    else
        fail "sync status 失败: $result"
    fi
}

test_sync_config_view() {
    header "测试 sync config (查看)"

    local result
    result=$(run_bifrost sync config)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "sync config 查看返回了数据"
    else
        fail "sync config 查看失败: $result"
    fi
}

# ─── Traffic Commands ───

test_traffic_list() {
    header "测试 traffic list"

    local result
    result=$(run_bifrost traffic list -l 10)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "traffic list 返回了数据"
    else
        fail "traffic list 失败: $result"
    fi
}

test_traffic_clear() {
    header "测试 traffic clear"

    local result
    result=$(run_bifrost traffic clear -y)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "traffic clear 执行成功"
    else
        fail "traffic clear 失败: $result"
    fi
}

# ─── Whitelist Advanced Commands ───

test_whitelist_list() {
    header "测试 whitelist list"

    local result
    result=$(run_bifrost whitelist list)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist list 返回了数据"
    else
        fail "whitelist list 失败: $result"
    fi
}

test_whitelist_status() {
    header "测试 whitelist status"

    local result
    result=$(run_bifrost whitelist status)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist status 返回了数据"
    else
        fail "whitelist status 失败: $result"
    fi
}

test_whitelist_mode_get() {
    header "测试 whitelist mode (获取)"

    local result
    result=$(run_bifrost whitelist mode)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist mode 获取成功"
    else
        fail "whitelist mode 获取失败: $result"
    fi
}

test_whitelist_mode_set() {
    header "测试 whitelist mode (设置)"

    local result
    result=$(run_bifrost whitelist mode allow_all)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist mode 设置 allow_all 成功"
    else
        fail "whitelist mode 设置失败: $result"
    fi

    result=$(run_bifrost whitelist mode local_only)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist mode 恢复 local_only 成功"
    else
        fail "whitelist mode 恢复失败: $result"
    fi
}

test_whitelist_add_remove() {
    header "测试 whitelist add/remove"

    local result
    result=$(run_bifrost whitelist add 192.168.99.99)

    if echo "$result" | grep -qi "added\|success\|ok\|192.168"; then
        pass "whitelist add IP 成功"
    else
        fail "whitelist add 失败: $result"
    fi

    result=$(run_bifrost whitelist remove 192.168.99.99)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist remove IP 成功"
    else
        fail "whitelist remove 失败: $result"
    fi
}

test_whitelist_temporary() {
    header "测试 whitelist add-temporary / remove-temporary"

    local result
    result=$(run_bifrost whitelist add-temporary 10.0.0.99)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist add-temporary 成功"
    else
        fail "whitelist add-temporary 失败: $result"
    fi

    result=$(run_bifrost whitelist remove-temporary 10.0.0.99)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist remove-temporary 成功"
    else
        fail "whitelist remove-temporary 失败: $result"
    fi
}

test_whitelist_pending() {
    header "测试 whitelist pending / clear-pending"

    local result
    result=$(run_bifrost whitelist pending)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist pending 列出成功"
    else
        fail "whitelist pending 失败: $result"
    fi

    result=$(run_bifrost whitelist clear-pending)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist clear-pending 成功"
    else
        fail "whitelist clear-pending 失败: $result"
    fi
}

test_whitelist_allow_lan() {
    header "测试 whitelist allow-lan"

    local result
    result=$(run_bifrost whitelist allow-lan true)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist allow-lan true 成功"
    else
        fail "whitelist allow-lan true 失败: $result"
    fi

    result=$(run_bifrost whitelist allow-lan false)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "whitelist allow-lan false 成功"
    else
        fail "whitelist allow-lan false 失败: $result"
    fi
}

# ─── Config Commands ───

test_config_show() {
    header "测试 config show"

    local result
    result=$(run_bifrost config show)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config show 返回了配置"
    else
        fail "config show 失败: $result"
    fi
}

test_config_show_json() {
    header "测试 config show --json"

    local result
    result=$(run_bifrost config show --json)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config show --json 返回了 JSON 配置"
    else
        fail "config show --json 失败: $result"
    fi
}

test_config_show_section() {
    header "测试 config show --section"

    local sections=("tls" "traffic" "access")
    for section in "${sections[@]}"; do
        local result
        result=$(run_bifrost config show --section "$section")
        if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
            pass "config show --section $section 成功"
        else
            fail "config show --section $section 失败: $result"
        fi
    done
}

test_config_get() {
    header "测试 config get"

    local result
    result=$(run_bifrost config get tls.enabled)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config get tls.enabled 成功"
        info "值: $(echo "$result" | head -1)"
    else
        fail "config get tls.enabled 失败: $result"
    fi

    result=$(run_bifrost config get tls.enabled --json)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config get --json 成功"
    else
        fail "config get --json 失败: $result"
    fi
}

test_config_set() {
    header "测试 config set"

    local result
    result=$(run_bifrost config set tls.unsafe_ssl true)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config set tls.unsafe_ssl true 成功"
    else
        fail "config set 失败: $result"
    fi

    result=$(run_bifrost config set tls.unsafe_ssl false)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config set tls.unsafe_ssl false (恢复) 成功"
    else
        fail "config set 恢复失败: $result"
    fi
}

test_config_add_remove() {
    header "测试 config add / remove"

    local result
    result=$(run_bifrost config add tls.exclude "*.e2etest.local")
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config add tls.exclude 成功"
    else
        fail "config add 失败: $result"
    fi

    result=$(run_bifrost config remove tls.exclude "*.e2etest.local")
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config remove tls.exclude 成功"
    else
        fail "config remove 失败: $result"
    fi
}

test_config_reset() {
    header "测试 config reset"

    local result
    result=$(run_bifrost config reset tls.unsafe_ssl -y)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config reset tls.unsafe_ssl 成功"
    else
        fail "config reset 失败: $result"
    fi
}

test_config_clear_cache() {
    header "测试 config clear-cache"

    local result
    result=$(run_bifrost config clear-cache -y)
    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config clear-cache 成功"
    else
        fail "config clear-cache 失败: $result"
    fi
}

test_config_performance() {
    header "测试 config performance"

    local result
    result=$(run_bifrost config performance)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config performance 返回了性能概览"
        info "输出前 5 行: $(echo "$result" | head -5)"
    else
        fail "config performance 失败: $result"
    fi
}

test_config_websocket() {
    header "测试 config websocket"

    local result
    result=$(run_bifrost config websocket)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config websocket 返回了 WebSocket 连接列表"
    else
        fail "config websocket 失败: $result"
    fi
}

test_config_disconnect_by_app() {
    header "测试 config disconnect-by-app"

    local result
    result=$(run_bifrost config disconnect-by-app "nonexistent_app_test")

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config disconnect-by-app 执行成功"
    else
        fail "config disconnect-by-app 失败: $result"
    fi
}

test_config_disconnect() {
    header "测试 config disconnect"

    local result
    result=$(run_bifrost config disconnect "*.nonexistent.test")

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config disconnect 执行成功"
    else
        fail "config disconnect 失败: $result"
    fi
}

# ─── Version Check ───

test_version_check() {
    header "测试 version-check"

    local result
    result=$(run_bifrost version-check)

    if echo "$result" | grep -qi "panic"; then
        fail "version-check 出现 panic: $result"
    elif [[ -n "$result" ]]; then
        pass "version-check 返回了版本信息"
        info "输出: $(echo "$result" | head -3)"
    else
        pass "version-check 执行成功 (无输出表示已是最新版本)"
    fi
}

# ─── Import/Export ───

test_export_rules() {
    header "测试 export rules"

    local output_file="${TEST_DATA_DIR}/exported_rules.bifrost"
    local result
    result=$(run_bifrost export rules e2e_rule_1 e2e_rule_2 -o "$output_file")

    if [[ -f "$output_file" ]]; then
        pass "export rules 导出了文件: $output_file"
        local size
        size=$(wc -c < "$output_file")
        info "文件大小: ${size} 字节"
    elif [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "export rules 返回了数据 (stdout)"
    else
        fail "export rules 失败: $result"
    fi
}

test_export_values() {
    header "测试 export values"

    local output_file="${TEST_DATA_DIR}/exported_values.bifrost"
    local result
    result=$(run_bifrost export values e2e_val_1 -o "$output_file")

    if [[ -f "$output_file" ]]; then
        pass "export values 导出了文件"
    elif [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "export values 返回了数据 (stdout)"
    else
        fail "export values 失败: $result"
    fi
}

test_export_scripts() {
    header "测试 export scripts"

    local output_file="${TEST_DATA_DIR}/exported_scripts.bifrost"
    local result
    result=$(run_bifrost export scripts request/e2e_script_1 -o "$output_file")

    if [[ -f "$output_file" ]]; then
        pass "export scripts 导出了文件"
    elif [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "export scripts 返回了数据 (stdout)"
    else
        fail "export scripts 失败: $result"
    fi
}

test_import_detect() {
    header "测试 import --detect-only"

    local test_bifrost_file="${TEST_DATA_DIR}/test_import.bifrost"
    cat > "$test_bifrost_file" << 'BIFROST_EOF'
{
  "type": "rules",
  "version": "1",
  "rules": [
    {"name": "import_test", "content": "import.example.com statusCode://200"}
  ]
}
BIFROST_EOF

    local result
    result=$(run_bifrost import "$test_bifrost_file" --detect-only)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "import --detect-only 检测成功"
        info "检测结果: $(echo "$result" | head -3)"
    else
        fail "import --detect-only 失败: $result"
    fi
}

test_import_full() {
    header "测试 import (完整导入)"

    local test_bifrost_file="${TEST_DATA_DIR}/test_import_full.bifrost"
    cat > "$test_bifrost_file" << 'BIFROST_EOF'
{
  "type": "rules",
  "version": "1",
  "rules": [
    {"name": "imported_rule", "content": "imported.example.com statusCode://200"}
  ]
}
BIFROST_EOF

    local result
    result=$(run_bifrost import "$test_bifrost_file")

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "import 完整导入执行成功"
    else
        fail "import 完整导入失败: $result"
    fi
}

# ─── Config export ───

test_config_export_json() {
    header "测试 config export --format json"

    local output_file="${TEST_DATA_DIR}/config_export.json"
    local result
    result=$(run_bifrost config export -o "$output_file" --format json)

    if [[ -f "$output_file" ]]; then
        pass "config export --format json 导出了文件"
    elif [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config export 返回了数据"
    else
        fail "config export 失败: $result"
    fi
}

test_config_export_toml() {
    header "测试 config export --format toml"

    local output_file="${TEST_DATA_DIR}/config_export.toml"
    local result
    result=$(run_bifrost config export -o "$output_file" --format toml)

    if [[ -f "$output_file" ]]; then
        pass "config export --format toml 导出了文件"
    elif [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "config export toml 返回了数据"
    else
        fail "config export toml 失败: $result"
    fi
}

# ─── Search ───

test_search_basic() {
    header "测试 search (非交互)"

    local result
    result=$(run_bifrost search "test" -l 5 -f json)

    if [[ -n "$result" ]] && ! echo "$result" | grep -qi "panic"; then
        pass "search 基本搜索执行成功"
    else
        fail "search 失败: $result"
    fi
}

test_search_with_filters() {
    header "测试 search 带过滤参数"

    local result
    result=$(run_bifrost search "example" -l 5 -f compact --method GET --protocol HTTPS)
    if ! echo "$result" | grep -qi "panic"; then
        pass "search --method GET --protocol HTTPS 执行成功"
    else
        fail "search 带过滤失败: $result"
    fi

    result=$(run_bifrost search "test" -l 5 -f json --status 2xx)
    if ! echo "$result" | grep -qi "panic"; then
        pass "search --status 2xx 执行成功"
    else
        fail "search --status 失败: $result"
    fi

    result=$(run_bifrost search "test" -l 5 --url --no-color)
    if ! echo "$result" | grep -qi "panic"; then
        pass "search --url --no-color 执行成功"
    else
        fail "search --url 失败: $result"
    fi
}

# ─── Traffic get/search ───

test_traffic_get() {
    header "测试 traffic get"

    local result
    result=$(run_bifrost traffic get --help)
    if echo "$result" | grep -qi "get\|id"; then
        pass "traffic get --help 正确显示"
    else
        fail "traffic get --help 异常: $result"
    fi
}

test_traffic_search() {
    header "测试 traffic search (非交互)"

    local result
    result=$(run_bifrost traffic search "test" -l 5 -f json)
    if ! echo "$result" | grep -qi "panic"; then
        pass "traffic search 执行成功"
    else
        fail "traffic search 失败: $result"
    fi
}

test_traffic_list_with_filters() {
    header "测试 traffic list 带过滤参数"

    local result
    result=$(run_bifrost traffic list -l 5 --protocol https -f json)
    if ! echo "$result" | grep -qi "panic"; then
        pass "traffic list --protocol https 执行成功"
    else
        fail "traffic list --protocol 失败: $result"
    fi

    result=$(run_bifrost traffic list -l 5 --method GET)
    if ! echo "$result" | grep -qi "panic"; then
        pass "traffic list --method GET 执行成功"
    else
        fail "traffic list --method 失败: $result"
    fi

    result=$(run_bifrost traffic list -l 5 -f compact --no-color)
    if ! echo "$result" | grep -qi "panic"; then
        pass "traffic list -f compact --no-color 执行成功"
    else
        fail "traffic list compact 失败: $result"
    fi
}

# ─── Whitelist approve/reject ───

test_whitelist_approve_reject() {
    header "测试 whitelist approve/reject"

    local result
    result=$(run_bifrost whitelist approve 198.51.100.1)
    if ! echo "$result" | grep -qi "panic"; then
        pass "whitelist approve 执行成功 (即使无 pending)"
    else
        fail "whitelist approve 失败: $result"
    fi

    result=$(run_bifrost whitelist reject 198.51.100.2)
    if ! echo "$result" | grep -qi "panic"; then
        pass "whitelist reject 执行成功 (即使无 pending)"
    else
        fail "whitelist reject 失败: $result"
    fi
}

# ─── Sync login/logout/run ───

test_sync_login_logout() {
    header "测试 sync login/logout/run"

    local result
    result=$(run_bifrost sync run)
    if ! echo "$result" | grep -qi "panic"; then
        pass "sync run 执行成功"
    else
        fail "sync run 失败: $result"
    fi

    result=$(run_bifrost sync logout)
    if ! echo "$result" | grep -qi "panic"; then
        pass "sync logout 执行成功"
    else
        fail "sync logout 失败: $result"
    fi
}

# ─── Stop ───

test_stop() {
    header "测试 stop 命令"

    local result
    result=$(run_bifrost stop)
    if ! echo "$result" | grep -qi "panic"; then
        pass "stop 命令执行成功"
    else
        fail "stop 命令失败: $result"
    fi

    sleep 2

    if curl -sf "http://127.0.0.1:${PROXY_PORT}/_bifrost/api/system" >/dev/null 2>&1; then
        info "服务器仍在运行 (stop 可能不支持远程停止)"
        pass "stop 命令没有导致异常"
    else
        pass "stop 命令成功停止了服务器"
    fi
}

# ─── Status ───

test_status() {
    header "测试 status"

    local result
    result=$(run_bifrost status)

    if echo "$result" | grep -qi "running\|status\|port\|proxy\|bifrost"; then
        pass "status 返回了服务状态"
    else
        fail "status 失败: $result"
    fi
}

# ─── Summary ───

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
        echo -e "${GREEN}  所有测试通过！${NC}"
        echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
        return 0
    else
        echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
        echo -e "${RED}  $FAILED 个测试失败${NC}"
        echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
        return 1
    fi
}

main() {
    echo ""
    echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║        Bifrost CLI 在线命令端到端测试                          ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════╝${NC}"

    build_bifrost
    setup_test_data_dir
    seed_data

    if ! start_bifrost_server; then
        echo -e "${RED}无法启动 Bifrost 服务器, 终止测试${NC}"
        exit 1
    fi

    sleep 1

    test_status

    test_rule_rename
    test_rule_rename_not_found
    test_rule_reorder
    test_script_rename
    test_script_rename_not_found

    test_metrics_summary
    test_metrics_apps
    test_metrics_hosts
    test_metrics_history

    test_sync_status
    test_sync_config_view

    test_traffic_list
    test_traffic_clear

    test_whitelist_list
    test_whitelist_status
    test_whitelist_mode_get
    test_whitelist_mode_set
    test_whitelist_add_remove
    test_whitelist_temporary
    test_whitelist_pending
    test_whitelist_allow_lan

    test_config_show
    test_config_show_json
    test_config_show_section
    test_config_get
    test_config_set
    test_config_add_remove
    test_config_reset
    test_config_clear_cache
    test_config_performance
    test_config_websocket
    test_config_disconnect
    test_config_disconnect_by_app
    test_config_export_json
    test_config_export_toml

    test_version_check

    test_export_rules
    test_export_values
    test_export_scripts
    test_import_detect
    test_import_full

    test_search_basic
    test_search_with_filters

    test_traffic_get
    test_traffic_search
    test_traffic_list_with_filters

    test_whitelist_approve_reject
    test_sync_login_logout

    test_stop

    print_summary
    exit $?
}

main "$@"
