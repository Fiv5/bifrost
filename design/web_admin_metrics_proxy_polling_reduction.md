# Web Admin 指标与代理请求降载

## 现状结论

该方向已经落地，但细节和旧文档不完全一致。

## 当前实现

- `useGlobalDataSync()` 启动时仍会各拉一次：
  - `fetchSystemProxy()`
  - `fetchCliProxy()`
  - `fetchOverview()`
- 这些请求已经不是定时轮询；页面常驻期间不会在全局层持续请求代理状态。
- metrics 实时更新通过 push 订阅 `needOverview + needMetrics` 维持。
- metrics history 不是全局启动时拉取，而是在 `Settings` 页面切到 `metrics` tab 后才触发 `fetchHistory(3600)`。

## 与旧文档的差异

- 旧文档写成“移除全局系统代理/CLI 代理轮询”，这一点是对的。
- 但不是“完全不在全局初始化请求代理状态”，而是“保留一次初始化拉取，移除周期轮询”。

## 结论

- 当前实现已经把高频轮询收敛掉。
- 历史 metrics 也已经变成页面按需加载。
- 文档重点应放在“从全局周期采集改为启动一次 + 页面按需 + push 增量”，而不是“彻底不请求”。

## 2026-03 追加优化

- `Traffic` 实时订阅不再由 `useGlobalDataSync()` 全局默认启动，而是改成仅在 `web/src/pages/Traffic/index.tsx` 挂载期间启动。
- WebSocket 订阅协议新增 `need_traffic`，后端只对显式 traffic 订阅者执行 `traffic_delta` 周期任务。
- `crates/bifrost-admin/src/push.rs` 的 traffic 周期广播增加空闲短路：
  - 如果当前客户端没有 `pending_ids`
  - 且 `TrafficDbStore::current_sequence()` 自上次已知序列后没有推进
  - 则本轮直接跳过 SQLite `query()` / `get_by_ids()`

## 这次优化解决的问题

- 打开管理端但停留在 `Settings`、`Rules` 等非 Traffic 页面时，不再持续触发 traffic 增量查询。
- 即使停留在 `Traffic` 页面，在“无代理流量、无活跃长连接”的空闲状态下，也不会每 500ms 反复扫库。

## 测试方案

- 打开非 `Traffic` 页面，确认 WebSocket 订阅不再携带 `need_traffic`。
- 打开 `Traffic` 页面，确认仍能收到新增记录与长连接状态更新。
- 真实服务空闲运行时，观测进程 CPU 锯齿明显下降，并确认采样热点不再停留在 `broadcast_traffic_delta -> TrafficDbStore::query/get_by_ids`。
