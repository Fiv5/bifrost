---
name: "e2e-test"
description: "创建和执行 Bifrost 代理的端到端测试；在添加新功能或修复 bug 后用于验证。"
---

# E2E 测试创建与执行

该技能用于创建和执行 Bifrost 代理的端到端测试，确保代理功能正确。

## 何时调用
- 添加新功能后需要验证
- 修复 bug 后需要回归测试
- 需要创建新的测试用例
- 需要运行现有的 E2E 测试

## 测试架构

E2E 测试采用三层架构：

```
Client (curl) → Proxy (bifrost) → Mock Server (echo)
```

- **Client**: curl 发起请求
- **Proxy**: 被测试的 bifrost 代理
- **Mock Server**: Echo 服务器返回请求详情用于验证

## 目录结构

```
rust/scripts/
├── rules/                      # 测试规则文件 (按功能分类)
│   ├── forwarding/             # 转发类测试
│   ├── request_modify/         # 请求修改测试
│   ├── response_modify/        # 响应修改测试
│   ├── redirect/               # 重定向测试
│   ├── priority/               # 优先级测试
│   ├── control/                # 控制类测试
│   └── template/               # 模板类测试 (values, 变量替换)
├── values/                     # 预定义值文件
├── mock_servers/               # Mock 服务器实现
├── test_utils/                 # 测试工具库
│   ├── assert.sh               # 断言函数库
│   └── http_client.sh          # HTTP 请求封装
├── test_rules.sh               # 核心规则测试 (依赖共享 mock 服务器)
├── test_values_e2e.sh          # Values E2E 测试 (独立，自管理服务)
├── test_values_cli.sh          # Values CLI 测试 (独立，无服务依赖)
├── test_pattern.sh             # Pattern 匹配测试 (独立，自管理服务)
└── run_all_tests_parallel.sh   # 并行测试运行器
```

## 执行测试

### 测试脚本分类

| 脚本 | 类型 | 端口 | 说明 |
|------|------|------|------|
| `test_rules.sh` | 依赖共享服务 | 8080/9000+ | 核心规则测试，需要外部 mock 服务器 |
| `test_values_e2e.sh` | 独立 | 18080/13000 | Values E2E 测试，自管理服务 |
| `test_values_cli.sh` | 独立 | 无 | Values CLI 测试，纯命令行 |
| `test_pattern.sh` | 独立 | 18080/3000 | Pattern 匹配测试，自管理服务 |

### 运行全量测试（推荐）

⚠️ **重要**：`run_all_tests_parallel.sh` 只覆盖 `test_rules.sh`（规则测试），不包含独立测试脚本。
要确保测试覆盖完整，必须同时运行所有测试脚本：

```bash
cd rust/scripts

# 方式 1：顺序执行全量测试（简单可靠）
./run_all_tests_parallel.sh && \
./test_values_cli.sh && \
./test_values_e2e.sh && \
./test_pattern.sh

# 方式 2：并行执行全量测试（最高效，需要端口隔离）
./run_all_tests_parallel.sh &                              # 代理: 9000+, Mock: 3000/3443
./test_values_cli.sh &                                     # 无服务依赖
./test_values_e2e.sh &                                     # 代理: 18080, Mock: 13000
PROXY_PORT=18090 ECHO_HTTP_PORT=13100 ECHO_HTTPS_PORT=13443 ./test_pattern.sh &  # 独立端口
wait
echo "全量测试完成"
```

**端口分配说明**（并行执行时避免冲突）：

| 脚本 | 代理端口 | Mock HTTP | Mock HTTPS |
|------|---------|----------|------------|
| `run_all_tests_parallel.sh` | 9000+ | 3000 | 3443 |
| `test_values_e2e.sh` | 18080 | 13000 | - |
| `test_pattern.sh` | 18090 | 13100 | 13443 |

### 单独运行规则测试

`run_all_tests_parallel.sh` 并行执行 `rules/` 目录下的所有规则文件：

```bash
cd rust/scripts
./run_all_tests_parallel.sh
```

特性：
- 自动检测 CPU 核心数，并行运行测试
- 为每个测试分配独立端口 (BASE_PORT=9000 递增)
- 共享一组 mock 服务器，减少资源开销
- 使用独立的 `BIFROST_DATA_DIR` 确保测试隔离

### 单独运行独立测试脚本

独立测试脚本自管理服务生命周期，可单独运行：

```bash
cd rust/scripts

./test_values_cli.sh      # 纯 CLI 测试，无服务依赖
./test_values_e2e.sh      # 自管理服务，端口 18080/13000
./test_pattern.sh         # 自管理服务，端口 18080/3000
```

### 可选参数

```bash
./run_all_tests_parallel.sh --verbose      # 详细输出
./run_all_tests_parallel.sh --fail-fast    # 遇到失败立即停止
./test_rules.sh --skip-mock-servers        # 跳过 mock 服务器启动（用于并行执行）
```

## 创建新测试

### 步骤 1: 确定测试分类

根据功能选择合适的分类目录：

