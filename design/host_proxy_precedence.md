# host 与 proxy 规则优先级修正方案

## 功能模块详细描述
- 修正当同一请求同时命中 `host://...` 与 `proxy://...` 时的实际转发行为。
- 期望语义与现有 E2E 命名一致：`host` 优先于 `proxy`，命中 `host` 后应直接按 host 目标转发，而不是继续走上游代理。

## 实现逻辑
- 保留规则解析与匹配结果，允许日志继续显示同时命中的协议，便于排查。
- 在 HTTP 上游发送阶段新增“是否使用上游代理”的统一判定：
- 当 `resolved_rules.host` 存在且未被忽略时，不使用 `resolved_rules.proxy`。
- 当 host 被忽略或不存在时，仍允许 `proxy` 生效。
- 复用同一判定到 HTTP/3 预判逻辑，避免 host 已生效时仍错误禁用直连上游路径。

## 依赖项
- `crates/bifrost-proxy/src/proxy/http/handler.rs`
- `crates/bifrost-e2e/src/tests/routing.rs`
- `crates/bifrost-e2e/src/tests/rule_priority.rs`

## 测试方案
- 执行 bifrost-e2e 定向回归：
- `cargo run -p bifrost-e2e -- --test routing_host_vs_proxy`
- `cargo run -p bifrost-e2e -- --test priority_host_vs_proxy`
- 执行 `bifrost-proxy` 相关单元测试，补充 host/proxy 优先级判定覆盖。
- 最终执行 rust-project-validate 要求的格式、lint、构建和工作区测试。

## 校验要求
- 两个失败用例必须恢复通过，且重试路径不再出现 502。
- `cargo clippy --all-targets --all-features -- -D warnings` 必须通过。
- `cargo test --workspace --all-features` 必须通过。

## 文档更新要求
- 本次不涉及 README、协议列表、Hook 表或 CLI 配置文档更新。
