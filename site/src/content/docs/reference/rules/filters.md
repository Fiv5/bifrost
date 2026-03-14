---
title: 过滤器
description: 请求与响应过滤条件的配置方式。
editUrl: false
---

> 此页面由 `docs/rules/filters.md` 自动同步生成。

# 过滤器规则

本章介绍控制规则生效条件的过滤器。

---

## includeFilter

包含过滤器，只有满足条件的请求才会应用规则。

### 语法

```
pattern rules... includeFilter://condition
```

### 过滤条件

| 条件类型   | 语法           | 说明           |
| ---------- | -------------- | -------------- |
| 方法       | `m:METHOD`     | 匹配请求方法   |
| 状态码     | `s:CODE`       | 匹配响应状态码 |
| 请求头     | `h:name=value` | 匹配请求头     |
| 响应头     | `H:name=value` | 匹配响应头     |
| 请求体包含 | `b:text`       | 请求体包含文本 |
| 响应体包含 | `B:text`       | 响应体包含文本 |
| IP         | `i:ip`         | 匹配客户端 IP  |
| 路径       | `p:/path`      | 匹配路径       |

### 方法过滤

```bash
# 只对 POST 请求生效
www.example.com resHeaders://(X-Method:POST) includeFilter://m:POST

# 只对 GET 请求生效
www.example.com resHeaders://(X-Method:GET) includeFilter://m:GET

# 只对 PUT 请求生效
www.example.com resHeaders://(X-Method:PUT) includeFilter://m:PUT
```

### 状态码过滤

> ⚠️ **注意**：小括号内不能有空格，含空格内容必须使用块变量

```bash
# 只对 500 响应生效
www.example.com replaceStatus://200 includeFilter://s:500

# 只对 404 响应生效（使用块变量）
www.example.com resBody://{not-found} includeFilter://s:404

# 对 4xx 响应生效
www.example.com resHeaders://(X-Error:true) includeFilter://s:4
```

块变量定义：

````
``` not-found
Not Found
```
````

### 请求头过滤

```bash
# 匹配带有特定头的请求
www.example.com host://debug.local includeFilter://h:X-Debug

# 匹配头部值
www.example.com host://admin.local includeFilter://h:X-Role=admin

# 匹配 Content-Type
www.example.com resHeaders://{X-Json: true} includeFilter://h:content-type=application/json
```

### 响应头过滤

```bash
# 匹配响应头
www.example.com resBody://(cached) includeFilter://H:X-Cache=HIT
```

> 注：`(cached)` 无空格，可使用行内值

### 路径过滤

```bash
# 匹配特定路径
www.example.com resHeaders://{X-Api: true} includeFilter://p:/api/

# 匹配路径模式
www.example.com resDelay://1000 includeFilter://p:/slow/
```

### 多条件组合

```bash
# AND 条件（同时满足）
www.example.com replaceStatus://200 includeFilter://m:POST includeFilter://s:500

# 方法 + 头部
www.example.com host://special.local includeFilter://m:POST includeFilter://h:X-Special
```

### 测试用例

| 测试场景   | 规则                                                   | 请求     | 预期         |
| ---------- | ------------------------------------------------------ | -------- | ------------ |
| POST 方法  | `test.com resHeaders://{X:1} includeFilter://m:POST`   | POST     | 应用规则     |
| POST 方法  | `test.com resHeaders://{X:1} includeFilter://m:POST`   | GET      | 不应用       |
| 状态码 500 | `test.com replaceStatus://200 includeFilter://s:500`   | 返回 500 | 状态码变 200 |
| 状态码 500 | `test.com replaceStatus://200 includeFilter://s:500`   | 返回 200 | 不变         |
| 头部匹配   | `test.com host://debug includeFilter://h:X-Debug=true` | 有头部   | 应用规则     |

---

## excludeFilter

排除过滤器，满足条件的请求不会应用规则。

### 语法

```
pattern rules... excludeFilter://condition
```

### 过滤条件

与 `includeFilter` 相同的条件语法。

### 示例

```bash
# 排除 GET 请求
www.example.com resDelay://1000 excludeFilter://m:GET

# 排除静态资源
www.example.com resHeaders://(X-Dynamic:true) excludeFilter://p:.js excludeFilter://p:.css

# 排除成功响应
www.example.com resHeaders://(X-Error:true) excludeFilter://s:200

# 排除特定头部
www.example.com host://default.local excludeFilter://h:X-Special
```

### 测试用例

| 测试场景 | 规则                                                | 请求 | 预期     |
| -------- | --------------------------------------------------- | ---- | -------- |
| 排除 GET | `test.com resHeaders://{X:1} excludeFilter://m:GET` | GET  | 不应用   |
| 排除 GET | `test.com resHeaders://{X:1} excludeFilter://m:GET` | POST | 应用规则 |

---

## ignore

忽略规则，使匹配的请求跳过后续规则处理。

### 语法

```
ignore://pattern
```

### 示例

