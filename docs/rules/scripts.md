# 脚本规则

本章介绍通过 JavaScript 脚本动态生成规则的功能，实现复杂的请求/响应处理逻辑。

---

## reqScript

在请求阶段通过 JavaScript 脚本动态生成规则。脚本可以访问请求上下文信息，并动态生成匹配规则。

### 语法

```
pattern reqScript://{script_name}
pattern reqScript:///path/to/script.js
pattern reqScript://http://example.com/script.js
```

### 可用全局变量

| 变量/方法           | 描述                              |
| ------------------- | --------------------------------- |
| `url`               | 完整请求 URL                      |
| `method`            | 请求方法 (GET/POST 等)            |
| `ip` / `clientIp`   | 客户端 IP 地址                    |
| `headers`           | 请求头对象                        |
| `body`              | 请求内容 (最大 16KB)              |
| `rules`             | 规则数组，通过 `push` 添加新规则  |
| `values`            | 临时值存储对象                    |
| `render(tpl, data)` | 微型模板渲染函数                  |
| `getValue(key)`     | 获取 Values 中的值                |
| `parseUrl`          | 同 Node.js 的 `url.parse`         |
| `parseQuery`        | 同 Node.js 的 `querystring.parse` |

### 示例

#### 基础用法

```bash
www.example.com/api reqScript://{api-router}
```

块变量定义：

````
``` api-router
if (method === 'GET') {
    rules.push('* resType://json');
    rules.push('* file://({"status":"ok"})');
} else if (method === 'POST') {
    rules.push('* statusCode://201');
} else {
    rules.push('* statusCode://405');
}
```
````

#### 基于请求头路由

```bash
www.example.com reqScript://{header-router}
```

````
``` header-router
var token = headers['authorization'];
if (!token) {
    rules.push('* statusCode://401');
    rules.push('* resBody://({"error":"Unauthorized"})');
} else if (token.indexOf('admin') >= 0) {
    rules.push('* host://admin-server:8080');
} else {
    rules.push('* host://user-server:8080');
}
```
````

#### 基于 URL 参数处理

```bash
www.example.com reqScript://{param-handler}
```

````
``` param-handler
var query = parseQuery(parseUrl(url).query || '');
var version = query.v || '1';

if (version === '2') {
    rules.push('* host://api-v2.example.com');
} else {
    rules.push('* host://api-v1.example.com');
}
```
````

### 测试用例

| 测试场景  | 规则                                | 预期               |
| --------- | ----------------------------------- | ------------------ |
| GET 请求  | 脚本判断 `method === 'GET'`         | 执行 GET 分支规则  |
| POST 请求 | 脚本判断 `method === 'POST'`        | 执行 POST 分支规则 |
| 无 Token  | 脚本检查 `headers['authorization']` | 返回 401           |
| 带 Token  | 脚本检查 `headers['authorization']` | 转发到对应服务器   |

---

## resScript

在响应阶段通过 JavaScript 脚本动态生成规则。与 `reqScript` 的区别是执行时机在响应阶段，适合基于响应内容动态处理。

### 语法

```
pattern resScript://{script_name}
pattern resScript:///path/to/script.js
pattern resScript://http://example.com/script.js
```

### 可用全局变量

与 `reqScript` 相同，但执行时机不同。

> ⚠️ **注意**：由于 `resScript` 在响应阶段执行，某些规则（如 `file`、`host` 等请求阶段规则）在此阶段生成可能不会生效。

### 示例

#### 基于响应状态码处理

```bash
www.example.com resScript://{status-handler}
```

````
``` status-handler
// 注意：resScript 适合用于响应阶段的规则
// 如修改响应头、响应体等
if (method === 'GET') {
    rules.push('* resHeaders://(X-Processed:true)');
}
```
````

#### 添加响应标记

```bash
www.example.com resScript://{add-marker}
```

````
``` add-marker
rules.push('* resHeaders://(X-Proxy:whistle)');
rules.push('* resHeaders://(X-Timestamp:' + Date.now() + ')');
```
````

