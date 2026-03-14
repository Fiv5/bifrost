---
title: 规则引擎
description: Bifrost 规则系统的能力全景与配置入口。
editUrl: false
---

> 此页面由 `docs/rule.md` 自动同步生成。

# 规则语法

Bifrost 通过简洁的规则配置来修改请求和响应。

## 语法结构

```txt
pattern operation [operations...] [filters...] [lineProps://...]
```

| 组成部分      | 是否必填 | 描述                                                          |
| :------------ | :------- | :------------------------------------------------------------ |
| **pattern**   | 是       | 匹配请求 URL 的表达式，详见 [pattern](./pattern.md)           |
| **operation** | 是       | 操作指令 `protocol://value`，详见 [operation](./operation.md) |
| **filters**   | 否       | 过滤条件，详见下文                                            |
| **lineProps** | 否       | 规则属性，详见下文                                            |

## Pattern 类型

Pattern 根据格式自动识别类型，优先级影响匹配顺序：

| 类型     | 格式示例                        | 优先级 |
| :------- | :------------------------------ | :----- |
| Domain   | `example.com` `example.com/api` | 100    |
| IP/CIDR  | `192.168.1.1` `192.168.0.0/16`  | 95     |
| Regex    | `/pattern/` `/pattern/i`        | 80     |
| Wildcard | `*.example.com` `$host` `api?`  | 55     |

取反匹配：所有类型均支持 `!` 前缀，如 `!*.example.com`

## 高级配置

### 1. 组合配置

单条规则支持多个操作指令：

```txt
www.example.com file:///static-files cache://3600 resCors://*
chatgpt.com http3://
```

### 2. 位置调换

operation 和 pattern 可调换位置，便于批量配置：

```txt
proxy://127.0.0.1:8080 www.example.com api.example.com
```

### 3. 简写支持

`host:port[/path]` 格式自动识别为 `host://` 协议：

```txt
example.com 127.0.0.1:3000/api
# 等价于
example.com host://127.0.0.1:3000/api
```

### 4. 多行配置

**反斜杠续行**：行末 `\` 将下一行合并

```txt
example.com \
host://127.0.0.1 \
reqHeaders://{test=1}
```

**line 块语法**：块内换行自动转空格

```txt
line`
proxy://127.0.0.1:8080
www.example.com
api.example.com
includeFilter://m:GET
excludeFilter:///admin/
`
```

### 5. 过滤器

通过 `includeFilter://` 和 `excludeFilter://` 添加过滤条件：

```txt
example.com host://127.0.0.1 includeFilter://m:GET excludeFilter:///admin/
```

**过滤条件类型**：

| 前缀      | 说明       | 示例                                |
| :-------- | :--------- | :---------------------------------- |
| `m:`      | HTTP 方法  | `m:GET` `m:GET,POST,PUT`            |
| `s:`      | 状态码     | `s:200` `s:200-299` `s:200,404,500` |
| `h:`      | 请求头存在 | `h:X-Custom-Header`                 |
| `reqH:`   | 请求头匹配 | `reqH:Content-Type=/json/`          |
| `resH:`   | 响应头匹配 | `resH:Content-Type=/json/`          |
| `i:`      | 客户端 IP  | `i:192.168.1.1` `i:192.168.0.0/16`  |
| `b:`      | 响应体匹配 | `b:/error/`                         |
| `/path/`  | 路径包含   | `/api/`                             |
| `/regex/` | 路径正则   | `/^\/api\/v\d+/`                    |

### 6. 规则属性

通过 `lineProps://` 设置规则属性：

| 属性        | 说明                 |
| :---------- | :------------------- |
| `important` | 提升优先级（+10000） |
| `disabled`  | 禁用规则             |

```txt
example.com host://127.0.0.1 lineProps://important
example.com host://127.0.0.1 lineProps://important,disabled
```

### 7. 变量替换

使用 `{varName}` 引用预定义变量，支持嵌套展开（最多 10 次迭代）：

```txt
example.com host://{myHost}
example.com resBody://{mockBody}
```

`${varName}` 格式为模板变量，不会被预处理展开。

## 注意事项

### 规则优先级

1. `lineProps://important` 规则优先匹配
2. 相同优先级按 Pattern 类型：Domain > IP > Regex > Wildcard
3. 同类型规则按从上到下顺序匹配

### 调试技巧

1. **逐步验证**：从简单规则开始，逐步添加复杂条件
2. **日志查看**：使用 Bifrost Network 界面的 Overview 面板查看规则匹配情况
3. **临时禁用**：使用 `#` 注释或 `lineProps://disabled` 暂时禁用规则

### 上游 HTTP/3 规则

`http3://` 用于为命中的请求启用“代理到目标服务”的上游 HTTP/3 尝试，默认关闭。

```txt
chatgpt.com http3://
api.example.com h3://
```

- `h3://` 是 `http3://` 的别名
- 仅在代理自己能够读取 HTTP 请求时生效
- 对普通绝对 URI 代理请求可直接生效
- 对浏览器常见的 HTTPS `CONNECT` 流量，通常需要启用 TLS interception 后，代理才能在解密后的上游转发阶段尝试 H3
- 纯 `CONNECT` 透传隧道不会把上游 TCP 连接自动切换成 QUIC/H3

## 扩展阅读

- [规则协议手册](./rules/README.md)：按协议查看各能力说明与示例
