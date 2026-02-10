#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULES_DIR="${SCRIPT_DIR}/rules"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

header() { echo -e "\n${CYAN}══════════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}  $1${NC}"; echo -e "${CYAN}══════════════════════════════════════════════════════════════${NC}\n"; }
info() { echo -e "${BLUE}ℹ${NC} $1"; }
warn() { echo -e "${YELLOW}⚠${NC} $1"; }

TOTAL_PASSED=0
TOTAL_FAILED=0
TOTAL_SKIPPED=0
FAILED_SUITES=()

usage() {
    echo "用法: $0 [选项] [测试目录或文件...]"
    echo ""
    echo "选项:"
    echo "  -h, --help         显示帮助信息"
    echo "  -p, --port PORT    指定代理端口 (默认: 8080)"
    echo "  -l, --list         列出所有可用的测试文件"
    echo "  -c, --category CAT 只运行指定分类的测试"
    echo "                     可选: forwarding, request_modify, response_modify,"
    echo "                           redirect, priority, control"
    echo "  --no-build         跳过编译步骤"
    echo "  --fail-fast        首次失败后停止"
    echo "  -v, --verbose      详细输出"
    echo ""
    echo "示例:"
    echo "  $0                                   # 运行所有测试"
    echo "  $0 -c forwarding                     # 只运行转发测试"
    echo "  $0 rules/forwarding/http_to_http.txt # 运行指定测试文件"
    echo "  $0 --list                            # 列出所有测试"
    exit 0
}

list_tests() {
    header "可用的测试文件"
    
    local categories=$(find "$RULES_DIR" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort)
    
    for category_dir in $categories; do
        local category=$(basename "$category_dir")
        echo -e "${CYAN}[$category]${NC}"
        
        find "$category_dir" -name "*.txt" -type f 2>/dev/null | sort | while read -r rule_file; do
            local filename=$(basename "$rule_file")
            local rule_count=$(grep -v '^#' "$rule_file" | grep -v '^[[:space:]]*$' | wc -l | tr -d ' ')
            local desc=$(grep -m1 '^#' "$rule_file" 2>/dev/null | sed 's/^# *//' || echo "")
            printf "  %-30s (%d rules) %s\n" "$filename" "$rule_count" "$desc"
        done
        echo ""
    done
    
    if [[ -n "$(find "$RULES_DIR" -maxdepth 1 -name "*.txt" -type f 2>/dev/null)" ]]; then
        echo -e "${CYAN}[root]${NC}"
        find "$RULES_DIR" -maxdepth 1 -name "*.txt" -type f 2>/dev/null | sort | while read -r rule_file; do
            local filename=$(basename "$rule_file")
            local rule_count=$(grep -v '^#' "$rule_file" | grep -v '^[[:space:]]*$' | wc -l | tr -d ' ')
            printf "  %-30s (%d rules)\n" "$filename" "$rule_count"
        done
    fi
    
    exit 0
}

collect_test_files() {
    local category="$1"
    local test_files=()
    
    if [[ -n "$category" ]]; then
        if [[ -d "${RULES_DIR}/${category}" ]]; then
            while IFS= read -r -d '' file; do
                test_files+=("$file")
            done < <(find "${RULES_DIR}/${category}" -name "*.txt" -type f -print0 2>/dev/null | sort -z)
        else
            echo -e "${RED}✗${NC} 分类不存在: $category"
            exit 1
        fi
    else
        while IFS= read -r -d '' file; do
            test_files+=("$file")
        done < <(find "$RULES_DIR" -name "*.txt" -type f -print0 2>/dev/null | sort -z)
    fi
    
    printf '%s\n' "${test_files[@]}"
}

run_single_test() {
    local rule_file="$1"
    local rel_path="${rule_file#$RULES_DIR/}"
    
    echo ""
    echo -e "${YELLOW}┌────────────────────────────────────────────────────────────────${NC}"
    echo -e "${YELLOW}│ 测试套件: $rel_path${NC}"
    echo -e "${YELLOW}└────────────────────────────────────────────────────────────────${NC}"
    
    local output_file=$(mktemp)
    local exit_code=0
    
    if [[ "$VERBOSE" == "true" ]]; then
        "$SCRIPT_DIR/test_rules.sh" $BUILD_FLAG -p "$PROXY_PORT" "$rule_file" 2>&1 | tee "$output_file" || exit_code=$?
    else
        "$SCRIPT_DIR/test_rules.sh" $BUILD_FLAG -p "$PROXY_PORT" "$rule_file" > "$output_file" 2>&1 || exit_code=$?
        
        if [[ $exit_code -ne 0 ]]; then
            echo -e "${RED}测试失败，输出如下:${NC}"
            cat "$output_file"
        else
            local summary=$(grep -A5 "Test Summary" "$output_file" | tail -4)
            echo "$summary"
        fi
    fi
    
    local passed=$(grep "Passed:" "$output_file" | grep -o '[0-9]*' | head -1 || echo "0")
    local failed=$(grep "Failed:" "$output_file" | grep -o '[0-9]*' | head -1 || echo "0")
    
    TOTAL_PASSED=$((TOTAL_PASSED + passed))
    TOTAL_FAILED=$((TOTAL_FAILED + failed))
    
    rm -f "$output_file"
    
    if [[ $exit_code -ne 0 ]]; then
        FAILED_SUITES+=("$rel_path")
        echo -e "${RED}✗${NC} 测试套件失败: $rel_path"
        
        if [[ "$FAIL_FAST" == "true" ]]; then
            echo -e "${RED}首次失败，停止测试${NC}"
            return 1
        fi
    else
        echo -e "${GREEN}✓${NC} 测试套件通过: $rel_path"
    fi
    
    return 0
}

