# TLS MITM HTTP/2 WebSocket 兼容

## 功能模块详细描述

- 修复 TLS 解包场景下浏览器与代理协商 `h2` 后，`wss` 无法建立连接的问题。
- 保持浏览器与代理自动协商最高可用协议，不通过降级 `http/1.1` 规避问题。

## 实现逻辑

### 1. 打开 TLS 解包服务端的 HTTP/2 extended CONNECT

- 在 TLS MITM 连接的 `hyper-util` HTTP/2 builder 上启用 `enable_connect_protocol()`。
- 让浏览器在 `h2` 下发起的 WebSocket extended CONNECT 能被代理正确接收。

### 2. 将 H2 WebSocket 请求纳入现有拦截链路

- 在拦截入口同时识别两类 WebSocket 请求：
  - HTTP/1.1 `Upgrade: websocket`
  - HTTP/2 `CONNECT` + `:protocol = websocket`
- H2 WebSocket 继续复用现有的握手转发、连接监控、帧捕获与流量记录逻辑。

### 3. 兼容上游仍是 HTTP/1.1 WebSocket 握手的场景

- 当下游是 H2 extended CONNECT、但上游仍按 HTTP/1.1 方式建立 WebSocket 时：
  - 代理为上游补齐 `Sec-WebSocket-Key`
  - 对下游返回 `200`，而不是 HTTP/1.1 的 `101`
  - 保留 `Sec-WebSocket-Protocol` / `Sec-WebSocket-Extensions` 的协商结果

## 依赖项

- 复用现有 `hyper` / `hyper-util` / `tokio-rustls`
- 复用现有 WebSocket 握手与帧转发实现

## 测试方案（含 e2e）

- 单测：
  - 验证 H2 extended CONNECT WebSocket 请求可被识别为 WebSocket
- 集成测试：
  - `CONNECT -> TLS MITM -> HTTP/2 extended CONNECT websocket -> ws echo`
  - 确认浏览器侧 ALPN 仍协商到 `h2`
  - 确认 WebSocket 文本帧可正常往返
- E2E：
  - 执行本次修改范围内的 TLS / WebSocket 相关测试

## 校验要求（含 rust-project-validate）

- 先执行本次修改相关测试
- 再执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - 按修改范围执行 `cargo test`
  - `cargo build --all-targets --all-features`

## 文档更新要求

- 本次不新增用户配置项
- `README.md` 无需更新
