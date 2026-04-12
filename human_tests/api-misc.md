# 杂项 Admin API 测试用例

## 功能模块说明

验证 Bifrost 杂项 Admin API 的完整功能，包括语法信息查询、应用图标获取、WebSocket 连接列表、审计日志、Bifrost 文件导入导出、同步管理等接口。

## 前置条件

1. 启动 Bifrost 服务（使用临时数据目录避免污染正式环境）：
   ```bash
   BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
   ```
2. 确保端口 8800 可用且服务已正常启动
3. 部分用例需要预先创建规则、脚本、Values 等数据

---

## 测试用例

### TC-AMS-01：获取语法信息（基本结构验证）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/syntax | jq 'keys'
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 对象，顶层键包含：`protocols`、`template_variables`、`patterns`、`protocol_aliases`、`scripts`、`filter_specs`

---

### TC-AMS-02：语法信息 — protocols 字段非空

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/syntax | jq '.protocols | length'
   ```

**预期结果**：
- 返回值大于 0（至少包含 HTTP、HTTPS 等协议）

---

### TC-AMS-03：语法信息 — scripts 字段包含三类脚本列表

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/syntax | jq '.scripts | keys'
   ```

**预期结果**：
- `scripts` 对象包含三个键：`request_scripts`、`response_scripts`、`decode_scripts`
- `decode_scripts` 至少包含内置的 `utf8` 和 `default` 两个解码脚本

**验证步骤**：
1. 确认内置解码脚本存在：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/syntax | jq '.scripts.decode_scripts[] | select(.name == "utf8" or .name == "default")'
   ```
2. 应输出两条记录，分别对应 `utf8` 和 `default`

---

### TC-AMS-04：语法信息 — filter_specs 字段非空

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/syntax | jq '.filter_specs | length'
   ```

**预期结果**：
- 返回值大于 0（至少包含一个过滤器规格）

---

### TC-AMS-05：语法信息 — protocol_aliases 为对象

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/syntax | jq '.protocol_aliases | type'
   ```

**预期结果**：
- 返回 `"object"`
- `protocol_aliases` 为键值对映射（可以为空对象 `{}`，但类型必须为 object）

---

### TC-AMS-06：语法 API 不支持 POST 方法

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/syntax -w "\n%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 405（Method Not Allowed）

---

### TC-AMS-07：获取已知应用图标（Safari）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -sI http://127.0.0.1:8800/_bifrost/api/app-icon/Safari
   ```

**预期结果**：
- HTTP 状态码 200
- 响应头 `Content-Type` 为 `image/png`
- 响应头 `Cache-Control` 包含 `public, max-age=86400`
- 响应头 `Access-Control-Allow-Origin` 为 `*`

---

### TC-AMS-08：获取不存在的应用图标（返回 404）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -w "\n%{http_code}" http://127.0.0.1:8800/_bifrost/api/app-icon/nonexistent_app_xyz_12345
   ```

**预期结果**：
- HTTP 状态码 404
- 返回 JSON 包含错误信息 `"Icon not found"`

---

### TC-AMS-09：应用图标 API — 缺少应用名称返回 400

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -w "\n%{http_code}" http://127.0.0.1:8800/_bifrost/api/app-icon/
   ```

**预期结果**：
- HTTP 状态码 400
- 返回 JSON 包含错误信息 `"App name is required"`

---

### TC-AMS-10：应用图标 API 不支持 POST 方法

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/app-icon/Safari -w "\n%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 405（Method Not Allowed）

---

### TC-AMS-11：获取 WebSocket 连接列表（初始为空）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/websocket/connections | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 对象包含：
  - `connections`：数组（初始状态为空数组 `[]`）
  - `total`：整数（初始为 `0`）

---

### TC-AMS-12：WebSocket 连接列表 — connections 元素结构验证

