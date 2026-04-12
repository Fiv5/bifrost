# Search Admin API 测试用例

## 功能模块说明

Search Admin API 提供全文搜索功能，支持在已记录的流量中按关键词搜索，可指定搜索范围（URL、请求头、响应头、请求体、响应体、WebSocket 消息、SSE 事件）以及多种过滤条件（协议、状态码、Content-Type、域名等）。API 路径前缀为 `/_bifrost/api/search`，支持两种模式：同步搜索（POST `/api/search`）和流式搜索（POST `/api/search/stream`，返回 SSE）。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 未被其他程序占用
3. 通过代理产生一些测试流量以供搜索：
   ```bash
   curl -x http://127.0.0.1:8800 http://httpbin.org/get
   curl -x http://127.0.0.1:8800 -X POST http://httpbin.org/post -d '{"test":"search_keyword_abc"}'
   curl -x http://127.0.0.1:8800 http://httpbin.org/status/404
   curl -x http://127.0.0.1:8800 http://httpbin.org/headers -H "X-Custom: bifrost_test_header"
   ```

---

## 测试用例

### TC-ASE-01：按关键词搜索（全局范围）

**操作步骤**：
1. 使用 curl 搜索关键词 `httpbin`：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{"keyword":"httpbin"}'
   ```

**预期结果**：
- 返回 HTTP 200
- 响应体包含：
  - `results`：匹配结果数组，每个元素包含 `record`（流量摘要）和 `matches`（匹配位置数组）
  - `total_searched`：已扫描记录数
  - `total_matched`：匹配记录数（>= 1）
  - `has_more`：布尔值
  - `search_id`：搜索 ID 字符串
- `results` 中每条记录的 `record.h`（host）或 `record.p`（path）中包含 `httpbin`

---

### TC-ASE-02：搜索请求体内容

**前置条件**：已通过前置步骤发送过包含 `search_keyword_abc` 的 POST 请求

**操作步骤**：
1. 使用 curl 搜索请求体中的关键词：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "search_keyword_abc",
       "scope": {"all": false, "request_body": true}
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- `total_matched` >= 1
- 匹配结果中包含之前发送的 POST 请求
- `matches` 数组中有 `field` 为请求体相关的匹配

---

### TC-ASE-03：仅搜索 URL 范围

**操作步骤**：
1. 使用 curl 仅在 URL 中搜索：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "/get",
       "scope": {"all": false, "url": true}
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- `total_matched` >= 1
- 所有匹配记录的 URL 中包含 `/get`
- `matches` 数组中 `field` 指向 URL 匹配

---

### TC-ASE-04：仅搜索请求头范围

**前置条件**：已通过前置步骤发送过包含 `X-Custom: bifrost_test_header` 头的请求

**操作步骤**：
1. 使用 curl 仅在请求头中搜索：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "bifrost_test_header",
       "scope": {"all": false, "request_headers": true}
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- `total_matched` >= 1
- 匹配结果中包含带有 `X-Custom` 头的请求

---

### TC-ASE-05：仅搜索响应头范围

**操作步骤**：
1. 使用 curl 仅在响应头中搜索：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "application/json",
       "scope": {"all": false, "response_headers": true}
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- `total_matched` >= 1（httpbin 的 GET/POST 响应通常包含 `application/json` 内容类型）
- `matches` 数组中 `field` 指向响应头匹配

---

### TC-ASE-06：仅搜索响应体范围

**操作步骤**：
1. 使用 curl 仅在响应体中搜索：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "httpbin.org",
       "scope": {"all": false, "response_body": true}
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- `total_matched` >= 1（httpbin 的响应体中通常包含 `httpbin.org`）

---

### TC-ASE-07：使用状态码过滤条件

**前置条件**：已通过前置步骤产生了状态码 404 的流量

**操作步骤**：
1. 使用 curl 搜索时过滤 4xx 状态码：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "httpbin",
       "filters": {"status_ranges": ["4xx"]}
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- `total_matched` >= 1
- 所有匹配结果的 `record.s`（status）在 400-499 范围内

---

### TC-ASE-08：使用域名过滤条件

**操作步骤**：
1. 使用 curl 搜索时过滤特定域名：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "",
       "filters": {"domains": ["httpbin.org"]}
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- `total_matched` >= 1
- 所有匹配结果的 `record.h`（host）为 `httpbin.org`

---

### TC-ASE-09：使用条件过滤（method = POST）

**操作步骤**：
1. 使用 curl 搜索时过滤 POST 方法：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "httpbin",
       "filters": {
         "conditions": [{"field":"method","operator":"eq","value":"POST"}]
       }
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- 所有匹配结果的 `record.m`（method）为 `"POST"`

---

### TC-ASE-10：组合多个过滤条件

**操作步骤**：
1. 使用 curl 同时使用关键词 + 域名过滤 + 状态码过滤：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "get",
       "scope": {"all": false, "url": true},
       "filters": {
         "domains": ["httpbin.org"],
         "status_ranges": ["2xx"]
       }
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- 所有匹配结果同时满足：URL 包含 `get`、域名为 `httpbin.org`、状态码在 200-299 范围

---

### TC-ASE-11：搜索无匹配结果

**操作步骤**：
1. 使用 curl 搜索一个不存在的关键词：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{"keyword":"zzz_nonexistent_keyword_12345"}'
   ```

