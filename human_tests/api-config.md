# 配置管理 Admin API 测试用例

## 功能模块说明

验证 Bifrost 配置管理相关的 Admin API 接口，包括全量配置查询、TLS 拦截配置管理、性能配置管理、缓存清理、连接断开等功能。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 未被占用
3. 服务启动成功后再执行测试用例

---

## 测试用例

### TC-ACF-01：获取全量配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下顶层字段：
  - `server`：对象，包含 `timeout_secs`、`http1_max_header_size`、`http2_max_header_list_size`、`websocket_handshake_max_header_size`
  - `tls`：对象，包含 TLS 拦截相关配置
  - `port`：数字，值为 `8800`
  - `host`：字符串，值为 `127.0.0.1`
- `tls` 对象包含：`enable_tls_interception`、`intercept_exclude`、`intercept_include`、`app_intercept_exclude`、`app_intercept_include`、`ip_intercept_exclude`、`ip_intercept_include`、`unsafe_ssl`、`disconnect_on_config_change`

---

### TC-ACF-02：获取 TLS 配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/tls | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `enable_tls_interception`：布尔值
  - `intercept_exclude`：字符串数组，域名排除列表
  - `intercept_include`：字符串数组，域名包含列表
  - `app_intercept_exclude`：字符串数组，应用排除列表
  - `app_intercept_include`：字符串数组，应用包含列表
  - `ip_intercept_exclude`：字符串数组，IP 排除列表
  - `ip_intercept_include`：字符串数组，IP 包含列表
  - `unsafe_ssl`：布尔值（因启动参数 `--unsafe-ssl`，此处为 `true`）
  - `disconnect_on_config_change`：布尔值

---

### TC-ACF-03：更新 TLS 配置 — 启用 TLS 拦截

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/tls \
     -H "Content-Type: application/json" \
     -d '{"enable_tls_interception": true}' | jq .
   ```
2. 验证配置已生效：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/tls | jq .enable_tls_interception
   ```

**预期结果**：
- PUT 请求返回 HTTP 200，响应为更新后的 TLS 配置 JSON
- `enable_tls_interception` 值为 `true`
- GET 请求验证配置已持久化，`enable_tls_interception` 为 `true`

---

### TC-ACF-04：更新 TLS 配置 — 设置域名排除列表

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/tls \
     -H "Content-Type: application/json" \
     -d '{"intercept_exclude": ["*.apple.com", "*.icloud.com"]}' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- `intercept_exclude` 数组包含 `*.apple.com` 和 `*.icloud.com`
- 其他字段保持不变

---

### TC-ACF-05：更新 TLS 配置 — 设置域名包含列表

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/tls \
     -H "Content-Type: application/json" \
     -d '{"intercept_include": ["api.example.com", "*.test.dev"]}' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- `intercept_include` 数组包含 `api.example.com` 和 `*.test.dev`

---

### TC-ACF-06：更新 TLS 配置 — 设置应用排除/包含列表

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/tls \
     -H "Content-Type: application/json" \
     -d '{"app_intercept_exclude": ["com.apple.Safari"], "app_intercept_include": ["com.google.Chrome"]}' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- `app_intercept_exclude` 包含 `com.apple.Safari`
- `app_intercept_include` 包含 `com.google.Chrome`

---

### TC-ACF-07：更新 TLS 配置 — 设置 unsafe_ssl 和 disconnect_on_config_change

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/tls \
     -H "Content-Type: application/json" \
     -d '{"unsafe_ssl": true, "disconnect_on_config_change": true}' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- `unsafe_ssl` 为 `true`
- `disconnect_on_config_change` 为 `true`

---

### TC-ACF-08：更新 TLS 配置 — 组合更新多个字段

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/tls \
     -H "Content-Type: application/json" \
     -d '{
       "enable_tls_interception": true,
       "intercept_exclude": ["*.apple.com"],
       "intercept_include": [],
       "disconnect_on_config_change": false
     }' | jq .
   ```

**预期结果**：
- 返回 HTTP 200
- `enable_tls_interception` 为 `true`
- `intercept_exclude` 为 `["*.apple.com"]`
- `intercept_include` 为空数组 `[]`
- `disconnect_on_config_change` 为 `false`

---

### TC-ACF-09：获取性能配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/performance | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `traffic`：对象，包含：
    - `max_records`：数字，最大记录数
    - `max_db_size_bytes`：数字，数据库最大大小
    - `max_body_memory_size`：数字，Body 内存缓存大小
    - `max_body_buffer_size`：数字，Body 缓冲区大小
    - `max_body_probe_size`：数字，Body 探测大小
    - `binary_traffic_performance_mode`：布尔值
    - `inject_bifrost_badge`：布尔值
    - `file_retention_days`：数字，文件保留天数
    - `sse_stream_flush_bytes`：数字
    - `sse_stream_flush_interval_ms`：数字
    - `ws_payload_flush_bytes`：数字
    - `ws_payload_flush_interval_ms`：数字
    - `ws_payload_max_open_files`：数字
  - `body_store_stats`：对象或 null，Body 存储统计
  - `frame_store_stats`：对象或 null，Frame 存储统计
  - `ws_payload_store_stats`：对象或 null，WebSocket Payload 存储统计
  - `resource_alerts`：对象，资源告警信息

---

### TC-ACF-10：更新性能配置 — 修改 max_records

**操作步骤**：
1. 先获取当前 max_records 值：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/performance | jq .traffic.max_records
   ```
