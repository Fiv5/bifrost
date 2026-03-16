# 流量详情页 SSE/WS 订阅在桌面端命中错误端口修复

## 功能模块详细描述

桌面端流量详情页在打开活跃的 SSE 请求时，需要持续订阅管理端 `/_bifrost/api/traffic/{id}/sse/stream` 才能实时显示最新事件。当前实现中 `EventSource` 直接使用相对路径，导致请求发往 WebView 自身端口，而不是桌面 runtime 当前绑定的 Bifrost core 端口；结果是详情页只能在重新打开时通过普通 `fetch` 看到最新内容，实时推送失效。

同一问题也影响详情页里的 WebSocket frame 流订阅，因为它也使用了相同的相对路径构造方式。

## 实现逻辑

- 复用 `web/src/runtime.ts` 中已有的 `buildApiUrl()` 作为 `EventSource` 的 URL 构造入口。
- 将流量详情页 `Messages` 面板中的：
  - `/_bifrost/api/traffic/{id}/frames/stream`
  - `/_bifrost/api/traffic/{id}/sse/stream`
  统一改为绝对后端地址。
- 保持现有 query 参数、订阅时机、关闭时机和消息解析逻辑不变，只修正目标端口来源。

## 依赖项

- `web/src/runtime.ts`
- `web/src/components/TrafficDetail/panes/Messages/index.tsx`

## 测试方案（含 e2e）

- 桌面模式构建前端，确认 TypeScript 能通过。
- 执行与 SSE 详情实时更新相关的现有 E2E 回归，重点覆盖活跃 SSE 请求打开详情后的消息持续增长行为。
- 手工验证桌面端下详情面板发出的 `EventSource` 请求落在当前 Bifrost core 端口，而非 WebView 端口。

## 校验要求（含 rust-project-validate）

- 按顺序执行 `e2e-test` 技能。
- 在 E2E 完成后执行 `rust-project-validate` 技能要求的格式、lint、测试和构建校验。

## 文档更新要求

- 本次变更不涉及对外 API / 配置变更，`README.md` 无需更新。
