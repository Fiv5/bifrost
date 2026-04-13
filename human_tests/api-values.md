# Values 管理 API 测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 可用且服务已正常启动
3. 所有 API 请求基础地址：`http://127.0.0.1:8800/_bifrost/`

---

## 测试用例

### TC-AVA-01：列出所有 Values（初始为空）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/values | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `values` 数组和 `total` 字段
- `values` 为空数组 `[]`
- `total` 为 `0`

---

### TC-AVA-02：创建一个 Value

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d '{"name": "test-key", "value": "hello-world"}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Value 'test-key' created"`

---

### TC-AVA-03：通过名称获取单个 Value

**前置条件**：已通过 TC-AVA-02 创建 `test-key`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/values/test-key | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含以下字段：
  - `name` 为 `"test-key"`
  - `value` 为 `"hello-world"`
  - `created_at` 为非空时间戳字符串
  - `updated_at` 为非空时间戳字符串

---

### TC-AVA-04：创建 Value 后验证列表中可见

**前置条件**：已通过 TC-AVA-02 创建 `test-key`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/values | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- `total` 为 `1`
- `values` 数组中包含一个对象，其 `name` 为 `"test-key"`，`value` 为 `"hello-world"`

---

### TC-AVA-05：更新已有 Value 的内容

**前置条件**：已通过 TC-AVA-02 创建 `test-key`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/values/test-key \
     -H "Content-Type: application/json" \
     -d '{"value": "updated-value"}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Value 'test-key' updated"`

---

### TC-AVA-06：更新后验证新内容

**前置条件**：已通过 TC-AVA-05 更新 `test-key`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/values/test-key | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- `name` 为 `"test-key"`
- `value` 为 `"updated-value"`（已更新为新内容）
- `updated_at` 时间戳晚于 `created_at`

---

### TC-AVA-07：删除 Value

**前置条件**：已通过 TC-AVA-02 创建 `test-key`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/values/test-key | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Value 'test-key' deleted"`

---

### TC-AVA-08：删除后验证已从列表中移除

**前置条件**：已通过 TC-AVA-07 删除 `test-key`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/values | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- `total` 为 `0`
- `values` 为空数组 `[]`

---

### TC-AVA-09：获取不存在的 Value 返回 404

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8800/_bifrost/api/values/non-existent
   ```

**预期结果**：
- HTTP 状态码 `404`
- 响应体包含 `"Value 'non-existent' not found"`

---

### TC-AVA-10：创建重复名称的 Value 返回 409

**前置条件**：先创建一个 Value

**操作步骤**：
1. 创建初始 Value：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d '{"name": "dup-key", "value": "first"}'
   ```
2. 再次使用相同名称创建：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d '{"name": "dup-key", "value": "second"}'
   ```

**预期结果**：
- 第二次创建返回 HTTP 状态码 `409`
- 响应体包含 `"Value 'dup-key' already exists"`

---

### TC-AVA-11：创建名称为空的 Value 返回 400

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d '{"name": "", "value": "some-value"}'
   ```

**预期结果**：
- HTTP 状态码 `400`
- 响应体包含 `"Value name cannot be empty"`

---

### TC-AVA-12：更新不存在的 Value 返回 404

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X PUT http://127.0.0.1:8800/_bifrost/api/values/no-such-key \
     -H "Content-Type: application/json" \
     -d '{"value": "new-value"}'
   ```

**预期结果**：
- HTTP 状态码 `404`
- 响应体包含 `"Value 'no-such-key' not found"`

---

### TC-AVA-13：删除不存在的 Value 返回 404

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X DELETE http://127.0.0.1:8800/_bifrost/api/values/no-such-key
   ```

**预期结果**：
- HTTP 状态码 `404`
- 响应体包含 `"Value 'no-such-key' not found"`

---

### TC-AVA-14：创建多个 Value 后列表正确显示

**操作步骤**：
1. 依次创建多个 Value：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d '{"name": "key-a", "value": "value-a"}'
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d '{"name": "key-b", "value": "value-b"}'
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d '{"name": "key-c", "value": "value-c"}'
   ```
2. 查询列表：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/values | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- `total` 为 `3`（或加上之前未清理的数量）
- `values` 数组中包含 `key-a`、`key-b`、`key-c` 三个条目
- 每个条目均包含 `name`、`value`、`created_at`、`updated_at` 字段

---

### TC-AVA-15：发送无效 JSON 返回 400

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d 'not-valid-json'
   ```

**预期结果**：
- HTTP 状态码 `400`
- 响应体包含 `"Invalid JSON"`

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
