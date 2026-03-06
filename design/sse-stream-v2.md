## 背景
当前 SSE 在实现上同时产生两套“消息形态”：
- `response-body`：完整的 `text/event-stream` 原始文本流（权威数据源）
- `frames`：按 `\\n\\n` 分隔后的事件帧（与 WebSocket 共用 frames/stream/存储）

由于 frames 默认不携带 `payload_preview`，前端 Messages 需要额外请求 `GET /frames/{frame_id}` 回补 `full_payload` 才能渲染，产生“空→有”的闪烁与额外请求放大。同时 SSE frames 的持久化与 WebSocket frames 混用，不符合 SSE 的语义定位（SSE 本质是 HTTP 响应体流）。

## 目标
- SSE 的权威数据源仅为 `response-body`（完整文本入库），不再将 SSE 事件持久化为 frames
- 新增 SSE 专用增量流 API，语义上与 WebSocket frames 解耦
- 打开 SSE 详情页时：
  - 若请求已结束：用普通 HTTP 拉取 `response-body`，前端将其解析渲染为 Events
  - 若请求未结束：使用统一的 SSE 流式输出接口，由服务端负责拼接“已落库 + 未落库 + 实时增量”，保证顺序与完整性
- 消灭 Messages 首屏闪烁（不依赖回补请求）

## 非目标
- 不改变现有 WebSocket frames 的语义与存储方式
- 不要求 UI 端维护复杂的重连补洞逻辑（由服务端保证“订阅即拿全量 + 持续增量”）

## 现状（关键事实）
### 落库与未落库
- traffic record 在 DB 模式由 `TrafficDbStore` 写入 SQLite；文件模式由 `TrafficStore` 维护并异步 flush
- SSE raw 响应体通过 `BodyStreamWriter::write_chunk` 持续写入文件，`TrafficRecord.response_body_ref` 会在连接建立后很早就指向该文件（size 可能滞后，但文件内容增长）

### SSE frames 的来源
- 代理侧 `SseTeeBody` 同时：
  - 写 raw 响应体到 `response_body_ref` 文件
  - 按 `\\n\\n` 事件边界拆分，并调用 `connection_monitor.record_sse_event` 生成 `frame_type=sse` 的 frames（并可能持久化到 FrameStore）

## 新架构（v2）
### 总体分层
1. **SSE Body（权威）**
   - 仅以 `TrafficRecord.response_body_ref` 为准（完整文本流）
2. **SSE Delta Stream（实时）**
   - 新增 `GET /api/traffic/{id}/sse/stream`
   - 服务端输出“全量事件 + 持续增量事件”，事件结构与 WebSocket frames 解耦

### 关键决策
- SSE 不再调用 `connection_monitor.record_sse_event`，不生成/不持久化 SSE frames
- WebSocket 仍沿用现有 frames/stream + FrameStore

## 服务端组件设计
### 1) SSE 事件解析器（Server-side）
输入：`text/event-stream` 原始字节流
输出：结构化事件 `SseEvent`

解析规则（与现有前端 parse 语义对齐）：
- 以空行作为事件边界（`\\n\\n`，兼容 `\\r\\n`）
- 支持 `id:` `event:` `retry:` 与多行 `data:`（按 `\\n` 连接）
- 忽略 `:` 注释行
- 对未闭合尾部事件保留 remainder，等待后续字节补齐

### 2) SSE 实时广播 Hub（仅内存，不落盘）
为每个 SSE traffic id 维护：
- `broadcast::Sender<SseEventEnvelope>`：实时推送已解析事件
- `ring buffer（可选）`：保存最近 N 条事件用于短时重连补洞（可用 seq 做游标）

该 Hub 由代理侧在收到 raw chunk 并完成事件切分后向 Hub 推送，避免依赖“轮询 tail 文件”带来的延迟与不确定性。

### 3) SSE Stream Handler（Admin API）
新增接口：`GET /api/traffic/{id}/sse/stream`

职责：
- 判断请求状态（open/closed）
- 对 open 请求输出：
  1) **历史（已落库/已写入）**：从 `response_body_ref` 文件读取当前全量内容，解析为事件流并输出
  2) **实时增量**：订阅 SSE Hub，从“当前时刻”开始持续输出新事件
  3) **一致性保证**：通过 `seq` 去重与顺序控制，避免“历史尾部与实时首部”重复
- 对 closed 请求：
  - 可直接返回 409/400 提示前端走 `response-body` 普通拉取解析
  - 或返回一次性事件流后结束（两者二选一，推荐前端走普通拉取以减少长连接开销）

