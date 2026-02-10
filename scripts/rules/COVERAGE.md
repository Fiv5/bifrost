# 协议覆盖清单

本文档记录 Bifrost 代理支持的所有协议类型及其测试覆盖状态。

## 覆盖状态说明

- ✅ 已覆盖 - 有对应测试用例
- ⚠️ 部分覆盖 - 有基本测试但缺少边界情况
- ❌ 未覆盖 - 需要添加测试
- 🔄 待验证 - 测试已添加但未运行验证
- ➖ 不适用 - 当前版本不支持或不需要测试

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

| 协议          | 状态 | 测试文件                      | 说明           |
| ------------- | ---- | ----------------------------- | -------------- |
| `reqHeaders`  | ✅   | request_modify/headers.txt    | 请求头修改     |
| `reqBody`     | 🔄   | request_modify/body.txt       | 请求体替换     |
| `reqPrepend`  | 🔄   | request_modify/body.txt       | 请求体前置     |
| `reqAppend`   | 🔄   | request_modify/body.txt       | 请求体追加     |
| `reqReplace`  | 🔄   | request_modify/body.txt       | 请求体内容替换 |
| `reqCookies`  | ✅   | request_modify/cookies.txt    | 请求 Cookie    |
| `reqCors`     | ⚠️   | response_modify/cors.txt      | 请求 CORS      |
| `reqDelay`    | ✅   | response_modify/delay.txt     | 请求延迟       |
| `reqSpeed`    | 🔄   | advanced/speed.txt            | 请求速度限制   |
| `reqType`     | 🔄   | advanced/content_type.txt     | 请求内容类型   |
| `reqCharset`  | 🔄   | advanced/content_type.txt     | 请求字符集     |
| `reqWrite`    | ❌   | -                             | 请求写入       |
| `reqWriteRaw` | ❌   | -                             | 请求原始写入   |
| `method`      | ✅   | request_modify/method.txt     | HTTP 方法      |
| `auth`        | 🔄   | advanced/auth.txt             | 基本认证       |
| `ua`          | ✅   | request_modify/ua.txt         | User-Agent     |
| `referer`     | ✅   | request_modify/referer.txt    | Referer        |
| `urlParams`   | 🔄   | request_modify/url_params.txt | URL 参数       |
| `params`      | 🔄   | request_modify/url_params.txt | 参数合并       |

## 3. 响应修改协议 (Response Modification)

| 协议            | 状态 | 测试文件                    | 说明            |
| --------------- | ---- | --------------------------- | --------------- |
| `resHeaders`    | ✅   | response_modify/headers.txt | 响应头修改      |
| `resBody`       | 🔄   | response_modify/body.txt    | 响应体替换      |
| `resPrepend`    | 🔄   | response_modify/body.txt    | 响应体前置      |
| `resAppend`     | 🔄   | response_modify/body.txt    | 响应体追加      |
| `resReplace`    | 🔄   | response_modify/body.txt    | 响应体内容替换  |
| `resCookies`    | ✅   | response_modify/cookies.txt | 响应 Cookie     |
| `resCors`       | ✅   | response_modify/cors.txt    | 响应 CORS       |
| `resDelay`      | ✅   | response_modify/delay.txt   | 响应延迟        |
| `resSpeed`      | 🔄   | advanced/speed.txt          | 响应速度限制    |
| `resType`       | 🔄   | advanced/content_type.txt   | 响应内容类型    |
| `resCharset`    | 🔄   | advanced/content_type.txt   | 响应字符集      |
| `resWrite`      | ❌   | -                           | 响应写入        |
| `resWriteRaw`   | ❌   | -                           | 响应原始写入    |
| `statusCode`    | ✅   | response_modify/status.txt  | 状态码设置      |
| `replaceStatus` | ⚠️   | response_modify/status.txt  | 状态码替换      |
| `cache`         | 🔄   | advanced/cache.txt          | 缓存控制        |
| `attachment`    | 🔄   | advanced/cache.txt          | 附件下载        |
| `forwardedFor`  | 🔄   | advanced/auth.txt           | X-Forwarded-For |
| `trailers`      | ❌   | -                           | HTTP Trailers   |
| `resMerge`      | ❌   | -                           | 响应合并        |
| `headerReplace` | ❌   | -                           | 头部替换        |

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
| `filter`        | ✅   | control/filter.txt         | 过滤器      |
| `ignore`        | ✅   | control/ignore.txt         | 忽略规则    |
| `skip`          | ⚠️   | control/ignore.txt         | ignore 别名 |
| `enable`        | 🔄   | control/enable_disable.txt | 启用规则    |
| `disable`       | 🔄   | control/enable_disable.txt | 禁用规则    |
| `delete`        | 🔄   | control/enable_disable.txt | 删除规则    |
| `G`             | 🔄   | control/group.txt          | 分组协议    |
| `P`             | 🔄   | control/group.txt          | G 别名      |
| `style`         | 🔄   | control/group.txt          | 样式协议    |
| `lineProps`     | 🔄   | control/line_props.txt     | 行属性配置  |
| `includeFilter` | 🔄   | control/include_filter.txt | 包含过滤器  |
| `excludeFilter` | 🔄   | control/exclude_filter.txt | 排除过滤器  |

