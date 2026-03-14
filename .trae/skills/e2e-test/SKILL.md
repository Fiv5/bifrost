---
name: "e2e-test"
description: "创建和执行 Bifrost 代理的端到端测试；在添加新功能或修复 bug 后用于验证。必须优先于 rust-project-validate 技能执行。"
---

# E2E 测试创建与执行

该技能用于创建和执行 Bifrost 的端到端测试，优先覆盖本次变更涉及的能力。

## 何时调用

- 添加新功能后需要验证
- 修复 bug 后需要回归测试
- 需要创建新的测试用例
- 需要运行现有的 E2E 测试

## 参考文档

在执行任务前，**必须先阅读** E2E 测试框架的详细文档：

- [e2e-tests README](e2e-tests/readme.md) - 测试架构、目录结构、断言库、现有用例
- [测试覆盖率](e2e-tests/rules/COVERAGE.md) - 当前覆盖范围与缺口
- [项目规则](../../rules/project_rules.md) - 仓库级开发与验证要求

## 使用说明

- 启动代理服务时，必须显式设置临时数据目录，避免覆盖本机已有数据；目录前缀统一使用 `./.bifrost-e2e`
- 若需要手动启动服务，优先使用“先编译、再启动”的方式，而不是直接假设 `cargo run` 或旧进程已生效
- 对涉及前端静态资源或管理端推送逻辑的改动，优先执行：

```bash
CARGO_TARGET_DIR=./.bifrost-ui-target cargo build --bin bifrost
BIFROST_DATA_DIR=./.bifrost-e2e-test ./.bifrost-ui-target/debug/bifrost start -p 8800 --unsafe-ssl
```

- 启动后，必须同时检查：
  - 目标端口是否真的由最新 `bifrost` 进程监听，例如 `lsof -nP -iTCP:8800 -sTCP:LISTEN`
  - 管理端 API 是否 ready，例如 `curl -sS http://127.0.0.1:8800/_bifrost/api/proxy/address`
- 如果启动过程中出现证书安装交互，先显式处理掉，再继续测试，避免把“服务未启动完成”误判为功能问题
- 不同场景的步骤已拆分为独立文档；按需打开对应文件执行即可
- 当 UI 现象和 API 现象不一致时，优先做“进程级 + API + WebSocket frame”三层交叉验证：
  - 先确认当前测试页面连到的是哪一个 `bifrost` 进程
  - 再确认 `/_bifrost/api/traffic`、`/_bifrost/api/traffic/{id}` 返回的真实状态
  - 最后抓浏览器 `/api/push` 的 `framesent/framereceived`，判断是“服务端没推”还是“页面没订阅/没消费”
- 测试结束后清理临时目录和残留进程

## 场景文档

- [01-快速构建启动](01-快速构建启动.md)
- [02-SSE请求详情分批推送验证](02-SSE请求详情分批推送验证.md)
- [03-运行全量测试](03-运行全量测试.md)
- [04-运行单个测试](04-运行单个测试.md)
- [05-创建新测试](05-创建新测试.md)
- [06-调试与端到端验证方法](06-调试与端到端验证方法.md)
