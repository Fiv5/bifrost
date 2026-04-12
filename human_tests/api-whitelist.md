# 白名单/访问控制管理 API 测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 可用且服务已正常启动
3. 所有 API 请求基础地址：`http://127.0.0.1:8800/_bifrost/`

---

## 测试用例

### TC-AWL-01：获取当前白名单配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含以下字段：
  - `mode`：字符串，值为 `"allow_all"`、`"local_only"`、`"whitelist"` 或 `"interactive"` 之一
  - `allow_lan`：布尔值
  - `whitelist`：字符串数组（IP/CIDR 列表）
  - `temporary_whitelist`：字符串数组
  - `userpass`：对象，包含 `enabled`、`accounts`、`loopback_requires_auth` 字段

---

### TC-AWL-02：添加 IP 到白名单

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/whitelist \
     -H "Content-Type: application/json" \
     -d '{"ip_or_cidr": "192.168.1.100"}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Added 192.168.1.100 to whitelist"`

---

### TC-AWL-03：添加 IP 后验证白名单列表

**前置条件**：已通过 TC-AWL-02 添加 `192.168.1.100`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist | jq '.whitelist'
   ```

**预期结果**：
- `whitelist` 数组中包含 `"192.168.1.100"`

---

### TC-AWL-04：添加 CIDR 格式到白名单

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/whitelist \
     -H "Content-Type: application/json" \
     -d '{"ip_or_cidr": "10.0.0.0/24"}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Added 10.0.0.0/24 to whitelist"`

---

### TC-AWL-05：从白名单中移除 IP

**前置条件**：已通过 TC-AWL-02 添加 `192.168.1.100`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/whitelist \
     -H "Content-Type: application/json" \
     -d '{"ip_or_cidr": "192.168.1.100"}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Removed 192.168.1.100 from whitelist"`

---

### TC-AWL-06：移除不存在的 IP 返回 404

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X DELETE http://127.0.0.1:8800/_bifrost/api/whitelist \
     -H "Content-Type: application/json" \
     -d '{"ip_or_cidr": "172.16.0.1"}'
   ```

**预期结果**：
- HTTP 状态码 `404`
- 响应体包含 `"172.16.0.1 not found in whitelist"`

---

### TC-AWL-07：获取当前访问控制模式

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist/mode | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `mode` 字段
- `mode` 值为 `"allow_all"`、`"local_only"`、`"whitelist"` 或 `"interactive"` 之一

---

### TC-AWL-08：设置访问控制模式为 whitelist

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/mode \
     -H "Content-Type: application/json" \
     -d '{"mode": "whitelist"}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回 `"mode": "whitelist"`

---

### TC-AWL-09：验证模式已变更

**前置条件**：已通过 TC-AWL-08 设置模式为 `whitelist`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist/mode | jq .
   ```

**预期结果**：
- `mode` 值为 `"whitelist"`

---

### TC-AWL-10：设置无效的访问控制模式返回 400

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/mode \
     -H "Content-Type: application/json" \
     -d '{"mode": "invalid_mode"}'
   ```

**预期结果**：
- HTTP 状态码 `400`

---

### TC-AWL-11：获取 allow-lan 状态

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist/allow-lan | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `allow_lan` 布尔值字段

---

### TC-AWL-12：设置 allow-lan 为 true

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/allow-lan \
     -H "Content-Type: application/json" \
     -d '{"allow_lan": true}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回 `"allow_lan": true`

---

### TC-AWL-13：验证 allow-lan 已更新

**前置条件**：已通过 TC-AWL-12 设置 `allow_lan` 为 `true`

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist/allow-lan | jq .
   ```

**预期结果**：
- `allow_lan` 值为 `true`

---

### TC-AWL-14：设置 allow-lan 为 false

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/allow-lan \
     -H "Content-Type: application/json" \
     -d '{"allow_lan": false}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回 `"allow_lan": false`

---

### TC-AWL-15：添加临时白名单 IP

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/whitelist/temporary \
     -H "Content-Type: application/json" \
     -d '{"ip": "192.168.1.200"}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Added 192.168.1.200 to temporary whitelist"`

---

### TC-AWL-16：添加临时白名单后验证列表

**前置条件**：已通过 TC-AWL-15 添加临时白名单 IP

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist | jq '.temporary_whitelist'
   ```

**预期结果**：
- `temporary_whitelist` 数组中包含 `"192.168.1.200"`

---

### TC-AWL-17：移除临时白名单 IP

**前置条件**：已通过 TC-AWL-15 添加 `192.168.1.200` 到临时白名单

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/whitelist/temporary \
     -H "Content-Type: application/json" \
     -d '{"ip": "192.168.1.200"}' | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Removed 192.168.1.200 from temporary whitelist"`

---

