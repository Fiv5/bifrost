# Traffic Fuzzy Search Filter Isolation

## 功能模块详细描述

Traffic 页的 `Add Filter` 条件只用于当前列表过滤，不应影响 Fuzzy Search 的关键词搜索结果。

## 实现逻辑

- 保留 Fuzzy Search 对 toolbar filter 与左侧 panel filter 的支持。
- 构造搜索请求时，不再把 `FilterBar` 的 `filterConditions` 写入 `filters.conditions`。
- 这样 Fuzzy Search 只受关键词、scope 和显式筛选面板影响，不会被列表临时过滤条件误伤。

## 依赖项

- `web/src/components/SearchMode/index.tsx`
- `web/tests/ui/traffic.spec.ts`

## 测试方案（含 e2e）

- 新增 UI 回归用例：
  - 在 Traffic 页添加一个不会命中任何记录的 `Add Filter`
  - 切换到 Fuzzy Search 并搜索已存在请求
  - 断言搜索请求中的 `filters.conditions` 为空
  - 断言搜索结果仍然能返回目标请求

## 校验要求（含 rust-project-validate）

- 先执行与本改动相关的 UI E2E
- 再执行 `rust-project-validate` 要求的 fmt / clippy / test / build

## 文档更新要求

- 本次不涉及 README / API / 配置项变更，无需额外更新公开文档
