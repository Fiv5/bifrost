# CONNECT 命中非 TLS 改写时自动启用 TLS 拦截

## 功能模块描述

- 修复 HTTPS / WSS 的 CONNECT 请求命中 `https -> http://...` 或 `wss -> ws://...` 改写规则时，未自动启用 TLS interception 导致浏览器出现 `ERR_SSL_PROTOCOL_ERROR` 的问题。
- 覆盖 HTTP CONNECT 与 SOCKS5 TLS 两条入口，保持规则行为一致。

## 实现逻辑

- 在 CONNECT / SOCKS 的 TLS 拦截决策中，除了现有的全局开关、include / exclude、`tlsIntercept://` / `tlsPassthrough://` 规则外，额外识别“命中非 TLS 上游改写”的场景。
- 当 `resolved_rules.host` 存在且 `host_protocol` 为 `http` 或 `ws` 时，说明客户端发起的是 TLS CONNECT，但目标规则要求把解密后的请求转发到明文上游，此时必须先做 TLS interception。
- 该自动拦截优先级低于显式规则：
  - `tlsIntercept://` 仍然强制拦截
  - `tlsPassthrough://` 仍然强制透传
- 仅在本地已有 CA 证书可用时生效；若 CA 不可用，维持现有行为。

## 依赖项

- `crates/bifrost-proxy/src/proxy/http/tunnel/mod.rs`
- `crates/bifrost-proxy/src/proxy/socks/tcp.rs`

## 测试方案

- 新增单元测试覆盖：
  - 全局 TLS interception 关闭时，`https -> http://localhost` 规则会自动启用拦截
  - `wss -> ws://localhost` 规则会自动启用拦截
  - 显式 `tlsPassthrough://` 仍能覆盖自动拦截
- 执行相关 Rust 单测验证 CONNECT / SOCKS 共用决策逻辑。
- 按仓库要求执行 `e2e-test`，随后执行 `rust-project-validate`。

## 校验要求

- 先执行 `e2e-test`
- 再执行 `rust-project-validate`
- 额外执行本次修改涉及的 `cargo test` 定向单测

## 文档更新要求

- 本次为行为修复，无需更新 `README.md`
