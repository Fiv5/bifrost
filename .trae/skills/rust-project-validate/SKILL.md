---
name: "rust-project-validate"
description: "运行 cargo fmt/clippy/build/test 验证项目规范；在每次任务结束前必须调用。"
---

# Rust 项目规范校验

该技能在任务结束前执行一键规范校验，确保代码风格、静态检查、构建与测试均通过。

## 何时调用
- 每次开发任务结束前必须调用
- 提交代码或发起评审前建议调用

## 执行内容
- 格式检查：`cargo fmt --all -- --check`
- Lint 检查：`cargo clippy --all-targets --all-features -- -D warnings`
- 完整构建：`cargo build --all-targets --all-features`
- 运行测试：`cargo test --all-features`

如果任一步失败，立即停止并返回失败报告。

## 输出
- 结构化报告，按步骤给出状态（通过/失败）与关键信息
- 当 `fmt --check` 失败时提示可使用 `cargo fmt --all` 自动修复

## 前置条件
- 项目为 Rust 工作空间，已正确安装 Rust toolchain 与 cargo
- 在仓库根目录执行

## 示例
运行本技能将顺序执行：
```
cargo fmt --all -- --check && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo build --all-targets --all-features && \
cargo test --all-features
```

## 注意
- 与项目规则一致：参考 [.trae/rules/project_rules.md](file:///Users/eden/work/github/whistle/rust/.trae/rules/project_rules.md)
