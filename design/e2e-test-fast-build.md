---
title: e2e-test 快速构建启动说明
status: draft
---

# 背景

e2e-test 技能文档中需要明确快速构建启动方式，避免每次执行 E2E 前触发前端构建，提升迭代效率。

# 目标

- 在技能文档中增加快速构建启动说明
- 指明 SKIP_FRONTEND_BUILD=1 与 make dev 的推荐用法

# 方案

- 在 e2e-test 技能文档的“执行测试”部分新增“快速构建启动（推荐）”小节
- 提供两种等价启动方式，便于本地调试与 E2E 验证

# 影响范围

- 文档更新：.trae/skills/e2e-test/SKILL.md