### TC-AWL-18：移除不存在的临时白名单 IP 返回 404

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X DELETE http://127.0.0.1:8800/_bifrost/api/whitelist/temporary \
     -H "Content-Type: application/json" \
     -d '{"ip": "10.10.10.10"}'
   ```

**预期结果**：
- HTTP 状态码 `404`
- 响应体包含 `"10.10.10.10 not found in temporary whitelist"`

---

### TC-AWL-19：获取待授权列表

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist/pending | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 数组（初始为空数组 `[]`）

---

### TC-AWL-20：批准待授权 IP

**前置条件**：存在一个待授权的 IP（需要先将模式设为 `interactive`，然后通过非白名单 IP 发起代理请求触发 pending 记录）

**操作步骤**：
1. 假设待授权 IP 为 `192.168.1.50`，执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/whitelist/pending/approve \
     -H "Content-Type: application/json" \
     -d '{"ip": "192.168.1.50"}' | jq .
   ```

**预期结果**：
- 如果 IP 存在于 pending 列表：
  - HTTP 状态码 200
  - 返回 JSON 包含 `"success": true`
  - 返回消息包含 `"Approved 192.168.1.50 and added to temporary whitelist"`
- 如果 IP 不在 pending 列表：
  - HTTP 状态码 `404`
  - 响应体包含 `"192.168.1.50 not found in pending authorizations"`

---

### TC-AWL-21：拒绝待授权 IP

**前置条件**：存在一个待授权的 IP

**操作步骤**：
1. 假设待授权 IP 为 `192.168.1.60`，执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/whitelist/pending/reject \
     -H "Content-Type: application/json" \
     -d '{"ip": "192.168.1.60"}' | jq .
   ```

**预期结果**：
- 如果 IP 存在于 pending 列表：
  - HTTP 状态码 200
  - 返回 JSON 包含 `"success": true`
  - 返回消息包含 `"Rejected 192.168.1.60 and added to session denied list"`
- 如果 IP 不在 pending 列表：
  - HTTP 状态码 `404`
  - 响应体包含 `"192.168.1.60 not found in pending authorizations"`

---

### TC-AWL-22：清除所有待授权记录

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/whitelist/pending | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 包含 `"success": true`
- 返回消息包含 `"Cleared all pending authorizations"`

---

### TC-AWL-23：清除后验证待授权列表为空

**前置条件**：已通过 TC-AWL-22 清除待授权列表

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist/pending | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回空数组 `[]`

---

### TC-AWL-24：SSE 待授权事件流连接

**操作步骤**：
1. 执行以下命令（等待 5 秒后自动超时中断）：
   ```bash
   curl -s -N --max-time 5 http://127.0.0.1:8800/_bifrost/api/whitelist/pending/stream
   ```

**预期结果**：
- 连接成功建立，HTTP 状态码 200
- 响应头 `Content-Type` 为 `text/event-stream`
- 响应头 `Cache-Control` 为 `no-cache`
- 连接保持打开状态直到超时
- 如果有新的 pending 事件产生，将以 SSE 格式（`data: {...}\n\n`）推送

---

### TC-AWL-25：设置模式为 interactive 并恢复

**操作步骤**：
1. 设置模式为 `interactive`：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/mode \
     -H "Content-Type: application/json" \
     -d '{"mode": "interactive"}' | jq .
   ```
2. 验证模式已变更：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist/mode | jq .
   ```
3. 恢复为 `allow_all`：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/mode \
     -H "Content-Type: application/json" \
     -d '{"mode": "allow_all"}' | jq .
   ```

**预期结果**：
- 第 1 步返回 `"success": true`，`"mode": "interactive"`
- 第 2 步返回 `"mode": "interactive"`
- 第 3 步返回 `"success": true`，`"mode": "allow_all"`

---

### TC-AWL-26：添加无效 IP 格式到临时白名单返回 400

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/whitelist/temporary \
     -H "Content-Type: application/json" \
     -d '{"ip": "not-a-valid-ip"}'
   ```

**预期结果**：
- HTTP 状态码 `400`
- 响应体包含 `"Invalid IP address"`

---

### TC-AWL-27：完整白名单配置综合验证

**操作步骤**：
1. 设置模式为 `whitelist`：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/mode \
     -H "Content-Type: application/json" \
     -d '{"mode": "whitelist"}'
   ```
2. 开启 allow-lan：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/whitelist/allow-lan \
     -H "Content-Type: application/json" \
     -d '{"allow_lan": true}'
   ```
3. 添加一个 IP 到永久白名单：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/whitelist \
     -H "Content-Type: application/json" \
     -d '{"ip_or_cidr": "192.168.2.1"}'
   ```
4. 添加一个 IP 到临时白名单：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/whitelist/temporary \
     -H "Content-Type: application/json" \
     -d '{"ip": "192.168.2.2"}'
   ```
5. 查询完整配置：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/whitelist | jq .
   ```

**预期结果**：
- `mode` 为 `"whitelist"`
- `allow_lan` 为 `true`
- `whitelist` 数组中包含 `"192.168.2.1"`
- `temporary_whitelist` 数组中包含 `"192.168.2.2"`
- `userpass` 对象存在且结构完整

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