| 分类 | 目录 | 用途 |
|------|------|------|
| 转发 | `rules/forwarding/` | 基础请求转发、host 映射 |
| 请求修改 | `rules/request_modify/` | 修改请求头、请求体、URL |
| 响应修改 | `rules/response_modify/` | 修改响应头、响应体、状态码 |
| 重定向 | `rules/redirect/` | 301/302 重定向测试 |
| 优先级 | `rules/priority/` | 规则优先级和匹配顺序 |
| 控制 | `rules/control/` | 延迟、限速、mock 等控制功能 |
| 模板 | `rules/template/` | 变量替换、Values 引用 |

### 步骤 2: 创建规则文件

在对应分类目录下创建 `.txt` 规则文件：

```
# rules/your_category/your_test.txt
# 测试描述

# 测试用例标识 - 功能描述
pattern.test.com protocol://target
```

### 步骤 3: 创建测试脚本

使用以下模板创建测试脚本：

```bash
#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

source "$SCRIPT_DIR/test_utils/assert.sh"
source "$SCRIPT_DIR/test_utils/http_client.sh"

PROXY_PORT="${PROXY_PORT:-18888}"
ECHO_HTTP_PORT="${ECHO_HTTP_PORT:-18081}"
PROXY="http://127.0.0.1:${PROXY_PORT}"

passed=0
failed=0

test_your_feature() {
    local pattern="your-feature.test.com"
    local test_url="https://${pattern}/api/test"
    
    https_request "$test_url"
    
    if assert_status_2xx "$HTTP_STATUS" "Your feature test"; then
        _log_pass "Your feature test passed"
        ((passed++))
    else
        _log_fail "Your feature test failed"
        ((failed++))
    fi
}

main() {
    echo "=========================================="
    echo "  Your Feature E2E Tests"
    echo "=========================================="
    
    test_your_feature
    
    echo ""
    echo "=========================================="
    echo "Results: $passed passed, $failed failed"
    echo "=========================================="
    
    [ $failed -eq 0 ]
}

main "$@"
```

### 步骤 4: 添加测试到现有脚本（可选）

如果测试属于现有功能扩展，可在对应的 `test_*.sh` 中添加测试函数：

```bash
test_new_feature() {
    local pattern="new-feature.test.com"
    local test_url="https://${pattern}/test"
    
    https_request "$test_url"
    
    if assert_status_2xx "$HTTP_STATUS" "New feature"; then
        _log_pass "New feature test passed"
        ((passed++))
    else
        _log_fail "New feature test failed"
        ((failed++))
    fi
}
```

## 断言库参考

### 状态码断言

```bash
assert_status "$HTTP_STATUS" "200" "描述"
assert_status_2xx "$HTTP_STATUS" "描述"    # 200-299
assert_status_3xx "$HTTP_STATUS" "描述"    # 300-399
assert_status_4xx "$HTTP_STATUS" "描述"    # 400-499
assert_status_5xx "$HTTP_STATUS" "描述"    # 500-599
```

### 响应头断言

```bash
assert_header "$RESPONSE_HEADERS" "Content-Type" "application/json" "描述"
assert_header_exists "$RESPONSE_HEADERS" "X-Custom-Header" "描述"
assert_header_not_exists "$RESPONSE_HEADERS" "X-Removed-Header" "描述"
assert_header_contains "$RESPONSE_HEADERS" "X-Header" "partial-value" "描述"
```

### 响应体断言

```bash
assert_body_contains "$RESPONSE_BODY" "expected content" "描述"
assert_body_not_contains "$RESPONSE_BODY" "unexpected content" "描述"
assert_body_equals "$RESPONSE_BODY" "exact content" "描述"
assert_body_matches "$RESPONSE_BODY" "regex.*pattern" "描述"
```

### JSON 断言

```bash
assert_json_field "$RESPONSE_BODY" ".field.path" "expected_value" "描述"
assert_json_field_exists "$RESPONSE_BODY" ".field.path" "描述"
assert_json_field_not_exists "$RESPONSE_BODY" ".field.path" "描述"
assert_json_field_contains "$RESPONSE_BODY" ".field.path" "partial" "描述"
```

## HTTP 请求函数

### 基础请求

```bash
# HTTP 请求（通过代理）
http_request "$url"
http_request "$url" "POST" "$body"
http_request "$url" "PUT" "$body" "Content-Type: application/json"

# HTTPS 请求（通过代理，跳过证书验证）
https_request "$url"
https_request "$url" "POST" "$body"
https_request "$url" "DELETE"
```

### 请求后可用变量

```bash
$HTTP_STATUS      # HTTP 状态码 (如 200, 404, 500)
$RESPONSE_HEADERS # 响应头 (完整文本)
$RESPONSE_BODY    # 响应体 (完整内容)
```

### 自定义请求头

```bash
http_request "$url" "GET" "" "Authorization: Bearer token" "X-Custom: value"
```

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PROXY_PORT` | 18888 | 代理服务端口 |
| `ECHO_HTTP_PORT` | 18081 | Echo HTTP 服务端口 |
| `ECHO_HTTPS_PORT` | 18444 | Echo HTTPS 服务端口 |
| `BIFROST_DATA_DIR` | `.bifrost-test` | 代理数据目录 |

## 独立 E2E 测试模板

对于需要自管理服务生命周期的独立测试，使用以下模板：

```bash
#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

