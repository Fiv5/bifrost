## 现状结论

旧文档依赖 `frames/stream + payload_preview` 的方案已经不是当前主实现。现在的 SSE 合并路径建立在专用 `/sse/stream` 接口上。

## 当前实现

- 打开的 SSE 详情页通过 `/traffic/{id}/sse/stream` 接收结构化事件。
- 前端收到事件后：
  - 把事件加入 `sseEvents` 列表；
  - 若返回里带 `raw`，同时调用 `appendSseResponseBody()` 把原始文本拼接回 `responseBody`。
- 已结束连接不会继续订阅流，而是直接从 `responseBody` 全文本地解析消息。

## 当前收益

- Messages 首屏不再依赖 `/frames/{frame_id}` 二次回补。
- Body 与 Messages 都围绕同一条 SSE 原始流收敛。
- 旧文档中提到的 `payload_preview` 补全逻辑已不是关键路径。

## 结论

这份文档应视为“已废弃的过渡方案”；当前以 [`design/sse-stream-v2.md`](../design/sse-stream-v2.md) 描述的专用流方案为准。
