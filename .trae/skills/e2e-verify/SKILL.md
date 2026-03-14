---
name: e2e-verify
description: |
  面向 Bifrost 管理端的端到端 UI 与 API 验证工具。
  适用于浏览器测试、场景回归、管理端接口验证与页面快照排查。
  Use when: 端到端验证、功能验证、E2E 测试、UI 测试、浏览器测试、API 测试、接口验证
---

# E2E Verify

该技能用于验证 `web/` 管理端和 `/_bifrost/api` 接口，优先使用已存在的场景与脚本，不要重新发明一套测试入口。

## 何时调用

- 需要验证 Bifrost 管理端页面流程
- 需要回归 `Traffic`、`Rules`、`Values`、`Scripts`、`Settings` 等核心页面
- 需要验证管理端 API 是否可用
- 需要基于页面快照定位元素或调试场景失败

## 前置条件

- 前端开发服务器通过 `web/` 目录启动：`pnpm dev`
- 默认 UI 入口：`http://localhost:3000/_bifrost/`
- 默认管理端 API：`http://127.0.0.1:9900/_bifrost/api`
- 若 9900 端口未启动，或者本次修改涉及管理端静态资源 / push / Traffic 页面，请优先使用“先编译、再启动”的方式：

```bash
CARGO_TARGET_DIR=./.bifrost-ui-target cargo build --bin bifrost
BIFROST_DATA_DIR=./.bifrost-e2e-ui ./.bifrost-ui-target/debug/bifrost start -p 9900 --unsafe-ssl
```

- 启动后必须确认：
  - `lsof -nP -iTCP:9900 -sTCP:LISTEN`
  - `curl -sS http://127.0.0.1:9900/_bifrost/api/proxy/address`

- 路由定义查看 [web/src/App.tsx](../../../web/src/App.tsx)
- API 说明查看 [crates/bifrost-admin/ADMIN_API.md](../../../crates/bifrost-admin/ADMIN_API.md)

## 目录

- `scripts/browser-test.js`：UI 测试主入口
- `scripts/api-test.js`：API 测试主入口
- `scripts/scenarios/`：内置场景
- `logs/`：快照和调试输出
- `screenshots/`：截图输出

## 快速开始

```bash
cd .trae/skills/e2e-verify/scripts
pnpm install
```

### 场景测试

先查看可用场景：

```bash
node browser-test.js scenario --list
```

当前内置场景：

- `stream-sse`
- `stream-ws`
- `traffic-delete`

运行场景：

```bash
node browser-test.js scenario stream-sse
node browser-test.js scenario stream-ws --headless --verbose
node browser-test.js scenario traffic-delete --actions
```

### 浏览器命令

```bash
node browser-test.js launch http://localhost:3000/_bifrost/ -i
node browser-test.js watch http://localhost:3000/_bifrost/
node browser-test.js sessions
node browser-test.js tools snapshot
```

### API 命令

```bash
node api-test.js --api /_bifrost/api/system/overview -p 9900 -v
node api-test.js --api /_bifrost/api/rules -p 9900
```

## 常用工作流

1. 优先运行已有场景，而不是新写一套交互脚本
2. 如果元素定位不稳，先用 `launch -i` 或 `watch` 查看快照
3. API 问题优先用 `api-test.js` 单独确认
4. 需要新增场景时，直接在 `scripts/scenarios/` 下补 JSON
5. 如果页面现象和 API 不一致，必须抓浏览器 `/api/push` websocket frame，确认是“后端没推”还是“页面没订阅/没消费”

## CLI 对齐说明

`browser-test.js help` 与本技能文档必须保持同步。若后续新增 CLI 参数或场景 schema，必须同时更新这两个位置。
