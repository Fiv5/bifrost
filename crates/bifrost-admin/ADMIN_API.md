# Bifrost Admin API 文档

本文档详细描述了 Bifrost 代理服务的管理端 API 接口。

## 概述

- **路由前缀**: `/_bifrost`
- **API 前缀**: `/_bifrost/api/`
- **公开接口前缀**: `/_bifrost/public/`
- **静态文件**: `/_bifrost/` 下的非 API 路径提供 Web UI 静态文件

### CORS 支持

所有 API 接口支持 CORS 跨域请求：

- `Access-Control-Allow-Origin: *`
- `Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS`
- `Access-Control-Allow-Headers: Content-Type, Authorization`

### 响应格式

所有 API 响应均为 JSON 格式，Content-Type 为 `application/json`。

**成功响应示例**:

```json
{
  "success": true,
  "message": "操作成功"
}
```

**错误响应示例**:

```json
{
  "error": "错误信息",
  "status": 400
}
```

---

## 1. Rules API - 规则管理

用于管理代理规则文件，支持规则的增删改查和启用/禁用。

### 1.1 获取规则列表

```
GET /api/rules
```

**响应**:

```json
[
  {
    "name": "default",
    "enabled": true,
    "rule_count": 10
  }
]
```

| 字段       | 类型    | 说明                           |
| ---------- | ------- | ------------------------------ |
| name       | string  | 规则文件名称                   |
| enabled    | boolean | 是否启用                       |
| rule_count | number  | 有效规则数量（排除空行和注释） |

**应用场景**: Web UI 展示规则列表，显示每个规则文件的状态。

### 1.2 创建规则

```
POST /api/rules
```

**请求体**:

```json
{
  "name": "my-rules",
  "content": "example.com proxy://127.0.0.1:8080",
  "enabled": true
}
```

| 字段    | 类型    | 必填 | 说明                |
| ------- | ------- | ---- | ------------------- |
| name    | string  | 是   | 规则文件名称        |
| content | string  | 是   | 规则内容            |
| enabled | boolean | 否   | 是否启用，默认 true |

**响应**: 成功响应

**应用场景**: 通过 API 或 Web UI 创建新的规则文件。

### 1.3 获取规则详情

```
GET /api/rules/{name}
```

**路径参数**:

- `name`: 规则文件名称（需 URL 编码）

**响应**:

```json
{
  "name": "default",
  "content": "example.com proxy://127.0.0.1:8080\n*.test.com file:///path/to/mock.json",
  "enabled": true
}
```

**应用场景**: 编辑规则时获取规则内容。

### 1.4 更新规则

```
PUT /api/rules/{name}
```

**路径参数**:

- `name`: 规则文件名称

**请求体**:

```json
{
  "content": "新的规则内容",
  "enabled": true
}
```

| 字段    | 类型    | 必填 | 说明     |
| ------- | ------- | ---- | -------- |
| content | string  | 否   | 规则内容 |
| enabled | boolean | 否   | 是否启用 |

**应用场景**: 修改规则内容或状态。

### 1.5 删除规则

```
DELETE /api/rules/{name}
```

**路径参数**:

- `name`: 规则文件名称

**应用场景**: 删除不再需要的规则文件。

### 1.6 启用规则

```
PUT /api/rules/{name}/enable
```

**应用场景**: 快速启用指定规则。

### 1.7 禁用规则

```
PUT /api/rules/{name}/disable
```

**应用场景**: 临时禁用规则而不删除。

---

## 2. Traffic API - 流量管理

用于查看和管理代理流量记录，支持 WebSocket 和 SSE 连接的帧数据查看。

### 2.1 获取流量列表

```
GET /api/traffic
```

**查询参数**:

