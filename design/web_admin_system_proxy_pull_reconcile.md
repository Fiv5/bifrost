# Web Admin System Proxy Pull Reconcile

## 背景

- Settings 页面此前将 `system_proxy` 纳入 `settings_scopes`，依赖 push 首次快照和后续广播同步状态。
- `systemProxy` 的真实状态来自操作系统代理配置，启停过程中可能存在异步收敛窗口。
- 当前服务端 push 构建 `system_proxy` 快照时，会读取 `SystemProxyManager::get_current()` 的系统真实状态，而不是前端刚提交的目标状态。

## 问题

- 用户在管理端切换 `systemProxy` 后，前端先收到 `PUT /api/proxy/system` 的成功响应。
- 随后配置变更触发 push 广播；若操作系统尚未完成切换，push 会把旧状态再次推到前端。
- 前端把旧快照直接写回 store，导致开关被“刷回去”，表现为操作失败或状态抖动。

## 方案

- 将 `system_proxy` 从 Settings 页的 `settings_scopes` 订阅中移除，不再通过 settings push 收敛。
- `PUT /api/proxy/system` 改为服务端确认模型：
  - 执行启用/关闭系统代理；
  - 在服务端短暂等待并回读系统真实代理状态；
  - 仅将真实状态返回给前端，而不是直接回显请求目标值。
- `useProxyStore.toggleSystemProxy` 保持轻量：
  - 请求 `PUT /api/proxy/system`；
  - 直接消费服务端返回的真实状态；
  - 若返回值仍未达到目标态，则提示未完成收敛，但不发起高频补拉。

## 影响范围

- Settings 页不再消费 `system_proxy` 的 push 快照。
- Traffic 页工具栏、StatusBar 仍复用同一个 `useProxyStore`，因此会共享切换后的拉取收敛结果。
- 其他 settings scope 继续保持 push 模型，不受本次调整影响。

## 测试方案

- 打开 Settings -> Proxy，切换 System Proxy，确认开关不会被旧 push 快照刷回。
- 切换时确认浏览器不会高频请求 `GET /api/proxy/system`。
- 确认 `PUT /api/proxy/system` 会等待真实状态收敛后再返回，返回值与系统实际状态一致。
- 验证 Traffic 页工具栏与 StatusBar 能同步看到收敛后的 `systemProxy` 状态。
- 执行相关 UI E2E 场景与项目校验流程。

## 校验要求

- 先执行与管理端 Settings 相关的 E2E / UI 验证。
- 再执行 `rust-project-validate` 规定的格式、lint、测试与构建校验。

## 文档更新

- 当前仅为前端状态同步策略调整，不涉及对外 API/README 变更。
