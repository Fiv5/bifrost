# Group Admin API 测试用例

## 功能模块说明

Group Admin API 提供团队规则管理功能。通过 `/_bifrost/api/group` 可以访问远程团队组信息，通过 `/_bifrost/api/group-rules/{group_id}` 可以管理指定团队组下的规则（列出、查看详情、创建、更新、启用/禁用、删除）。此功能依赖远程同步服务（Sync Manager），需要先配置远程连接。

**注意**：Group API 的部分功能需要配置远程同步服务才能使用。如果未配置同步服务，API 将返回 503 Service Unavailable。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 未被其他程序占用
3. 如需测试团队规则相关功能，需已配置远程同步连接

---

## 测试用例

### TC-AGR-01：未配置同步服务时访问 Group API 返回 503

**前置条件**：Bifrost 未配置远程同步服务

**操作步骤**：
1. 使用 curl 请求团队组列表：
   ```bash
   curl -s -w "\n%{http_code}" http://127.0.0.1:8800/_bifrost/api/group
   ```

**预期结果**：
- 返回 HTTP 503
- 响应体包含错误信息 "Sync manager not available"

---

### TC-AGR-02：未配置同步服务时访问 Group Rules API 返回 503

**前置条件**：Bifrost 未配置远程同步服务

**操作步骤**：
1. 使用 curl 请求团队规则列表（使用任意 group_id）：
   ```bash
   curl -s -w "\n%{http_code}" http://127.0.0.1:8800/_bifrost/api/group-rules/test-group-id
   ```

**预期结果**：
- 返回 HTTP 503
- 响应体包含错误信息 "Sync manager not available"

---

### TC-AGR-03：列出团队组列表

**前置条件**：已配置远程同步服务，且远程存在至少一个团队组

**操作步骤**：
1. 使用 curl 获取团队组列表：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/group
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体为远程服务返回的团队组列表 JSON
- 包含 `data` 字段，内有团队组数组
- 每个团队组包含 `group_id`、`name` 等字段

---

### TC-AGR-04：获取单个团队组详情

**前置条件**：已配置远程同步服务，记录一个有效的 `GROUP_ID`

**操作步骤**：
1. 使用 curl 获取团队组详情：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/group/{GROUP_ID}
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体为该团队组的完整 JSON 信息
- 包含 `data` 字段，内有 `group_id`、`name`、`visibility` 等字段

---

### TC-AGR-05：列出团队组规则（同时同步远程规则）

**前置条件**：已配置远程同步服务，记录一个有效的 `GROUP_ID`

**操作步骤**：
1. 使用 curl 获取指定团队组的规则列表：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/group-rules/{GROUP_ID}
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体为 JSON 对象，包含：
  - `group_id`：传入的团队组 ID
  - `group_name`：团队组名称
  - `writable`：布尔值，表示当前用户是否有写权限
  - `rules`：规则数组，每个规则包含 `name`、`enabled`、`sort_order`、`rule_count`、`created_at`、`updated_at` 等字段

---

### TC-AGR-06：查看团队组中单条规则详情

**前置条件**：已通过 TC-AGR-05 获取规则列表，记录一个规则的 `RULE_NAME`

**操作步骤**：
1. 使用 curl 获取规则详情（注意 RULE_NAME 需 URL 编码）：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/group-rules/{GROUP_ID}/{RULE_NAME}
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体为 JSON 对象，包含：
  - `name`：规则名称
  - `content`：规则内容文本
  - `enabled`：布尔值
  - `sort_order`：排序序号
  - `created_at`：创建时间
  - `updated_at`：更新时间
  - `sync`：同步信息对象，包含 `status`、`remote_id`、`remote_updated_at`

---

### TC-AGR-07：创建团队组规则

**前置条件**：已配置远程同步服务，`GROUP_ID` 指向一个可写（writable=true）的团队组

**操作步骤**：
1. 使用 curl 创建新规则：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/group-rules/{GROUP_ID} \
     -H "Content-Type: application/json" \
     -d '{"name":"test-rule","content":"example.com redirect-to https://example.org"}'
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体为新创建规则的详情 JSON
- `name` 为 `"test-rule"`
- `sync.status` 为 `"synced"`
- `sync.remote_id` 非空（已同步到远程）

---

### TC-AGR-08：更新团队组规则内容

**前置条件**：已通过 TC-AGR-07 创建规则，记录 `RULE_NAME`

**操作步骤**：
1. 使用 curl 更新规则内容：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/group-rules/{GROUP_ID}/{RULE_NAME} \
     -H "Content-Type: application/json" \
     -d '{"content":"example.com redirect-to https://example.net"}'
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体为更新后的规则详情
- `content` 已变为新内容
- `updated_at` 已更新
- `sync.status` 为 `"synced"`

---

### TC-AGR-09：启用团队组规则

**前置条件**：已创建规则，默认 enabled 为 false

**操作步骤**：
1. 使用 curl 启用规则：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/group-rules/{GROUP_ID}/{RULE_NAME}/enable
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体包含成功消息 "Rule 'xxx' enabled successfully"
- 再次查看规则详情，`enabled` 为 `true`

---

### TC-AGR-10：禁用团队组规则

**前置条件**：已通过 TC-AGR-09 启用规则

**操作步骤**：
1. 使用 curl 禁用规则：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/group-rules/{GROUP_ID}/{RULE_NAME}/disable
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体包含成功消息 "Rule 'xxx' disabled successfully"
- 再次查看规则详情，`enabled` 为 `false`

---

### TC-AGR-11：删除团队组规则

**前置条件**：已创建规则，记录 `RULE_NAME`

**操作步骤**：
1. 使用 curl 删除规则：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/group-rules/{GROUP_ID}/{RULE_NAME}
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体包含成功消息 "Rule deleted"
- 再次获取该规则详情返回 404
- 远程同步服务上对应的规则也被删除

---

### TC-AGR-12：对不可写团队组创建规则返回 403

**前置条件**：已配置远程同步服务，`GROUP_ID` 指向一个不可写（writable=false）的团队组

**操作步骤**：
1. 使用 curl 尝试创建规则：
   ```bash
   curl -s -w "\n%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/group-rules/{GROUP_ID} \
     -H "Content-Type: application/json" \
     -d '{"name":"unauthorized-rule","content":"test"}'
   ```

**预期结果**：
- 返回 HTTP 403
- 响应体包含错误信息 "No write permission for this group"

---

### TC-AGR-13：group-rules 缺少 group_id 参数返回 400

**操作步骤**：
1. 使用 curl 访问 group-rules 根路径（无 group_id）：
   ```bash
   curl -s -w "\n%{http_code}" http://127.0.0.1:8800/_bifrost/api/group-rules/
   ```

**预期结果**：
- 返回 HTTP 400
- 响应体包含错误信息 "group_id is required"

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
