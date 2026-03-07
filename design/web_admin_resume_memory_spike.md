# Web 管理端睡眠恢复导致请求风暴与内存暴涨：原因分析与技术方案

## 背景与现象

- 现象：Web 管理端从睡眠状态恢复后，短时间内出现大量请求/重连。
- 影响：服务端内存从约 200MB 暴涨到约 8GB，存在 OOM 风险。

## 结论摘要

该问题更符合“服务端对慢/断连客户端缺少背压，导致消息在内存中无界堆积”的特征，而不仅是请求量大。

当前 Admin 的 Push WebSocket 推送实现对每个客户端使用 `mpsc::unbounded_channel()` 作为发送队列，广播任务会以固定频率向所有客户端入队消息；当浏览器休眠/后台导致客户端不读或读非常慢时，发送任务无法及时 drain 队列，消息持续累积，最终出现内存爆炸。

同时，HTTP 增量接口 `/api/traffic/updates` 也存在“恢复后快速追数据（has_more → 0ms）”以及 `pending_ids` 无上限的放大因素，会让恢复瞬间的负载与响应体显著增大，进一步放大服务端压力。

## 根因定位（静态证据）

### 1) Push WebSocket：无界发送队列 + 固定频率广播（核心风险）

- 每个客户端的发送队列：`tokio::sync::mpsc::unbounded_channel()`，没有背压。
  - 见：[push.rs](file:///Users/eden/work/github/bifrost/crates/bifrost-admin/src/push.rs#L178-L206)
- 广播任务：traffic 500ms、overview 1s、metrics 500ms（基础 tick）、history 5s。
  - 见：[start_push_tasks](file:///Users/eden/work/github/bifrost/crates/bifrost-admin/src/push.rs#L916-L970)
- 广播侧入队永远成功（只要接收端任务未退出），即使客户端写阻塞导致接收端 drain 很慢。
  - `PushClient::send` 只判断 `sender.send(msg).is_ok()`；队列满不会发生，因为 unbounded。
  - 见：[push.rs](file:///Users/eden/work/github/bifrost/crates/bifrost-admin/src/push.rs#L195-L197)

**推导：**睡眠期间客户端读慢/不读 → 发送任务无法及时从队列取出并写 socket → 广播任务持续入队 → 队列无界增长 → 进程内存暴涨。

### 2) WS 连接任务生命周期：存在“半任务泄漏”风险（加剧资源消耗）

- 每条 WS 连接会 spawn `sender_task` 与 `receiver_task`，`select!` 任一结束即退出，但未显式取消另一个任务。
  - 见：[websocket.rs](file:///Users/eden/work/github/bifrost/crates/bifrost-admin/src/handlers/websocket.rs#L151-L207)

### 3) HTTP 增量接口：恢复后快速追数据 + `pending_ids` 无上限（负载放大）

- 前端轮询在 `has_more=true` 时使用 `setTimeout(..., 0)` 追数据，会在恢复后快速拉取积压增量。
  - 见：[useTrafficStore.ts](file:///Users/eden/work/github/bifrost/web/src/stores/useTrafficStore.ts#L899-L906)
- `BATCH_LIMIT=1000`，单次响应体可能很大；并且 `pending_ids` 直接 `join(',')` 全量发送。
  - 见：[useTrafficStore.ts](file:///Users/eden/work/github/bifrost/web/src/stores/useTrafficStore.ts#L63-L816)
- 服务端 `pending_ids` 解析与后续 `get_by_ids` 没有数量/长度上限。
  - 见：[traffic.rs](file:///Users/eden/work/github/bifrost/crates/bifrost-admin/src/handlers/traffic.rs#L545-L574)、[get_traffic_updates](file:///Users/eden/work/github/bifrost/crates/bifrost-admin/src/handlers/traffic.rs#L418-L482)
- Push WS 的订阅同样允许超大 `pending_ids`，且服务端会把 pending 集合继续写回订阅（可能越滚越大）。
  - 见：[push.rs](file:///Users/eden/work/github/bifrost/crates/bifrost-admin/src/push.rs#L337-L375)、[websocket.rs](file:///Users/eden/work/github/bifrost/crates/bifrost-admin/src/handlers/websocket.rs#L78-L116)

## 如何避免：技术方案

### A. 服务端方案（优先级最高）

#### A0. 目标与约束

- 目标：任意客户端行为下，服务端内存使用存在明确上界；恢复/重连时不会触发消息堆积型 OOM。
- 约束：浏览器 WebSocket API 不暴露 ping/pong 给业务层，但会自动响应服务端 ping；服务端可通过协议层心跳判断连接存活。

#### A1. Push 发送队列改为有界 + 慢客户端策略

目标：从根上消除无界堆积，保证“单连接内存上限可计算”。

- 将 `mpsc::unbounded_channel()` 改为 `mpsc::channel(N)`（例如 64/128）。
- `send` 侧使用 `try_send`：
  - 队列满时采取策略之一：
    1. 直接断开连接（推荐，最简单且安全）；
    2. 丢弃本次消息，并设置一个 `dropped_count`，超过阈值后断开；
    3. 对可替代的消息类型（overview/metrics）仅保留最新（用 watch/缓存覆盖）。
- 发送任务写 socket 时增加超时（例如 3s/10s），超时即关闭连接。

预期效果：单连接最多持有 `N * 平均消息大小` 的队列内存，避免 OOM。

#### A2. WebSocket 心跳与断连清理（必须）

目标：当浏览器休眠/网络切换导致“半开连接”或客户端不再消费时，服务端能尽快识别并主动关闭连接，结束订阅与广播入队。

- 服务端发送协议层 ping：
  - `ping_interval_ms`：建议 10s（可配置）
  - `pong_timeout_ms`：建议 30s（例如连续 3 次 ping 未观察到 pong 即关闭）
  - 实现要点：在 WS sender 循环中用 `tokio::select!` 复用一个 interval；每次 tick 发送 `Message::Ping`，并记录 `last_ping_ts`。
- 服务端统计 pong：
  - receiver 任务接收 `Message::Pong(_)` 时更新 `last_pong_ts`（共享原子/锁内 `Instant`）。
  - 超过 `pong_timeout_ms` 未更新则关闭连接、abort 两侧任务并 `unregister_client`。
- 退出清理：
  - sender/receiver 任一退出后，显式 abort 另一任务，避免“半任务残留”导致的资源占用持续。

补充：如需跨代理/中间件的稳健性，可额外引入应用层心跳（客户端定时发送 `{"type":"ping"}`，服务端回复 `{"type":"pong"}` 并更新时间戳），但优先使用协议层 ping/pong。

#### A3. 消息体优化：共享序列化结果，减少重复分配

目标：减少“客户端数 × 序列化开销 × 内存临时对象”。

- 广播前先把 payload 序列化为 `Bytes/Arc<[u8]>`，每个客户端复用同一份数据。
- 对 traffic delta/history 这类大 payload，优先推送“游标变化 + has_more”，由客户端按需拉取。

#### A4. 订阅参数限额与裁剪（必须）

目标：避免异常客户端通过超长订阅或巨大消息直接把服务端打穿（CPU/内存/IO）。

- WebSocket 文本消息大小上限：
  - 服务端在处理 `Message::Text(text)` 前先做 `text.len()` 判断（例如 32KB/64KB 上限），超限直接关闭连接并记录告警。
- `pending_ids` 限额与裁剪：
  - 单 id 最大长度（例如 128/256）
  - 数量上限（例如 500 或 2000，推荐与前端“可见范围订阅”策略一致，优先 500）
  - 超限策略：优先裁剪（保留最新/可见范围），其次返回 400 并断开（取决于是否需要兼容老版本前端）。
- `history_limit`/`metrics_interval_ms` clamp：
  - `history_limit`：限定在 `[0, 500]` 或 `[0, 2000]`（结合实际 payload）
  - `metrics_interval_ms`：限定在服务端已有常量范围内（`METRICS_INTERVAL_MIN_MS..=METRICS_INTERVAL_MAX_MS`）
- 订阅更新频率限制（可选）：
  - 对单连接的订阅更新做最小间隔（例如 200ms），避免前端滚动时高频刷订阅导致 CPU 开销放大。

#### A5. “仅支持最新 500 个连接/请求”的服务端治理（必须）

目标：限制服务端需要维护的订阅状态规模，避免在异常场景下连接/订阅无限扩张。

- 最大 WS 客户端连接数：默认仅支持 3 个独立通道（`MAX_PUSH_CLIENTS = 3`）
  - 注册新 client 时若超过上限，关闭最旧的 client 连接并移除其订阅状态。
  - 独立通道定义：独立的客户端页面（Tab/Window）为一个独立通道。每个页面加载后在内存中生成一个 `x-client-id`，并在后续请求中携带，用于服务端分桶限流与淘汰。
  - 识别方式：
    - HTTP：使用请求头 `X-Client-Id: <id>`
    - WebSocket：浏览器原生 WebSocket 无法自定义请求头，使用 query 参数 `x_client_id=<id>` 携带（若未来接入可注入 header 的客户端，可同时支持 header 优先）。
  - 淘汰范围：以 `x-client-id` 为 key 维护活跃通道列表；当同一 key 下连接数超过 3 时，淘汰最旧连接。
- 单连接订阅的“请求/记录”上限：`MAX_SUBSCRIBED_IDS = 500`
  - 对 `pending_ids`/`visible_ids` 等“按 id 订阅”的字段做统一上限与裁剪，超限移除旧数据。
  - 服务端侧在每次广播更新订阅状态时，确保不会把 `pending_ids` 回写得越来越大（必要时裁剪）。

### B. 前端方案（降低恢复瞬时压力）

#### B1. 可见性驱动的暂停与恢复（必须）

- `visibilitychange` / `pagehide` 时：
  - 断开 push（或暂停轮询），清理 timer；
  - 记录 last_sequence/lastId。
- `pageshow` / 可见时：
  - 重新连接 push 或恢复轮询；
  - 恢复阶段使用“渐进补拉”：把 `has_more -> 0ms` 改为“带下限的追数据间隔”（例如最小 200ms，且连续追 3 次后强制 backoff）。

补充：detail 页面切换或组件卸载时，必须主动断开订阅（push/frames/sse 的 EventSource），避免同一 Tab 内产生多路重复订阅。

补充：每个页面首次加载生成 `x-client-id`（仅存内存，不落盘），并在所有 HTTP 请求中通过 `X-Client-Id` 透传；push WebSocket 通过 `x_client_id` query 透传同一值。

#### B2. 连接心跳（必须）

目标：尽快识别连接不可用并触发重连，降低“静默断开”与恢复时雪崩。

- 优先使用服务端 ping/pong（浏览器自动响应），前端无需额外实现。

约束：本方案选择仅使用协议层 ping/pong，不引入额外的文本心跳协议。

#### B3. 限制 `pendingIds` 规模，并引入“可见范围订阅”（必须）

目标：配合虚拟滚动，只订阅“当前需要渲染/交互”的数据更新，避免全量 pending 带来的服务端查询与推送放大。

- pendingIds 保留最近 N 条（与服务端一致，推荐 500），其余交由“按需拉取”。
- 引入可见范围订阅（推荐方案）：
  - 前端虚拟列表在 `onItemsRendered` 计算可见区 id 列表（可含上下 buffer，例如可见 + 2 屏）。
  - push 订阅携带 `visible_ids`（上限 500），服务端只推送：
    - `inserts`：按 `last_sequence` 的增量（仍可保留小批量，避免错过新增）
    - `updates`：仅针对 `visible_ids`（或额外包含少量“正在 open 的连接”）
  - 当用户滚动导致 visible 变化时，做节流更新订阅（例如 200ms 一次）。

#### B4. 重新连接不清零：只刷新可见范围状态（必须）

目标：恢复/重连时避免“全量重建 + 全量补拉”，只对当前屏幕范围做状态校准。

- 断线重连后：
  - 继续携带上次的 `last_sequence/lastId`，不清空本地 recordsMap；
  - 立即触发一次“可见范围 refresh”：针对 `visible_ids` 主动拉取详情/状态。
- 恢复策略：
  - 若 `has_more` 很大，优先限制单次补拉次数与最小间隔；
  - 用户未滚动到的范围不补拉（直到进入可见范围再拉取/订阅）。

#### B3. 多 Tab 复用单连接（可选优化）

- 使用 `BroadcastChannel` 或 SharedWorker：
  - 让同一浏览器实例内多个 Tab 共享一个 push 连接；
  - 其他 Tab 通过 channel 订阅数据，显著降低服务端连接数与广播压力。

### D. 协议与数据结构调整建议（用于对齐）

当前 push 订阅结构为 `ClientSubscription { last_sequence, pending_ids, need_* ... }`。为支持“可见范围订阅”与限额治理，建议升级为：

- 新增字段：
  - `visible_ids: string[]`：当前需要高频更新的记录 id（上限 500）
  - `subscribe_mode: 'pending' | 'visible'`：兼容旧逻辑，逐步切换默认到 `visible`
- 行为约束：
  - 服务端对所有 id 列表字段统一做裁剪并回写裁剪后的结果（便于前端自洽）
  - 重连后不依赖 push 首包快照；前端自行对 `visible_ids` 执行 refresh 拉取

### E. 需要确认的对齐点

- `MAX_SUBSCRIBED_IDS` 取值：500
- 心跳选择：仅协议层 ping/pong
- 重连后快照：前端独立 refresh 拉取，不由 push 首包携带
- 超限策略：默认仅支持 3 个独立通道；超过后关闭旧连接

### C. 观测与验收（验证避免复发）

- 指标：
  - push 客户端数、每连接发送队列长度、丢弃/断开次数
  - 每类 push 消息的平均/最大字节数
  - `/traffic/updates` QPS、响应体大小分位数
  - 进程 RSS/heap 与 GC/allocator 指标（如 jemalloc stats）
- 验收场景：
  1. 单 Tab 休眠 10 分钟后恢复，内存不应持续增长，恢复后应趋于稳定。
  2. 多 Tab（例如 10 个）同时休眠/恢复，服务端内存应有上界且不会爆炸。
  3. 人为模拟慢客户端（限速/丢包）时，服务端应主动断开或丢弃而不是堆积。