source "$SCRIPT_DIR/test_utils/assert.sh"

PROXY_PORT="${PROXY_PORT:-19888}"
ECHO_HTTP_PORT="${ECHO_HTTP_PORT:-19081}"

BIFROST_BIN=""
PROXY_PID=""
ECHO_PID=""
TEST_DATA_DIR=""

passed=0
failed=0

cleanup() {
    [ -n "$PROXY_PID" ] && kill "$PROXY_PID" 2>/dev/null || true
    [ -n "$ECHO_PID" ] && kill "$ECHO_PID" 2>/dev/null || true
    [ -n "$TEST_DATA_DIR" ] && rm -rf "$TEST_DATA_DIR" || true
}
trap cleanup EXIT

setup_test_environment() {
    TEST_DATA_DIR=$(mktemp -d)
    mkdir -p "${TEST_DATA_DIR}/.bifrost/values"
    mkdir -p "${TEST_DATA_DIR}/.bifrost/rules"
    
    # 创建测试规则文件
    cat > "${TEST_DATA_DIR}/.bifrost/rules/test.txt" << 'EOF'
# Test rules
*.test.com http://127.0.0.1:${ECHO_HTTP_PORT}
EOF
}

compile_bifrost() {
    echo "Compiling bifrost..."
    (cd "$PROJECT_DIR" && cargo build --release --bin bifrost) || return 1
    BIFROST_BIN="${PROJECT_DIR}/target/release/bifrost"
}

start_echo_server() {
    python3 "${SCRIPT_DIR}/mock_servers/http_echo_server.py" "${ECHO_HTTP_PORT}" &
    ECHO_PID=$!
    sleep 1
}

start_proxy() {
    local rules_file="${TEST_DATA_DIR}/.bifrost/rules/test.txt"
    export BIFROST_DATA_DIR="${TEST_DATA_DIR}"
    
    "$BIFROST_BIN" --port "${PROXY_PORT}" start \
        --skip-cert-check --unsafe-ssl \
        --rules-file "${rules_file}" &
    PROXY_PID=$!
    sleep 2
}

http_request() {
    local url="$1"
    local method="${2:-GET}"
    local body="${3:-}"
    local proxy="http://127.0.0.1:${PROXY_PORT}"
    
    local response
    if [ -n "$body" ]; then
        response=$(curl -s --proxy "$proxy" -X "$method" -d "$body" -D - "$url" 2>/dev/null)
    else
        response=$(curl -s --proxy "$proxy" -X "$method" -D - "$url" 2>/dev/null)
    fi
    
    HTTP_STATUS=$(echo "$response" | head -1 | grep -oE '[0-9]{3}' | head -1)
    RESPONSE_HEADERS=$(echo "$response" | sed -n '1,/^\r*$/p')
    RESPONSE_BODY=$(echo "$response" | sed '1,/^\r*$/d')
}

test_basic_proxy() {
    local test_url="http://example.test.com/api/test"
    http_request "$test_url"
    
    if assert_status_2xx "$HTTP_STATUS" "Basic proxy test"; then
        _log_pass "Basic proxy test passed"
        ((passed++))
    else
        _log_fail "Basic proxy test failed"
        ((failed++))
    fi
}

main() {
    echo "=========================================="
    echo "  Your Feature E2E Tests"
    echo "=========================================="
    
    setup_test_environment || { echo "Failed to setup"; exit 1; }
    compile_bifrost || { echo "Failed to compile"; exit 1; }
    start_echo_server
    start_proxy
    
    test_basic_proxy
    
    echo ""
    echo "=========================================="
    echo "Results: $passed passed, $failed failed"
    echo "=========================================="
    
    [ $failed -eq 0 ]
}

main "$@"
```

## 调试技巧

### 查看请求详情

Echo 服务器返回完整的请求信息，可用于调试：

```bash
# 查看代理转发的请求
curl -s --proxy http://127.0.0.1:18888 http://test.com/api | jq .
```

### 启用详细日志

```bash
RUST_LOG=debug BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- -p 8080 --unsafe-ssl
```

### 单独测试规则

```bash
# 临时启动代理测试特定规则
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- -p 8080 --unsafe-ssl --rules-file ./scripts/rules/your_test.txt
```

## 最佳实践

1. **每个测试用例独立**: 测试之间不应有依赖关系
2. **清理测试环境**: 使用 trap 确保测试结束后清理资源
3. **使用有意义的描述**: 断言描述应清晰说明测试目的
4. **测试边界条件**: 包括空值、特殊字符、大数据等边界情况
5. **更新覆盖率文档**: 新增测试后更新 `rules/COVERAGE.md`

## 相关文档

- [项目规则](file:///Users/eden/work/github/whistle/.trae/rules/project_rules.md)
- [Scripts README](file:///Users/eden/work/github/whistle/rust/scripts/readme.md)
- [测试覆盖率](file:///Users/eden/work/github/whistle/rust/scripts/rules/COVERAGE.md)
