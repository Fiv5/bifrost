# CLI `status` 活跃规则摘要

## 背景

当前 `bifrost status` 会展示进程状态与 Rule Groups 信息，但如果用户想直接查看“当前运行实例实际生效的活跃规则合并结果”，还需要额外执行一次 `bifrost rule active`。

这会让 `status` 作为运行态概览命令的信息不完整，排查规则优先级或规则是否真正生效时需要来回切换命令。

## 目标

- 在 `bifrost status` 输出末尾追加活跃规则摘要
- 仅当服务处于运行状态时展示该摘要
- 复用现有 `rule active` 的管理端接口与展示格式，避免两处实现漂移

## 实现方案

### 复用 active-summary

继续使用已有运行时接口：

```text
GET /_bifrost/api/rules/active-summary
```

该接口已经返回：

- 活跃规则文件数量
- 本地规则与 group 规则列表
- 变量冲突信息
- `merged_content`（按解析顺序合并后的规则内容）

### CLI 结构调整

- 在 `crates/bifrost-cli/src/commands/rule.rs` 中抽取可复用的：
  - active-summary 拉取逻辑
  - active-summary 文本格式化逻辑
- `bifrost rule active` 继续使用同一套逻辑输出
- `crates/bifrost-cli/src/commands/status.rs` 在确认服务运行后，现有 `Rule Groups` 输出后追加活跃规则摘要

### 输出边界

- 服务运行中：
  - `status` 保留现有基础状态信息
  - 保留现有 `Rule Groups` 区块
  - 追加 `Active Rules Summary` 区块
- 服务未运行：
  - 保持现有停止态输出
  - 不追加 `Active Rules Summary` 区块
- 服务运行但 active-summary 拉取失败：
  - 在 `status` 中输出一行提示，说明无法从运行中的服务获取活跃规则摘要

## 测试方案

### 单元测试

- `status` 运行态渲染时包含 `Active Rules Summary`
- `status` 停止态渲染时不包含 `Active Rules Summary`
- active-summary 格式化在有 `merged_content` 和空内容时都能稳定输出

### E2E 测试

- 更新 `e2e-tests/tests/test_cli_online_commands_e2e.sh`
- 启动服务并写入启用规则后执行 `bifrost status`
- 断言输出包含：
  - `Active Rules Summary`
  - `Merged Rules (in parsing order)`
  - 已启用规则对应的合并内容

### 真实场景测试

- 更新 `human_tests/cli-start-stop-status.md`
- 覆盖两类场景：
  - 服务运行时 `status` 展示活跃规则摘要
  - 服务停止时 `status` 不展示该区块

## 影响文件

- `crates/bifrost-cli/src/commands/rule.rs`
- `crates/bifrost-cli/src/commands/status.rs`
- `e2e-tests/tests/test_cli_online_commands_e2e.sh`
- `human_tests/cli-start-stop-status.md`
- `human_tests/readme.md`