### 测试用例

| 测试场景   | 规则                                    | 预期             |
| ---------- | --------------------------------------- | ---------------- |
| 添加响应头 | 脚本 `rules.push('* resHeaders://...')` | 响应包含新增头部 |
| 修改响应体 | 脚本 `rules.push('* resAppend://...')`  | 响应体被追加内容 |

---

## reqScript vs resScript 对比

| 特性       | reqScript            | resScript      |
| ---------- | -------------------- | -------------- |
| 执行时机   | 请求阶段             | 响应阶段       |
| 可用规则   | 所有规则             | 仅响应阶段规则 |
| 适用场景   | 路由、请求修改、Mock | 响应修改、日志 |
| host/proxy | ✅ 生效              | ❌ 不生效      |
| file/tpl   | ✅ 生效              | ❌ 不生效      |
| resHeaders | ✅ 生效              | ✅ 生效        |
| resBody    | ✅ 生效              | ✅ 生效        |

---

## 脚本调试技巧

### 1. 使用 console.log

```javascript
console.log("URL:", url);
console.log("Method:", method);
console.log("Headers:", JSON.stringify(headers));
```

### 2. 错误处理

```javascript
try {
  var data = JSON.parse(body);
  // 处理逻辑
} catch (e) {
  console.log("Parse error:", e.message);
  rules.push("* statusCode://400");
}
```

### 3. 条件组合

```javascript
var isApi = url.indexOf("/api/") >= 0;
var isGet = method === "GET";
var hasToken = !!headers["authorization"];

if (isApi && isGet && hasToken) {
  rules.push("* host://api-server");
} else if (isApi && !hasToken) {
  rules.push("* statusCode://401");
}
```

---

## 使用场景

### 1. A/B 测试路由

```bash
www.example.com reqScript://{ab-test}
```

````
``` ab-test
var userId = headers['x-user-id'] || '';
var hash = 0;
for (var i = 0; i < userId.length; i++) {
````

    hash = ((hash << 5) - hash) + userId.charCodeAt(i);

}
if (Math.abs(hash) % 100 < 50) {
rules.push('_ host://experiment-a.example.com');
} else {
rules.push('_ host://experiment-b.example.com');
}

```

```

### 2. 请求签名验证

```bash
www.example.com/api reqScript://{sign-verify}
```

````
``` sign-verify
var sign = headers['x-signature'];
var timestamp = headers['x-timestamp'];
var now = Date.now();

if (!sign || !timestamp) {
    rules.push('* statusCode://401');
    rules.push('* resBody://({"error":"Missing signature"})');
} else if (now - parseInt(timestamp) > 300000) {
    rules.push('* statusCode://401');
    rules.push('* resBody://({"error":"Request expired"})');
}
// 签名验证通过则不添加规则，正常转发
```
````

### 3. 动态 Mock 数据

```bash
www.example.com/api/users reqScript://{dynamic-mock}
```

````
``` dynamic-mock
var query = parseQuery(parseUrl(url).query || '');
var page = parseInt(query.page) || 1;
var size = parseInt(query.size) || 10;

values.mockData = JSON.stringify({
    page: page,
    size: size,
    total: 100,
    data: []
});

rules.push('* resType://json');
rules.push('* resBody://{mockData}');
```
````

---

## 注意事项

1. **脚本大小**：脚本内容不宜过大，复杂逻辑建议使用插件开发
2. **执行时间**：脚本应快速执行，避免阻塞请求
3. **安全性**：脚本中不要包含敏感信息
4. **调试**：使用 `console.log` 输出调试信息，可在 Whistle Network 面板查看
5. **规则顺序**：通过 `rules.push` 添加的规则按顺序执行

---

## 关联协议

- [reqRules](./reqRules.md) - 请求阶段批量规则（不支持脚本）
- [resRules](./resRules.md) - 响应阶段批量规则（不支持脚本）
