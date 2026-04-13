# Scripts 管理 API 测试用例

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 服务启动成功后，确认管理端可访问：`http://127.0.0.1:8800/_bifrost/`

---

## 测试用例

### TC-ASC-01：获取脚本列表 — 初始状态

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/scripts | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象包含三个数组字段：
  - `request`（请求脚本列表）
  - `response`（响应脚本列表）
  - `decode`（解码脚本列表）
- `request` 和 `response` 数组为空
- `decode` 数组包含内置解码器 `utf8` 和 `default`

---

### TC-ASC-02：获取脚本列表 — 内置 decode 脚本结构

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/scripts | jq '.decode[] | select(.name == "utf8")'
   ```

**预期结果**：
- 返回对象包含字段：
  - `name` 为 `"utf8"`
  - `script_type` 为 `"decode"`
  - `description` 为 `"Built-in UTF-8 (lossy) decoder"`
  - `created_at` 为 `0`（内置脚本无创建时间）
  - `updated_at` 为 `0`

---

### TC-ASC-03：创建 request 类型脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/scripts/request/test-req-script \
     -H "Content-Type: application/json" \
     -d '{"content": "function onRequest(request, ctx) { return request; }", "description": "测试请求脚本"}' | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象包含：
  - `info.name` 为 `"test-req-script"`
  - `info.script_type` 为 `"request"`
  - `info.description` 为 `"测试请求脚本"`
  - `info.created_at` > 0（时间戳）
  - `info.updated_at` > 0（时间戳）
  - `content` 为提交的脚本内容

---

### TC-ASC-04：创建 response 类型脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/scripts/response/test-res-script \
     -H "Content-Type: application/json" \
     -d '{"content": "function onResponse(response, ctx) { return response; }"}' | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象，`info.name` 为 `"test-res-script"`，`info.script_type` 为 `"response"`

---

### TC-ASC-05：创建 decode 类型脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/scripts/decode/test-decode-script \
     -H "Content-Type: application/json" \
     -d '{"content": "function decode(phase, request, reqBody, response, resBody, ctx) { return { text: \"decoded\" }; }"}' | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象，`info.name` 为 `"test-decode-script"`，`info.script_type` 为 `"decode"`

---

### TC-ASC-06：创建脚本后列表中可见

**前置条件**：已通过 TC-ASC-03 创建了 test-req-script

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/scripts | jq '.request | length'
   ```

**预期结果**：
- `request` 数组长度 >= 1
- 数组中包含名为 `test-req-script` 的脚本

---

### TC-ASC-07：获取脚本详情

**前置条件**：已通过 TC-ASC-03 创建了 test-req-script

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/scripts/request/test-req-script | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象包含：
  - `info.name` 为 `"test-req-script"`
  - `info.script_type` 为 `"request"`
  - `content` 为之前保存的脚本内容 `"function onRequest(request, ctx) { return request; }"`

---

### TC-ASC-08：获取内置 decode 脚本详情（只读）

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/scripts/decode/utf8 | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象包含：
  - `info.name` 为 `"utf8"`
  - `info.script_type` 为 `"decode"`
  - `info.description` 为 `"Built-in UTF-8 (lossy) decoder"`
  - `content` 包含内置解码器说明文本

---

### TC-ASC-09：获取不存在的脚本返回 404

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8800/_bifrost/api/scripts/request/nonexistent-script
   ```

**预期结果**：
- HTTP 状态码为 404

---

### TC-ASC-10：更新已有脚本

**前置条件**：已通过 TC-ASC-03 创建了 test-req-script

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/scripts/request/test-req-script \
     -H "Content-Type: application/json" \
     -d '{"content": "function onRequest(request, ctx) { request.headers[\"X-Test\"] = \"1\"; return request; }"}' | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回的 `content` 为更新后的脚本内容

---

### TC-ASC-11：更新后获取详情验证内容已变更

**前置条件**：已通过 TC-ASC-10 更新了 test-req-script

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/scripts/request/test-req-script | jq -r '.content'
   ```

**预期结果**：
- 返回内容包含 `X-Test`，确认已更新为新版本

---

### TC-ASC-12：重命名脚本

**前置条件**：已创建 test-req-script

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/scripts/rename/request/test-req-script \
     -H "Content-Type: application/json" \
     -d '{"new_name": "renamed-req-script"}' | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回成功消息，包含 `"Script test-req-script renamed to renamed-req-script"`

---

### TC-ASC-13：重命名后旧名称不可访问

**前置条件**：已通过 TC-ASC-12 重命名脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8800/_bifrost/api/scripts/request/test-req-script
   ```

**预期结果**：
- HTTP 状态码为 404（旧名称已不存在）

---

### TC-ASC-14：重命名后新名称可访问

**前置条件**：已通过 TC-ASC-12 重命名脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/scripts/request/renamed-req-script | jq '.info.name'
   ```

**预期结果**：
- HTTP 状态码为 200
- `info.name` 为 `"renamed-req-script"`

---

### TC-ASC-15：删除脚本

**前置条件**：已存在 renamed-req-script

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/scripts/request/renamed-req-script | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回成功消息，包含 `"Script renamed-req-script deleted"`

---

### TC-ASC-16：删除后脚本不可访问

**前置条件**：已通过 TC-ASC-15 删除脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8800/_bifrost/api/scripts/request/renamed-req-script
   ```

**预期结果**：
- HTTP 状态码为 404

---

### TC-ASC-17：删除不存在的脚本返回 404

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X DELETE http://127.0.0.1:8800/_bifrost/api/scripts/request/nonexistent-script
   ```

