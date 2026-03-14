# Web Admin Settings Tab Pull

## 背景

- `Settings` 页面此前把多类配置统一挂在 `settings_update` push 通道上。
- 各配置项的更新节奏、服务端生效时机和前端展示时机并不一致，导致 push 快照容易把页面局部状态刷回旧值。
- 为了降低耦合和问题面，需要把 `Settings` 数据流简化为“进入 tab 时主动拉取一次”。

## 方案

- 移除 `Settings` 页面对 `settings_update` 的订阅，不再通过 `settings_scopes` 接收配置快照。
- 每次打开对应 tab 时主动发起一次拉取：
  - `proxy`：代理配置、TLS 配置、代理地址、system proxy、cli proxy、桌面运行时
  - `certificate`：证书信息、代理地址
  - `metrics`：history、app metrics、host metrics
  - `access`：whitelist 状态、pending authorizations
  - `performance`：性能配置
- 各 tab 内的保存/切换仍沿用现有写接口，成功后直接使用接口返回值或显式刷新，不依赖 push 收敛。
- 全局实时通道仍保留给非 Settings 场景；桌面端改端口后只负责重连全局 push，不再给 Settings 配置做同步。

## 影响范围

- `Settings` 页面内的配置类展示统一变为 pull-on-tab-open。
- `systemProxy` 不再受 settings push 影响，继续走独立接口。
- Access 页顶部 pending 列表改为主动拉取，不再消费 settings push。

## 测试方案

- 打开每个 Settings tab，确认都会触发对应的拉取请求并能展示最新数据。
- 修改 Proxy / TLS / Access / Performance 配置后，切走再切回对应 tab，确认能重新拉到最新状态。
- 刷新 Settings 页面，确认不需要 settings push 也能正常恢复各 tab 数据。

## 校验要求

- 先执行 Settings 相关 UI / E2E 验证。
- 再执行项目格式、lint、测试与构建校验。

## 文档更新

- 当前为前端内部数据流调整，不涉及对外 API 文档或 README 变更。
