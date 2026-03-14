# Web Admin 资源 Push 通道统一

## 现状结论

这份设计大体已经落地，但“全部资源都纯 push 化”仍不准确；当前是按资源类型采用不同程度的 push 优先策略。

## 当前已落地能力

- 订阅协议已支持：
  - `need_values`
  - `need_scripts`
  - `need_replay_saved_requests`
  - `need_replay_groups`
  - `settings_scopes`
- push 消息已支持：
  - `values_update`
  - `scripts_update`
  - `settings_update`
  - `replay_saved_requests_update`
  - `replay_groups_update`
- `Values` / `Scripts` / `Replay` / `Settings` 已不同程度消费这些消息。

## 当前真实同步模型

- `Values`：push-first。
- `Scripts`：push-first。
- `Replay`：保存请求与分组使用 push 快照；历史更新仍有独立消息。
- `Settings`：按 scope 订阅更新。
- `Rules`：规则列表本身仍通过 HTTP 拉取，但会复用 `need_values` 的 push 数据做补全与校验。

## 文档修正

- 当前统一的是“资源同步总线”，不是“所有页面首屏都完全不再 GET”。
- 需要保留“部分资源仍用 HTTP 首次拉取，push 负责收敛变更”这一事实。
