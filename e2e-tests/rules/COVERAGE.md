# 协议覆盖清单

本文档记录 Bifrost 代理支持的所有协议类型及其测试覆盖状态。

## 覆盖状态说明

- ✅ 已覆盖 - 有对应测试用例且通过
- ⚠️ 部分覆盖 - 有基本测试但部分用例失败
- ❌ 未覆盖 - 需要添加测试
- 🔄 待验证 - 测试已添加但未运行验证
- ➖ 不适用 - 当前版本不支持或不需要测试

## 最新测试执行结果

**执行时间**: 2026-02-10
**总断言数**: 413
**通过**: 339 (82%)
**失败**: 74 (18%)
**测试套件**: 28 通过 / 21 失败

## 1. 基础路由协议 (Basic Routing)

| 协议               | 状态 | 测试文件                     | 说明                 |
| ------------------ | ---- | ---------------------------- | -------------------- |
| `host`             | ✅   | forwarding/http_to_http.txt  | 主机重定向           |
| `xhost`            | 🔄   | forwarding/http_to_http.txt  | 扩展主机重定向       |
| `http`             | ✅   | forwarding/http_to_http.txt  | HTTP 转发            |
| `https`            | ✅   | forwarding/http_to_https.txt | HTTPS 转发           |
| `ws`               | ✅   | forwarding/ws_forward.txt    | WebSocket            |
| `wss`              | ✅   | forwarding/ws_forward.txt    | WebSocket Secure     |
| `proxy`            | 🔄   | advanced/proxy.txt           | 二级代理             |
| `http-proxy`       | 🔄   | advanced/proxy.txt           | proxy 别名           |
| `https2http-proxy` | 🔄   | advanced/proxy.txt           | HTTPS→HTTP 代理      |
| `http2https-proxy` | 🔄   | advanced/proxy.txt           | HTTP→HTTPS 代理      |
| `internal-proxy`   | 🔄   | advanced/proxy.txt           | 内部代理             |
| `pac`              | ❌   | -                            | PAC 自动配置         |
| `redirect`         | ✅   | redirect/redirect.txt        | 302 重定向           |
| `locationHref`     | ✅   | redirect/redirect.txt        | location.href 重定向 |
| `file`             | 🔄   | template/values.txt          | 本地文件             |
| `tpl`              | 🔄   | template/tpl_file.txt        | 模板文件             |
| `rawfile`          | 🔄   | template/tpl_file.txt        | 原始文件             |

## 2. 请求修改协议 (Request Modification)

| 协议         | 状态 | 测试文件                   | 说明           |
| ------------ | ---- | -------------------------- | -------------- |
| `reqHeaders` | ⚠️   | request_modify/headers.txt | 请求头修改     |
| `reqBody`    | 🔄   | request_modify/body.txt    | 请求体替换     |
| `reqPrepend` | 🔄   | request_modify/body.txt    | 请求体前置     |
| `reqAppend`  | 🔄   | request_modify/body.txt    | 请求体追加     |
| `reqReplace` | 🔄   | request_modify/body.txt    | 请求体内容替换 |
| `reqCookies` | ⚠️   | request_modify/cookies.txt | 请求 Cookie    |
| `reqCors`    | ⚠️   | response_modify/cors.txt   | 请求 CORS      |
| `reqDelay`   | ✅   | response_modify/delay.txt  | 请求延迟       |
| `reqSpeed`   | 🔄   | advanced/speed.txt         | 请求速度限制   |
| `reqType`    | 🔄   | advanced/content_type.txt  | 请求内容类型   |
| `reqCharset` | 🔄   | advanced/content_type.txt  | 请求字符集     |

| `method` | ✅ | request_modify/method.txt | HTTP 方法 |
| `auth` | 🔄 | advanced/auth.txt | 基本认证 |
| `ua` | ⚠️ | request_modify/ua.txt | User-Agent |
| `referer` | ✅ | request_modify/referer.txt | Referer |
| `urlParams` | 🔄 | request_modify/url_params.txt | URL 参数 |
| `params` | 🔄 | request_modify/url_params.txt | 参数合并 |