| 参数                     | 类型    | 说明                   |
| ------------------------ | ------- | ---------------------- |
| method                   | string  | HTTP 方法过滤          |
| status                   | number  | 精确状态码             |
| status_min               | number  | 最小状态码             |
| status_max               | number  | 最大状态码             |
| url / url_contains       | string  | URL 包含               |
| host                     | string  | 主机名包含             |
| domain                   | string  | 域名包含               |
| path / path_contains     | string  | 路径包含               |
| content_type             | string  | 响应 Content-Type 包含 |
| request_content_type     | string  | 请求 Content-Type 包含 |
| protocol                 | string  | 协议 (http/https)      |
| has_rule_hit             | boolean | 是否命中规则           |
| header / header_contains | string  | 请求头或响应头包含     |
| client_ip                | string  | 客户端 IP 包含         |
| limit                    | number  | 返回数量限制，默认 100 |
| offset                   | number  | 分页偏移量             |

**响应**:

```json
{
  "total": 1000,
  "offset": 0,
  "limit": 100,
  "records": [
    {
      "id": "req-123",
      "timestamp": 1700000000000,
      "method": "GET",
      "url": "https://example.com/api",
      "status": 200,
      "content_type": "application/json",
      "request_size": 100,
      "response_size": 500,
      "duration_ms": 150,
      "host": "example.com",
      "path": "/api",
      "protocol": "https",
      "client_ip": "127.0.0.1",
      "has_rule_hit": true,
      "matched_rule_count": 1,
      "matched_protocols": ["proxy"],
      "is_websocket": false,
      "is_sse": false,
      "frame_count": 0
    }
  ]
}
```

**应用场景**: Web UI 流量列表展示，支持多维度过滤。

### 2.2 增量获取流量更新

```
GET /api/traffic/updates
```

**查询参数**:

- `after_id`: 上次获取的最后一条记录 ID
- `pending_ids`: 需要更新状态的记录 ID 列表（逗号分隔）
- `limit`: 返回数量限制
- 支持所有流量过滤参数

**响应**:

```json
{
  "new_records": [...],
  "updated_records": [...],
  "has_more": false,
  "server_total": 1000
}
```

**应用场景**: 实时刷新流量列表，只获取增量数据。

### 2.3 获取流量详情

```
GET /api/traffic/{id}
```

**响应**: 完整的 `TrafficRecord` 对象，包含请求头、响应头等详细信息。

```json
{
  "id": "req-123",
  "timestamp": 1700000000000,
  "method": "GET",
  "url": "https://example.com/api",
  "status": 200,
  "content_type": "application/json",
  "request_size": 100,
  "response_size": 500,
  "duration_ms": 150,
  "timing": {
    "dns_ms": 10,
    "connect_ms": 20,
    "tls_ms": 30,
    "send_ms": 5,
    "wait_ms": 80,
    "receive_ms": 5,
    "total_ms": 150
  },
  "request_headers": [["Content-Type", "application/json"]],
  "response_headers": [["Content-Type", "application/json"]],
  "client_ip": "127.0.0.1",
  "host": "example.com",
  "path": "/api",
  "protocol": "https",
  "is_tunnel": false,
  "has_rule_hit": true,
  "matched_rules": [
    {
      "pattern": "example.com",
      "protocol": "proxy",
      "value": "127.0.0.1:8080"
    }
  ]
}
```

### 2.4 清空流量记录

```
DELETE /api/traffic
```

**应用场景**: 清空所有流量记录，释放内存。

### 2.5 获取请求体

```
GET /api/traffic/{id}/request-body
```

**响应**:

```json
{
  "success": true,
  "data": "请求体内容（Base64 或文本）"
}
```

### 2.6 获取响应体

```
GET /api/traffic/{id}/response-body
```

**响应**:

```json
{
  "success": true,
  "data": "响应体内容"
}
```

### 2.7 获取 WebSocket/SSE 帧列表

```
GET /api/traffic/{id}/frames
```

**查询参数**:

- `after`: 获取指定帧 ID 之后的帧
- `limit`: 返回数量限制，默认 100

**响应**:

```json
{
  "frames": [
    {
      "frame_id": 1,
      "direction": "send",
      "frame_type": "text",
      "timestamp": 1700000000000,
      "payload_preview": "Hello...",
      "payload_size": 100
    }
  ],
  "socket_status": {
    "is_open": true,
    "send_count": 10,
    "receive_count": 20,
    "send_bytes": 1000,
    "receive_bytes": 2000,
    "frame_count": 30
  },
  "last_frame_id": 30,
  "has_more": false,
  "is_monitored": true
}
```

**应用场景**: 查看 WebSocket 或 SSE 连接的消息帧。

### 2.8 获取帧详情

```
GET /api/traffic/{id}/frames/{frame_id}
```

**响应**: 包含完整 payload 的帧详情。

### 2.9 订阅帧流 (SSE)

```
GET /api/traffic/{id}/frames/stream
```

**响应**: Server-Sent Events 流，实时推送新帧。

```
Content-Type: text/event-stream

data: {"frame_id":1,"direction":"receive","frame_type":"text",...}

data: {"frame_id":2,"direction":"send","frame_type":"text",...}
```

**应用场景**: 实时监控 WebSocket 连接的消息。

### 2.10 取消订阅帧流

```
DELETE /api/traffic/{id}/frames/unsubscribe
```

---

## 3. Metrics API - 指标监控

用于获取代理服务的性能指标和统计数据。

### 3.1 获取当前指标

```
GET /api/metrics
```

**响应**: 当前时刻的指标快照。

**应用场景**: 实时监控面板展示。

### 3.2 获取历史指标

```
GET /api/metrics/history
```

**查询参数**:

- `limit`: 返回历史记录数量

**响应**: 历史指标数组。

**应用场景**: 绘制指标趋势图。

---

## 4. System API - 系统信息

用于获取代理服务的系统状态和配置信息。

### 4.1 获取系统信息

```
GET /api/system
```

**响应**: 系统基本信息，包括启动时间、版本等。

### 4.2 获取系统概览

```
GET /api/system/overview
```

**响应**:

```json
{
  "system": {
    "uptime_seconds": 3600,
    "version": "0.1.0"
  },
  "metrics": {...},
  "rules": {
    "total": 5,
    "enabled": 3
  },
  "traffic": {
    "recorded": 1000
  },
  "server": {
    "port": 9900,
    "admin_url": "http://127.0.0.1:9900/_bifrost/"
  },
  "pending_authorizations": 0
}
```

**应用场景**: Web UI 首页仪表盘展示。

---

## 5. Values API - 变量管理

用于管理可在规则中引用的变量值。

### 5.1 获取变量列表

```
GET /api/values
```

**响应**:

```json
{
  "values": [
    {
      "name": "API_HOST",
      "value": "api.example.com"
    }
  ],
  "total": 1
}
```

### 5.2 创建变量

```
POST /api/values
```

**请求体**:

```json
{
  "name": "API_HOST",
  "value": "api.example.com"
}
```

### 5.3 获取变量

```
GET /api/values/{name}
```

### 5.4 更新变量

```
PUT /api/values/{name}
```

**请求体**:

```json
{
  "value": "new-value"
}
```

### 5.5 删除变量

```
DELETE /api/values/{name}
```

**应用场景**: 管理规则中使用的动态变量，如环境切换、API 地址配置等。

---

## 6. Whitelist API - 访问控制

用于管理客户端 IP 白名单和访问授权。

### 6.1 获取白名单

```
GET /api/whitelist
```

**响应**:

```json
{
  "mode": "whitelist",
  "allow_lan": true,
  "whitelist": ["192.168.1.0/24", "10.0.0.1"],
  "temporary_whitelist": ["192.168.1.100"]
}
```

| 字段                | 类型     | 说明                                                 |
| ------------------- | -------- | ---------------------------------------------------- |
| mode                | string   | 访问模式: allow_all/local_only/whitelist/interactive |
| allow_lan           | boolean  | 是否允许局域网访问                                   |
| whitelist           | string[] | 永久白名单（IP 或 CIDR）                             |
| temporary_whitelist | string[] | 临时白名单（会话级）                                 |