**前置条件**：存在活跃的 WebSocket 代理连接（可通过代理访问一个 WebSocket 服务产生）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/websocket/connections | jq '.connections[0] | keys'
   ```

**预期结果**：
- 如果有活跃连接，每个连接对象包含字段：`id`、`frame_count`、`socket_status`、`is_monitored`
- `id`：字符串
- `frame_count`：整数
- `socket_status`：字符串
- `is_monitored`：布尔值

---

### TC-AMS-13：获取审计日志（默认分页）

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/admin/audit | jq 'keys'
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 对象包含字段：`total`、`items`、`limit`、`offset`
- `total`：整数（≥ 0）
- `items`：数组
- `limit`：默认值 `50`
- `offset`：默认值 `0`

---

### TC-AMS-14：审计日志 — 自定义分页参数

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s "http://127.0.0.1:8800/_bifrost/api/admin/audit?limit=10&offset=0" | jq '{limit, offset, total}'
   ```

**预期结果**：
- `limit` 为 `10`
- `offset` 为 `0`
- `total` 为整数（≥ 0）
- `items` 数组长度 ≤ 10

---

### TC-AMS-15：审计日志 — limit 上限为 500

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s "http://127.0.0.1:8800/_bifrost/api/admin/audit?limit=9999" | jq '.limit'
   ```

**预期结果**：
- 返回 `limit` 为 `500`（超过上限自动截断为 500）

---

### TC-AMS-16：审计日志 — limit=0 回退为默认值 50

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s "http://127.0.0.1:8800/_bifrost/api/admin/audit?limit=0" | jq '.limit'
   ```

**预期结果**：
- 返回 `limit` 为 `50`（limit 为 0 时回退为默认值）

---

### TC-AMS-17：审计日志 — 不支持 POST 方法

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/admin/audit -w "\n%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 405（Method Not Allowed）

---

### TC-AMS-18：Bifrost 文件导出规则

**前置条件**：已创建至少一个规则文件

**操作步骤**：
1. 先创建一个测试规则：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/rules \
     -H "Content-Type: application/json" \
     -d '{"name": "export-test", "content": "example.com 127.0.0.1:3000", "enabled": true}'
   ```
2. 导出规则为 .bifrost 文件：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/bifrost-file/export/rules \
     -H "Content-Type: application/json" \
     -d '{"rule_names": ["export-test"]}' -o /tmp/bifrost-export-rules.bifrost -w "%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200
- 输出文件 `/tmp/bifrost-export-rules.bifrost` 非空
- 响应为二进制文件（.bifrost 格式）

---

### TC-AMS-19：Bifrost 文件检测类型

**前置条件**：已通过 TC-AMS-18 导出规则文件

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/bifrost-file/detect \
     --data-binary @/tmp/bifrost-export-rules.bifrost | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 对象包含：
  - `file_type`：字符串，值为 `"rules"` 或对应的规则类型标识
  - `meta`：对象，包含文件元数据

---

### TC-AMS-20：Bifrost 文件导入规则

**前置条件**：已通过 TC-AMS-18 导出规则文件

**操作步骤**：
1. 先删除原规则：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/rules/export-test
   ```
2. 导入 .bifrost 文件：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/bifrost-file/import \
     --data-binary @/tmp/bifrost-export-rules.bifrost | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 对象包含：
  - `success`：`true`
  - `file_type`：规则类型标识
  - `data`：对象，包含 `rule_names`（数组，含 `"export-test"`）和 `rule_count`

---

### TC-AMS-21：Bifrost 文件导出 Values

**前置条件**：已创建至少一个 Value

**操作步骤**：
1. 先创建一个测试 Value：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/values \
     -H "Content-Type: application/json" \
     -d '{"name": "test-value", "value": "hello"}'
   ```
2. 导出 Values：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/bifrost-file/export/values \
     -H "Content-Type: application/json" \
     -d '{"value_names": ["test-value"]}' -o /tmp/bifrost-export-values.bifrost -w "%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200
- 输出文件 `/tmp/bifrost-export-values.bifrost` 非空

---

### TC-AMS-22：Bifrost 文件导出 Scripts

**前置条件**：已创建至少一个脚本

**操作步骤**：
1. 先创建一个测试脚本：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/scripts \
     -H "Content-Type: application/json" \
     -d '{"name": "test-script", "script_type": "request", "content": "function onRequest(context, url, request) { return request; }"}'
   ```
