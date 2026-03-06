# Traffic DB 启动序列号重排性能优化

## 背景

当前 SQLite 流量库（`traffic.db`）在启动初始化时，会对历史 `traffic_records` 的 `sequence` 字段做一次全表重排（按旧序列升序，重新编号为 `1..N`），并把进程内的 `current_sequence` 初始化为 `N+1`。

当数据量达到 10,000 条时，启动会变慢到分钟级，影响可用性；目标是启动在 10 秒内完成。

## 现状与问题定位

### 现状逻辑

- 启动初始化调用 `TrafficDbStore::resequence_records`：
  - `SELECT id FROM traffic_records ORDER BY sequence ASC` 拉取全部 `id`
  - 对每条记录执行 `UPDATE traffic_records SET sequence = ? WHERE id = ?`

### 性能瓶颈

`rusqlite::Connection::execute` 在未显式事务包裹时，会以 autocommit 方式执行，导致每条 `UPDATE` 形成一次独立事务提交。  
对 10,000 条数据意味着 10,000 次事务提交（含 fsync），会把启动时间拉长到分钟级。

## 目标

- 在保持现有语义（序列号启动后连续、从 1 开始）前提下，将启动重排的耗时降低到秒级
- 让整体启动在 10 秒内完成（10k 数据量）

## 方案选型

### 方案 A：启动重排保持不变，但改为单事务批量更新（推荐）

做法：

- 仍按旧逻辑生成 `id` 列表与新序列
- 将所有 `UPDATE` 包在一个 SQLite 事务中提交
- 复用同一个 prepared statement，避免重复 SQL 编译

优点：

- 不改变对外行为与历史数据语义
- 实现成本低、风险小
- 对 10k 级别数据可显著降耗

缺点：

- 启动仍是 O(N) 扫描与更新，超大数据量时仍会有可见耗时

### 方案 B：取消启动重排，改用 `MAX(sequence)` 初始化 next sequence

做法：

- 启动时只执行 `SELECT MAX(sequence)`，不更新任何历史记录

优点：

- 启动耗时最小（O(1) 查询）
- 避免对历史数据进行写放大

缺点：

- `sequence` 可能出现空洞（由于删除/清理），不再保证连续
- 若前端/逻辑强依赖“连续编号”，需要同步调整展示/分页策略

### 方案 C：不存储连续序列号，查询时用窗口函数生成展示序号

做法：

- 存储稳定的插入顺序（rowid/自增主键或 timestamp + tie-breaker）
- 查询时通过 `ROW_NUMBER() OVER (ORDER BY ...)` 生成展示序号

优点：

- 启动零成本
- 展示序号天然连续、可按查询范围动态生成

缺点：

- 需要改造 API 返回字段或新增字段
- 对分页/增量语义需要重新定义（cursor 不能再直接用展示序号）

## 落地与验证

优先落地方案 A，以最小改动恢复启动性能，并保留后续切换到方案 B/C 的空间。

验证手段：

- 构造 10k 条历史流量记录的 `traffic.db`，对比优化前后启动耗时
- 运行端到端测试，确认分页、增量更新（after_seq / cursor）、清理逻辑不回归

