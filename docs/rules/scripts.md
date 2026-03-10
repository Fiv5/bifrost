# 脚本规则

本章介绍通过 JavaScript 脚本对请求/响应进行处理的能力：

- `reqScript://{script_name}`：请求阶段脚本（转发到上游前执行）
- `resScript://{script_name}`：响应阶段脚本（收到上游响应后执行）
- `decode://{script_name}`：body decode 脚本（请求/响应落库前执行，用于解码/脱敏/格式化）

> 说明：脚本名称对应 `~/.bifrost/scripts/{type}/{script_name}.js`。

---

## reqScript

### 语法

```
pattern reqScript://my-script
```

### 可用全局变量

| 变量 | 说明 |
| --- | --- |
| `request` | 请求对象（可修改 `method` / `headers` / `body`） |
| `ctx` | 执行上下文（含 `requestId` / `values` / `matchedRules` 等） |
| `log` / `console` | 日志（会在管理端脚本测试面板展示） |
| `file` | 文件 API（受沙箱目录与白名单限制） |
| `net` | 网络 API（可开关/限速/限超时） |

### 示例

```javascript
// 给所有请求加 header，并记录到沙箱文件
request.headers["X-Debug-Id"] = ctx.requestId;
file.appendText("state/trace.log", ctx.requestId + "\n");
```

---

## resScript

### 语法

```
pattern resScript://my-script
```

### 可用全局变量

| 变量 | 说明 |
| --- | --- |
| `response` | 响应对象（可修改 `status` / `statusText` / `headers` / `body`） |
| `ctx` | 执行上下文 |
| `log` / `console` | 日志 |
| `file` | 文件 API |
| `net` | 网络 API |

### 示例

```javascript
// 给响应加调试头
response.headers["X-Processed-By"] = "bifrost";
```

---

## decode

decode 脚本用于在 **落库之前** 对请求/响应的 body 做解码、脱敏、压缩/解压后的二次处理等。

### 语法

```
pattern decode://my-decode
```

### 执行阶段

- `ctx.phase === "request"`：解码请求体（此时 `response === null`）
- `ctx.phase === "response"`：解码响应体（此时 `response.request` 带有请求快照）
- `ctx.phase === "websocket_send"`：解码 WebSocket 客户端→服务端帧 payload（payload 作为 requestBodyBytes）
- `ctx.phase === "websocket_recv"`：解码 WebSocket 服务端→客户端帧 payload（payload 作为 responseBodyBytes）

### 内置解码器

- `decode://utf8`：内置 UTF-8（lossy）解码器
- `decode://default`：等价于 `decode://utf8`

### 输出约定

decode 脚本需要输出一个 JSON 对象：

```javascript
// 推荐：直接 return
return { code: "0", data: "decoded text", msg: "" };

// 也支持：设置 ctx.output
// ctx.output = { code: "0", data: "decoded text", msg: "" };
```

- `code === "0"`：成功，`data` 会作为新的 body 内容用于落库
- 否则：`msg` 会作为新的 body 内容用于落库（便于排查失败原因）

---

## 沙箱与配置

### file API

- 读写路径默认相对 `sandbox.file.sandbox_dir`（通常为 `scripts/_sandbox/`）
- 相对路径禁止 `..`，避免目录穿越
- 绝对路径仅允许访问 `sandbox.file.allowed_dirs` 白名单中的目录
- 单次读写大小受 `sandbox.file.max_bytes` 限制

可用方法：

- `file.readText(path)`
- `file.writeText(path, content)`
- `file.appendText(path, content)`
- `file.exists(path)`
- `file.remove(path)`
- `file.listDir(path?)`

### net API

- `net.fetch(url, optionsJson?)` / `net.request(...)` 返回 JSON 字符串，建议 `JSON.parse(...)`
- 仅允许 `http/https`
- 请求/响应体大小与超时分别受 `sandbox.net.max_request_bytes` / `sandbox.net.max_response_bytes` / `sandbox.net.timeout_ms` 限制

`optionsJson` 示例：

```javascript
var resp = JSON.parse(net.fetch("https://httpbin.org/get", JSON.stringify({
  method: "GET",
  timeoutMs: 3000,
  headers: { "X-Debug": "1" },
})));
log.info("status:", resp.status);
```

### config.toml

配置位于 `~/.bifrost/config.toml` 的 `sandbox` 字段下：

```toml
[sandbox.file]
sandbox_dir = "_sandbox"              # 相对 scripts/ 的目录名，或绝对路径
allowed_dirs = ["/var/log"]           # 允许访问的系统目录（绝对路径）
max_bytes = 1048576                    # 单次文件读写最大字节数

[sandbox.net]
enabled = true
timeout_ms = 5000
max_request_bytes = 262144
max_response_bytes = 1048576

[sandbox.limits]
timeout_ms = 10000
max_memory_bytes = 33554432
max_decode_input_bytes = 2097152
max_decompress_output_bytes = 10485760
```

说明：

- `max_memory_bytes`：QuickJS 沙箱内存上限，超出会导致脚本失败
- `max_decode_input_bytes`：decode 输入 bytes 上限，超过会跳过 decode（避免大 payload 解码造成性能/内存风险）
- `max_decompress_output_bytes`：HTTP body 解压输出上限，超过会放弃解压并回退到原始压缩数据（避免压缩炸弹）

### 管理端动态修改

在管理端 **Scripts** 页面左侧目录树顶部点击齿轮按钮，可以在线修改 `sandbox` 配置，并持久化到 `config.toml`。
