## 现状结论

这份设计已经基本落地，当前流量详情页的 SSE 路径确实使用独立的 `/traffic/{id}/sse/stream`，不再依赖 `/frames/{id}` 回补首屏消息。

## 当前实现

- 前端打开中的 SSE 连接会建立：
  - `EventSource /traffic/{id}/sse/stream?from=begin&batch=1`
- 已结束的 SSE 连接则直接读取 `responseBody`，在前端本地解析为事件列表。
- 消息面板与正文面板共享同一份原始流：
  - 消息列表消费结构化事件；
  - `raw` 字段会继续 append 到 `useTrafficStore.responseBody`，保证正文持续增长。

## 与原设计的差异

- 旧文档中“完全取消 SSE frames 入库”的表述不应再视为仓库级事实；当前流量详情主路径已经解耦，但仓库里仍能看到部分 SSE frame 记录逻辑在其他链路中存在。
- 真正已经稳定落地的是“详情页主消费路径改成 SSE 专用流 + response body 解析”，而不是“全仓库彻底删除 SSE frame 语义”。

## 文档结论

- 如果讨论 Traffic Detail 的 SSE 展示链路，本方案已经是当前真实实现。
- 如果讨论底层存储是否彻底去掉 SSE frame，需要单独做更细的设计核对，不能直接沿用旧文档中的绝对表述。
