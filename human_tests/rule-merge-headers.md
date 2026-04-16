# 规则合并 - reqHeaders/resHeaders 同名覆盖

## 功能模块说明

当多条 `reqHeaders://` 或 `resHeaders://` 规则匹配同一请求时，更具体的路径规则应该覆盖更宽泛路径规则的同名 header 值。不同名的 header 应该累积合并。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录）：
```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
```

2. 准备测试规则文本（通过 CLI 或 API 添加规则）

## 测试用例列表

### TC-RMH-01: reqHeaders 同名 header 被更具体路径覆盖（单元测试验证）

**操作步骤**：
1. 运行单元测试：
```bash
cargo test -p bifrost-cli -- test_later_reqheaders_rule_should_override_earlier_same_header --no-capture
```

**预期结果**：
- 测试通过
- `x-tt-env` 的值为 `ppe_fix_disabled_skill_loading`（来自更具体的 `/api/v1/` 规则）
- `x-use-ppe` 的值为 `1`（两条规则值相同，不冲突）

### TC-RMH-02: DomainMatcher 路径深度影响优先级（单元测试验证）

**操作步骤**：
1. 运行路径深度优先级测试：
```bash
cargo test -p bifrost-core -- test_priority_path_depth_exact --no-capture
cargo test -p bifrost-core -- test_priority_path_depth_prefix --no-capture
cargo test -p bifrost-core -- test_priority_specific_path_beats_root_with_protocol --no-capture
```

**预期结果**：
- 所有测试通过
- `example.com/api/v1/` (depth=2) 优先级高于 `example.com/api` (depth=1) 高于 `example.com/` (depth=0)
- `https://example.com/api/v1/` priority=122 > `https://example.com/` priority=120
- Prefix 路径同理：`/api/v1/*` priority > `/api/*` priority

### TC-RMH-03: 真实代理场景 - reqHeaders 通过 API 验证

**操作步骤**：
1. 启动服务后，通过 API 创建规则：
```bash
curl -s -X POST http://127.0.0.1:8800/_bifrost/api/rules \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-header-merge",
    "content": "`httpbin.org/` reqHeaders://{env1}\n`httpbin.org/get` reqHeaders://{env2}",
    "enabled": true
  }'
```
2. 通过 API 创建对应的 values：
```bash
curl -s -X POST http://127.0.0.1:8800/_bifrost/api/values \
  -H "Content-Type: application/json" \
  -d '{"name": "env1", "value": "x-test-header: value-from-root\nx-extra: extra-value"}'

curl -s -X POST http://127.0.0.1:8800/_bifrost/api/values \
  -H "Content-Type: application/json" \
  -d '{"name": "env2", "value": "x-test-header: value-from-specific"}'
```
3. 通过代理访问：
```bash
curl -x http://127.0.0.1:8800 http://httpbin.org/get 2>/dev/null | jq '.headers'
```

**预期结果**：
- `X-Test-Header` 值为 `value-from-specific`（具体路径 `/get` 规则覆盖根路径 `/` 规则）
- `X-Extra` 值为 `extra-value`（仅根路径规则设置，不冲突，应保留）

### TC-RMH-04: 转发类协议仍保持 first-match-wins

**操作步骤**：
1. 运行现有转发类测试确认无回归：
```bash
cargo test -p bifrost-core -- matcher::domain --no-capture
```

**预期结果**：
- 所有 domain matcher 测试通过
- 转发类协议（host://, http://, https://）行为未改变

### TC-RMH-05: 两条 reqHeaders 同名 key 覆盖 + 客户端请求也带同名 header（E2E 验证）

**操作步骤**：
1. 运行 E2E 测试：
```bash
cargo test -p bifrost-e2e -- test_reqheaders_same_key_override --no-capture
```

**预期结果**：
- 测试通过
- 远端 mock 服务只收到一个 `x-same-key` header
- 该 header 的值为 `second`（第二条规则覆盖第一条规则和客户端原始值）
- 客户端发送的 `X-Same-Key: client-original` 被规则覆盖，不会到达远端

**覆盖场景说明**：
- 两条规则：`reqHeaders://X-Same-Key=first` 和 `reqHeaders://X-Same-Key=second`
- 客户端请求自带 `X-Same-Key: client-original`
- 规则按顺序依次 insert 到 HeaderMap，后设置覆盖先设置
- 最终远端只收到一个 `X-Same-Key: second`

## 清理步骤

1. 删除测试创建的规则和 values
2. 停止测试服务：`cargo run --bin bifrost -- stop -p 8800`
3. 删除临时数据目录：`rm -rf ./.bifrost-test`