build_proxy_once() {
    header "编译代理服务器"
    
    if [[ "$SKIP_BUILD" == "true" ]]; then
        info "跳过编译步骤"
        BUILD_FLAG="--no-build"
        return 0
    fi
    
    if [[ -f "${PROJECT_DIR}/target/release/bifrost" ]]; then
        local mod_time=$(stat -f %m "${PROJECT_DIR}/target/release/bifrost" 2>/dev/null || stat -c %Y "${PROJECT_DIR}/target/release/bifrost" 2>/dev/null)
        local now=$(date +%s)
        local age=$((now - mod_time))
        
        if [[ $age -lt 86400 ]]; then
            echo -e "${GREEN}✓${NC} 使用已编译的代理 (编译于 $((age / 60)) 分钟前)"
            BUILD_FLAG="--no-build"
            return 0
        fi
    fi

    info "正在编译代理服务器..."
    cd "$PROJECT_DIR"
    cargo build --release --bin bifrost 2>&1 | tail -5
    echo -e "${GREEN}✓${NC} 代理服务器编译完成"
    BUILD_FLAG="--no-build"
}

start_echo_servers_once() {
    header "启动 Echo 服务器"
    
    "$SCRIPT_DIR/mock_servers/start_servers.sh" stop 2>/dev/null || true
    sleep 1
    "$SCRIPT_DIR/mock_servers/start_servers.sh" start-bg
    sleep 2
    
    if curl -s "http://127.0.0.1:3000/health" >/dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} Echo 服务器已启动"
    else
        echo -e "${RED}✗${NC} Echo 服务器启动失败"
        exit 1
    fi
}

print_final_summary() {
    header "最终测试结果"
    
    echo -e "总断言数: $((TOTAL_PASSED + TOTAL_FAILED))"
    echo -e "通过: ${GREEN}${TOTAL_PASSED}${NC}"
    echo -e "失败: ${RED}${TOTAL_FAILED}${NC}"
    echo ""
    
    if [[ ${#FAILED_SUITES[@]} -gt 0 ]]; then
        echo -e "${RED}失败的测试套件:${NC}"
        for suite in "${FAILED_SUITES[@]}"; do
            echo "  - $suite"
        done
        echo ""
    fi
    
    if [[ $TOTAL_FAILED -eq 0 ]]; then
        echo -e "${GREEN}═══════════════════════════════════════${NC}"
        echo -e "${GREEN}  ✓ 所有测试通过！${NC}"
        echo -e "${GREEN}═══════════════════════════════════════${NC}"
        return 0
    else
        echo -e "${RED}═══════════════════════════════════════${NC}"
        echo -e "${RED}  ✗ 有 ${#FAILED_SUITES[@]} 个测试套件失败${NC}"
        echo -e "${RED}═══════════════════════════════════════${NC}"
        return 1
    fi
}

cleanup() {
    "$SCRIPT_DIR/mock_servers/start_servers.sh" stop 2>/dev/null || true
}

trap cleanup EXIT

PROXY_PORT="${PROXY_PORT:-8080}"
CATEGORY=""
SKIP_BUILD="false"
FAIL_FAST="false"
VERBOSE="false"
BUILD_FLAG=""
SPECIFIC_FILES=()

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                usage
                ;;
            -p|--port)
                PROXY_PORT="$2"
                shift 2
                ;;
            -l|--list)
                list_tests
                ;;
            -c|--category)
                CATEGORY="$2"
                shift 2
                ;;
            --no-build)
                SKIP_BUILD="true"
                shift
                ;;
            --fail-fast)
                FAIL_FAST="true"
                shift
                ;;
            -v|--verbose)
                VERBOSE="true"
                shift
                ;;
            *)
                if [[ -f "$1" ]]; then
                    SPECIFIC_FILES+=("$1")
                elif [[ -f "${SCRIPT_DIR}/$1" ]]; then
                    SPECIFIC_FILES+=("${SCRIPT_DIR}/$1")
                elif [[ -f "${RULES_DIR}/$1" ]]; then
                    SPECIFIC_FILES+=("${RULES_DIR}/$1")
                else
                    warn "未知参数或文件不存在: $1"
                fi
                shift
                ;;
        esac
    done
}

main() {
    parse_args "$@"
    
    header "Bifrost 端到端测试运行器"
    echo "代理端口: $PROXY_PORT"
    if [[ -n "$CATEGORY" ]]; then
        echo "测试分类: $CATEGORY"
    fi
    echo ""
    
    build_proxy_once
    start_echo_servers_once
    
    local test_files=()
    
    if [[ ${#SPECIFIC_FILES[@]} -gt 0 ]]; then
        test_files=("${SPECIFIC_FILES[@]}")
    else
        while IFS= read -r file; do
            [[ -n "$file" ]] && test_files+=("$file")
        done < <(collect_test_files "$CATEGORY")
    fi
    
    local total_suites=${#test_files[@]}
    
    if [[ $total_suites -eq 0 ]]; then
        warn "没有找到测试文件"
        exit 0
    fi
    
    info "找到 $total_suites 个测试套件"
    
    local current=0
    for test_file in "${test_files[@]}"; do
        current=$((current + 1))
        echo ""
        echo -e "${BLUE}[$current/$total_suites]${NC}"
        
        if ! run_single_test "$test_file"; then
            if [[ "$FAIL_FAST" == "true" ]]; then
                break
            fi
        fi
    done
    
    print_final_summary
}

main "$@"