## 3. 响应修改协议 (Response Modification)

| 协议         | 状态 | 测试文件                    | 说明           |
| ------------ | ---- | --------------------------- | -------------- |
| `resHeaders` | ⚠️   | response_modify/headers.txt | 响应头修改     |
| `resBody`    | ⚠️   | response_modify/body.txt    | 响应体替换     |
| `resPrepend` | 🔄   | response_modify/body.txt    | 响应体前置     |
| `resAppend`  | 🔄   | response_modify/body.txt    | 响应体追加     |
| `resReplace` | 🔄   | response_modify/body.txt    | 响应体内容替换 |
| `resCookies` | ⚠️   | response_modify/cookies.txt | 响应 Cookie    |
| `resCors`    | ⚠️   | response_modify/cors.txt    | 响应 CORS      |
| `resDelay`   | ✅   | response_modify/delay.txt   | 响应延迟       |
| `resSpeed`   | 🔄   | advanced/speed.txt          | 响应速度限制   |
| `resType`    | 🔄   | advanced/content_type.txt   | 响应内容类型   |
| `resCharset` | 🔄   | advanced/content_type.txt   | 响应字符集     |

| `statusCode` | ✅ | response_modify/status.txt | 状态码设置 |
| `replaceStatus` | ⚠️ | response_modify/status.txt | 状态码替换 |
| `cache` | 🔄 | advanced/cache.txt | 缓存控制 |
| `attachment` | 🔄 | advanced/cache.txt | 附件下载 |
| `trailers` | ❌ | - | HTTP Trailers |
| `resMerge` | ❌ | - | 响应合并 |
| `headerReplace` | ❌ | - | 头部替换 |

## 4. 内容注入协议 (Content Injection)

| 协议          | 状态 | 测试文件                | 说明            |
| ------------- | ---- | ----------------------- | --------------- |
| `htmlAppend`  | 🔄   | content_inject/html.txt | HTML 追加       |
| `htmlPrepend` | 🔄   | content_inject/html.txt | HTML 前置       |
| `htmlBody`    | 🔄   | content_inject/html.txt | HTML Body 替换  |
| `jsAppend`    | 🔄   | content_inject/js.txt   | JS 追加         |
| `jsPrepend`   | 🔄   | content_inject/js.txt   | JS 前置         |
| `jsBody`      | 🔄   | content_inject/js.txt   | JS Body 替换    |
| `cssAppend`   | 🔄   | content_inject/css.txt  | CSS 追加        |
| `cssPrepend`  | 🔄   | content_inject/css.txt  | CSS 前置        |
| `cssBody`     | 🔄   | content_inject/css.txt  | CSS Body 替换   |
| `html`        | 🔄   | content_inject/html.txt | htmlAppend 别名 |
| `js`          | 🔄   | content_inject/js.txt   | jsAppend 别名   |
| `css`         | 🔄   | content_inject/css.txt  | cssAppend 别名  |

## 5. URL 处理协议 (URL Processing)

| 协议          | 状态 | 测试文件                      | 说明            |
| ------------- | ---- | ----------------------------- | --------------- |
| `urlReplace`  | 🔄   | request_modify/url_params.txt | URL 替换        |
| `pathReplace` | 🔄   | -                             | urlReplace 别名 |

## 6. 控制协议 (Control)

| 协议            | 状态 | 测试文件                   | 说明        |
| --------------- | ---- | -------------------------- | ----------- |
| `filter`        | ⚠️   | control/filter.txt         | 过滤器      |
| `ignore`        | ⚠️   | control/ignore.txt         | 忽略规则    |
| `skip`          | ⚠️   | control/ignore.txt         | ignore 别名 |
| `enable`        | 🔄   | control/enable_disable.txt | 启用规则    |
| `disable`       | 🔄   | control/enable_disable.txt | 禁用规则    |
| `delete`        | 🔄   | control/enable_disable.txt | 删除规则    |
| `G`             | 🔄   | control/group.txt          | 分组协议    |
| `P`             | 🔄   | control/group.txt          | G 别名      |
| `style`         | 🔄   | control/group.txt          | 样式协议    |
| `lineProps`     | ✅   | control/line_props.txt     | 行属性配置  |
| `includeFilter` | ⚠️   | control/include_filter.txt | 包含过滤器  |
| `excludeFilter` | 🔄   | control/exclude_filter.txt | 排除过滤器  |

