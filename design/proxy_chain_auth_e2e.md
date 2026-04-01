# 双 Bifrost 代理链路鉴权 E2E 方案

## 功能模块描述

为 `proxy://` 路由补充一个真实端到端场景：启动两个独立的 Bifrost 代理服务，入口代理通过 `proxy://user:pass@127.0.0.1:<upstream_port>` 将请求转发到上游代理，上游代理再按 `host://` 规则转发到最终 mock 服务。

目标是覆盖两点：

- `proxy://` 规则可以把 HTTP 请求真正转发到另一个 Bifrost 代理
- `proxy://` 中配置的用户名密码会被编码并作为上游代理鉴权头带出
- 黑盒 shell E2E 能以脚本方式稳定复现上述链路

## 实现逻辑

- 在 `bifrost-proxy` 的 HTTP 请求处理链路中，为 `resolved_rules.proxy` 增加专门的“上游 HTTP 代理”发送路径
- 该路径直接连接 `proxy://` 指定的代理地址，并以 absolute-form 请求行把原始目标 URL 发给上游代理
- 当 `proxy://` 带有 `user:pass@` 时，构造 `Proxy-Authorization: Basic ...` 请求头
- 在 `bifrost-e2e` 中新增独立测试：
  - 启动最终 mock 服务
  - 启动上游 Bifrost，配置 `chain.test host://127.0.0.1:<mock_port>`
  - 启动入口 Bifrost，配置 `chain.test proxy://user:pass@127.0.0.1:<upstream_port>`
  - 通过入口代理请求 `http://chain.test/...`
  - 断言最终响应成功，且 mock 收到 `proxy-authorization` 头
- 在 `e2e-tests/tests/` 中新增 shell E2E：
  - 直接启动 release 版 `bifrost` 二进制，分别使用独立 `BIFROST_DATA_DIR`
  - 使用规则夹具渲染入口代理与上游代理的规则文件
  - 引入一个轻量 `proxy echo` 服务，专门校验下游代理是否收到了 `Proxy-Authorization`
  - 一个脚本同时覆盖“Bifrost -> Bifrost 代理链路成功”和“Bifrost -> proxy echo 鉴权成功”

## 依赖项

- `crates/bifrost-proxy` 现有 HTTP 代理处理逻辑
- `crates/bifrost-e2e` 的 `ProxyInstance`、`CurlCommand`、`EnhancedMockServer`
- `e2e-tests/tests` shell 框架、规则夹具渲染工具、进程管理工具
- `e2e-tests/mock_servers` 中新增的 `proxy_echo_server.py`
- `tokio` / `hyper` 现有 HTTP/1 客户端连接能力

## 测试方案

- 新增 `routing` 分类 Rust E2E：验证双代理链路与鉴权头透传
- 新增 shell E2E：验证 release 二进制、规则文件渲染、双代理进程启动、代理链路与下游鉴权
- 单独执行目标测试，避免一次跑全量
- 补充局部单元测试，校验 `proxy://user:pass@host:port` 解析逻辑

## 校验要求

- 先执行目标 E2E：
  - `cargo run -p bifrost-e2e -- --test routing_proxy_chain_with_auth`
  - `bash e2e-tests/tests/test_proxy_chain_auth_e2e.sh`
- 再执行项目校验：
  - `cargo test --workspace --all-features`
  - `rust-project-validate`

## 文档更新要求

- 本次为测试覆盖与实现补齐，不新增外部配置项
- 若实现语义与现有 `docs/rules/routing.md` 中“代理认证”描述不一致，再同步更新规则文档
