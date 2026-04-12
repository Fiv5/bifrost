# 高级配置管理 Admin API 测试用例

## 功能模块说明

验证 Bifrost 高级配置管理相关的 Admin API 接口，包括沙箱配置（Sandbox Config）、服务器配置（Server Config）、UI 配置（UI Config）、IP-TLS 待处理管理（IP-TLS Pending）以及活跃连接管理（Connections）。这些接口是 `api-config.md` 中未覆盖的进阶端点。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 未被占用
3. 服务启动成功后再执行测试用例

---

## 测试用例

### TC-ACA-01：获取沙箱配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/sandbox | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下顶层字段：
  - `file`：对象，包含 `sandbox_dir`（字符串）、`allowed_dirs`（字符串数组）、`max_bytes`（数字）
  - `net`：对象，包含 `enabled`（布尔值）、`timeout_ms`（数字）、`max_request_bytes`（数字）、`max_response_bytes`（数字）
  - `limits`：对象，包含 `timeout_ms`（数字）、`max_memory_bytes`（数字）、`max_decode_input_bytes`（数字）、`max_decompress_output_bytes`（数字）

---

### TC-ACA-02：更新沙箱配置 — 修改网络设置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{"net": {"enabled": false, "timeout_ms": 5000}}' | jq .
   ```
2. 验证配置已生效：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/sandbox | jq .net
   ```

**预期结果**：
- PUT 请求返回 HTTP 200，响应为更新后的完整沙箱配置 JSON
- `net.enabled` 值为 `false`
- `net.timeout_ms` 值为 `5000`
- GET 请求验证配置已持久化

---

### TC-ACA-03：更新沙箱配置 — 修改文件限制

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{"file": {"max_bytes": 2097152}}' | jq .file.max_bytes
   ```

**预期结果**：
- 返回 HTTP 200
- `file.max_bytes` 值为 `2097152`（2 MB）

---

### TC-ACA-04：更新沙箱配置 — 修改执行限制

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{"limits": {"timeout_ms": 10000, "max_memory_bytes": 33554432}}' | jq .limits
   ```

**预期结果**：
- 返回 HTTP 200
- `limits.timeout_ms` 值为 `10000`
- `limits.max_memory_bytes` 值为 `33554432`（32 MB）

---

### TC-ACA-05：更新沙箱配置 — file.max_bytes 为 0 被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{"file": {"max_bytes": 0}}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "file.max_bytes must be > 0"

---

### TC-ACA-06：更新沙箱配置 — allowed_dirs 必须为绝对路径

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{"file": {"allowed_dirs": ["relative/path"]}}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "allowed_dirs must be absolute paths"

---

### TC-ACA-07：更新沙箱配置 — net.timeout_ms 为 0 被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{"net": {"timeout_ms": 0}}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "net.timeout_ms must be > 0"

---

### TC-ACA-08：获取服务器配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/server | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `timeout_secs`：数字（默认 `30`）
  - `http1_max_header_size`：数字（默认 `65536`，即 64 KB）
  - `http2_max_header_list_size`：数字（默认 `262144`，即 256 KB）
  - `websocket_handshake_max_header_size`：数字（默认 `65536`，即 64 KB）

---

### TC-ACA-09：更新服务器配置 — 修改超时时间

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/server \
     -H "Content-Type: application/json" \
     -d '{"timeout_secs": 60}' | jq .
   ```
2. 验证配置已生效：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/server | jq .timeout_secs
   ```

**预期结果**：
- PUT 请求返回 HTTP 200，响应为更新后的服务器配置 JSON
- `timeout_secs` 值为 `60`
- GET 请求验证配置已持久化

---

### TC-ACA-10：更新服务器配置 — 修改 Header 大小限制

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/server \
     -H "Content-Type: application/json" \
     -d '{"http1_max_header_size": 131072, "websocket_handshake_max_header_size": 131072}' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- `http1_max_header_size` 值为 `131072`（128 KB）
- `websocket_handshake_max_header_size` 值为 `131072`（128 KB）

---

### TC-ACA-11：更新服务器配置 — timeout_secs 为 0 被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/server \
     -H "Content-Type: application/json" \
     -d '{"timeout_secs": 0}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "timeout_secs must be > 0"

---

### TC-ACA-12：更新服务器配置 — http2_max_header_list_size 超出上限被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/server \
     -H "Content-Type: application/json" \
     -d '{"http2_max_header_list_size": 4294967296}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "http2_max_header_list_size must be <= 4294967295"

---

### TC-ACA-13：获取 UI 配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/ui | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `pinnedFilters`：数组（初始可能为空 `[]`）
  - `filterPanel`：对象，包含 `collapsed`（布尔值）、`width`（数字）、`collapsedSections`（对象，含 `pinned`、`clientIp`、`clientApp`、`domain` 布尔字段）
  - `detailPanelCollapsed`：布尔值

---

### TC-ACA-14：更新 UI 配置 — 设置固定过滤器

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/ui \
     -H "Content-Type: application/json" \
     -d '{
       "pinnedFilters": [
         {"id": "filter-1", "type": "domain", "value": "example.com", "label": "Example"}
       ]
     }' | jq .
   ```
2. 验证配置已生效：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/ui | jq .pinnedFilters
   ```

**预期结果**：
- PUT 请求返回 HTTP 200，响应为更新后的 UI 配置 JSON
- `pinnedFilters` 数组包含 1 个元素，`id` 为 `filter-1`，`type` 为 `domain`，`value` 为 `example.com`
- GET 请求验证配置已持久化

---

