---
title: 规则模式
description: Rules 内的匹配模式与优先级细节。
editUrl: false
---

> 此页面由 `docs/rules/patterns.md` 自动同步生成。

# 匹配模式详解

本章详细介绍 Bifrost 规则的 URL 匹配模式语法。

---

## 匹配模式概述

匹配模式用于指定哪些请求应该应用规则。Bifrost 支持多种匹配方式：

| 类型 | 示例 | 说明 |
|------|------|------|
| 域名匹配 | `www.example.com` | 精确匹配域名 |
| 路径匹配 | `www.example.com/api` | 匹配域名和路径 |
| 端口匹配 | `www.example.com:8080` | 匹配特定端口 |
| 通配符匹配 | `*.example.com` | 单级子域名通配 |
| 多级通配 | `**.example.com` | 多级子域名通配 |
| 正则匹配 | `/\/api\/v\d+/` | 正则表达式匹配 |

---

## 域名匹配

### 精确域名匹配

```bash
# 精确匹配 www.example.com
www.example.com host://127.0.0.1

# 不会匹配 api.example.com 或 example.com
```

### 裸域名匹配

```bash
# 匹配 example.com（不带子域名）
example.com host://127.0.0.1
```

### 测试用例

| 测试场景 | 规则模式 | 请求 URL | 是否匹配 |
|---------|---------|---------|---------|
| 精确域名 | `www.example.com` | `http://www.example.com/` | ✅ |
| 精确域名 | `www.example.com` | `http://api.example.com/` | ❌ |
| 精确域名 | `www.example.com` | `http://www.example.com/path` | ✅ |

---

## 路径匹配

### 路径前缀匹配

```bash
# 匹配所有以 /api 开头的路径
www.example.com/api host://api-server.local

# 匹配 /api, /api/users, /api/orders 等
```

### 精确路径匹配

```bash
# 只匹配特定路径
www.example.com/api/users$ host://users-service.local

# $ 表示路径结尾
```

### 测试用例

| 测试场景 | 规则模式 | 请求 URL | 是否匹配 |
|---------|---------|---------|---------|
| 路径前缀 | `example.com/api` | `http://example.com/api` | ✅ |
| 路径前缀 | `example.com/api` | `http://example.com/api/users` | ✅ |
| 路径前缀 | `example.com/api` | `http://example.com/v2/api` | ❌ |
| 精确路径 | `example.com/api$` | `http://example.com/api` | ✅ |
| 精确路径 | `example.com/api$` | `http://example.com/api/` | ❌ |

---

## 端口匹配

### 特定端口匹配

```bash
# 只匹配 8080 端口
www.example.com:8080 host://127.0.0.1

# 不会匹配 80 或 443 端口
```

### 默认端口

```bash
# HTTP 默认 80 端口
www.example.com host://127.0.0.1

# HTTPS 默认 443 端口
https://www.example.com host://127.0.0.1
```

### 测试用例

| 测试场景 | 规则模式 | 请求 URL | 是否匹配 |
|---------|---------|---------|---------|
| 特定端口 | `example.com:8080` | `http://example.com:8080/` | ✅ |
| 特定端口 | `example.com:8080` | `http://example.com/` | ❌ |
| 默认端口 | `example.com` | `http://example.com:80/` | ✅ |

---

## 通配符匹配

### 单级通配符 (*)

匹配一个域名层级（不包含点号）。

```bash
# 匹配 api.example.com, www.example.com
# 不匹配 a.b.example.com
*.example.com host://backend.local
```

### 多级通配符 (**)

匹配多个域名层级（包含点号）。

```bash
# 匹配 a.example.com, a.b.example.com, x.y.z.example.com
**.example.com host://backend.local
```

### 路径通配符

```bash
# 匹配 /api/users, /api/orders 等
www.example.com/api/* host://api-server.local

# 匹配任意深度路径
www.example.com/api/** host://api-server.local
```

### 测试用例

| 测试场景 | 规则模式 | 请求 URL | 是否匹配 |
|---------|---------|---------|---------|
| 单级通配 | `*.example.com` | `http://api.example.com/` | ✅ |
| 单级通配 | `*.example.com` | `http://a.b.example.com/` | ❌ |
| 多级通配 | `**.example.com` | `http://a.b.example.com/` | ✅ |
| 多级通配 | `**.example.com` | `http://example.com/` | ❌ |
| 路径通配 | `example.com/api/*` | `http://example.com/api/users` | ✅ |
| 路径通配 | `example.com/api/*` | `http://example.com/api/v1/users` | ❌ |
| 深度路径通配 | `example.com/api/**` | `http://example.com/api/v1/users` | ✅ |

---

## 正则表达式匹配

### 基础正则

```bash
# 匹配 /api/v1, /api/v2, /api/v3 等
/\/api\/v\d+/ host://api-server.local

# 正则以 / 开始和结束
```

