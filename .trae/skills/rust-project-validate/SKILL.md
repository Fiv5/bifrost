---
name: "rust-project-validate"
description: "运行 cargo fmt/clippy/build/e2e/test 验证项目规范；在每次任务结束前必须调用。"
---

# Rust 项目规范校验

该技能在任务结束前执行一键规范校验，确保代码风格、静态检查、构建与测试均通过。

## 何时调用

- 每次开发任务结束前必须调用
- 提交代码或发起评审前建议调用

## 执行内容

- 格式检查：`cargo fmt --all -- --check`
- Lint 检查：`cargo clippy --all-targets --all-features -- -D warnings`
- 运行代理服务，构造测试用例，进行端到端测试，覆盖 HTTP/1.1、HTTP/2、HTTPS、SOCKS5、CONNECT-UDP 等场景，覆盖 TLS 与非 TLS 情况，覆盖 TSL 解包和不解包场景，覆盖 HTTP/3 场景。
- 运行测试：`cargo test --all-features`，务必按照修改范围执行，避免执行所有测试用例，造成测试用例执行时间过长，影响任务完成。
- 完整构建：`cargo build --all-targets --all-features`

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
cargo fmt --all -- --check # 检查代码格式是否符合规范
cargo clippy --all-targets --all-features -- -D warnings # 检查代码是否符合 Rust 编码规范
cargo run --bin  bifrost -- start -p 9900 # 启动代理，并单独运行测试用例
cargo test --all-features # 执行单元测试，按需执行
cargo build --all-targets --all-features # 最终构建项目
```

## 注意

- 与项目规则一致：参考 [.trae/rules/project_rules.md](file:///Users/eden/work/github/whistle/rust/.trae/rules/project_rules.md)