### TC-ACA-15：更新 UI 配置 — 修改过滤面板状态

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/ui \
     -H "Content-Type: application/json" \
     -d '{
       "filterPanel": {
         "collapsed": true,
         "width": 300,
         "collapsedSections": {"pinned": false, "clientIp": true, "clientApp": true, "domain": false}
       },
       "detailPanelCollapsed": true
     }' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- `filterPanel.collapsed` 为 `true`
- `filterPanel.width` 为 `300`
- `filterPanel.collapsedSections.clientIp` 为 `true`
- `filterPanel.collapsedSections.domain` 为 `false`
- `detailPanelCollapsed` 为 `true`

---

### TC-ACA-16：更新 UI 配置 — 无效 JSON 被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/ui \
     -H "Content-Type: application/json" \
     -d '{invalid json}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "Invalid JSON"

---

### TC-ACA-17：获取 IP-TLS 待处理列表

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/ip-tls/pending | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 数组
- 无待处理项时返回空数组 `[]`

---

### TC-ACA-18：审批 IP TLS 拦截 — IP 不在待处理列表中

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/ip-tls/pending/approve \
     -H "Content-Type: application/json" \
     -d '{"ip": "192.168.1.100"}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 404（Not Found）
- 错误信息包含 "192.168.1.100 not found in pending IP TLS list"

---

### TC-ACA-19：审批 IP TLS 拦截 — 无效 IP 地址被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/ip-tls/pending/approve \
     -H "Content-Type: application/json" \
     -d '{"ip": "not-a-valid-ip"}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "Invalid IP address"

---

### TC-ACA-20：跳过 IP TLS 拦截 — IP 不在待处理列表中

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/ip-tls/pending/skip \
     -H "Content-Type: application/json" \
     -d '{"ip": "10.0.0.1"}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 404（Not Found）
- 错误信息包含 "10.0.0.1 not found in pending IP TLS list"

---

### TC-ACA-21：清除所有 IP-TLS 待处理项

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/config/ip-tls/pending | jq .
   ```
2. 验证待处理列表已清空：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/ip-tls/pending | jq .
   ```

**预期结果**：
- DELETE 请求返回 HTTP 200
- 响应包含 "Cleared all pending IP TLS decisions"
- GET 请求返回空数组 `[]`

---

### TC-ACA-22：IP-TLS 待处理 SSE 事件流连接

**操作步骤**：
1. 执行以下命令（限时 3 秒自动断开）：
   ```bash
   timeout 3 curl -s -N http://127.0.0.1:8800/_bifrost/api/config/ip-tls/pending/stream -H "Accept: text/event-stream" -D- 2>&1 | head -10
   ```

**预期结果**：
- 响应头包含 `Content-Type: text/event-stream`
- 响应头包含 `Cache-Control: no-cache`
- 连接保持打开状态直到超时断开（SSE 长连接）

---

### TC-ACA-23：获取活跃连接列表

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/connections | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `connections`：数组，每个元素包含 `req_id`（字符串）、`host`（字符串）、`port`（数字）、`intercept_mode`（布尔值）、`client_app`（字符串或 null）
  - `total`：数字，等于 `connections` 数组的长度

---

### TC-ACA-24：按应用名称断开连接

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/connections/disconnect-by-app \
     -H "Content-Type: application/json" \
     -d '{"app": "com.example.testapp"}' | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `success`：布尔值，为 `true`
  - `disconnected_count`：数字（无匹配连接时为 `0`）
  - `message`：字符串（如 `No active connections found for app 'com.example.testapp'`）

---

### TC-ACA-25：按应用名称断开连接 — 空应用名被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/connections/disconnect-by-app \
     -H "Content-Type: application/json" \
     -d '{"app": ""}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "App name cannot be empty"

---

### TC-ACA-26：对沙箱配置 API 使用不支持的 HTTP 方法

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/config/sandbox -w "\n%{http_code}"
   ```
2. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/server -w "\n%{http_code}"
   ```
3. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/config/ui -w "\n%{http_code}"
   ```

**预期结果**：
- 三个请求均返回 HTTP 405（Method Not Allowed）

---

### TC-ACA-27：更新沙箱配置 — 组合更新多个子配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{
       "file": {"allowed_dirs": ["/tmp", "/var/data"], "max_bytes": 1048576},
       "net": {"enabled": true, "timeout_ms": 3000, "max_request_bytes": 524288, "max_response_bytes": 1048576},
       "limits": {"timeout_ms": 5000, "max_memory_bytes": 16777216, "max_decode_input_bytes": 2097152, "max_decompress_output_bytes": 4194304}
     }' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- `file.allowed_dirs` 包含 `/tmp` 和 `/var/data`
- `file.max_bytes` 为 `1048576`
- `net.enabled` 为 `true`
- `net.timeout_ms` 为 `3000`
- `net.max_request_bytes` 为 `524288`
- `net.max_response_bytes` 为 `1048576`
- `limits.timeout_ms` 为 `5000`
- `limits.max_memory_bytes` 为 `16777216`
- `limits.max_decode_input_bytes` 为 `2097152`
- `limits.max_decompress_output_bytes` 为 `4194304`

---

### TC-ACA-28：更新沙箱配置 — sandbox_dir 为空字符串被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{"file": {"sandbox_dir": "  "}}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "sandbox_dir cannot be empty"

---

### TC-ACA-29：更新沙箱配置 — limits.max_memory_bytes 为 0 被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d '{"limits": {"max_memory_bytes": 0}}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "limits.max_memory_bytes must be > 0"

---

### TC-ACA-30：更新沙箱配置 — 无效 JSON 被拒绝

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/sandbox \
     -H "Content-Type: application/json" \
     -d 'not valid json' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "Invalid JSON"

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