```bash
# 忽略静态资源
ignore://*.js
ignore://*.css
ignore://*.png

# 忽略特定路径
ignore://www.example.com/health
ignore://www.example.com/metrics

# 忽略特定域名
ignore://internal.example.com
```

### 测试用例

| 测试场景 | 规则                     | 请求     | 预期         |
| -------- | ------------------------ | -------- | ------------ |
| 忽略路径 | `ignore://test.com/skip` | `/skip`  | 请求直接通过 |
| 忽略路径 | `ignore://test.com/skip` | `/other` | 继续匹配规则 |

---

## enable

启用特定功能或特性。

### 语法

```
pattern enable://feature
```

### 可用特性

| 特性        | 说明                     |
| ----------- | ------------------------ |
| `intercept` | 启用 HTTPS 拦截          |
| `hide`      | 隐藏请求（不在 UI 显示） |
| `abort`     | 中断请求                 |
| `proxy`     | 启用代理                 |

### 示例

```bash
# 启用 HTTPS 拦截
https://www.example.com enable://intercept

# 隐藏请求
www.example.com/internal enable://hide

# 中断请求
www.example.com/blocked enable://abort
```

### 测试用例

| 测试场景   | 规则                                  | 预期               |
| ---------- | ------------------------------------- | ------------------ |
| 中断请求   | `test.com enable://abort`             | 请求被中断         |
| HTTPS 拦截 | `https://test.com enable://intercept` | HTTPS 流量可被解密 |

---

## disable

禁用特定功能或特性。

### 语法

```
pattern disable://feature
```

### 可用特性

| 特性        | 说明                        |
| ----------- | --------------------------- |
| `intercept` | 禁用 HTTPS 拦截（隧道透传） |
| `proxy`     | 禁用代理                    |
| `cache`     | 禁用缓存                    |

### 示例

```bash
# 禁用 HTTPS 拦截
https://www.example.com disable://intercept

# 禁用缓存
www.example.com disable://cache
```

### 测试用例

| 测试场景 | 规则                                   | 预期           |
| -------- | -------------------------------------- | -------------- |
| 禁用拦截 | `https://test.com disable://intercept` | HTTPS 隧道透传 |

---

## delete

删除请求/响应中的头部、Cookie 或 URL 参数。

### 语法

```
pattern delete://target.name
```

### 目标类型

| 目标        | 语法              | 说明                |
| ----------- | ----------------- | ------------------- |
| 请求头      | `reqHeaders.name` | 删除请求头          |
| 响应头      | `resHeaders.name` | 删除响应头          |
| 请求 Cookie | `reqCookies.name` | 删除请求 Cookie     |
| 响应 Cookie | `resCookies.name` | 删除响应 Set-Cookie |
| URL 参数    | `urlParams.name`  | 删除 URL 参数       |

### 示例

```bash
# 删除请求头
www.example.com delete://reqHeaders.X-Custom

# 删除响应头
www.example.com delete://resHeaders.X-Powered-By

# 删除请求 Cookie
www.example.com delete://reqCookies.session

# 删除响应 Cookie
www.example.com delete://resCookies.tracking

# 删除 URL 参数
www.example.com delete://urlParams.debug
```

### 多个删除

```bash
# 删除多个头部
www.example.com delete://reqHeaders.X-A delete://reqHeaders.X-B

# 删除多个 Cookie
www.example.com delete://reqCookies.a delete://reqCookies.b
```

### 测试用例

| 测试场景    | 规则                                    | 预期                |
| ----------- | --------------------------------------- | ------------------- |
| 删除请求头  | `test.com delete://reqHeaders.X-Custom` | 请求不含 X-Custom   |
| 删除响应头  | `test.com delete://resHeaders.Server`   | 响应不含 Server     |
| 删除 Cookie | `test.com delete://reqCookies.session`  | Cookie 不含 session |
| 删除参数    | `test.com delete://urlParams.debug`     | URL 不含 debug 参数 |

---

## skip

跳过后续规则匹配。

### 语法

```
pattern skip://
```

### 示例

```bash
# 匹配后跳过
www.example.com/api skip://
```

---

## 规则组合

过滤器可以与其他规则组合使用：

```bash
# 多个过滤器
www.example.com host://backend.local includeFilter://m:POST excludeFilter://p:/health

# 过滤器 + 修改规则
www.example.com resHeaders://{X-Debug: true} includeFilter://h:X-Debug

# 删除 + 过滤器
www.example.com delete://reqHeaders.X-Internal includeFilter://m:GET

# 条件忽略
www.example.com ignore://pattern includeFilter://p:/static/
```

---

## 注意事项

1. **条件顺序**：多个 `includeFilter` 之间是 AND 关系
2. **优先级**：`excludeFilter` 优先于 `includeFilter`
3. **状态码过滤**：`s:` 过滤器用于响应阶段的规则
4. **头部大小写**：头部名称匹配不区分大小写
5. **性能考虑**：Body 过滤（`b:`/`B:`）需要读取整个 Body，可能影响性能
