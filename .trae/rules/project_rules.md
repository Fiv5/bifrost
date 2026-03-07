# Bifrost 项目开发规则

## 技术方案文档，务必存放在 `design` 目录下

- 所有技术方案文档都必须放在 `design` 目录下
- 每个功能模块的方案都必须有一个独立的文件，文件名格式为 `功能模块名.md`
- 相同功能模块的方案放在一个文件中，持续更新，保持与代码的同步。
- 每次需求都必须设计一个技术方案，方案文件必须放在 `design` 目录下

## 校验要求

- 每次开发任务结束前必须使用技能进行规范校验
- 使用技能：rust-project-validate

## 文档更新要求

- 如果修改涉及新功能、API 变更或配置变更，需要同步更新 `README.md`
- 如果添加了新的协议，需要更新 README 中的协议列表
- 如果添加了新的 Hook，需要更新 README 中的 Hook 表格
- 如果修改了命令行参数或配置选项，需要更新相关文档说明

## 启动服务的要求

- 启动服务时必须配置临时数据目录，避免覆盖正在运行的服务数据
- 必须采用立即编译运行的方式，避免使用已编译的二进制文件
- 示例：

```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- start -p 8800 --unsafe-ssl
```

## E2E 测试要求

- 添加新功能或修复 bug 后需要创建/执行端到端测试进行验证
- 使用技能：e2e-test

## 日志配置规范

### 日志级别优先级（从高到低）

1. `RUST_LOG` 环境变量 - 支持精细化控制，如 `RUST_LOG=bifrost_proxy=debug,info`
2. 命令行参数 `-l/--log-level` - 仅 bifrost-cli 支持
3. 默认值 `info`

### 各入口点日志初始化规范

| 入口点      | 初始化方式                     | 说明                              |
| ----------- | ------------------------------ | --------------------------------- |
| bifrost-cli | `init_logging(&cli.log_level)` | 使用 bifrost-core 统一函数        |
| bifrost-e2e | `tracing_subscriber::fmt()`    | 从 `--verbose` 和 `RUST_LOG` 读取 |

### verbose_logging 双轨机制

项目使用两套日志控制机制：

1. **tracing 级别** - 控制全局日志输出（通过 `EnvFilter`）
2. **verbose_logging 布尔值** - 控制详细业务日志（规则匹配、请求转发等）

规则：当日志级别为 `debug` 或 `trace` 时，`verbose_logging` 自动设为 `true`

```rust
let verbose_logging = matches!(log_level.as_str(), "debug" | "trace");
```

### 新增入口点时的要求

1. 必须支持 `RUST_LOG` 环境变量优先
2. 如果有命令行参数，作为 `RUST_LOG` 未设置时的回退
3. 如果需要传递 `verbose_logging`，必须根据日志级别正确设置
4. 使用 `bifrost_core::init_logging()` 统一初始化（已支持 RUST_LOG 回退）

### 关键文件位置

- 日志初始化：`crates/bifrost-core/src/logging.rs`
- CLI 入口：`crates/bifrost-cli/src/main.rs`
- E2E 入口：`crates/bifrost-e2e/src/main.rs`
- ProxyConfig 定义：`crates/bifrost-proxy/src/server.rs`

### 日志要求

- 所有日志输出必须符合 tracing 标准，包含 `target`、`level`、`message` 等字段
- 所有日志级别必须根据 `RUST_LOG` 环境变量或 `--log-level` 参数进行控制
- 所有日志输出必须包含 `file`、`line`、`module` 等字段，方便定位问题
- 开发新需求时务必添加详细的日志，方便调试和问题定位
