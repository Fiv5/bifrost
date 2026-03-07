## 背景
SSE 请求的响应体会持续追加新分片，详情页希望在已拉取的响应体基础上，实时合并服务端推送的分片内容，并保持消息面板作为额外解析视图。

## 目标
- SSE 响应体在详情页内自动追加新的事件内容
- SSE Messages 列表首屏不出现空数据闪烁
- 已通过响应体接口拉取的历史内容保持不变
- 不影响 WebSocket 与普通 HTTP 请求的展示逻辑

## 非目标
- 修改后端 SSE 数据存储或接口协议
- 引入新的后端配置项（优先默认值满足需求）

## 方案（A：后端 frames/stream 直接携带 payload）
### 核心思路
- 后端在产生 SSE frame 时同时生成 `payload_preview`（截断后的文本预览）
- 前端 Messages 列表与实时订阅只依赖 `payload_preview` 进行解析与渲染，避免二次请求导致闪烁
- 前端 Body 仍以 `response-body` 作为真实累积文本源；实时分片合并仍可基于推送内容追加

### 后端改动点
1. 代理侧捕获 SSE event 时生成 preview
   - 位置：`SseTeeBody::record_event` 调用 `connection_monitor.record_sse_event(...)`
   - 调整：`record_sse_event` 传入 `payload_preview = Some(truncate_utf8(payload, SSE_PREVIEW_LIMIT))`
2. 历史列表补全（兼容历史数据）
   - 位置：Admin 的 `GET /api/traffic/{id}/frames`
   - 调整：若 `frame_type == sse && payload_preview is None && payload_ref is File`，读取文件头部 N 字节并填充 `payload_preview`（只影响响应，不强制回写存储）
3. 实时通道携带 preview
   - `frames/stream` 复用同一条 `WebSocketFrameRecord`，只要 record 时填充了 `payload_preview`，即可随推送下发

### 预览长度与截断策略
- 默认 `SSE_PREVIEW_LIMIT = 4096`（字符/字节以 UTF-8 安全截断为准）
- 渲染与搜索仅使用 preview；需要完整内容时仍可点开详情通过 `GET /frames/{frame_id}` 取 `full_payload`

## 已结束（Closed）的 SSE 请求如何渲染
### 期望行为
- 请求关闭后不再依赖 `frames/stream`
- Messages 使用 `GET /frames` 获取历史事件预览并渲染
- Body 使用 `GET /response-body` 获取完整累积文本并渲染

### 数据来源优先级
1. Messages：`GET /frames` 的 `payload_preview`（已由后端补全，避免空数据）
2. Body：`GET /response-body` 的完整文本（持久化的 body_cache 文件）
3. 单条详情：用户点击某条消息需要展开时，调用 `GET /frames/{frame_id}` 返回 `full_payload`

### 兼容存量数据
- 历史数据可能没有 `payload_preview`：由 `GET /frames` 服务端补全 preview 解决（无需前端额外补拉）

## 数据流
1. `fetchTrafficDetail` 拉取 `responseBody`
2. `Messages` 通过 `GET /frames` 拉取首屏 events（每条包含 `payload_preview`）
3. 若连接仍未关闭，`Messages` 订阅 `frames/stream` 持续收到带 preview 的新 events
4. 前端可选：将新 event 追加到 `responseBody`（用于 Body 实时更新）

## 边界与回退
- `payload_preview` 为空且补全失败：Messages 显示占位但不触发二次补拉（避免抖动/风暴）
- `full_payload` 获取失败时跳过展开渲染
- 记录切换时重置合并状态