### 6.2 添加白名单

```
POST /api/whitelist
```

**请求体**:

```json
{
  "ip_or_cidr": "192.168.1.0/24"
}
```

### 6.3 移除白名单

```
DELETE /api/whitelist
```

**请求体**:

```json
{
  "ip_or_cidr": "192.168.1.0/24"
}
```

### 6.4 获取访问模式

```
GET /api/whitelist/mode
```

**响应**:

```json
{
  "mode": "whitelist"
}
```

### 6.5 设置访问模式

```
PUT /api/whitelist/mode
```

**请求体**:

```json
{
  "mode": "whitelist"
}
```

**模式说明**:

- `open`: 允许所有连接
- `whitelist`: 仅允许白名单 IP
- `strict`: 严格模式

### 6.6 获取 LAN 访问设置

```
GET /api/whitelist/allow-lan
```

### 6.7 设置 LAN 访问

```
PUT /api/whitelist/allow-lan
```

**请求体**:

```json
{
  "allow_lan": true
}
```

### 6.8 添加临时白名单

```
POST /api/whitelist/temporary
```

**请求体**:

```json
{
  "ip": "192.168.1.100"
}
```

**应用场景**: 临时授权某个 IP 访问，重启后失效。

### 6.9 移除临时白名单

```
DELETE /api/whitelist/temporary
```

### 6.10 获取待授权列表

```
GET /api/whitelist/pending
```

**响应**: 等待授权的 IP 列表。

**应用场景**: 当有新 IP 尝试连接时，可以在 Web UI 中审批。

### 6.11 批准授权

```
POST /api/whitelist/pending/approve
```

**请求体**:

```json
{
  "ip": "192.168.1.100"
}
```

### 6.12 拒绝授权

```
POST /api/whitelist/pending/reject
```

### 6.13 清空待授权列表

```
DELETE /api/whitelist/pending
```

---

## 7. Cert API - 证书管理

用于 HTTPS 代理的 CA 证书下载和管理。

### 7.1 获取证书信息

```
GET /api/cert/info
```

**响应**:

```json
{
  "available": true,
  "local_ips": ["192.168.1.100", "10.0.0.1"],
  "download_urls": [
    "http://192.168.1.100:9900/_bifrost/public/cert",
    "http://10.0.0.1:9900/_bifrost/public/cert"
  ],
  "qrcode_urls": ["http://192.168.1.100:9900/_bifrost/public/cert/qrcode"]
}
```

**应用场景**: Web UI 展示证书下载链接和二维码。

### 7.2 下载 CA 证书

```
GET /public/cert
```

**响应**:

- Content-Type: `application/x-pem-file`
- Content-Disposition: `attachment; filename="bifrost-ca.crt"`

**应用场景**: 移动设备或浏览器下载并安装 CA 证书以支持 HTTPS 代理。

### 7.3 获取证书下载二维码

```
GET /public/cert/qrcode
```

**响应**: SVG 格式的二维码图片。

**应用场景**: 移动设备扫码下载证书。

---

## 8. Proxy API - 系统代理

用于管理操作系统级别的代理设置。

### 8.1 获取系统代理状态

```
GET /api/proxy/system
```

**响应**:

```json
{
  "supported": true,
  "enabled": true,
  "host": "127.0.0.1",
  "port": 9900,
  "bypass": "localhost,127.0.0.1,::1,*.local"
}
```

### 8.2 设置系统代理

```
PUT /api/proxy/system
```

**请求体**:

```json
{
  "enabled": true,
  "bypass": "localhost,127.0.0.1,::1,*.local"
}
```

**注意**: 在某些系统上可能需要管理员权限。

**错误响应**（需要管理员权限时）:

```json
{
  "error": "requires_admin",
  "message": "System proxy requires administrator privileges..."
}
```

