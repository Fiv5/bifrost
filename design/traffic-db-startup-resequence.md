# Traffic DB 启动序列号初始化

## 现状结论

这份文档描述的“启动时全表重排 sequence”已经不是当前实现。现有实现直接采用 `MAX(sequence)` 初始化内存中的 `current_sequence`。

## 当前实现

- `traffic_records.sequence` 是 SQLite 主键。
- `TrafficDbStore::new()` 启动时通过 `get_max_sequence()` 执行 `SELECT MAX(sequence) FROM traffic_records`。
- `current_sequence` 初始化为 `max + 1`。
- 不再对历史记录做 resequence，也不存在全表 `UPDATE` 的启动开销。

## 当前语义

- 序号单调递增，但允许因为删除/清理出现空洞。
- 前端展示已经适配稳定但不必连续的序号模型，因此无需再追求“启动后重排为 1..N”。

## 结论

- 原方案已废弃。
- 后续如果需要讨论 sequence 行为，应以“稳定、单调、允许空洞”为前提，而不是恢复启动重排。
