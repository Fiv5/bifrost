---
name: "rust-project-validate"
description: "运行 cargo fmt/clippy/build/e2e/test 验证项目规范；在每次任务结束前必须调用，重要的是必须在端到端测试之后执行，为提交代码做最后准备"
---

# Rust 项目规范校验

必须：该技能在任务结束前执行一键规范校验，确保代码风格、静态检查、构建与测试均通过。
避免：还没进行 api和交互测试的情况下，就开始执行规范校验，因为这时候代码可能还不是最终版本，还不能确保通过所有测试用例。

## 何时调用

- 每次开发任务结束前必须调用
- 提交代码或发起评审前建议调用

## 执行内容

1. 格式检查：`cargo fmt --all -- --check`
2. Lint 检查：`cargo clippy --all-targets --all-features -- -D warnings`（避免无差别检查，应该按改动范围检查）
3. 执行端到端用例，按本次任务的测试范围执行
4. 运行测试：优先按修改范围执行 `cargo test`，避免无差别跑完整套耗时用例
5. 完整构建：`cargo build --all-targets --all-features`（避免无差别构建，应该按改动范围构建）

如果任一步失败，立即停止并返回失败报告。

## 输出

- 结构化报告，按步骤给出状态（通过/失败）与关键信息
- 当 `fmt --check` 失败时提示可使用 `cargo fmt --all` 自动修复

## 前置条件

- 项目为 Rust 工作空间，已正确安装 Rust toolchain 与 cargo
- 在仓库根目录执行

## 示例

运行本技能将顺序执行：

```bash
cargo fmt --all -- --check # 检查代码格式是否符合规范
cargo clippy --all-targets --all-features -- -D warnings # 检查代码是否符合 Rust 编码规范
cargo test -p bifrost-e2e -- --list # 先确认本次需要覆盖的 E2E 范围
cargo test -p <changed-crate> --all-features # 按修改范围执行测试
cargo build --all-targets --all-features # 最终构建项目
```

## 注意

- 与项目规则一致：参考 [../../rules/project_rules.md](../../rules/project_rules.md)