## 8. 模板变量 (Template Variables)

| 变量               | 状态 | 测试文件                   | 说明         |
| ------------------ | ---- | -------------------------- | ------------ |
| `${reqId}`         | 🔄   | template/template_vars.txt | 请求 ID      |
| `${now}`           | 🔄   | template/template_vars.txt | 当前时间戳   |
| `${random}`        | 🔄   | template/template_vars.txt | 随机数       |
| `${randomInt(N)}`  | 🔄   | template/template_vars.txt | 随机整数     |
| `${randomUUID}`    | 🔄   | template/template_vars.txt | 随机 UUID    |
| `${url}`           | ✅   | template/template_vars.txt | 完整 URL     |
| `${host}`          | ✅   | template/template_vars.txt | 主机名:端口  |
| `${hostname}`      | ✅   | template/template_vars.txt | 主机名       |
| `${port}`          | ✅   | template/template_vars.txt | 端口         |
| `${path}`          | ✅   | template/template_vars.txt | 路径         |
| `${pathname}`      | ✅   | template/template_vars.txt | 路径(无查询) |
| `${search}`        | ✅   | template/template_vars.txt | 查询字符串   |
| `${query.key}`     | ⚠️   | template/template_vars.txt | 查询参数     |
| `${method}`        | ✅   | template/template_vars.txt | HTTP 方法    |
| `${reqH.key}`      | ⚠️   | template/template_vars.txt | 请求头       |
| `${reqCookie.key}` | ⚠️   | template/template_vars.txt | 请求 Cookie  |
| `${clientIp}`      | 🔄   | template/template_vars.txt | 客户端 IP    |
| `${env.VAR}`       | 🔄   | template/template_vars.txt | 环境变量     |
| `${{var}}`         | 🔄   | template/template_vars.txt | URL 编码     |
| `$${var}`          | ⚠️   | template/template_vars.txt | 转义         |
| `${var.replace()}` | 🔄   | template/template_vars.txt | 替换操作     |

## 10. 值来源 (Value Sources)

| 类型     | 状态 | 测试文件                      | 说明                      |
| -------- | ---- | ----------------------------- | ------------------------- |
| 内联值   | ✅   | 多个文件                      | `127.0.0.1:8080`          |
| 内联参数 | ✅   | request_modify/url_params.txt | `key=value&k2=v2`         |
| 括号内容 | ❌   | -                             | `({"ok":true})`           |
| 值引用   | ✅   | template/values.txt           | `{valueName}`             |
| 文件路径 | ✅   | template/values.txt           | `/path/to/file`           |
| 远程 URL | ❌   | -                             | `http://example.com/data` |

## 10.1 Values 系统测试 (Values System)

**端到端测试脚本**: `test_values_e2e.sh` (Mock Server + Proxy + Client)
**CLI 测试脚本**: `test_values_cli.sh` (CLI 命令测试)
**测试值文件**: `scripts/values/`

### CLI 测试 (test_values_cli.sh)

| 测试类型         | 状态 | 说明             |
| ---------------- | ---- | ---------------- |
| CLI set/get      | ✅   | 值设置和获取     |
| CLI list         | ✅   | 列出所有值       |
| CLI delete       | ✅   | 删除值           |
| CLI import .txt  | ✅   | 导入 txt 格式    |
| CLI import .json | ✅   | 导入 json 格式   |
| CLI import .kv   | ✅   | 导入 kv 格式     |
| 多行值           | ✅   | 多行内容处理     |
| 特殊字符         | ✅   | 特殊字符处理     |
| Unicode 值       | ✅   | Unicode 字符支持 |
| 空值             | ✅   | 空值处理         |
| 值覆盖           | ✅   | 同名值覆盖       |