## 数据模型（SSE v2）
### SseEvent（结构化）
```json
{
  "seq": 123,
  "event": "message",
  "id": "881024182",
  "retry": 3000,
  "data": "{...}",
  "raw": "id: ...\\nevent: ...\\ndata: ...",
  "ts": 1772775869778
}
```

说明：
- `seq`：服务端为该 traffic 单调递增的事件序号（用于去重/排序/重连）
- `raw`：可选，用于 debug/复制；UI 默认展示 `data`，并在需要时展示 `raw`
- `ts`：服务端接收该事件完成切分的时间戳（history 回放可为空或用推断值）

## API 设计
### 1) 获取响应体（已结束/通用）
`GET /api/traffic/{id}/response-body`
- 返回完整原始文本流
- closed 时前端用此解析 events 并渲染 Messages

### 2) SSE 专用流式输出（仅 open）
`GET /api/traffic/{id}/sse/stream`

返回：`Content-Type: text/event-stream`

事件类型：
- `event: sse_event`：每条事件一条消息
- `id: {seq}`：用于客户端记录游标（可选支持 Last-Event-ID）
`data:` 为上述 `SseEvent` 的 JSON

参数（建议）：
- `from=begin|tail`：默认 `begin`，打开详情时一般取 begin
- `last_seq`（可选）：用于断线重连补洞（服务端若实现 ring buffer，可从 `last_seq+1` 开始补发）

服务端“订阅即拿全量”的处理：
- `from=begin`：先解析当前 body 文件并逐条输出（可分页/分批 flush），然后接入 Hub 输出实时增量
- `from=tail`：跳过历史解析，仅接入 Hub

## 前端渲染策略（与状态机）
### closed 请求（socket_status.is_open=false）
- `GET /response-body` 获取全文
- 前端本地解析为 events 列表并渲染 Messages
- Body 直接展示全文

### open 请求（socket_status.is_open=true）
- 建立 `EventSource /sse/stream?from=begin`
- 服务端输出全量事件（来自 body 文件解析）+ 持续增量事件（来自 Hub）
- 前端：
  - Messages 直接消费结构化 `SseEvent` 渲染（无需 frames、无需回补 full_payload）
  - Body：
    - 可选 1：仍走 `response-body` 初次拉取 + 订阅流时将 `raw` 追加到 body（实时更新）
    - 可选 2：不单独拉 `response-body`，直接用 stream 的 `raw` 拼接形成 body（需要保证格式与边界）

## 一致性与去重（服务端保证）
关键问题：历史解析与实时订阅的交界处可能重复（历史文件末尾事件也可能刚通过 Hub 推送）。

推荐策略：
- 服务端在解析历史时生成 `seq` 并记下 `last_history_seq`
- 接入 Hub 后仅转发 `seq > last_history_seq` 的事件
- 若 Hub 的 seq 与历史 seq 来源不同，则用 “事件指纹” 去重（例如 `hash(raw)` + 事件顺序），并在设计中固定为统一 seq 生成器

## 存储与迁移
### 取消 SSE frames 入库
- 代理侧不再调用 `connection_monitor.record_sse_event`，也不再写 FrameStore
- Admin 的 `/frames` 与 `/frames/stream` 继续保留给 WebSocket
- UI 的 SSE Messages 走新的 `/sse/stream`（open）或 `response-body`（closed）

### 与 DB 模式的关系
- traffic record（DB）仍保存 `response_body_ref`，即使 size 不实时更新也不影响读取（读取应以文件实际长度为准）
- open 请求的“数据库已落库 + 未落库 + 实时”由服务端在 `/sse/stream` 内部统一拼接：
  - 已落库：DB 能查到 record，且 `response_body_ref` 指向文件
  - 未落库：body 文件内容持续增长（不依赖 DB flush），Hub 负责兜住增量

## 失败与回退
- `/sse/stream` 若发现请求已结束：返回明确错误，前端回退到 `response-body` 拉取解析
- 解析失败的 event：作为 `event: sse_event` 但标记 `parse_error=true` 并携带 `raw`，避免整条流中断

## 测试策略
- 新增后端单元测试：
  - chunk 边界拆分（跨 chunk 的 `\\n\\n`）
  - 多行 `data:` 拼接
  - `\\r\\n` 兼容
- 新增 e2e：
  - open SSE：订阅 `/sse/stream?from=begin` 能拿到全量 + 增量
  - closed SSE：`response-body` 解析 events 与 open 模式输出一致