**预期结果**：
- 返回 HTTP 200
- `results` 为空数组
- `total_matched` 为 `0`
- `has_more` 为 `false`

---

### TC-ASE-12：空关键词且无过滤条件返回 400

**操作步骤**：
1. 使用 curl 发送空搜索请求：
   ```bash
   curl -s -w "\n%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{"keyword":""}'
   ```

**预期结果**：
- 返回 HTTP 400
- 响应体包含错误信息 "Search keyword cannot be empty without any filters"

---

### TC-ASE-13：使用 limit 参数限制返回结果数

**操作步骤**：
1. 使用 curl 搜索并限制返回 1 条：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{"keyword":"httpbin","limit":1}'
   ```

**预期结果**：
- 返回 HTTP 200
- `results` 数组长度 <= 1
- 如果总匹配数 > 1，`has_more` 为 `true`
- `next_cursor` 非空，可用于翻页

---

### TC-ASE-14：使用 cursor 参数进行分页搜索

**前置条件**：已通过 TC-ASE-13 搜索，获取到 `next_cursor` 值

**操作步骤**：
1. 使用 curl 传入 cursor 进行下一页搜索：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{"keyword":"httpbin","limit":1,"cursor":{NEXT_CURSOR}}'
   ```

**预期结果**：
- 返回 HTTP 200
- `results` 返回第二页的搜索结果
- 结果不与第一页重复

---

### TC-ASE-15：流式搜索（SSE 模式）

**操作步骤**：
1. 使用 curl 发起流式搜索：
   ```bash
   curl -s -N -X POST http://127.0.0.1:8800/_bifrost/api/search/stream \
     -H "Content-Type: application/json" \
     -d '{"keyword":"httpbin"}'
   ```

**预期结果**：
- 返回 HTTP 200
- Content-Type 为 `text/event-stream`
- 收到多个 SSE 事件：
  - `event: result`：每条匹配记录作为单独事件推送，data 为 JSON 格式的 `SearchResultItem`
  - `event: progress`：搜索进度更新，包含 `total_searched`、`total_matched`、`has_more_hint` 等字段
  - `event: done`：搜索完成事件，包含最终的 `total_searched`、`total_matched`、`has_more`、`search_id`

---

### TC-ASE-16：使用 has_rule_hit 过滤命中规则的流量

**操作步骤**：
1. 使用 curl 搜索命中过规则的流量：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/search \
     -H "Content-Type: application/json" \
     -d '{
       "keyword": "",
       "filters": {"has_rule_hit": true, "domains": ["httpbin.org"]}
     }'
   ```

**预期结果**：
- 返回 HTTP 200
- 如果有匹配规则的流量，`results` 非空，且所有结果的流量都命中过规则
- 如果没有命中规则的流量，`results` 为空数组

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
