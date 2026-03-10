# Whistle 规则手册

本文档是 Whistle 代理规则的完整参考手册，涵盖了所有支持的规则协议。

## 规则语法基础

### 基本格式

```
pattern protocol://value [filters...]
```

| 部分       | 说明         | 示例                                              |
| ---------- | ------------ | ------------------------------------------------- |
| `pattern`  | URL 匹配模式 | `www.example.com`, `*.example.com`, `/api\/v\d+/` |
| `protocol` | 规则协议     | `host`, `reqHeaders`, `resBody`                   |
| `value`    | 协议参数值   | `127.0.0.1:8080`, `(X-Custom:value)`              |
| `filters`  | 可选过滤器   | `includeFilter://m:POST`                          |

### 值的来源

规则的 `value` 支持多种来源：

| 来源       | 语法                       | 示例                                        |
| ---------- | -------------------------- | ------------------------------------------- |
| 内联值     | `protocol://value`         | `host://127.0.0.1:8080`                     |
| 内联参数   | `protocol://key=value`     | `reqHeaders://X-Custom=test&X-Custom2=test` |
| 小括号内容 | `protocol://(content)`     | `resBody://({"ok":true,"code":0})`          |
| 内嵌值引用 | `protocol://{varName}`     | `resBody://{myResponse}`                    |
| 文件路径   | `protocol:///path/to/file` | `file:///etc/mock.json`                     |
| 远程 URL   | `protocol://http://...`    | `file://http://example.com/data`            |

> ⚠️ **重要**：`{name}` 语法是**引用内嵌值/Values**，不是直接定义 JSON 对象！

#### 内联值定义

````markdown
```var_name
data 这里全都是数据
```

a.com reqHeaders://{var_name} # 内联值使用
````

#### 小括号内容不能有空格

使用小括号 `()` 包裹行内值时，**括号内不能有空格**。如果内容包含空格，必须使用块变量。

```
# ❌ 错误写法 - 括号内有空格
example.com resBody://(hello world)
example.com resHeaders://(X-Key: value)
example.com resBody://({"error": "Not Found"})

# ✅ 正确写法 1 - 括号内无空格
example.com resBody://(hello)
example.com resHeaders://(X-Key:value)
example.com resBody://({"error":"NotFound"})

# ✅ 正确写法 2 - 使用块变量（推荐用于复杂内容）
example.com resBody://{errorResponse}
example.com resHeaders://{customHeaders}
```

### ⚠️ 重要语法规则

#### 操作符后不支持空格

**基本原则：操作符（`://`）后面不能有空格**。如果值中包含空格，必须使用以下方式之一：

1. **块变量（推荐）**：将包含空格的值定义为块变量
2. **URI 编码**：将空格编码为 `%20`

````
# ❌ 错误写法 - 操作符后有空格会导致解析错误
example.com ua://Mozilla Firefox

# ✅ 正确写法 1 - 使用块变量
example.com ua://{myUA}

