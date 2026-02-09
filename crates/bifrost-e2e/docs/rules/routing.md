# 路由与转发规则

本章介绍控制请求目标地址和转发方式的规则。

---

## host

将请求重定向到指定主机，是最常用的路由规则。

### 语法

```
pattern host://target[:port]
```

### 参数说明

| 参数     | 说明            | 示例                           |
| -------- | --------------- | ------------------------------ |
| `target` | 目标主机名或 IP | `127.0.0.1`, `api.backend.com` |
| `port`   | 可选，目标端口  | `8080`, `3000`                 |

### 基础示例

```bash
# 域名重定向到本地
www.example.com host://127.0.0.1

# 域名重定向到指定端口
www.example.com host://127.0.0.1:8080

# 域名重定向到另一域名
www.example.com host://api.backend.com

# 带端口的目标
www.example.com host://api.backend.com:3000
```

### 通配符匹配

```bash
# 单级子域名通配（匹配 a.example.com, b.example.com）
*.example.com host://backend.local

# 多级子域名通配（匹配 a.b.example.com, x.y.z.example.com）
**.example.com host://backend.local
```

### 路径匹配

```bash
# 匹配特定路径
www.example.com/api host://api-server.local

# 路径通配
www.example.com/api/* host://api-server.local
```

### 测试用例

| 测试场景     | 规则                                    | 请求                            | 预期                   |
| ------------ | --------------------------------------- | ------------------------------- | ---------------------- |
| 基础重定向   | `test.com host://127.0.0.1:MOCK_PORT`   | `GET http://test.com/`          | 请求到达 Mock 服务器   |
| 带端口重定向 | `test.com host://127.0.0.1:8888`        | `GET http://test.com/`          | 请求转发到 8888 端口   |
| 路径保留     | `test.com host://127.0.0.1:MOCK_PORT`   | `GET http://test.com/api/users` | 路径 `/api/users` 保留 |
| 通配符匹配   | `*.test.com host://127.0.0.1:MOCK_PORT` | `GET http://api.test.com/`      | 匹配成功               |

---

## xhost

与 `host` 类似，但即使请求被其他规则处理，`xhost` 仍然会执行。

### 语法

```
pattern xhost://target[:port]
```

### 示例

```bash
www.example.com xhost://127.0.0.1:8080
```

---

## proxy

通过 HTTP 代理转发请求。

### 语法

```
pattern proxy://proxy_host:proxy_port
```

### 参数说明

| 参数         | 说明           |
| ------------ | -------------- |
| `proxy_host` | 代理服务器地址 |
| `proxy_port` | 代理服务器端口 |

### 示例

```bash
# 通过代理转发所有请求
* proxy://proxy.company.com:8080

# 特定域名通过代理
*.internal.com proxy://proxy.internal:3128

# 带认证的代理（通过 URL）
example.com proxy://user:pass@proxy.com:8080
```

### 测试用例

| 测试场景      | 规则                                              | 预期                   |
| ------------- | ------------------------------------------------- | ---------------------- |
| HTTP 代理转发 | `test.com proxy://127.0.0.1:PROXY_PORT`           | 请求通过代理服务器转发 |
| 代理认证      | `test.com proxy://user:pass@127.0.0.1:PROXY_PORT` | 代理收到认证信息       |

---

## xproxy

与 `proxy` 类似，但始终执行，不受其他规则影响。

### 语法

```
pattern xproxy://proxy_host:proxy_port
```

---

## https-proxy

通过 HTTPS 代理转发请求。

### 语法

```
pattern https-proxy://proxy_host:proxy_port
```

### 示例

```bash
*.example.com https-proxy://secure-proxy.com:443
```

---

## socks

通过 SOCKS 代理转发请求。

### 语法

```
pattern socks://proxy_host:proxy_port
```

### 参数说明

支持 SOCKS4、SOCKS4a、SOCKS5 协议。

### 示例

```bash
# SOCKS5 代理
* socks://127.0.0.1:1080

# 带认证的 SOCKS 代理
example.com socks://user:pass@socks-proxy.com:1080
```

### 测试用例

| 测试场景    | 规则                                    | 预期                    |
| ----------- | --------------------------------------- | ----------------------- |
| SOCKS5 转发 | `test.com socks://127.0.0.1:SOCKS_PORT` | 请求通过 SOCKS 代理转发 |

---

## xsocks

与 `socks` 类似，但始终执行。

### 语法

```
pattern xsocks://proxy_host:proxy_port
```

---

## tunnel

隧道透传，不拦截请求内容，直接转发。

### 语法

```
pattern tunnel://target_host:target_port
```

### 使用场景

- 需要完全透明转发的场景
- 自定义协议穿透
- 避免 SSL 拦截

### 示例

```bash
# 透传到指定服务器
secure.example.com tunnel://backend.internal:443

# WebSocket 透传
ws.example.com tunnel://ws-server.internal:8080
```

### 测试用例

| 测试场景       | 规则                                    | 预期                 |
| -------------- | --------------------------------------- | -------------------- |
| HTTPS 隧道透传 | `test.com tunnel://127.0.0.1:MOCK_PORT` | 请求直接转发，不解密 |

---

## pac

使用 PAC (Proxy Auto-Config) 脚本决定路由。

### 语法

```
pattern pac://pac_script_url
pattern pac://{pac-script}
```

### 示例

> ⚠️ **注意**：小括号内不能有空格，PAC 脚本必须使用块变量

```bash
# 远程 PAC 文件
* pac://http://proxy.company.com/proxy.pac

# 内联 PAC 脚本（使用块变量）
* pac://{proxy-pac}
```

块变量定义：

````
``` proxy-pac
function FindProxyForURL(url, host) { return "PROXY proxy.com:8080"; }
```
````

---

## 规则组合

路由规则可以与其他规则组合使用：

```bash
# 路由 + 请求头修改
www.example.com host://backend.local reqHeaders://(X-Forwarded-Host:www.example.com)

# 路由 + 过滤器
www.example.com host://backend.local includeFilter://m:GET

# 路由 + 响应修改
www.example.com host://backend.local resCors://*
```

---

## 注意事项

1. **端口保留**：使用 `host` 时，原始请求的路径和查询参数会保留
2. **Host 头部**：默认情况下，`Host` 头部会更新为目标主机
3. **HTTPS 处理**：对于 HTTPS 请求，需要安装 Whistle 证书才能进行内容修改
4. **优先级**：`xhost`/`xproxy`/`xsocks` 比普通版本优先级更高
