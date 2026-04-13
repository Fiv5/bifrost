# 项目结构与模块说明

## 项目结构

```text
.
├── crates/
│   ├── bifrost-core/
│   ├── bifrost-proxy/
│   ├── bifrost-tls/
│   ├── bifrost-storage/
│   ├── bifrost-script/
│   ├── bifrost-admin/
│   ├── bifrost-cli/
│   ├── bifrost-e2e/
│   ├── bifrost-tests/
│   └── bifrost-sync/
├── web/
├── desktop/
├── docs/
├── e2e-tests/
└── tests/
```

## 模块说明

### `bifrost-core`

核心规则库，负责规则解析、匹配器和协议定义。

### `bifrost-proxy`

代理服务器实现，负责 HTTP/HTTPS/SOCKS5/WebSocket/隧道等协议处理。

### `bifrost-tls`

TLS 证书管理模块，负责 CA 证书生成、动态签发与缓存。

### `bifrost-storage`

配置、规则、Values、状态等持久化能力。

### `bifrost-script`

基于 QuickJS 的脚本引擎与沙箱执行环境。

### `bifrost-admin`

管理后台静态资源与 Admin API。

### `bifrost-cli`

命令行工具，提供服务启动、规则管理、流量查询、配置维护等命令。

### `bifrost-e2e`

Rust 端到端测试 runner。

### `bifrost-tests`

测试辅助 crate。

### `bifrost-sync`

远程同步模块，负责规则与配置的远程同步能力。
