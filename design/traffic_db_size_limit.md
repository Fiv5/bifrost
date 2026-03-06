# Traffic DB 最大空间限制方案
 
 ## 背景
 当前 SQLite 流量库仅按记录数量与保留时间清理，数据库文件大小仍可能持续增长，需要新增最大空间限制并与过期时间共同生效。
 
 ## 目标
 - 支持配置 traffic 数据库最大空间（max_db_size_bytes），可通过管理端配置与持久化
 - 超过上限后自动清理最早记录
 - 清理具备 0.25 的缓冲，避免高频清理
 - 与过期时间协同，满足任一条件即可触发清理
 
 ## 方案
 ### 配置
 - 统一配置字段：traffic.max_db_size_bytes
 - 默认值：2 GB
 - 管理端 API：PUT /api/config/performance
 - 配置持久化：config.toml
 
 ### 清理策略
 - 触发时机：
   - 写入计数达到 CLEANUP_CHECK_INTERVAL 时检查
   - 定时任务清理过期记录后执行 WAL checkpoint
 - 清理条件（满足其一）：
   - 记录数超过 max_records
   - 数据库文件大小超过 max_db_size_bytes
   - 记录时间早于 retention_hours
 - 缓冲策略：
   - 目标大小 = max_db_size_bytes - max_db_size_bytes * 0.25
   - 按平均单条记录大小估算删除数量，优先删除最早记录
 
 ### 数据库压缩
 - 全量清空时执行 VACUUM
 - 达到 max_db_size_bytes 并清理后执行 VACUUM
 - 过期清理后执行 WAL checkpoint(TRUNCATE)
 
 ## 风险与对策
 - VACUUM 开销高：仅在超出最大空间且删除后执行
 - 平均记录大小估算误差：使用保守的平均值，确保回落到 0.75 目标
 
 ## 验证
 - 管理端配置生效并持久化
 - 超过最大空间时，记录数下降且 db_size 回落
 - 过期时间或大小任一触发均可清理