### 端到端测试 (test_values_e2e.sh)

测试架构: `Client (curl) → Proxy (bifrost) → Mock Server (echo)`

| 测试类型     | 状态 | 规则示例                          | 说明           |
| ------------ | ---- | --------------------------------- | -------------- |
| 内联响应体   | ✅   | resBody://\`{...}\`               | backticks 内联 |
| 内联请求头   | ✅   | reqHeaders://\`X-Header:value\`   | 请求头内联     |
| 内联响应头   | ✅   | resHeaders://\`X-Header:value\`   | 响应头内联     |
| 值引用响应体 | ✅   | resBody://{mockResponse}          | 值文件引用     |
| 值引用请求头 | ✅   | reqHeaders://{authHeaders}        | 请求头值引用   |
| 值引用响应头 | ✅   | resHeaders://{customHeaders}      | 响应头值引用   |
| 多值引用组合 | ✅   | reqHeaders://{a} resHeaders://{b} | 多值组合       |
| JSON 格式值  | ✅   | resBody://{jsonResponse}          | JSON 响应体    |
| 多行头部值   | ✅   | reqHeaders://{multiHeaders}       | 多行请求头     |

**Values 测试文件清单** (`scripts/values/`):

- `authHeaders.txt` - 认证头部测试值
- `customHeaders.txt` - 自定义头部测试值
- `mockResponse.txt` - Mock 响应体测试值
- `jsonResponse.txt` - JSON 格式响应测试值
- `multiHeaders.txt` - 多行头部测试值
- `emptyValue.txt` - 空值测试
- `specialChars.txt` - 特殊字符测试值

## 11. 模式匹配 (Pattern Matching)

| 模式          | 状态 | 测试文件                      | 说明                          |
| ------------- | ---- | ----------------------------- | ----------------------------- |
| 精确域名      | ✅   | combination/pattern_match.txt | `example.com`                 |
| 单层通配符    | ✅   | pattern/domain_wildcard.txt   | `*.example.com` (不含点)      |
| 多层通配符    | ✅   | pattern/domain_wildcard.txt   | `**.example.com` (可含点)     |
| 路径前缀      | ✅   | combination/pattern_match.txt | `example.com/api`             |
| 路径通配符    | ⚠️   | combination/pattern_match.txt | `example.com/*`               |
| ^前缀路径单星 | ⚠️   | pattern/path_wildcard.txt     | `^example.com/api/*` (不含/?) |
| ^前缀路径双星 | ⚠️   | pattern/path_wildcard.txt     | `^example.com/api/**` (不含?) |
| ^前缀路径三星 | ⚠️   | pattern/path_wildcard.txt     | `^example.com/api/***` (含?)  |
| 正则匹配      | ✅   | combination/pattern_match.txt | `/regex/`                     |
| 正则 i 标志   | ✅   | combination/pattern_match.txt | `/regex/i`                    |
| 正则 u 标志   | ✅   | combination/pattern_match.txt | `/regex/u` (Unicode)          |
| 正则捕获      | ✅   | combination/pattern_match.txt | `/(\w+)/` → `$1`              |
| 通配符捕获    | ✅   | pattern/domain_wildcard.txt   | `*.example.com` → `$1`        |
| IP 匹配       | ⚠️   | combination/pattern_match.txt | `127.0.0.1`                   |
| CIDR 匹配     | ⚠️   | priority/ip_vs_cidr.txt       | `192.168.0.0/16`              |
| 端口匹配      | ✅   | combination/pattern_match.txt | `example.com:8080`            |
| 端口通配符    | ⚠️   | pattern/port_wildcard.txt     | `example.com:8*8`             |
| http\* 协议   | ⚠️   | pattern/protocol_wildcard.txt | `http*://` 匹配 http/https    |
| ws\* 协议     | ⚠️   | pattern/protocol_wildcard.txt | `ws*://` 匹配 ws/wss          |
| // 协议       | ⚠️   | pattern/protocol_wildcard.txt | `//` 匹配所有协议             |
| ws/wss 协议   | ⚠️   | pattern/protocol_wildcard.txt | `ws://`, `wss://`             |
| tunnel 协议   | ⚠️   | pattern/protocol_wildcard.txt | `tunnel://`                   |

