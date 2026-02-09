#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULES_DIR="${SCRIPT_DIR}/rules"
TEST_SCRIPT="${SCRIPT_DIR}/test_rules.sh"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIP=0
FAILED_RULES=()
PASSED_RULES=()
SKIPPED_RULES=()

header() { 
    echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
}

usage() {
    echo "用法: $0 [选项] [规则名称...]"
    echo ""
    echo "批量执行 Bifrost 规则端到端测试"
    echo ""
    echo "选项:"
    echo "  -h, --help         显示帮助信息"
    echo "  -l, --list         列出所有可用的规则文件"
    echo "  -p, --port PORT    指定代理端口 (默认: 8080)"
    echo "  -c, --continue     测试失败时继续执行后续测试"
    echo ""
    echo "示例:"
    echo "  $0                           # 运行所有规则测试"
    echo "  $0 host redirect             # 只运行 host 和 redirect 测试"
    echo "  $0 -c                        # 运行所有测试，失败时继续"
    echo "  $0 --list                    # 列出所有可用规则"
    exit 0
}

list_rules() {
    header "可用的规则文件"
    if [[ -d "$RULES_DIR" ]]; then
        for rule_file in "$RULES_DIR"/*.txt; do
            if [[ -f "$rule_file" ]]; then
                local name=$(basename "$rule_file" .txt)
                local desc=$(grep -m1 '^#' "$rule_file" 2>/dev/null | sed 's/^# *//' || echo "无描述")
                printf "  ${CYAN}%-20s${NC} %s\n" "$name" "$desc"
            fi
        done
    else
        echo -e "${YELLOW}⚠ 规则目录不存在: $RULES_DIR${NC}"
    fi
    exit 0
}

PROXY_PORT="${PROXY_PORT:-8080}"
CONTINUE_ON_FAIL=false
RULE_NAMES=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            usage
            ;;
        -l|--list)
            list_rules
            ;;
        -p|--port)
            PROXY_PORT="$2"
            shift 2
            ;;
        -c|--continue)
            CONTINUE_ON_FAIL=true
            shift
            ;;
        *)
            RULE_NAMES+=("$1")
            shift
            ;;
    esac
done

run_single_test() {
    local rule_file="$1"
    local rule_name=$(basename "$rule_file" .txt)
    
    echo ""
    echo -e "${BLUE}════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  测试规则: ${CYAN}${rule_name}${NC}"
    echo -e "${BLUE}════════════════════════════════════════════════════════${NC}"
    echo ""
    
    local output
    local exit_code=0
    
    output=$("$TEST_SCRIPT" -p "$PROXY_PORT" "$rule_file" 2>&1) || exit_code=$?
    
    echo "$output"
    
    local pass_count=$(echo "$output" | grep -c "✓ PASS" 2>/dev/null || true)
    local fail_count=$(echo "$output" | grep -c "✗ FAIL" 2>/dev/null || true)
    local skip_count=$(echo "$output" | grep -c "⚠ WARN" 2>/dev/null || true)
    
    pass_count=${pass_count:-0}
    fail_count=${fail_count:-0}
    skip_count=${skip_count:-0}
    
    TOTAL_PASS=$((TOTAL_PASS + pass_count))
    TOTAL_FAIL=$((TOTAL_FAIL + fail_count))
    TOTAL_SKIP=$((TOTAL_SKIP + skip_count))
    
    if [[ $exit_code -eq 0 ]]; then
        PASSED_RULES+=("$rule_name")
        echo -e "\n${GREEN}✓ 规则 ${rule_name} 测试通过${NC}"
    else
        FAILED_RULES+=("$rule_name")
        echo -e "\n${RED}✗ 规则 ${rule_name} 测试失败${NC}"
        
        if [[ "$CONTINUE_ON_FAIL" != "true" ]]; then
            return 1
        fi
    fi
    
    return 0
}

main() {
    header "Bifrost 规则批量测试"
    echo "代理端口: $PROXY_PORT"
    echo "规则目录: $RULES_DIR"
    echo "继续模式: $CONTINUE_ON_FAIL"
    echo ""
    
    if [[ ! -d "$RULES_DIR" ]]; then
        echo -e "${RED}✗ 规则目录不存在: $RULES_DIR${NC}"
        exit 1
    fi
    
    local rule_files=()
    
    if [[ ${#RULE_NAMES[@]} -gt 0 ]]; then
        for name in "${RULE_NAMES[@]}"; do
            local file="${RULES_DIR}/${name}.txt"
            if [[ -f "$file" ]]; then
                rule_files+=("$file")
            else
                echo -e "${YELLOW}⚠ 规则文件不存在: $file${NC}"
                SKIPPED_RULES+=("$name")
            fi
        done
    else
        for file in "$RULES_DIR"/*.txt; do
            if [[ -f "$file" ]]; then
                rule_files+=("$file")
            fi
        done
    fi
    
    if [[ ${#rule_files[@]} -eq 0 ]]; then
        echo -e "${YELLOW}⚠ 没有找到规则文件${NC}"
        exit 0
    fi
    
    echo "将测试以下规则文件:"
    for file in "${rule_files[@]}"; do
        echo "  - $(basename "$file" .txt)"
    done
    echo ""
    
    local test_failed=false
    for rule_file in "${rule_files[@]}"; do
        if ! run_single_test "$rule_file"; then
            test_failed=true
            if [[ "$CONTINUE_ON_FAIL" != "true" ]]; then
                break
            fi
        fi
    done
    
    header "批量测试结果汇总"
    
    echo -e "${GREEN}总通过数: $TOTAL_PASS${NC}"
    echo -e "${RED}总失败数: $TOTAL_FAIL${NC}"
    echo -e "${YELLOW}总跳过数: $TOTAL_SKIP${NC}"
    echo ""
    
    if [[ ${#PASSED_RULES[@]} -gt 0 ]]; then
        echo -e "${GREEN}通过的规则:${NC}"
        for rule in "${PASSED_RULES[@]}"; do
            echo -e "  ${GREEN}✓${NC} $rule"
        done
        echo ""
    fi
    
    if [[ ${#FAILED_RULES[@]} -gt 0 ]]; then
        echo -e "${RED}失败的规则:${NC}"
        for rule in "${FAILED_RULES[@]}"; do
            echo -e "  ${RED}✗${NC} $rule"
        done
        echo ""
    fi
    
    if [[ ${#SKIPPED_RULES[@]} -gt 0 ]]; then
        echo -e "${YELLOW}跳过的规则:${NC}"
        for rule in "${SKIPPED_RULES[@]}"; do
            echo -e "  ${YELLOW}⚠${NC} $rule"
        done
        echo ""
    fi
    
    local total_rules=$((${#PASSED_RULES[@]} + ${#FAILED_RULES[@]}))
    echo "────────────────────────────────────────────────"
    echo -e "规则测试: ${GREEN}${#PASSED_RULES[@]}${NC}/${total_rules} 通过"
    echo "────────────────────────────────────────────────"
    
    if [[ ${#FAILED_RULES[@]} -eq 0 ]]; then
        echo -e "\n${GREEN}🎉 所有规则测试通过！${NC}"
        exit 0
    else
        echo -e "\n${RED}💥 有 ${#FAILED_RULES[@]} 个规则测试失败${NC}"
        exit 1
    fi
}

main "$@"
