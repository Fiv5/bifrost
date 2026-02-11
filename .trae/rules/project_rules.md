# Bifrost 项目开发规则

## 代码修改后的验证流程

每次修改完成后，必须按顺序执行以下验证步骤：

### 1. 代码格式检查

```bash
cargo fmt --all -- --check
```

如果格式有问题，使用 `cargo fmt --all` 自动修复。

### 2. Lint 检查

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### 3. 完整构建

```bash
cargo build --all-targets --all-features
```

### 4. 运行测试用例

```bash
cargo test --all-features
```

### 5. 更新项目文档（如有需要）

- 如果修改涉及新功能、API 变更或配置变更，需要同步更新 `README.md`
- 如果添加了新的协议，需要更新 README 中的协议列表
- 如果添加了新的 Hook，需要更新 README 中的 Hook 表格
- 如果修改了命令行参数或配置选项，需要更新相关文档说明

## 一键验证命令

提交前可使用以下命令进行完整验证：

```bash
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo build --all-targets --all-features && cargo test --all-features
```

## 启动服务的要求

必须配置临时数据目录，避免启动服务时覆盖了正在运行的服务数据
例如：

```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- -p 8080  --unsafe-ssl
```
