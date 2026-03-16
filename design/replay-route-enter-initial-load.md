# Replay 路由进入首屏拉取

## 功能模块详细描述

修复管理端在单页路由内从其他 tab 切换到 `/_bifrost/replay` 时，左侧 Replay 请求列表不能立即显示的问题。

当前表现是：

- 直接刷新 `/_bifrost/replay` 页面时，请求列表能够正常出现
- 但在已建立全局 push 连接的前提下，从 `Traffic` 等页面切到 `Replay`，左侧列表可能保持空白，直到当前路由再次刷新

本次修复范围仅包含 Replay 页进入时的首屏数据初始化，不调整 Replay 的保存、执行、历史记录和 push 同步协议。

## 实现逻辑

1. 保持 Replay 页面现有的 push 订阅逻辑，用于接收 `saved requests` 与 `groups` 的后续快照更新。
2. 在 Replay 页面首次挂载时，显式执行一次 HTTP 拉取：
   - `loadGroups()`
   - `loadSavedRequests()`
3. 这样无论 push 连接是“新建连接”还是“对既有连接追加订阅”，Replay 页面都能在进入路由后立即获得首屏列表数据。
4. 增加 UI 回归用例，覆盖“先进入 Traffic，再点击侧边栏切换到 Replay”场景，避免问题回归。

## 依赖项

- `web/src/pages/Replay/index.tsx`
- `web/src/stores/useReplayStore.ts`
- `web/tests/ui/admin-replay.spec.ts`

## 测试方案（含 e2e）

1. 通过管理端 API 预先创建一个已保存的 Replay 请求。
2. 先打开 `/_bifrost/traffic`，确保此时全局 push 连接已经建立。
3. 再通过侧边栏切换到 `/_bifrost/replay`。
4. 断言无需刷新页面，左侧 Replay 列表立即显示预先创建的请求。
5. 按任务要求执行 Replay 相关 UI E2E 回归后，再执行 rust-project-validate。

## 校验要求（含 rust-project-validate）

- 先执行本次变更相关的 Replay UI / E2E 验证
- 再依次执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`（按修改范围执行）
  - `cargo build --all-targets --all-features`

## 文档更新要求

- 本次为管理端页面初始化修复，不涉及 README、API 或配置项变更
