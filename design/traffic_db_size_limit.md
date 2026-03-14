# Traffic DB 最大空间限制

## 现状结论

这个方案已经实现，但真实清理逻辑分散在 `TrafficDbStore` 与 `AdminState` 两层，不只是单纯的 SQLite 文件大小判断。

## 当前实现

- 配置字段已经存在：
  - `traffic.max_db_size_bytes`
  - 默认值 `2 GiB`
  - 可经由 `PUT /api/config/performance` 更新并持久化
- DB 层：
  - 写入过程中会检查 `traffic.db` 自身大小；
  - 超限时会按 25% 低水位回落到 `target_size = max - max / 4`。
- 管理端状态层：
  - 还会把 `traffic.db + body_cache + frames + ws_payload` 作为整体磁盘占用做兜底清理。
- 过期清理任务会周期执行，并在清理后做 `wal_checkpoint(TRUNCATE)`；`VACUUM` 只在部分 compact 路径执行，不是每次热点清理都跑。

## 文档修正

- 旧文档里“Traffic DB 最大空间限制”如果只理解为 SQLite 文件上限，范围已经偏窄。
- 当前真实语义更接近“Traffic 相关数据总占用受 max_db_size_bytes 约束，SQLite 自身也有内部低水位回收逻辑”。
