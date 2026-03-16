# TLS MITM ServerConfig 缓存

## 背景

当前 MITM 连接路径已经具备按域名缓存动态证书的能力，但 `rustls::ServerConfig` 仍在每次新连接时现建：

- HTTP CONNECT MITM：每次拦截连接都会重新构建一次 `ServerConfig` 并设置 ALPN。
- SOCKS TLS MITM：每次拦截连接都会重新构建一次 `ServerConfig`。

这会把本可复用的 TLS 服务端配置重复分配到热路径上，带来额外的 CPU 与短期内存抖动。

## 功能模块详细描述

- 为 MITM TLS 服务端配置增加共享缓存。
- 缓存粒度为 `域名 + ALPN 协议列表`，避免 HTTP/2 和无 ALPN 场景错误复用同一配置。
- 缓存挂载在共享的 TLS 解析组件上，使 HTTP CONNECT MITM 与 SOCKS TLS MITM 复用同一实现。

## 实现逻辑

### 1. 在 `bifrost-tls` 增加 `ServerConfigCache`

- 使用 LRU 缓存保存 `Arc<rustls::ServerConfig>`。
- 键包含：
  - `domain`
  - `alpn_protocols`
- 容量策略与现有证书缓存保持一致，默认 1000。

### 2. 在 `SniResolver` 内收敛缓存逻辑

- 保留现有 `CertCache`，继续负责证书缓存。
- 新增 `ServerConfigCache`，负责 MITM `ServerConfig` 缓存。
- `resolve_server_config_with_alpn()` 的流程：
  1. 先按 `domain + ALPN` 查询 `ServerConfigCache`
  2. 未命中时复用现有证书缓存结果
  3. 构建新的 `ServerConfig`
  4. 写回缓存并返回 `Arc<ServerConfig>`

### 3. 代理侧统一通过 `TlsConfig::resolve_server_config()` 获取配置

- HTTP CONNECT MITM：
  - 传入 `["h2", "http/1.1"]`
- SOCKS TLS MITM：
  - 传入空 ALPN
- 当存在 `sni_resolver` 时走缓存路径；仅保留 `cert_generator` 的回退路径维持兼容。

## 依赖项

- 复用现有 `lru` 与 `parking_lot`
- 复用现有 `DynamicCertGenerator`
- 复用现有 `SniResolver` 与 `SingleCertResolver`

## 测试方案（含 e2e）

- 单元测试：
  - 验证 `ServerConfigCache` 可读写
  - 验证不同 ALPN 不会错误复用同一缓存项
  - 验证 `SniResolver` 对相同 `domain + ALPN` 返回同一个 `Arc<ServerConfig>`
  - 验证 `clear_cache()` 会同时清理证书缓存与 `ServerConfig` 缓存
- 代理侧测试：
  - 验证 `TlsConfig::resolve_server_config()` 在启用 `sni_resolver` 时走缓存
- E2E：
  - 执行与 TLS MITM 相关的现有代理测试，确认行为不变

## 校验要求（含 rust-project-validate）

- 先执行本次任务相关 E2E / 单测
- 再执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - 按修改范围执行 `cargo test`
  - `cargo build --all-targets --all-features`

## 文档更新要求

- 本次不引入新的用户可见配置项，也不改变 CLI / API 行为
- `README.md` 无需更新
