# 管理端 TLS 白名单变更后的重连提示

## 功能模块详细描述

- 在管理端操作 TLS 白名单后，成功提示除了展示“已添加/已移除”结果，还要明确提醒用户重启目标应用并重新打开目标域名。
- 本次覆盖两个入口：
  - `Settings -> Proxy -> TLS Interception Patterns`
  - `Network -> 选择 CONNECT 请求 -> 详情 Response 面板 -> Intercept this app`
- 本次将 TLS 白名单定义为域名白名单（`intercept_include`）和应用白名单（`app_intercept_include`）。

## 实现逻辑

- 在 `web/src/utils/tlsInterceptionNotice.ts` 抽取统一的成功提示方法。
- 当新增或删除域名白名单、应用白名单成功后，统一追加重连提醒文案：
  `Restart the target app and reopen the target domain to establish a new connection.`
- `TrafficDetail` 中 CONNECT 请求的 Response 面板在将应用加入解包白名单成功后，也复用同一提示。
- 保持失败提示与其他 TLS 设置项一致，不改变接口调用和状态更新逻辑。

## 依赖项

- 复用前端现有 `antd` 的 `message.success` 提示能力。
- 复用现有 TLS 配置更新接口 `updateTlsConfig`。

## 测试方案（含 e2e）

- 更新 `web/tests/ui/admin-settings.spec.ts`，在新增 TLS 域名白名单后断言成功提示中包含重连提醒。
- 新增 `web/tests/ui/traffic-push.spec.ts` 场景，验证 `Network -> CONNECT 请求 -> Response 面板 -> Intercept this app` 会展示重连提醒，并且配置接口写入 `app_intercept_include`。
- 按项目要求先执行相关 E2E，再执行 `rust-project-validate`。

## 校验要求（含 rust-project-validate）

- 执行与管理端设置页相关的 UI E2E，确认提示展示正确且不影响原有保存逻辑。
- 在 E2E 完成后执行 `cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`、按改动范围执行测试与构建。

## 文档更新要求

- 当前变更仅涉及交互提示与测试说明，无需更新 `README.md`。
- 若后续把同类提示扩展到 TLS 黑名单或其他配置项，应同步补充到管理端 UI E2E 说明。
