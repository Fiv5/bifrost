# Web Admin Rules 列表性能优化

## 背景

- 打开 `Rules` 页面时，前端会请求 `GET /_bifrost/api/rules`
- 旧实现中，后端会先加载全部规则文件，再对每个规则内容执行一次完整 `validate_rules_with_context`
- 当规则文件数量较多、单文件内容较大时，页面首次加载会出现明显延迟和 CPU 峰值

## 问题定位

- `crates/bifrost-admin/src/handlers/rules.rs`
  - `list_rules()` 为了返回列表摘要，逐条重新校验规则内容
- `crates/bifrost-storage/src/rules.rs`
  - `list_summaries()` 依赖 `load_all()`，会把所有规则正文完整载入

列表页实际只依赖 `name`、`enabled`、`rule_count`，不需要在列表请求阶段执行全量语法校验。

## 实现方案

1. `GET /api/rules` 改为使用 `rules_storage.list_summaries()`
2. `RulesStorage::list_summaries()` 改为逐文件读取轻量摘要
3. 对 `.bifrost` 规则文件，仅解析 `meta` 与 `options.rule_count`
4. 保留 legacy `.json` 规则文件的兼容摘要读取

## 预期收益

- 打开 `Rules` 页面时不再触发 N 次规则校验
- 列表读取避免为摘要场景解析完整规则正文
- CPU 峰值和页面首屏等待时间显著下降

## 测试方案

- API smoke test：启动临时 `bifrost` 实例后访问 `/_bifrost/api/rules`
- Rust 单测：执行 `bifrost-storage` 相关测试，确认摘要读取与排序行为正常
- 最终执行 `rust-project-validate` 规定的 fmt / clippy / test / build

## 文档影响

- 本次无用户可见 API 字段新增，也无配置变更
- 暂不需要更新 `README.md`
