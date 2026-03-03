---
name: "e2e-test"
description: "创建和执行 Bifrost 代理的端到端测试；在添加新功能或修复 bug 后用于验证。必须优先于 rust-project-validate 技能执行。"
---

# E2E 测试创建与执行

该技能用于创建和执行 Bifrost 代理的端到端测试，确保代理功能正确。

## 何时调用

- 添加新功能后需要验证
- 修复 bug 后需要回归测试
- 需要创建新的测试用例
- 需要运行现有的 E2E 测试

## 参考文档

在执行任务前，**必须先阅读** E2E 测试框架的详细文档：

- [e2e-tests README](rust/e2e-tests/readme.md) - 测试架构、目录结构、断言库、现有用例等
- [测试覆盖率](rust/e2e-tests/rules/COVERAGE.md) - 当前测试覆盖情况

## 执行测试

### 运行全量测试（推荐）

```bash
cd rust/scripts

# 顺序执行全量测试
./run_all_tests_parallel.sh && \
./test_values_cli.sh && \
./test_values_e2e.sh && \
./test_pattern.sh && \
./tests/test_frames_admin_api.sh && \
./tests/test_rules_admin_api.sh && \
./tests/test_values_admin_api.sh && \
./tests/test_whitelist_admin_api.sh && \
./tests/test_cert_admin_api.sh && \
./tests/test_proxy_admin_api.sh && \
./tests/test_system_admin_api.sh
```

### 运行单个测试

```bash
cd rust/scripts

# 规则测试
./test_rules.sh rules/forwarding/http_to_http.txt

# 独立测试脚本
./test_values_cli.sh
./test_values_e2e.sh
./test_pattern.sh

# Admin API 测试
./tests/test_rules_admin_api.sh
```

## 创建新测试

### 步骤 1: 确定测试类型

| 类型           | 说明                 | 创建方式                               |
| -------------- | -------------------- | -------------------------------------- |
| 规则测试       | 测试代理规则行为     | 在 `rules/` 目录下创建 `.txt` 规则文件 |
| Admin API 测试 | 测试管理接口         | 在 `tests/` 目录下创建脚本             |
| 独立 E2E 测试  | 自管理服务的完整测试 | 在 `scripts/` 目录下创建脚本           |

### 步骤 2: 创建规则文件（规则测试）

在对应分类目录下创建 `.txt` 规则文件：

```bash
# 分类目录: forwarding, request_modify, response_modify, redirect, priority, control, template
cat > rules/your_category/your_test.txt << 'EOF'
# 测试描述

# 测试用例标识 - 功能描述
pattern.test.com protocol://target
EOF
```

### 步骤 3: 创建测试脚本

使用 `test_utils/` 中的工具库：

```bash
#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/test_utils/assert.sh"
source "$SCRIPT_DIR/test_utils/http_client.sh"

PROXY_PORT="${PROXY_PORT:-18888}"
PROXY="http://127.0.0.1:${PROXY_PORT}"

passed=0
failed=0

test_your_feature() {
    local test_url="https://your-feature.test.com/api/test"
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
    echo "Results: $passed passed, $failed failed"
    [ $failed -eq 0 ]
}

main "$@"
```

### 步骤 4: 更新覆盖率文档

新增测试后更新 `rules/COVERAGE.md`。

## 验证准则

1. **测试必须通过**: 所有相关测试用例必须通过
2. **测试独立性**: 测试之间不应有依赖关系
3. **清理资源**: 使用 trap 确保测试结束后清理资源
4. **边界条件**: 包括空值、特殊字符、大数据等边界情况
5. **断言描述**: 断言描述应清晰说明测试目的

## 调试、端到端验证方法(最佳方式)

> 构造测试用例，进行端到端测试(API 验证，和交互验证)，覆盖 HTTP/1.1、HTTP/2、HTTPS、SOCKS5、CONNECT-UDP 等场景，覆盖 TLS 与非 TLS 情况，覆盖 TSL 解包和不解包场景，覆盖 HTTP/3 场景。
> 所有管理端接口都必须是 `http://127.0.0.1:{port}/_bifrost/api/` 开头

```bash
# 编译并启动代理服务
RUST_LOG=debug BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start  -p 8890 --unsafe-ssl

# 查看代理转发的请求详情
curl -s --proxy http://127.0.0.1:8890 http://test.com/api | jq .

# 使用 curl 调用管理端接口
curl -s -X POST http://127.0.0.1:8890/_bifrost/api/replay/execute -H "Content-Type: application/json" -d '{"request":{"method":"GET","url":"https://httpbin.org/get","headers":[["Accept","*/*"]]},"rule_config":{"mode":"enabled"}}'
# 观察所有的日志：代理日志、管理端日志、接口日志、代理请求日志
# 必须使用 chrome mcp工具打开 http://127.0.0.1:8890/_bifrost/ 验证交互功能

# 特别说明提高验证效率的方法
# 如果仅仅改动了 web 页面功能，不涉及管理端接口，那么只需要验证 web 页面功能是否正常即可。则启动代理服务后，可以单独启动 web 调试环境 pnpm dev ，，直接在浏览器中打开 http://127.0.0.1:3000 进行页面验证。
```