### 8.3 获取系统代理支持状态

```
GET /api/proxy/system/support
```

**响应**:

```json
{
  "supported": true,
  "platform": "macOS"
}
```

---

## 9. WebSocket Connections API

### 9.1 获取 WebSocket 连接列表

```
GET /api/websocket/connections
```

**响应**:

```json
{
  "connections": [
    {
      "id": "req-123",
      "frame_count": 100,
      "socket_status": {
        "is_open": true,
        "send_count": 50,
        "receive_count": 50,
        "send_bytes": 5000,
        "receive_bytes": 10000,
        "frame_count": 100
      },
      "is_monitored": true
    }
  ],
  "total": 1
}
```

**应用场景**: 查看当前活跃的 WebSocket 连接。

---

## 数据结构参考

### TrafficRecord

完整的流量记录，包含请求和响应的所有信息。

| 字段             | 类型                | 说明              |
| ---------------- | ------------------- | ----------------- |
| id               | string              | 唯一标识          |
| timestamp        | number              | 时间戳（毫秒）    |
| method           | string              | HTTP 方法         |
| url              | string              | 完整 URL          |
| status           | number              | HTTP 状态码       |
| content_type     | string?             | 响应 Content-Type |
| request_size     | number              | 请求大小（字节）  |
| response_size    | number              | 响应大小（字节）  |
| duration_ms      | number              | 总耗时（毫秒）    |
| timing           | RequestTiming?      | 详细耗时          |
| request_headers  | [string, string][]? | 请求头            |
| response_headers | [string, string][]? | 响应头            |
| client_ip        | string              | 客户端 IP         |
| host             | string              | 主机名            |
| path             | string              | 路径              |
| protocol         | string              | 协议 (http/https) |
| is_tunnel        | boolean             | 是否为隧道连接    |
| has_rule_hit     | boolean             | 是否命中规则      |
| matched_rules    | MatchedRule[]?      | 命中的规则列表    |
| is_websocket     | boolean             | 是否为 WebSocket  |
| is_sse           | boolean             | 是否为 SSE        |
| socket_status    | SocketStatus?       | Socket 状态       |
| frame_count      | number              | 帧数量            |

### RequestTiming

请求各阶段耗时。

| 字段       | 类型    | 说明         |
| ---------- | ------- | ------------ |
| dns_ms     | number? | DNS 解析耗时 |
| connect_ms | number? | TCP 连接耗时 |
| tls_ms     | number? | TLS 握手耗时 |
| send_ms    | number? | 发送请求耗时 |
| wait_ms    | number? | 等待响应耗时 |
| receive_ms | number? | 接收响应耗时 |
| total_ms   | number  | 总耗时       |

### MatchedRule

命中的规则信息。

| 字段     | 类型   | 说明     |
| -------- | ------ | -------- |
| pattern  | string | 匹配模式 |
| protocol | string | 规则协议 |
| value    | string | 规则值   |

### SocketStatus

WebSocket/SSE 连接状态。

| 字段          | 类型    | 说明         |
| ------------- | ------- | ------------ |
| is_open       | boolean | 连接是否打开 |
| send_count    | number  | 发送消息数   |
| receive_count | number  | 接收消息数   |
| send_bytes    | number  | 发送字节数   |
| receive_bytes | number  | 接收字节数   |
| frame_count   | number  | 总帧数       |
| close_code    | number? | 关闭码       |
| close_reason  | string? | 关闭原因     |

### FrameDirection

帧方向枚举。

| 值      | 说明 |
| ------- | ---- |
| send    | 发送 |
| receive | 接收 |

### FrameType

帧类型枚举。

| 值           | 说明     |
| ------------ | -------- |
| text         | 文本帧   |
| binary       | 二进制帧 |
| ping         | Ping 帧  |
| pong         | Pong 帧  |
| close        | 关闭帧   |
| continuation | 续帧     |
| sse          | SSE 事件 |