## 7. 脚本与插件协议 (Scripts & Plugins)

| 协议          | 状态 | 测试文件 | 说明        |
| ------------- | ---- | -------- | ----------- |
| `plugin`      | ❌   | -        | 插件协议    |
| `rulesFile`   | ❌   | -        | 规则文件    |
| `resScript`   | ❌   | -        | 响应脚本    |
| `frameScript` | ❌   | -        | Frame 脚本  |
| `log`         | ❌   | -        | 日志协议    |
| `weinre`      | ❌   | -        | Weinre 调试 |
| `rule`        | ❌   | -        | 规则引用    |
| `pipe`        | ❌   | -        | 管道处理    |

## 8. 安全协议 (Security)

| 协议          | 状态 | 测试文件 | 说明         |
| ------------- | ---- | -------- | ------------ |
| `cipher`      | ❌   | -        | TLS 加密选项 |
| `sniCallback` | ❌   | -        | SNI 回调     |
| `tlsOptions`  | ❌   | -        | cipher 别名  |

## 9. 模板变量 (Template Variables)

| 变量               | 状态 | 测试文件                   | 说明         |
| ------------------ | ---- | -------------------------- | ------------ |
| `${reqId}`         | 🔄   | template/template_vars.txt | 请求 ID      |
| `${now}`           | 🔄   | template/template_vars.txt | 当前时间戳   |
| `${random}`        | 🔄   | template/template_vars.txt | 随机数       |
| `${randomInt(N)}`  | 🔄   | template/template_vars.txt | 随机整数     |
| `${randomUUID}`    | 🔄   | template/template_vars.txt | 随机 UUID    |
| `${url}`           | 🔄   | template/template_vars.txt | 完整 URL     |
| `${host}`          | 🔄   | template/template_vars.txt | 主机名:端口  |
| `${hostname}`      | 🔄   | template/template_vars.txt | 主机名       |
| `${port}`          | 🔄   | template/template_vars.txt | 端口         |
| `${path}`          | 🔄   | template/template_vars.txt | 路径         |
| `${pathname}`      | 🔄   | template/template_vars.txt | 路径(无查询) |
| `${search}`        | 🔄   | template/template_vars.txt | 查询字符串   |
| `${query.key}`     | 🔄   | template/template_vars.txt | 查询参数     |
| `${method}`        | 🔄   | template/template_vars.txt | HTTP 方法    |
| `${reqH.key}`      | 🔄   | template/template_vars.txt | 请求头       |
| `${reqCookie.key}` | 🔄   | template/template_vars.txt | 请求 Cookie  |
| `${clientIp}`      | 🔄   | template/template_vars.txt | 客户端 IP    |
| `${env.VAR}`       | 🔄   | template/template_vars.txt | 环境变量     |
| `${{var}}`         | 🔄   | template/template_vars.txt | URL 编码     |
| `$${var}`          | 🔄   | template/template_vars.txt | 转义         |
| `${var.replace()}` | 🔄   | template/template_vars.txt | 替换操作     |

## 10. 值来源 (Value Sources)

| 类型     | 状态 | 测试文件                      | 说明                      |
| -------- | ---- | ----------------------------- | ------------------------- |
| 内联值   | ✅   | 多个文件                      | `127.0.0.1:8080`          |
| 内联参数 | 🔄   | request_modify/url_params.txt | `key=value&k2=v2`         |
| 括号内容 | ❌   | -                             | `({"ok":true})`           |
| 值引用   | 🔄   | template/values.txt           | `{valueName}`             |
| 文件路径 | 🔄   | template/values.txt           | `/path/to/file`           |
| 远程 URL | ❌   | -                             | `http://example.com/data` |

## 11. 模式匹配 (Pattern Matching)

