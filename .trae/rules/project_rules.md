# Bifrost 项目开发规则

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
- 示例：

```bash
BIFROST_DATA_DIR=./.bifrost-test cargo run --bin bifrost -- -p 8080 --unsafe-ssl
```

## E2E 测试要求

- 添加新功能或修复 bug 后需要创建/执行端到端测试进行验证
- 使用技能：e2e-test
