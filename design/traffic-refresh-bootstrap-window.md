# Traffic 刷新首屏窗口稳定性

## 功能模块详细描述

修复 Traffic 页面在刷新或首次进入时，只加载一批历史窗口、导致用户看不到数据库全部流量的问题；同时保证首屏窗口、历史回填和实时增量在同一序号模型下稳定合并。

## 实现逻辑

- `/_bifrost/api/traffic/updates` 在没有 `after_seq` / `after_id` 时，返回“最新 500 条”作为首屏启动窗口。
- 前端记录两个边界：
  - `lastSequence`：当前已加载的最新序号，用于实时增量继续向后拉取。
  - `oldestSequence`：当前已加载的最老序号，用于后台历史回填继续向前拉取。
- 历史回填改为轻量 GET 分页：`/api/traffic?direction=backward&cursor=<oldestSequence>&limit=500`，页面启动后后台持续拉取直到没有更老数据。
- 历史回填增加 retry/backoff：单页失败不会永久停止，而是按退避策略继续重试，直到历史真正补齐或用户主动清空/重置。
- 前端列表始终维护“按 sequence 升序”的不变量，但不做每次全量排序：
  - 历史批次先转成升序，再与当前数组做前插/线性归并。
  - 实时新增批次与当前数组做后插/线性归并。
  - 仅状态更新的记录原位替换，不改变位置。
- 浏览器窗口从 hidden 恢复或 push 连接重新建立后，前端会主动补一轮 `traffic/updates` catch-up，避免仅依赖 websocket 首批补数导致 reconnect 窗口漏 backlog。
- 为避免“全量历史模式”下每次 records 变化都触发整表派生计算，前端把两类高频派生改成增量维护：
  - 客户端筛选结果基于 mutation 做增量插入、替换、删除，只在筛选条件变化或全量 reset 时重算。
  - `clientApps` / `clientIps` / `domains` 改为 store 内按计数增量维护，不再每次遍历全部 records 重建。
- WebSocket push 的 `send_initial_traffic` 与定时 fallback 补数在 `last_sequence` 为空时也复用“最新窗口”语义，避免 HTTP 首屏和 push 首屏各拿一批不同的数据。
- push 客户端的 `last_sequence` 更新保持单调递增，避免首次补数与实时 delta 交错时把游标回退。

## 依赖项

- `crates/bifrost-admin/src/traffic_db/store.rs`
- `crates/bifrost-admin/src/handlers/traffic.rs`
- `crates/bifrost-admin/src/push.rs`
- `web/src/stores/useTrafficStore.ts`
- `web/src/api/traffic.ts`
- `web/src/types/index.ts`

## 测试方案（含 e2e）

- 新增 UI E2E：构造超过首屏窗口的大批量流量，只让最新几条命中特定筛选条件，验证首次进入和刷新后仍能看到这些记录。
- 新增 UI E2E：构造只存在于更老历史页中的记录，验证页面启动后会自动后台回填并最终显示出来。
- 复用现有 UI E2E：验证窗口 hidden 后恢复时会显式 catch-up 并补齐 backlog，不再漏掉恢复期间的新流量。
- 追加 Rust 单测：验证“最新窗口查询”返回的是最新记录且顺序为升序。
- 追加 push 单测：验证 `last_sequence` 为空时的初始 push 返回最新窗口，而不是最老窗口。

## 校验要求（含 rust-project-validate）

- 先执行本次相关 E2E / UI 测试。
- 任务结束前执行 `rust-project-validate` 要求的校验顺序：`fmt`、`clippy`、按改动范围测试、构建。

## 文档更新要求

- 本次为行为修复，不涉及 README / API 公共文档变更。
