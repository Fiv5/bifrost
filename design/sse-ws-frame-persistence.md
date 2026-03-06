# SSE/WS 帧内容持久化修复方案

## 背景与问题

- 近期性能优化导致 SSE/WebSocket 帧在未进入监控模式时丢失 payload 预览字段
- 结果是请求详情与订阅前的帧仅有时间信息，无具体内容

## 目标

- 未监控连接也能将帧的 payload 预览写入磁盘
- 维持内存占用优化：未监控时仅保留较小的 payload 预览
- 订阅后保持实时帧完整性

## 方案设计

- 构造帧记录后，拆分为两份
  - 持久化版本：保留 payload_preview（payload_ref 仅在监控时写入）
  - 内存版本：未监控时保留较小 payload_preview，payload_ref 置空
- 持久化与广播逻辑使用各自版本
  - append_frame 使用持久化版本
  - 内存队列与订阅广播使用内存版本（监控开启后自然包含内容）

## 影响范围

- ConnectionMonitor 中 record_frame 与 record_sse_event
- E2E 测试补充 payload_preview 断言

## 验证方式

- WebSocket/SSE 帧接口返回 payload_preview 不为空
- 端到端测试：test_websocket_frames.sh、test_sse_frames.sh

## 流式落盘与缓存上限提升方案

### 背景补充

- 当前预览上限用于截断 payload_preview，且决定是否生成 payload_ref
- 若直接上调预览上限，会放大内存中的预览体积
- SSE 事件切分依赖内存缓冲，buffer 超限会丢弃未形成事件边界的内容

### 目标

- 缓存上限提升为“磁盘容量受限”，内存占用保持稳定
- SSE/WS 数据持续落盘，订阅时可即时推送且不丢数据
- 客户端按需加载正文内容，默认仅返回小预览

### 核心思路

- 预览上限与落盘阈值彻底解耦
  - preview_limit 仅控制内存/接口返回的预览大小
  - storage_limit 仅控制是否持久化 payload_ref 与正文
- 正文优先落盘
  - 当 payload 超过 storage_limit 时，始终写入磁盘并返回 BodyRef::File
  - 未监控连接也写入磁盘，内存只保留小预览

### SSE 流式落盘与边界解析

- 引入按连接的 SSE spool 文件
  - 每个 SSE 响应流到来时，按 chunk 追加写入文件
  - 维护事件边界索引（offset + length），用于按需读取事件正文
- 边界解析改为小窗口滑动
  - 仅保留上一个 chunk 的末尾窗口用于查找 \n\n
  - 避免 buffer 超限导致的内容丢弃
- 事件记录只保存预览与索引引用
  - payload_ref 指向 spool 文件 + offset/length
  - payload_preview 仅保留小文本预览

### WebSocket 帧正文落盘

- 对超大 frame payload 直接写入 BodyStore 文件
- payload_preview 仍使用小上限，确保内存稳定

### API 与客户端加载策略

- 新增/扩展读取接口支持 range
  - 通过 BodyRef(offset, length) 按需读取正文
- 订阅时推送元信息 + BodyRef，详情页按需拉取正文
- 列表不展示正文的场景
  - preview_limit 可降到极小或为 0，仅用于诊断或快速扫读
  - 详情页读取落盘正文，实时订阅只需要 BodyRef 即可

### 风险与治理

- 磁盘膨胀：引入按连接/全局最大磁盘额度与 LRU 回收
- IO 压力：批量写入与异步 flush，订阅优先级高于落盘回收

### 影响范围

- ConnectionMonitor：record_frame/record_sse_event 持久化与 preview 解耦
- SseTeeBody：去除超限丢弃逻辑，改为小窗口解析 + 落盘索引
- BodyStore：支持流式写入与 range 读取
