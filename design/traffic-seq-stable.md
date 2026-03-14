# Traffic 序号稳定性

## 现状结论

该设计已经落地，当前列表序号来自后端稳定下发的 `sequence` / `seq`，不再依赖前端数组下标。

## 当前实现

- SQLite 模式下，`traffic_records.sequence` 是主键并作为稳定序列号保存。
- 启动时通过 `SELECT MAX(sequence)` 初始化下一条序号，而不是前端重算。
- 前端 `useTrafficStore` / `useSearchStore` 在把紧凑记录转换为列表项时直接使用后端序号字段。

## 结果

- 清理旧记录、刷新列表、增量更新都不会导致已显示序号整体重排。
- 序号稳定性已经从“前端展示规则”变成“后端持久化事实”。