## 12. 规则优先级 (Priority)

| 场景             | 状态 | 测试文件                       | 说明           |
| ---------------- | ---- | ------------------------------ | -------------- |
| 精确 vs 通配符   | ⚠️   | priority/exact_vs_wildcard.txt | 精确优先       |
| 通配符层级       | ⚠️   | priority/wildcard_level.txt    | 更具体优先     |
| 规则顺序         | ⚠️   | priority/order.txt             | 先定义优先     |
| IP vs CIDR       | ⚠️   | priority/ip_vs_cidr.txt        | 精确 IP 优先   |
| important 优先级 | ✅   | control/line_props.txt         | important 属性 |

## 13. 规则组合 (Combination)

| 场景                   | 状态 | 测试文件                    | 说明         |
| ---------------------- | ---- | --------------------------- | ------------ |
| 转发 + 请求头          | ⚠️   | combination/multi_rules.txt | 组合规则     |
| 转发 + 响应头 + 状态码 | ⚠️   | combination/multi_rules.txt | 多规则       |
| 多重请求头             | ⚠️   | combination/multi_rules.txt | 同类型多规则 |
| 完整修改链             | ⚠️   | combination/multi_rules.txt | 全方位修改   |

## 14. 高级语法 (Advanced Syntax)

| 语法          | 状态 | 测试文件                | 说明         |
| ------------- | ---- | ----------------------- | ------------ |
| `line\`...\`` | ⚠️   | advanced/line_block.txt | 换行配置语法 |

---

## 统计

| 分类       | 已覆盖 | 部分覆盖 | 待验证 | 未覆盖 |
| ---------- | ------ | -------- | ------ | ------ |
| 基础路由   | 8      | 0        | 9      | 1      |
| 请求修改   | 4      | 4        | 7      | 2      |
| 响应修改   | 3      | 5        | 8      | 4      |
| 内容注入   | 0      | 0        | 12     | 0      |
| URL 处理   | 0      | 0        | 1      | 1      |
| 控制协议   | 1      | 4        | 6      | 0      |
| 脚本插件   | 0      | 0        | 0      | 8      |
| 安全协议   | 0      | 0        | 0      | 3      |
| 模板变量   | 9      | 4        | 8      | 0      |
| 值来源     | 4      | 0        | 1      | 1      |
| Values系统 | 20     | 0        | 0      | 0      |
| 模式匹配   | 12     | 11       | 0      | 0      |
| 规则优先级 | 1      | 4        | 0      | 0      |
| 规则组合   | 0      | 4        | 0      | 0      |

**总计**: 已覆盖 62 | 部分覆盖 36 | 待验证 52 | 未覆盖 20

---

## 优先补充清单

以下功能需要优先修复或添加测试用例:

### 高优先级 (当前失败的测试)

1. `filter` / `ignore` / `includeFilter` - 控制协议测试失败
2. `path_wildcard` / `port_wildcard` / `protocol_wildcard` - 通配符匹配问题
3. `priority/*` - 优先级测试失败
4. `template_vars` - 部分模板变量不工作 (reqHeaders, reqCookies, 转义符号)
5. `request_modify/cookies.txt` / `headers.txt` - 请求修改问题
6. `response_modify/*` - 响应修改问题
7. `combination/*` - 组合规则问题

### 中优先级 (扩展功能)

1. `pac` - PAC 自动配置
2. `headerReplace` - 头部替换
3. `resMerge` - 响应合并
4. 括号内容值来源 `({"ok":true})`
5. 远程 URL 值来源

### 低优先级 (特殊场景)

1. `trailers` - HTTP Trailers
2. `rulesFile` - 规则文件引用
3. `resScript` - 响应脚本