``` myUA
Mozilla Firefox
````

# ✅ 正确写法 2 - 使用 URI 编码

example.com ua://Mozilla%20Firefox

````

#### 内嵌值（块变量）语法

内嵌值使用 `{varName}` 语法引用，变量内容在规则文件中单独定义。

**定义方式**：使用 ``` varName 和 ``` 包围内容

````

# 规则引用内嵌值

example.com resBody://{jsonResponse}
example.com reqHeaders://{customHeaders}

# 内嵌值定义

```jsonResponse
{
  "code": 0,
  "message": "success",
  "data": {
    "name": "test user"
  }
}
```

```customHeaders
X-Custom-Header: value with spaces
X-Another-Header: another value
```

````

**内嵌值的优势**：
- 支持多行内容
- 支持包含空格、特殊字符的值
- 便于管理复杂的配置内容
- 可在多个规则中复用
- 可存储在 Whistle 的 Values 模块中共享

---

## 规则分类总览

### 1. 路由与转发规则

控制请求的目标地址和转发方式。

| 协议       | 说明                   | 详情                                                |
| ---------- | ---------------------- | --------------------------------------------------- |
| `host`     | 将请求重定向到指定主机 | [routing.md](./routing.md#host)                     |
| `redirect` | URL 重定向 (301/302)   | [status-redirect.md](./status-redirect.md#redirect) |
| `proxy`    | 通过 HTTP 代理转发     | [routing.md](./routing.md#proxy)                    |
| `socks`    | 通过 SOCKS 代理转发    | [routing.md](./routing.md#socks)                    |
| `tunnel`   | 隧道透传（不拦截）     | [routing.m                                          |
d](./routing.md#tunnel) |

### 2. 请求修改规则
修改发送到服务器的请求内容。

| 协议 | 说明 | 详情 |
|------|------|------|
| `reqHeaders` | 设置/修改请求头 | [request-modification.md](./request-modification.md#reqheaders) |
| `ua` | 设置 User-Agent | [request-modification.md](./request-modification.md#ua) |
| `referer` | 设置 Referer | [request-modification.md](./request-modification.md#referer) |
| `method` | 修改请求方法 | [request-modification.md](./request-modification.md#method) |
| `reqCookies` | 设置请求 Cookie | [request-modification.md](./request-modification.md#reqcookies) |
| `reqBody` | 设置请求 Body | [body-manipulation.md](./body-manipulation.md#reqbody) |
| `reqType` | 设置请求 Content-Type | [request-modification.md](./request-modification.md#reqtype) |
| `reqCharset` | 设置请求字符集 | [request-modification.md](./request-modification.md#reqcharset) |

### 3. 响应修改规则
修改返回给客户端的响应内容。

| 协议 | 说明 | 详情 |
|------|------|------|
| `resHeaders` | 设置/修改响应头 | [response-modification.md](./response-modification.md#resheaders) |
| `resCookies` | 设置响应 Cookie | [response-modification.md](./response-modification.md#rescookies) |
| `resCors` | 设置 CORS 响应头 | [response-modification.md](./response-modification.md#rescors) |
| `resBody` | 设置响应 Body | [body-manipulation.md](./body-manipulation.md#resbody) |
| `resType` | 设置响应 Content-Type | [response-modification.md](./response-modification.md#restype) |
| `resCharset` | 设置响应字符集 | [response-modification.md](./response-modification.md#rescharset) |

### 4. URL 操作规则
动态修改请求 URL。

| 协议 | 说明 | 详情 |
|------|------|------|
| `urlParams` | 添加/修改 URL 参数 | [url-manipulation.md](./url-manipulation.md#urlparams) |
| `pathReplace` | 替换 URL 路径 | [url-manipulation.md](./url-manipulation.md#pathreplace) |

### 5. Body 操作规则
对请求/响应 Body 进行处理。

| 协议 | 说明 | 详情 |
|------|------|------|
| `reqBody` | 设置请求 Body | [body-manipulation.md](./body-manipulation.md#reqbody) |
| `resBody` | 设置响应 Body | [body-manipulation.md](./body-manipulation.md#resbody) |
| `reqReplace` | 替换请求 Body 内容 | [body-manipulation.md](./body-manipulation.md#reqreplace) |
| `resReplace` | 替换响应 Body 内容 | [body-manipulation.md](./body-manipulation.md#resreplace) |
| `reqMerge` | 合并请求 Body (JSON) | [body-manipulation.md](./body-manipulation.md#reqmerge) |
| `resMerge` | 合并响应 Body (JSON) | [body-manipulation.md](./body-manipulation.md#resmerge) |
| `xfile` | file 的穿透版本 | [body-manipulation.md](./body-manipulation.md#xfile) |
| `xrawfile` | rawfile 的穿透版本 | [body-manipulation.md](./body-manipulation.md#xrawfile) |
| `xtpl` | tpl 的穿透版本 | [body-manipulation.md](./body-manipulation.md#xtpl) |

### 5.1 HTML/CSS/JS 注入规则
向特定类型的响应内容注入代码。

| 协议 | 说明 | 详情 |
|------|------|------|
| `htmlAppend` | HTML 末尾追加内容 | [body-manipulation.md](./body-manipulation.md#htmlappend) |
| `htmlPrepend` | HTML 开头插入内容 | [body-manipulation.md](./body-manipulation.md#htmlprepend) |
| `htmlBody` | 替换 HTML 响应内容 | [body-manipulation.md](./body-manipulation.md#htmlbody) |
| `jsAppend` | JS 末尾追加代码 | [body-manipulation.md](./body-manipulation.md#jsappend) |
| `jsPrepend` | JS 开头插入代码 | [body-manipulation.md](./body-manipulation.md#jsprepend) |
| `jsBody` | 替换 JS 响应内容 | [body-manipulation.md](./body-manipulation.md#jsbody) |
| `cssAppend` | CSS 末尾追加样式 | [body-manipulation.md](./body-manipulation.md#cssappend) |
| `cssPrepend` | CSS 开头插入样式 | [body-manipulation.md](./body-manipulation.md#cssprepend) |
| `cssBody` | 替换 CSS 响应内容 | [body-manipulation.md](./body-manipulation.md#cssbody) |

### 6. 状态码与重定向规则
控制 HTTP 状态码和重定向行为。

| 协议 | 说明 | 详情 |
|------|------|------|
| `statusCode` | 直接返回状态码（不请求后端） | [status-redirect.md](./status-redirect.md#statuscode) |
| `replaceStatus` | 替换状态码（请求后端后替换） | [status-redirect.md](./status-redirect.md#replacestatus) |
| `redirect` | URL 重定向 | [status-redirect.md](./status-redirect.md#redirect) |

### 7. 延迟与限速规则
模拟网络延迟和带宽限制。

| 协议 | 说明 | 详情 |
|------|------|------|
| `reqDelay` | 请求延迟 (ms) | [timing-throttle.md](./timing-throttle.md#reqdelay) |
| `resDelay` | 响应延迟 (ms) | [timing-throttle.md](./timing-throttle.md#resdelay) |
| `reqSpeed` | 请求速度限制 (kb/s) | [timing-throttle.md](./timing-throttle.md#reqspeed) |
| `resSpeed` | 响应速度限制 (kb/s) | [timing-throttle.md](./timing-throttle.md#resspeed) |

### 8. 过滤器规则
控制规则的生效条件。

| 协议 | 说明 | 详情 |
|------|------|------|
| `includeFilter` | 包含过滤器（满足条件才生效） | [filters.md](./filters.md#includefilter) |
| `excludeFilter` | 排除过滤器（满足条件不生效） | [filters.md](./filters.md#excludefilter) |
| `ignore` | 忽略规则 | [filters.md](./filters.md#ignore) |

### 9. 控制规则
启用或禁用特定功能。

| 协议 | 说明 | 详情 |
|------|------|------|
| `enable` | 启用特性 | [filters.md](./filters.md#enable) |
| `disable` | 禁用特性 | [filters.md](./filters.md#disable) |
| `delete` | 删除头部/Cookie/参数 | [filters.md](./filters.md#delete) |

### 10. Mock 响应规则
直接返回 Mock 数据。

| 协议 | 说明 | 详情 |
|------|------|------|
| `file` | 返回文件内容 | [body-manipulation.md](./body-manipulation.md#file) |
| `tpl` | 模板响应（支持变量替换） | [body-manipulation.md](./body-manipulation.md#tpl) |

### 11. Header 替换规则
局部替换请求/响应头内容。

| 协议 | 说明 | 详情 |
|------|------|------|
| `headerReplace` | 替换头部内容 | [request-modification.md](./request-modification.md#headerreplace) |

### 12. 脚本规则
通过 JavaScript 脚本动态生成规则。

| 协议 | 说明 | 详情 |
|------|------|------|
| `reqScript` | 请求阶段脚本规则 | [scripts.md](./scripts.md#reqscript) |
| `resScript` | 响应阶段脚本规则 | [scripts.md](./scripts.md#resscript) |
| `decode` | 请求/响应 body decode 脚本（落库前解码） | [scripts.md](./scripts.md#decode) |

### 13. WebSocket 规则
WebSocket 请求转发和代理。

| 协议 | 说明 | 详情 |
|------|------|------|
| `ws` | WebSocket 转发 (ws://) | [websocket.md](./websocket.md#ws) |
| `wss` | WebSocket Secure 转发 (wss://) | [websocket.md](./websocket.md#wss) |

---

## 匹配模式

详细的匹配模式语法请参阅 [patterns.md](./patterns.md)。

| 类型 | 示例 | 说明 |
|------|------|------|
| 精确匹配 | `www.example.com` | 完全匹配域名 |
| 路径匹配 | `www.example.com/api` | 匹配域名+路径前缀 |
| 端口匹配 | `www.example.com:8080` | 匹配域名+端口 |
| 单级通配 | `*.example.com` | 匹配一级子域名 |
| 多级通配 | `**.example.com` | 匹配多级子域名 |
| 路径通配 | `example.com/api/*` | 匹配路径前缀 |
| 正则匹配 | `/api\/v\d+/` | 正则表达式匹配 |
| 大小写不敏感 | `/example/i` | 忽略大小写 |

---

## 模板变量

> ⚠️ **重要**：使用模板变量时，必须用反引号包裹值，如 `protocol://\`...${now}...\``

### 字符串变量

| 变量 | 说明 | 示例输出 |
|------|------|---------|
| `${now}` | Date.now() | `1704067200000` |
| `${random}` | Math.random() | `0.8234567891` |
| `${randomUUID}` | crypto.randomUUID() | `a1b2c3d4-e5f6-...` |
| `${randomInt(n)}` | 从 [0, n] 取随机正整数 | `5` |
| `${randomInt(n1-n2)}` | 从 [n1, n2] 取随机正整数 | `15` |
| `${reqId}` | Whistle 给每个请求分配的 ID | `1752301623294-339` |
| `${url}` | 请求完整 URL | `http://example.com/api?a=1` |
| `${url.protocol}` | url.parse(fullUrl).protocol | `https:` |
| `${url.hostname}` | url.parse(fullUrl).hostname | `example.com` |
| `${url.host}` | url.parse(fullUrl).host | `example.com:8080` |
| `${url.port}` | url.parse(fullUrl).port | `8080` |
| `${url.path}` | url.parse(fullUrl).path | `/api/users?a=1` |
| `${url.pathname}` | url.parse(fullUrl).pathname | `/api/users` |
| `${url.search}` | url.parse(fullUrl).search | `?a=1` |
| `${query.xxx}` | 请求参数 xxx 的值 | `value` |
| `${querystring}` | url.parse(fullUrl).search \|\| '?' | `?a=1` |
| `${method}` | 请求方法 | `GET` |
| `${reqHeaders.xxx}` | 请求头字段 xxx 的值 | `application/json` |
| `${resHeaders.xxx}` | 响应头字段 xxx 的值 | `text/html` |
| `${version}` | Whistle 版本号 | `2.9.100` |
| `${port}` | Whistle 端口号 | `9900` |
| `${clientIp}` | 客户端 IP | `192.168.1.1` |
| `${clientPort}` | 客户端端口 | `60582` |
| `${serverIp}` | 服务端 IP | `10.0.0.1` |
| `${serverPort}` | 服务端端口 | `443` |
| `${reqCookies.xxx}` | 请求 cookie xxx 的值 | `session_id` |
| `${resCookies.xxx}` | 响应 cookie xxx 的值 | `token` |
| `${statusCode}` | 响应状态码 | `200` |
| `${env.xxx}` | process.env.xxx | `production` |

### 使用示例

```bash
# 内联值使用模板变量（注意反引号）
www.example.com resHeaders://`{X-Request-Id: ${randomUUID}}`

# 块变量使用模板变量
www.example.com file://`{my-template}`

# 小括号内容使用模板变量
www.example.com tpl://`({"time": ${now}})`
```

---

## 规则优先级

详细的规则优先级请参阅 [rule-priority.md](./rule-priority.md)。

### 匹配优先级

当多个规则匹配同一请求时，优先级顺序：

1. **精确匹配** > 路径匹配 > 通配符匹配 > 正则匹配
2. **更长的路径** > 更短的路径

### 规则执行优先级

| 规则类型 | 执行行为 | 说明 |
|---------|---------|------|
| **转发类** | 先定义的优先 | host, proxy, socks 等，只有第一个生效 |
| **修改类** | 后定义的覆盖 | reqHeaders, urlParams 等，相同字段后面覆盖前面 |

**转发类规则**（互斥，第一个匹配的生效）：
- `host`, `xhost`, `proxy`, `xproxy`, `socks`, `xsocks`, `tunnel`, `redirect`

**修改类规则**（可合并，后面覆盖前面）：
- `reqHeaders`, `resHeaders`, `reqCookies`, `resCookies`, `urlParams`
- `reqBody`, `resBody`, `statusCode`（最后一个生效）

---

## 文档导航

- [匹配模式详解](./patterns.md)
- [规则优先级与执行顺序](./rule-priority.md)
- [路由与转发规则](./routing.md)
- [请求修改规则](./request-modification.md)
- [响应修改规则](./response-modification.md)
- [URL 操作规则](./url-manipulation.md)
- [Body 操作规则](./body-manipulation.md)
- [状态码与重定向](./status-redirect.md)
- [延迟与限速规则](./timing-throttle.md)
- [过滤器规则](./filters.md)
- [脚本规则](./scripts.md)
- [WebSocket 规则](./websocket.md)
````