**预期结果**：
- HTTP 状态码为 404

---

### TC-ASC-18：禁止修改内置 decode 脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X PUT http://127.0.0.1:8800/_bifrost/api/scripts/decode/utf8 \
     -H "Content-Type: application/json" \
     -d '{"content": "modified content"}'
   ```

**预期结果**：
- HTTP 状态码为 400
- 响应体包含 `"Built-in decode script is read-only"`

---

### TC-ASC-19：禁止删除内置 decode 脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X DELETE http://127.0.0.1:8800/_bifrost/api/scripts/decode/utf8
   ```

**预期结果**：
- HTTP 状态码为 400
- 响应体包含 `"Built-in decode script is read-only"`

---

### TC-ASC-20：禁止重命名内置 decode 脚本

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8800/_bifrost/api/scripts/rename/decode/utf8 \
     -H "Content-Type: application/json" \
     -d '{"new_name": "my-utf8"}'
   ```

**预期结果**：
- HTTP 状态码为 400
- 响应体包含 `"Built-in decode script is read-only"`

---

### TC-ASC-21：运行/测试脚本 — request 类型

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/scripts/test \
     -H "Content-Type: application/json" \
     -d '{
       "type": "request",
       "content": "function onRequest(request, ctx) { request.headers[\"X-Added\"] = \"hello\"; return request; }",
       "mock_request": {
         "url": "https://example.com/api",
         "method": "GET",
         "headers": {},
         "body": null
       }
     }' | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象包含脚本执行结果

---

### TC-ASC-22：运行/测试脚本 — response 类型

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/scripts/test \
     -H "Content-Type: application/json" \
     -d '{
       "type": "response",
       "content": "function onResponse(response, ctx) { response.headers[\"X-Modified\"] = \"true\"; return response; }",
       "mock_request": {
         "url": "https://example.com/api",
         "method": "GET"
       },
       "mock_response": {
         "status": 200,
         "headers": {},
         "body": "hello world"
       }
     }' | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象包含脚本执行结果

---

### TC-ASC-23：运行/测试脚本 — decode 类型

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/scripts/test \
     -H "Content-Type: application/json" \
     -d '{
       "type": "decode",
       "content": "function decode(phase, request, reqBody, response, resBody, ctx) { return { text: \"decoded output\" }; }",
       "mock_request": {
         "url": "https://example.com/api",
         "method": "POST",
         "body": "raw data"
       },
       "mock_response": {
         "status": 200,
         "body": "response data"
       }
     }' | jq .
   ```

**预期结果**：
- HTTP 状态码为 200
- 返回 JSON 对象包含 decode 脚本执行结果

---

### TC-ASC-24：脚本名称验证 — 空名称

**操作步骤**：
1. 执行命令（路径中脚本名为空）：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X PUT http://127.0.0.1:8800/_bifrost/api/scripts/request/ \
     -H "Content-Type: application/json" \
     -d '{"content": "function onRequest(r,c){return r;}"}'
   ```

**预期结果**：
- HTTP 状态码为 400
- 响应体包含错误信息（如 `"Invalid path: missing script name"` 或 `"empty script name"`）

---

### TC-ASC-25：脚本名称验证 — 名称过长（超过 128 字符）

**操作步骤**：
1. 执行命令：
   ```bash
   LONG_NAME=$(python3 -c "print('a' * 129)")
   curl -s -X PUT "http://127.0.0.1:8800/_bifrost/api/scripts/request/${LONG_NAME}" \
     -H "Content-Type: application/json" \
     -d '{"content": "function onRequest(r,c){return r;}"}' | jq .
   ```

**预期结果**：
- 返回错误响应
- 响应体包含 `"Script name cannot exceed 128 characters"`

---

### TC-ASC-26：脚本名称验证 — 包含特殊字符

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X PUT "http://127.0.0.1:8800/_bifrost/api/scripts/request/test%40script" \
     -H "Content-Type: application/json" \
     -d '{"content": "function onRequest(r,c){return r;}"}' | jq .
   ```

**预期结果**：
- 返回错误响应
- 响应体包含 `"Script name can only contain alphanumeric characters, hyphens, underscores, and slashes"`

---

### TC-ASC-27：脚本名称验证 — 包含路径穿越字符

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X PUT "http://127.0.0.1:8800/_bifrost/api/scripts/request/test..script" \
     -H "Content-Type: application/json" \
     -d '{"content": "function onRequest(r,c){return r;}"}' | jq .
   ```

**预期结果**：
- 返回错误响应
- 响应体包含 `"Script name cannot contain '..'"`

---

### TC-ASC-28：脚本名称验证 — 合法名称（字母数字、连字符、下划线）

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/scripts/request/my-test_script01 \
     -H "Content-Type: application/json" \
     -d '{"content": "function onRequest(r,c){return r;}"}' | jq '.info.name'
   ```

**预期结果**：
- HTTP 状态码为 200
- `info.name` 为 `"my-test_script01"`（名称包含连字符、下划线、数字均合法）

---

### TC-ASC-29：无效脚本类型返回 400

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8800/_bifrost/api/scripts/invalid-type/some-name
   ```

**预期结果**：
- HTTP 状态码为 400
- 响应体包含 `"Invalid script type"`

---

### TC-ASC-30：不支持的 HTTP 方法返回 405

**操作步骤**：
1. 执行命令：
   ```bash
   curl -s -o /dev/null -w "%{http_code}" -X PATCH http://127.0.0.1:8800/_bifrost/api/scripts/request/some-name
   ```

**预期结果**：
- HTTP 状态码为 405（Method Not Allowed）

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
```
