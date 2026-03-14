# Web Admin Rules / Values 同步策略

## 现状结论

旧文档中的“页面级首次加载 + 手动刷新”只说对了一半。当前实现已经继续演进为：

- `Rules`：规则列表仍按需 HTTP 拉取；全局值通过 push 订阅。
- `Values`：页面直接通过 push 订阅 `values_update` 获取快照，不再依赖首次 GET。

## 当前实现

- 全局初始化阶段不再轮询 `rules` / `values`。
- [`web/src/pages/Rules/index.tsx`](../web/src/pages/Rules/index.tsx)
  - 首次进入时如果规则列表为空，调用 `fetchRules()`。
  - 同时订阅 `need_values`，让规则编辑器拿到最新 values 快照。
- [`web/src/pages/Values/index.tsx`](../web/src/pages/Values/index.tsx)
  - 进入页面即通过 `pushService.connect({ need_values: true })` 获取 values 快照。
  - 断开页面时取消订阅。

## 与旧方案的差异

- `Values` 不再是“首次 GET + 手动刷新”模型，而是 push-first。
- `Rules` 也不是完全手动刷新模型，因为 values 依赖已经走 push 通道。

## 结论

这份文档不应再把 Rules / Values 视作同一种同步模式；当前是“Rules 混合模式，Values 推送优先”。
