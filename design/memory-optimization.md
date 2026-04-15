## 现状结论

这份文档只有部分结论能直接映射到当前代码，建议按“已落地”与“仍需验证”拆开理解。

## 已落地项

- 请求/响应体读取已经引入 `max_body_probe_size` 与 bounded read 逻辑：
  - 对未知长度或疑似大体积 body，不再无上限读入内存。
  - 超限时跳过 body 规则/脚本，改走流式转发。
- SQLite 流量存储只保留活跃连接的轻量 `recent_cache`，详细数据按需从 DB 读取，避免常驻大对象。
- 前端 SSE 消息列表有 `MAX_SSE_EVENTS = 20_000` 上限。
- SSE response body 在前端合并时也有字符上限保护。
- `SseHub` 当前只维护连接是否打开、收发计数和字节数，不再承担历史事件 ring 缓存。
- 帧连接 metadata 已落到 `frame_connection_metadata` 表，启动时不再依赖一堆 `frames/*.meta.json` 预热。

## 与 SSE / Traffic 现状对齐

- SSE 历史内容的权威来源已经是 response body / `sse/stream` 读取链路，不是内存事件缓存。
- `TrafficDbStore::recent_cache` 当前缓存的是 summary，而不是完整 `TrafficRecord`。
- 活跃 WebSocket / SSE / tunnel 连接仍会保留必要的运行态 summary，连接结束后再逐步从 cache 收敛。
- 管理端 `Traffic` 列表前端仍然是“虚拟列表 + 常驻窗口上限”的模型，并没有落地单独的服务端分页窗口化方案；所以不能把“按页按需加载历史列表”写成已实现事实。

## 不宜直接写成“已完成事实”的部分

- SSE 解析缓冲的统一上限、parse_error 截断语义，仍需要以具体实现逐项核对，不能只沿用旧设计表述。
- “启动期帧元数据加载按保留时间过滤”也不应在未逐项核实前写成全部完成。

## SQLite cache_size 与连接池优化（2026-04-16）

### 问题
空闲状态（无搜索操作）RSS 达到 300MB。根因是 9 个 SQLite 连接的 `cache_size` PRAGMA 配置过大，理论上限合计 176MB 堆内存页面缓存，叠加 macOS malloc 40% 碎片率导致实际占用远超预期。

### 优化措施

| 连接 | 变更前 cache_size | 变更后 cache_size | 节省上限 |
|------|-------------------|-------------------|----------|
| traffic.db 写 | 10000 (~40MB) | 2000 (~8MB) | -32MB |
| traffic.db 读 ×4→×2 | 5000×4 (~80MB) | 1000×2 (~8MB) | -72MB |
| frame metadata 写 | 5000 (~20MB) | 1000 (~4MB) | -16MB |
| frame metadata 读 | 2000 (~8MB) | 500 (~2MB) | -6MB |
| replay.db 写 | 5000 (~20MB) | 1000 (~4MB) | -16MB |
| replay.db 读 | 2000 (~8MB) | 500 (~2MB) | -6MB |
| **合计** | **176MB** | **28MB** | **-148MB** |

### 其他优化
- `frame_store.metadata_cache` 从无界 `HashMap` 改为 `LruCache`（上限 1000 条），避免长时间运行后条目无限增长
- traffic.db 读连接池从 4 降到 2，减少空闲连接的 cache 占用

### 性能影响
- cache_size 降低后，SQLite page cache miss 率会略微上升，但已有 mmap 层兜底（OS 级文件缓存），对用户无感知延迟
- metadata_cache LRU 驱逐后自动回退到 SQLite 查询，延迟增加 ~0.1ms

## 结论

- 当前项目已经有一套明确的 body 读取消峰措施。
- SSE 与帧元数据相关的内存优化还需要更细粒度文档，而不是继续使用这一页的概述性提案。