| 模式          | 状态 | 测试文件                      | 说明                          |
| ------------- | ---- | ----------------------------- | ----------------------------- |
| 精确域名      | ✅   | combination/pattern_match.txt | `example.com`                 |
| 单层通配符    | ✅   | pattern/domain_wildcard.txt   | `*.example.com` (不含点)      |
| 多层通配符    | ✅   | pattern/domain_wildcard.txt   | `**.example.com` (可含点)     |
| 路径前缀      | ✅   | combination/pattern_match.txt | `example.com/api`             |
| 路径通配符    | ✅   | combination/pattern_match.txt | `example.com/*`               |
| ^前缀路径单星 | ✅   | pattern/path_wildcard.txt     | `^example.com/api/*` (不含/?) |
| ^前缀路径双星 | ✅   | pattern/path_wildcard.txt     | `^example.com/api/**` (不含?) |
| ^前缀路径三星 | ✅   | pattern/path_wildcard.txt     | `^example.com/api/***` (含?)  |
| 正则匹配      | ✅   | combination/pattern_match.txt | `/regex/`                     |
| 正则 i 标志   | ✅   | combination/pattern_match.txt | `/regex/i`                    |
| 正则 u 标志   | ✅   | combination/pattern_match.txt | `/regex/u` (Unicode)          |
| 正则捕获      | ✅   | combination/pattern_match.txt | `/(\w+)/` → `$1`              |
| 通配符捕获    | ✅   | pattern/domain_wildcard.txt   | `*.example.com` → `$1`        |
| IP 匹配       | ✅   | combination/pattern_match.txt | `127.0.0.1`                   |
| CIDR 匹配     | ✅   | priority/ip_vs_cidr.txt       | `192.168.0.0/16`              |
| 端口匹配      | ✅   | combination/pattern_match.txt | `example.com:8080`            |
| 端口通配符    | ✅   | pattern/port_wildcard.txt     | `example.com:8*8`             |
| http\* 协议   | ✅   | pattern/protocol_wildcard.txt | `http*://` 匹配 http/https    |
| ws\* 协议     | ✅   | pattern/protocol_wildcard.txt | `ws*://` 匹配 ws/wss          |
| // 协议       | ✅   | pattern/protocol_wildcard.txt | `//` 匹配所有协议             |
| ws/wss 协议   | ✅   | pattern/protocol_wildcard.txt | `ws://`, `wss://`             |
| tunnel 协议   | ✅   | pattern/protocol_wildcard.txt | `tunnel://`                   |

## 12. 规则优先级 (Priority)

| 场景             | 状态 | 测试文件                       | 说明           |
| ---------------- | ---- | ------------------------------ | -------------- |
| 精确 vs 通配符   | ✅   | priority/exact_vs_wildcard.txt | 精确优先       |
| 通配符层级       | ✅   | priority/wildcard_level.txt    | 更具体优先     |
| 规则顺序         | ✅   | priority/order.txt             | 先定义优先     |
| IP vs CIDR       | ✅   | priority/ip_vs_cidr.txt        | 精确 IP 优先   |
| important 优先级 | 🔄   | priority/important.txt         | important 属性 |

## 13. 规则组合 (Combination)

| 场景                   | 状态 | 测试文件                    | 说明         |
| ---------------------- | ---- | --------------------------- | ------------ |
| 转发 + 请求头          | 🔄   | combination/multi_rules.txt | 组合规则     |
| 转发 + 响应头 + 状态码 | 🔄   | combination/multi_rules.txt | 多规则       |
| 多重请求头             | 🔄   | combination/multi_rules.txt | 同类型多规则 |
| 完整修改链             | 🔄   | combination/multi_rules.txt | 全方位修改   |

## 14. 高级语法 (Advanced Syntax)

| 语法          | 状态 | 测试文件                | 说明         |
| ------------- | ---- | ----------------------- | ------------ |
| `line\`...\`` | 🔄   | advanced/line_block.txt | 换行配置语法 |

---

## 统计

| 分类       | 已覆盖 | 部分覆盖 | 待验证 | 未覆盖 |
| ---------- | ------ | -------- | ------ | ------ |
| 基础路由   | 8      | 0        | 9      | 1      |
| 请求修改   | 7      | 1        | 7      | 2      |
| 响应修改   | 6      | 1        | 8      | 4      |
| 内容注入   | 0      | 0        | 12     | 0      |
| URL 处理   | 0      | 0        | 1      | 1      |
| 控制协议   | 2      | 1        | 6      | 0      |
| 脚本插件   | 0      | 0        | 0      | 8      |
| 安全协议   | 0      | 0        | 0      | 3      |
| 模板变量   | 0      | 0        | 21     | 0      |
| 值来源     | 1      | 0        | 3      | 2      |
| 模式匹配   | 23     | 0        | 0      | 0      |
| 规则优先级 | 4      | 0        | 0      | 0      |
| 规则组合   | 0      | 0        | 4      | 0      |

**总计**: 已覆盖 51 | 部分覆盖 3 | 待验证 71 | 未覆盖 21

---

## 优先补充清单

以下功能需要优先添加测试用例:

### 高优先级 (影响核心功能)

1. `pac` - PAC 自动配置
2. `headerReplace` - 头部替换
3. `resMerge` - 响应合并
4. `reqWrite/reqWriteRaw` - 请求写入
5. `resWrite/resWriteRaw` - 响应写入
6. 括号内容值来源 `({"ok":true})`
7. 远程 URL 值来源

### 中优先级 (扩展功能)

1. `trailers` - HTTP Trailers
2. `plugin` - 插件系统
3. `rulesFile` - 规则文件引用
4. `resScript` - 响应脚本
5. `cipher/tlsOptions` - TLS 选项

### 低优先级 (特殊场景)

1. `log` - 日志协议
2. `weinre` - 调试工具
3. `sniCallback` - SNI 回调
4. `frameScript` - Frame 脚本
