# Replay 创建分组去重

## 功能模块详细描述

修复 Replay 页面新建分组后列表里临时出现两个同名分组、刷新页面后恢复正常的问题。

当前表现是：

- 点击新建分组后，左侧分组树会短暂出现两个相同分组
- 实际后端只创建了一条记录，刷新页面后会恢复成单条
- 说明问题出在前端状态同步，而不是数据库重复写入

## 实现逻辑

1. 保持后端创建分组后广播 `replay_groups_update` 全量快照的现有行为。
2. 前端 `createGroup` 不再无条件把创建结果 append 到 `groups`。
3. 改为按 `group.id` 做 upsert：
   - 如果 push 快照已经先把该分组写进 store，则只覆盖同 id 项
   - 如果 push 还没到，则补入新分组
4. 这样无论“push 先到”还是“POST 响应先到”，前端分组列表最终都只保留一条记录。

## 依赖项

- `web/src/stores/useReplayStore.ts`
- `web/tests/ui/admin-replay.spec.ts`

## 测试方案（含 e2e）

1. 打开 Replay 页面。
2. 通过 UI 新建一个分组。
3. 通过管理端 API 读取该分组 id。
4. 断言前端列表中对应 `data-group-id` 的节点数量为 1。
5. 按任务要求执行 Replay UI E2E 回归后，再执行 rust-project-validate。

## 校验要求（含 rust-project-validate）

- 先执行本次变更相关的 Replay UI E2E 回归
- 再依次执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`（按修改范围执行）
  - `cargo build --all-targets --all-features`

## 文档更新要求

- 本次为管理端交互修复，不涉及 README、API 或配置项变更
