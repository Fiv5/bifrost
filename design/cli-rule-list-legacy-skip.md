# CLI `rule list` legacy 解析容错

## 功能模块详细描述

修复 `bifrost rule list` 在本地规则目录存在损坏的 legacy `.json` 规则文件时整体失败的问题。

### 问题表现
1. 本地 `rules/` 目录里只要有一个 legacy 规则文件缺字段或 JSON 格式损坏，`bifrost rule list` 就直接报错退出。
2. 其余可正常解析的本地规则无法展示，用户无法继续查看已有规则状态。
3. 列表命令的失败范围过大，不符合“坏文件跳过即可”的容错预期。

### 范围边界
- 本次修复只影响 `rule list` 对本地规则的列出逻辑。
- 不修改 group 规则读取范围；`rule list` 仍然只列出本地规则，不包含其他 group 的规则。

## 实现逻辑

### CLI 修复 (`crates/bifrost-cli/src/commands/rule.rs`)
- `rule list` 改为调用 `RulesStorage::list_summaries()`
- 复用已有“逐项解析失败则跳过并记录 warning”的摘要加载逻辑
- 输出仍保持 `name [enabled|disabled]` 格式，只是坏的 legacy 文件不再阻断整体命令

### 存储层回归 (`crates/bifrost-storage/src/rules.rs`)
- 增加 invalid legacy 文件回归测试
- 验证 `load_all()` 与 `list_summaries()` 遇到缺少 `name` 的 legacy 文件时会跳过坏文件，并保留可用规则

## 依赖项
- 无新增依赖

## 测试方案

### 单元测试
- `test_load_all_skips_invalid_legacy_rule_file`
- `test_list_summaries_skips_invalid_legacy_rule_file`

### E2E 测试
- 新增 CLI 回归脚本，构造 1 个正常规则和 1 个损坏的 legacy 规则文件
- 执行 `bifrost rule list`，断言命令成功、输出包含正常规则、不包含解析失败中断

### 真实场景测试
- 更新 `human_tests/cli-rule-management.md`
- 新增“损坏 legacy 规则文件被跳过”的回归用例，并在临时数据目录下逐条执行

## 校验要求
- `cargo test -p bifrost-storage`
- `cargo test --workspace --all-features`
- `bash e2e-tests/tests/test_rule_list_legacy_skip.sh`
- `bash scripts/ci/local-ci.sh`
- `rust-project-validate`

## 文档更新要求
- 更新 `human_tests/cli-rule-management.md`
- 更新 `human_tests/readme.md`