2. 导出脚本：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/bifrost-file/export/scripts \
     -H "Content-Type: application/json" \
     -d '{"script_names": ["test-script"]}' -o /tmp/bifrost-export-scripts.bifrost -w "%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200
- 输出文件 `/tmp/bifrost-export-scripts.bifrost` 非空

---

### TC-AMS-23：Bifrost 文件导出 Network 记录

**操作步骤**：
1. 执行以下命令（即使没有记录也应正常返回）：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/bifrost-file/export/network \
     -H "Content-Type: application/json" \
     -d '{"record_ids": []}' -w "\n%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200
- 返回二进制文件内容或空的 .bifrost 文件

---

### TC-AMS-24：Bifrost 文件导出 Templates

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/bifrost-file/export/templates \
     -H "Content-Type: application/json" \
     -d '{}' -o /tmp/bifrost-export-templates.bifrost -w "%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200
- 输出文件 `/tmp/bifrost-export-templates.bifrost` 生成成功

---

### TC-AMS-25：获取同步状态

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s http://127.0.0.1:8800/_bifrost/api/sync/status | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回 JSON 对象，包含同步状态信息（具体字段取决于实现，至少应为有效 JSON）

---

### TC-AMS-26：更新同步配置

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X PUT http://127.0.0.1:8800/_bifrost/api/sync/config \
     -H "Content-Type: application/json" \
     -d '{"enabled": false}' -w "\n%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200
- 同步配置更新成功

---

### TC-AMS-27：触发同步运行

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/sync/run | jq .
   ```

**预期结果**：
- HTTP 状态码 200
- 返回当前同步状态的 JSON 对象

---

### TC-AMS-28：同步登出

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/sync/logout -w "\n%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200（如果未登录也应正常返回同步状态）
- 返回 JSON 对象包含同步状态信息

---

### TC-AMS-29：保存同步会话 Token

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/sync/session \
     -H "Content-Type: application/json" \
     -d '{"token": "test-fake-token-12345"}' -w "\n%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200 或合理的错误码（如 token 无效可能返回错误）
- 返回有效 JSON 响应

---

### TC-AMS-30：同步 API — 不支持的 HTTP 方法

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X DELETE http://127.0.0.1:8800/_bifrost/api/sync/status -w "\n%{http_code}"
   ```
2. 执行以下命令：
   ```bash
   curl -s -X GET http://127.0.0.1:8800/_bifrost/api/sync/run -w "\n%{http_code}"
   ```

**预期结果**：
- 两个请求均返回 HTTP 405（Method Not Allowed）

---

### TC-AMS-31：同步请求登录

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X POST http://127.0.0.1:8800/_bifrost/api/sync/login -w "\n%{http_code}"
   ```

**预期结果**：
- HTTP 状态码 200（返回同步状态 JSON）或 500（如果无法打开登录页面）
- 返回有效 JSON 响应

---

### TC-AMS-32：Bifrost 文件 API — 不支持的路径返回 405

**操作步骤**：
1. 执行以下命令：
   ```bash
   curl -s -X GET http://127.0.0.1:8800/_bifrost/api/bifrost-file/detect -w "\n%{http_code}"
   ```
2. 执行以下命令：
   ```bash
   curl -s -X GET http://127.0.0.1:8800/_bifrost/api/bifrost-file/import -w "\n%{http_code}"
   ```

**预期结果**：
- 两个请求均返回 HTTP 405（Method Not Allowed），因为 detect 和 import 只接受 POST 方法

---

## 清理

测试完成后清理临时数据：
```bash
rm -rf .bifrost-test
rm -f /tmp/bifrost-export-rules.bifrost
rm -f /tmp/bifrost-export-values.bifrost
rm -f /tmp/bifrost-export-scripts.bifrost
rm -f /tmp/bifrost-export-templates.bifrost
```
