# Web 管理端睡眠恢复与内存放大

## 现状结论

旧文档定位的问题基本正确，但很多“待实施措施”现在已经实现了，文档需要从“方案”改成“现状说明”。

## 已实现修复

- Push 发送队列已经从无界改成有界 `mpsc::channel(PUSH_CHANNEL_CAPACITY)`。
- `PushClient::send()` 已使用 `try_send`，慢客户端会被自然淘汰，而不是无限堆积。
- WebSocket 连接已加入协议层 `Ping / Pong`。
- 服务端已支持基于 `x_client_id` 做客户端分桶与淘汰。
- 订阅侧对 `pending_ids` 已有限额，`websocket.rs` 中会裁剪到 `MAX_SUBSCRIBED_IDS`。
- 前端 `useTrafficStore` 已增加：
  - `MAX_PENDING_IDS = 500`
  - `POLL_MIN_INTERVAL = 200`
  - `HAS_MORE_BACKOFF_INTERVAL = 500`
- 页面隐藏时会暂停实时同步，恢复时再继续。

## 仍需谨慎的点

- 前端目前仍以 `pending_ids` 模型为主，没有实现文档里提到的 `visible_ids` 方案。
- 多 tab 复用单 push 连接也还不是当前实现。

## 结论

- 这份文档不应继续把“bounded queue / ping pong / client bucket / pending_ids 限额”写成待做事项。
- 当前更准确的定位是：核心防护已经落地，剩余的是进一步降低恢复瞬时流量的增量优化。