### 大小写不敏感

```bash
# i 标志表示忽略大小写
/\/API\/users/i host://users-service.local

# 匹配 /API/users, /api/users, /Api/Users 等
```

### 捕获组

```bash
# 使用捕获组
/\/api\/(v\d+)\/users/ host://users-$1.service.local

# 将 /api/v2/users 的请求转发到 users-v2.service.local
```

### 常用正则模式

| 模式 | 说明 | 示例匹配 |
|------|------|---------|
| `\d+` | 一个或多个数字 | `123`, `456` |
| `\w+` | 字母数字下划线 | `user_1`, `abc` |
| `[a-z]+` | 小写字母 | `abc`, `xyz` |
| `.*` | 任意字符 | 任何内容 |
| `[^/]+` | 非斜杠字符 | 路径段 |

### 测试用例

| 测试场景 | 规则模式 | 请求 URL | 是否匹配 |
|---------|---------|---------|---------|
| 版本号匹配 | `/\/api\/v\d+/` | `http://example.com/api/v1` | ✅ |
| 版本号匹配 | `/\/api\/v\d+/` | `http://example.com/api/latest` | ❌ |
| 大小写不敏感 | `/\/api/i` | `http://example.com/API` | ✅ |
| 大小写不敏感 | `/\/api/i` | `http://example.com/Api` | ✅ |

---

## 协议匹配

### HTTP/HTTPS 匹配

```bash
# 只匹配 HTTP 请求
http://www.example.com host://127.0.0.1

# 只匹配 HTTPS 请求
https://www.example.com host://127.0.0.1

# 匹配所有协议（默认）
www.example.com host://127.0.0.1
```

### WebSocket 匹配

```bash
# 匹配 WebSocket 请求
ws://www.example.com host://ws-server.local

# 匹配安全 WebSocket
wss://www.example.com host://wss-server.local
```

### 测试用例

| 测试场景 | 规则模式 | 请求 URL | 是否匹配 |
|---------|---------|---------|---------|
| HTTP 协议 | `http://example.com` | `http://example.com/` | ✅ |
| HTTP 协议 | `http://example.com` | `https://example.com/` | ❌ |
| 默认匹配 | `example.com` | `http://example.com/` | ✅ |
| 默认匹配 | `example.com` | `https://example.com/` | ✅ |

---

## IP 地址匹配

### IPv4 匹配

```bash
# 匹配特定 IP
192.168.1.100 host://internal-server.local

# 匹配 IP 段
192.168.1.* host://lan-server.local
```

### IPv6 匹配

```bash
# 匹配 IPv6 地址
[::1] host://localhost-server.local
```

### 测试用例

| 测试场景 | 规则模式 | 请求 URL | 是否匹配 |
|---------|---------|---------|---------|
| IPv4 精确 | `192.168.1.1` | `http://192.168.1.1/` | ✅ |
| IPv4 通配 | `192.168.1.*` | `http://192.168.1.100/` | ✅ |

---

## 特殊匹配

### 全局匹配

```bash
# 匹配所有请求
* host://proxy-server.local
```

### 排除匹配

```bash
# 使用 ! 排除
!www.example.com host://127.0.0.1

# 排除特定路径
www.example.com !/static/* host://127.0.0.1
```

---

## 匹配优先级

当多个规则都匹配时，按以下优先级顺序应用：

1. **精确匹配** > 路径匹配 > 通配符匹配 > 正则匹配
2. **更长的路径** > 更短的路径
3. **更具体的规则** > 更通用的规则
4. **后定义的规则** > 先定义的规则（同优先级时）

### 优先级示例

```bash
# 规则定义顺序
*.example.com host://general-backend.local        # 优先级 3
www.example.com host://www-backend.local          # 优先级 1
www.example.com/api host://api-backend.local      # 优先级 2

# 请求 www.example.com/api/users 会匹配到 api-backend.local
```

---

## 测试用例汇总

| 测试场景 | 规则模式 | 请求 URL | 预期 |
|---------|---------|---------|------|
| 精确域名 | `test.com` | `http://test.com/` | 匹配 |
| 路径前缀 | `test.com/api` | `http://test.com/api/users` | 匹配 |
| 单级通配 | `*.test.com` | `http://api.test.com/` | 匹配 |
| 单级通配 | `*.test.com` | `http://a.b.test.com/` | 不匹配 |
| 多级通配 | `**.test.com` | `http://a.b.test.com/` | 匹配 |
| 端口匹配 | `test.com:8080` | `http://test.com:8080/` | 匹配 |
| 正则匹配 | `/\/api\/v\d+/` | `http://test.com/api/v1` | 匹配 |
| 大小写不敏感 | `/\/api/i` | `http://test.com/API` | 匹配 |
| HTTP 协议 | `http://test.com` | `https://test.com/` | 不匹配 |
| 全局匹配 | `*` | 任何 URL | 匹配 |