2. 更新 max_records：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/performance \
     -H "Content-Type: application/json" \
     -d '{"max_records": 5000}' | jq .traffic.max_records
   ```
3. 验证更新已生效：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/config/performance | jq .traffic.max_records
   ```

**预期结果**：
- PUT 请求返回 HTTP 200
- 返回的 `traffic.max_records` 值为 `5000`
- GET 请求验证配置已持久化

---

### TC-ACF-11：更新性能配置 — 修改 max_db_size_bytes

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/performance \
     -H "Content-Type: application/json" \
     -d '{"max_db_size_bytes": 1073741824}' | jq .traffic.max_db_size_bytes
   ```

**预期结果**：
- 返回 HTTP 200
- `traffic.max_db_size_bytes` 为 `1073741824`（1 GB）

---

### TC-ACF-12：更新性能配置 — max_records 超出范围

**操作步骤**：
1. 执行以下命令（设置一个超大值）：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/performance \
     -H "Content-Type: application/json" \
     -d '{"max_records": 99999999}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息提示 max_records 必须在允许范围内

---

### TC-ACF-13：更新性能配置 — file_retention_days 超出限制

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/performance \
     -H "Content-Type: application/json" \
     -d '{"file_retention_days": 30}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息提示 file_retention_days 不能超过 7 天

---

### TC-ACF-14：更新性能配置 — 组合更新多个字段

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/performance \
     -H "Content-Type: application/json" \
     -d '{
       "max_records": 3000,
       "max_body_memory_size": 1048576,
       "binary_traffic_performance_mode": false,
       "inject_bifrost_badge": false,
       "file_retention_days": 3
     }' | jq .traffic
   ```

**预期结果**：
- 返回 HTTP 200
- `max_records` 为 `3000`
- `max_body_memory_size` 为 `1048576`
- `binary_traffic_performance_mode` 为 `false`
- `inject_bifrost_badge` 为 `false`
- `file_retention_days` 为 `3`

---

### TC-ACF-15：清理缓存

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/config/performance/clear-cache | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `body_cache_removed`：数字，清理的 Body 缓存文件数
  - `traffic_cache_removed`：数字，清理的流量记录数
  - `frame_cache_removed`：数字，清理的 Frame 文件数
  - `ws_payload_cache_removed`：数字，清理的 WebSocket Payload 文件数
  - `message`：字符串，清理结果描述

---

### TC-ACF-16：按域名断开连接

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/connections/disconnect \
     -H "Content-Type: application/json" \
     -d '{"domain": "example.com"}' | jq .
   ```

**预期结果**：
- 返回 HTTP 200，响应为 JSON 格式
- JSON 包含以下字段：
  - `success`：布尔值，为 `true`
  - `disconnected_count`：数字，断开的连接数（无活跃连接时为 `0`）
  - `message`：字符串，描述断开结果（如 `No active connections found matching 'example.com'`）

---

### TC-ACF-17：按域名断开连接 — 空域名

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/connections/disconnect \
     -H "Content-Type: application/json" \
     -d '{"domain": ""}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息提示 "Domain cannot be empty"

---

### TC-ACF-18：按域名断开连接 — 无效请求体

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/connections/disconnect \
     -H "Content-Type: application/json" \
     -d '{"invalid": "field"}' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 响应包含 JSON 解析错误信息

---

### TC-ACF-19：对配置 API 使用不支持的 HTTP 方法

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/config -w "\n%{http_code}"
   ```
2. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/tls -w "\n%{http_code}"
   ```
3. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/config/performance -w "\n%{http_code}"
   ```

**预期结果**：
- 三个请求均返回 HTTP 405（Method Not Allowed）

---

### TC-ACF-20：更新 TLS 配置 — 无效 JSON 请求体

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/tls \
     -H "Content-Type: application/json" \
     -d 'not valid json' -w "\n%{http_code}"
   ```

**预期结果**：
- 返回 HTTP 400（Bad Request）
- 错误信息包含 "Invalid JSON"

---

### TC-ACF-21：更新性能配置 — 无效 JSON 请求体

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/config/performance \
     -H "Content-Type: application/json" \
     -d '{broken json' -w "\n%{http_code}"
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
