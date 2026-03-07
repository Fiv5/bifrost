---
name: e2e-verify
description: |
  面向 Bifrost 管理端的端到端 UI 测试工具，基于 UID 快照系统实现可靠的元素定位和交互自动化。
  提供 76 个测试工具，覆盖输入、快照、导航、断言、网络、设备仿真和调试等功能。
  端到端 API 测试，支持独立的管理端 API 验证，可直接验证服务端接口。
  Use when: 端到端验证、功能验证、E2E 测试、UI 测试、浏览器测试、API 测试、接口验证
---

## ⚠️ 重要说明

**浏览器测试必须通过本地 devserver 端口访问**

- 默认入口：`http://localhost:3000/_bifrost/`
- 使用前请先启动前端开发服务器：`pnpm dev`（在 web 目录）
- 所有页面路由，请查看 web/src/App.tsx 文件中的配置
- 执行脚本的日志输出必须存放在 .trae/skills/e2e-verify/logs 目录中
- 必须先检查代理 服务是否启动9900端口，如果没有启动则需要使用 `cargo run --bin bifrost -- --start -p 9900 --unsafe-ssl` 启动

## 核心特性

- **UID 元素定位系统**：基于可访问性树的稳定元素定位，无需依赖 CSS 选择器
- **自愈错误建议**：元素未找到时自动提供相似元素建议
- **网络/控制台收集**：自动追踪所有网络请求和控制台日志
- **API 测试**：支持独立的 API 接口验证

## 本项目设计

- **Web 管理端**：`web/` 目录下的 React 应用，路由基路径为 `/_bifrost`
- **管理端 API**：`/_bifrost/api` 前缀，默认由 `http://127.0.0.1:9900` 提供
- **本地开发入口**：`http://localhost:3000/_bifrost/`（Vite dev server 端口 3000）
- **API 文档**：`crates/bifrost-admin/ADMIN_API.md`

## 文件结构

```
scripts/
├── browser-test.js              # UI 测试主入口（导出所有核心函数）
├── api-test.js                  # API 测试模块（独立的 API 验证能力）
├── scenarios/                   # 场景配置
│   ├── stream-sse.json           # SSE 流式消息展示
│   └── stream-ws.json            # WebSocket 流式消息展示
├── lib/
│   ├── config.js                # 配置常量
│   ├── logger.js                # 日志工具
│   ├── utils.js                 # 工具函数
│   ├── browser-manager.js       # 浏览器管理（启动/关闭/监听）
│   ├── snapshot-manager.js      # UID 快照管理器
│   ├── test-context.js          # 测试上下文（统一状态管理）
│   ├── network-collector.js     # 网络请求收集器
│   ├── scenario-executor.js     # 场景执行器
│   ├── action-handlers.js       # 动作处理器
│   ├── variable-resolver.js     # 变量解析器
│   └── tools/                   # 工具模块（76 个工具）
│       ├── index.js             # 工具注册表
│       ├── input.js             # 输入操作（16 个）
│       ├── snapshot.js          # 快照操作（6 个）
│       ├── navigation.js        # 导航操作（10 个）
│       ├── assert.js            # 断言操作（10 个）
│       ├── network.js           # 网络操作（10 个）
│       ├── emulation.js         # 设备仿真（11 个）
│       └── debugging.js         # 调试操作（13 个）
├── examples/                    # 示例脚本
│   ├── api-search-proxy.txt      # Search/System Proxy API 用例
│   └── seed-streaming-traffic.sh # SSE/WS 流量模拟
├── screenshots/                 # 截图存储目录
└── logs/                        # 日志存储目录
```

## 快速开始

### 前置条件

从 traffic 列表响应中取 `id` 再查询详情：

```bash
# 1. 进入脚本目录
cd .trae/skills/e2e-verify/scripts

# 2. 安装依赖
pnpm install

# 3. 启动前端开发服务器（在另一个终端）
cd ../../../../web
pnpm install
pnpm dev
```

### 场景测试（推荐）

场景测试是执行 E2E 测试的推荐方式，通过 JSON 配置文件定义测试步骤：

```bash
# 运行场景测试
node browser-test.js scenario app-create

# 无头模式运行（不显示浏览器窗口）
node browser-test.js scenario app-create --headless

# 测试后保持浏览器打开（用于调试）
node browser-test.js scenario app-create -k

# 测试结束时截图
node browser-test.js scenario app-create -s

# 组合选项
node browser-test.js scenario app-create --headless -s

# 查看可用场景列表
node browser-test.js scenario --list

# 查看支持的 actions
node browser-test.js scenario --actions

# SSE/WS 流式场景
node browser-test.js scenario stream-sse --verbose
node browser-test.js scenario stream-ws --verbose
```

#### 场景命令参数

| 参数               | 说明                            |
| ------------------ | ------------------------------- |
| `<场景名称>`       | 运行指定场景（不含 .json 后缀） |
| `--list`, `-l`     | 列出所有可用场景                |
| `--actions`, `-a`  | 列出所有支持的 actions          |
| `--headless`       | 无头模式运行                    |
| `--no-ppe`         | 禁用 PPE 模式                   |
| `-k, --keep-open`  | 测试后保持浏览器打开            |
| `-s, --screenshot` | 测试结束时截图                  |
| `--no-network`     | 禁用网络请求日志                |

#### 场景配置文件示例

场景文件位于 `scripts/scenarios/` 目录，格式如下：

```json
{
  "name": "Settings TLS 配置检查",
  "description": "切换到 Settings 页面并校验 TLS 配置项状态",
  "config": {
    "baseUrl": "http://localhost:3000/_bifrost",
    "waitForLogin": true,
    "loginTimeout": 120000,
    "timeout": 30000
  },
  "steps": [
    { "action": "log", "message": "==== 步骤 1: 进入首页 ====" },
    { "action": "goto", "url": "/" },
    { "action": "waitForLogin" },
    { "action": "takeSnapshot", "name": "首页" },
    { "action": "log", "message": "==== 步骤 2: 进入 Settings ====" },
    { "action": "goto", "url": "/settings" },
    { "action": "takeSnapshot", "name": "Settings" },
    {
      "action": "evaluate",
      "variable": "tlsStatus",
      "code": "return fetch('http://127.0.0.1:9900/_bifrost/api/config/tls').then(r => r.json()).then(d => { const v = typeof d.enable_tls_interception === 'boolean' ? (d.enable_tls_interception ? 'enabled' : 'disabled') : 'unknown'; window.__tlsStatus = v; return v; }).catch(() => { window.__tlsStatus = 'unknown'; return 'unknown'; });"
    },
    {
      "action": "assert",
      "condition": "window.__tlsStatus !== 'unknown'",
      "message": "TLS 配置读取成功"
    },
    { "action": "log", "message": "TLS 拦截状态: ${tlsStatus}" }
  ]
}
```

### 其他命令

```bash
# 启动浏览器并进入交互模式
node browser-test.js launch http://localhost:3000/_bifrost/ -i

# 监视模式（实时快照更新）
node browser-test.js watch http://localhost:3000/_bifrost/

# 查看已保存的会话
node browser-test.js sessions

# 查看工具列表
node browser-test.js tools [category]
```

### 命令行参数

| 命令     | 参数                 | 说明                       |
| -------- | -------------------- | -------------------------- |
| scenario | `<name> [options]`   | 运行场景测试（推荐）       |
| launch   | `[url] [options]`    | 启动浏览器并可选导航到 URL |
|          | `--headless`         | 无头模式运行               |
|          | `--no-ppe`           | 禁用 PPE 模式              |
|          | `-i, --interactive`  | 进入交互模式               |
| detach   | -                    | 启动分离模式的浏览器       |
| connect  | -                    | 连接到分离模式的浏览器     |
| verify   | `<config.json>`      | 从 JSON 配置文件运行验证   |
| run      | `<script.txt> [url]` | 从脚本文件运行测试         |
| watch    | `[url]`              | 监视模式，实时快照更新     |
| sessions | -                    | 列出已保存的浏览器会话     |
| tools    | `[category]`         | 列出可用工具               |

## Web 核心功能验证方法

推荐使用场景测试对核心功能进行可重复验证，按“页面入口 → 关键 UI → 关键 API”组织步骤：

- **入口与路由**：导航到 `http://localhost:3000/_bifrost/`，校验自动跳转到 `/traffic`
- **Traffic**：等待 `/_bifrost/api/traffic` 与 `/_bifrost/api/traffic/updates` 响应，断言列表区域可见并可打开详情
- **Replay**：等待 `/_bifrost/api/replay/groups` 与 `/_bifrost/api/replay/requests` 响应，断言分组与请求列表渲染
- **Rules**：等待 `/_bifrost/api/rules` 响应，断言规则列表与编辑区域可见
- **Values**：等待 `/_bifrost/api/values` 响应，断言变量列表可见
- **Scripts**：等待 `/_bifrost/api/scripts` 与 `/_bifrost/api/scripts/test` 响应，断言脚本列表与编辑区域可用
- **Settings**：等待 `/_bifrost/api/system/overview`、`/_bifrost/api/config`、`/_bifrost/api/metrics` 响应，断言配置与指标区域可见

建议在每个页面步骤中组合使用 `takeSnapshot`、`findElements`、`assertVisible`、`waitForResponse` 与 `getNetworkRequests` 来完成 UI 与 API 的双重校验。

## 工具参考

### 输入操作 (input) - 16 个

| 工具           | 参数                             | 说明                                |
| -------------- | -------------------------------- | ----------------------------------- |
| click          | uid?, selector?                  | 点击元素                            |
| clickAt        | x, y                             | 点击指定坐标                        |
| doubleClick    | uid?, selector?                  | 双击元素                            |
| hover          | uid?, selector?                  | 悬停元素                            |
| fill           | uid?, selector?, value, clear?   | 填充输入框                          |
| fillForm       | fields (array)                   | 批量填充表单                        |
| select         | uid?, selector?, value           | 选择下拉选项                        |
| drag           | uid?, selector?, x, y            | 拖拽元素到坐标                      |
| dragTo         | source (object), target (object) | 拖拽元素到目标                      |
| uploadFile     | uid?, selector?, file?, files?   | 上传文件                            |
| pressKey       | key, modifiers?                  | 按键（支持 Alt/Control/Meta/Shift） |
| type           | text, delay?                     | 键入文本                            |
| focus          | uid?, selector?                  | 聚焦元素                            |
| blur           | uid?, selector?                  | 取消聚焦                            |
| scroll         | x?, y?, uid?, selector?          | 滚动页面或元素                      |
| scrollIntoView | uid?, selector?, block?          | 滚动元素到视图                      |

### 快照操作 (snapshot) - 6 个

| 工具                   | 参数                              | 说明                      |
| ---------------------- | --------------------------------- | ------------------------- |
| takeSnapshot           | -                                 | 获取页面快照（文本格式）  |
| getSnapshotJSON        | -                                 | 获取页面快照（JSON 格式） |
| findElements           | query?, role?, name?, refresh?    | 查找元素                  |
| getInteractiveElements | refresh?                          | 获取可交互元素列表        |
| screenshot             | name?, fullPage?, type?, quality? | 截图                      |
| getElementInfo         | uid?, selector?                   | 获取元素详情              |

### 导航操作 (navigation) - 10 个

| 工具                | 参数                               | 说明                                          |
| ------------------- | ---------------------------------- | --------------------------------------------- |
| navigate            | url, waitUntil?                    | 导航到 URL                                    |
| goBack              | waitUntil?                         | 后退                                          |
| goForward           | waitUntil?                         | 前进                                          |
| reload              | waitUntil?                         | 刷新                                          |
| waitForNavigation   | waitUntil?, timeout?               | 等待导航完成                                  |
| waitForLoad         | state?, timeout?                   | 等待页面加载                                  |
| getPageInfo         | -                                  | 获取页面信息（title/url/viewport/readyState） |
| waitForUrl          | url?, pattern?, timeout?, partial? | 等待 URL 匹配                                 |
| setExtraHTTPHeaders | headers (object)                   | 设置请求头                                    |
| setUserAgent        | userAgent                          | 设置 UA                                       |

### 断言操作 (assert) - 10 个

| 工具            | 参数                             | 说明             |
| --------------- | -------------------------------- | ---------------- |
| assertVisible   | uid?, selector?, timeout?        | 断言元素可见     |
| assertHidden    | uid?, selector?, timeout?        | 断言元素隐藏     |
| assertText      | uid?, selector?, text, contains? | 断言元素文本     |
| assertValue     | uid?, selector?, value           | 断言输入值       |
| assertChecked   | uid?, selector?, checked?        | 断言勾选状态     |
| assertDisabled  | uid?, selector?, disabled?       | 断言禁用状态     |
| assertCount     | selector, count                  | 断言元素数量     |
| assertPageTitle | title, contains?                 | 断言页面标题     |
| assertUrl       | url?, pattern?, partial?         | 断言 URL         |
| assertNoErrors  | ignorePatterns?                  | 断言无控制台错误 |

### 网络操作 (network) - 10 个

| 工具                   | 参数                                                 | 说明         |
| ---------------------- | ---------------------------------------------------- | ------------ |
| getNetworkRequests     | urlPattern?, status?, method?, resourceType?, limit? | 获取网络请求 |
| getRequestContent      | id                                                   | 获取请求详情 |
| getFailedRequests      | limit?                                               | 获取失败请求 |
| waitForRequest         | url?, urlPattern?, timeout?                          | 等待请求     |
| waitForResponse        | url?, urlPattern?, timeout?                          | 等待响应     |
| clearNetworkRequests   | -                                                    | 清除请求记录 |
| setRequestInterception | enabled                                              | 启用请求拦截 |
| mockRequest            | url?, urlPattern?, response (object), once?          | Mock 请求    |
| blockRequests          | patterns?, resourceTypes?                            | 阻止请求     |
| getNetworkStats        | -                                                    | 获取网络统计 |

### 设备仿真 (emulation) - 11 个

预置设备（17 种）：

- **iPhone**: SE, XR, 12 Pro, 14, 14 Pro, 14 Pro Max
- **iPad**: Air, Mini, Pro 11, Pro 12.9
- **Android**: Pixel 5, Pixel 7 Pro, Samsung Galaxy S8+, S20 Ultra, S23
- **其他**: Surface Pro 7, Galaxy Fold

| 工具                 | 参数                                                    | 说明                                    |
| -------------------- | ------------------------------------------------------- | --------------------------------------- |
| setViewport          | width, height, deviceScaleFactor?, isMobile?, hasTouch? | 设置视口                                |
| emulateDevice        | device                                                  | 模拟设备                                |
| setGeolocation       | latitude, longitude, accuracy?                          | 设置地理位置                            |
| setPermissions       | permissions (array), origin?                            | 设置权限                                |
| setOffline           | offline                                                 | 设置离线模式                            |
| setTimezone          | timezoneId                                              | 设置时区                                |
| setColorScheme       | colorScheme                                             | 设置颜色模式（light/dark）              |
| setCPUThrottling     | rate?                                                   | 设置 CPU 限制（默认 4x）                |
| setNetworkConditions | download?, upload?, latency?, offline?                  | 设置网络条件                            |
| setSlowNetwork       | type?                                                   | 设置慢网络预设（3G/Slow 3G/Fast 3G/4G） |
| getAvailableDevices  | -                                                       | 获取可用设备列表                        |

### 调试操作 (debugging) - 13 个

| 工具                 | 参数                    | 说明                          |
| -------------------- | ----------------------- | ----------------------------- |
| getConsoleLogs       | level?, limit?          | 获取控制台日志                |
| getConsoleErrors     | limit?                  | 获取控制台错误                |
| getConsoleWarnings   | limit?                  | 获取控制台警告                |
| clearConsoleLogs     | -                       | 清除控制台日志                |
| evaluateScript       | script                  | 执行脚本                      |
| evaluateOnElement    | uid?, selector?, script | 在元素上执行脚本              |
| getPageMetrics       | -                       | 获取页面指标                  |
| getPerformanceTiming | -                       | 获取性能计时                  |
| getCoverage          | type?                   | 获取代码覆盖率（js/css）      |
| startCoverage        | type?                   | 开始覆盖率收集（js/css/both） |
| getStatus            | -                       | 获取测试状态                  |
| waitForDebugger      | timeout?                | 等待调试器                    |
| resumeDebugger       | -                       | 恢复调试器                    |

## UID 快照系统

### 快照格式

```
- navigation "Main Navigation" [e1]
  - link "Home" [e2]
  - link "About" [e3]
- main [e4]
  - heading "Welcome" [e5]
  - textbox "Email" [e6]
  - button "Submit" [e7]
```

### 使用 UID 定位元素

```javascript
// 通过 UID 点击
click {"uid": "e7"}

// 通过 UID 填充
fill {"uid": "e6", "value": "test@example.com"}

// 通过 UID 获取信息
getElementInfo {"uid": "e5"}
```

### 自愈建议

当元素未找到时，工具会返回相似元素建议：

```json
{
  "success": false,
  "error": "Element with UID 'e99' not found",
  "suggestions": [
    { "uid": "e9", "role": "button", "name": "Submit" },
    { "uid": "e19", "role": "button", "name": "Cancel" }
  ]
}
```

## 编程接口

### 核心导出

```javascript
const {
  // 浏览器管理
  launchBrowser,
  closeBrowser,
  setupPageListeners,

  // 测试上下文
  createTestContext,
  executeTool,

  // SSO 登录处理
  handleSSOIfNeeded,
  detectSSOPage,
  waitForSSOLogin,
  ensureLoggedIn,

  // 高级功能
  verifyUI,
  runScript,
  runInteractiveMode,

  // 会话管理
  saveBrowserSession,
  loadBrowserSession,
  sessionExists,

  // 工具查询
  tools,
  getToolByName,
  listTools,
  getToolsByCategory,
} = require("./browser-test");

// 网络收集器（独立模块）
const { NetworkCollector } = require("./lib/network-collector");
```

### 基础用法

```javascript
const {
  launchBrowser,
  closeBrowser,
  setupPageListeners,
  createTestContext,
  executeTool,
  handleSSOIfNeeded,
} = require("./browser-test");

async function main() {
  // 1. 启动浏览器
  const browser = await launchBrowser({ headless: false });
  const page = await browser.newPage();
  setupPageListeners(page);

  // 2. 创建测试上下文
  const ctx = await createTestContext(page, { timeout: 30000 });

  // 3. 执行工具
  await executeTool(ctx, "navigate", { url: "http://localhost:3000/_bifrost" });

  // 4. 处理 SSO 登录（自动检测，无需手动判断）
  await handleSSOIfNeeded(page, ctx, { timeout: 120000 });

  // 5. 获取页面快照
  const snapshot = await executeTool(ctx, "takeSnapshot", {});
  console.log(snapshot.result.formatted);

  await closeBrowser(browser);
}

main().catch(console.error);
```

### 完整测试流程示例（含网络监控）

以下示例展示了打开页面、SSO 登录处理、网络请求收集、页面分析的完整流程：

```javascript
const {
  launchBrowser,
  closeBrowser,
  setupPageListeners,
  createTestContext,
  executeTool,
  handleSSOIfNeeded,
} = require("./browser-test");
const { NetworkCollector } = require("./lib/network-collector");

async function main() {
  console.log("Starting browser...");
  const browser = await launchBrowser({ headless: false });
  const page = await browser.newPage();
  setupPageListeners(page);

  // 设置网络监控
  const networkCollector = new NetworkCollector();
  networkCollector.attach(page);
  networkCollector.start();

  console.log("Creating test context...");
  const ctx = await createTestContext(page, { timeout: 30000 });

  console.log("Navigating to http://localhost:3000/_bifrost...");
  await executeTool(ctx, "navigate", { url: "http://localhost:3000/_bifrost" });
  await new Promise((resolve) => setTimeout(resolve, 3000));

  // 自动处理 SSO 登录（检测到 SSO 页面时等待用户扫码）
  const handled = await handleSSOIfNeeded(page, ctx, { timeout: 120000 });
  if (handled) {
    console.log("Waiting for homepage to fully load...");
    await new Promise((resolve) => setTimeout(resolve, 5000));
  }

  // 获取页面快照
  console.log("Taking snapshot...");
  const snapshot = await executeTool(ctx, "takeSnapshot", {});
  console.log("\n=== PAGE STRUCTURE ===\n");
  console.log(snapshot.result?.formatted || "No snapshot available");

  // 获取网络请求统计
  const summary = networkCollector.getSummary();
  const apiRequests = networkCollector.getApiRequests();
  const failedRequests = networkCollector.getFailedRequests();

  console.log("\n=== NETWORK SUMMARY ===\n");
  console.log(`Total requests: ${summary.total}`);
  console.log(`API requests: ${apiRequests.length}`);
  console.log(`Failed requests: ${failedRequests.length}`);

  // 获取页面信息
  const pageInfo = await executeTool(ctx, "getPageInfo", {});
  console.log("\n=== PAGE INFO ===\n");
  console.log(`Title: ${pageInfo.result?.title}`);
  console.log(`URL: ${pageInfo.result?.url}`);

  await closeBrowser(browser);
  console.log("Done!");
}

main().catch((err) => {
  console.error("Error:", err);
  process.exit(1);
});
```

### NetworkCollector API

```javascript
const { NetworkCollector } = require("./lib/network-collector");

const collector = new NetworkCollector();
collector.attach(page); // 绑定到页面
collector.start(); // 开始收集

// 获取数据
collector.getSummary(); // { total, api, failed, requests }
collector.getApiRequests(); // API 请求列表
collector.getFailedRequests(); // 失败请求列表

collector.stop(); // 停止收集
collector.clear(); // 清空数据
```

### 快照输出格式说明

`takeSnapshot` 返回的 `formatted` 字段包含树形结构的页面可访问性信息：

```
- RootWebArea "NextOncall - 应用列表" [e2_0]
  - heading "下一代智能客服" [e2_1] <Typography>
  - button "bell 3" [e2_3] <Button>
  - main [e2_10]
    - radio "全部" [e2_12] <Radio>
    - textbox "请输入应用名称" [e2_19] <Input>
    - button "plus 新建应用" [e2_21] <Button>
```

格式说明：

- **缩进层级**：表示 DOM 元素的父子关系
- **角色标识**：如 `heading`、`button`、`radio`、`textbox` 等 ARIA 角色
- **元素名称**：引号内的文本，如 `"下一代智能客服"`
- **UID 标识**：方括号内的唯一标识符，如 `[e2_0]`，用于后续元素定位
- **React 组件**：尖括号内的组件名，如 `<Typography>`、`<Button>`、`<Radio>`

#### network 页面单个请求的详情检查

管理端详情接口路径：

- `/_bifrost/api/traffic/{id}`
- `/_bifrost/api/traffic/{id}/request-body`
- `/_bifrost/api/traffic/{id}/response-body`

从 traffic 列表响应中取 `id` 再查询详情：

```bash
cd .trae/skills/e2e-verify/scripts

node api-test.js --api /_bifrost/api/traffic --target http://localhost:3000
node api-test.js --api /_bifrost/api/traffic/{id} --target http://localhost:3000
node api-test.js --api /_bifrost/api/traffic/{id}/request-body --target http://localhost:3000
node api-test.js --api /_bifrost/api/traffic/{id}/response-body --target http://localhost:3000
```

### 场景测试执行示例

```bash
# 执行管理端核心功能 smoke 场景（需要先创建 scripts/scenarios/bifrost-smoke.json）
cd .trae/skills/e2e-verify/scripts
node browser-test.js scenario bifrost-smoke --verbose

# 输出示例：
# [02:57:14] INFO  [BrowserManager] Browser launched
# [02:57:14] INFO  [BrowserTest] Navigating to: http://localhost:3000/_bifrost
#
# ============================================================
# 🧪 场景: Bifrost 管理端基础验证
#    验证 Traffic/Replay/Rules/Values/Scripts/Settings 核心流程
# ============================================================
#
# 📌 ==== 步骤 1: 进入首页 ====
# [2/33] 导航到 /... ✅
# [4/33] 快照 [首页]... ✅
#
# 📌 ==== 步骤 2: 打开 Traffic 并查看详情 ====
# [6/33] 点击 menu "Traffic"... ✅
# [8/33] 点击列表首条记录... ✅
#
# 📌 ==== 步骤 3: 切换 Rules/Values/Scripts/Settings ====
# [12/33] 点击 menu "Rules"... ✅
# [16/33] 点击 menu "Values"... ✅
# [20/33] 点击 menu "Scripts"... ✅
# [24/33] 点击 menu "Settings"... ✅
#
#    ┌─ 网络请求 ─────────────────────────────────────
#    │ GET    ... /_bifrost/api/traffic
#    │ GET    ... /_bifrost/api/rules
#    │ GET    ... /_bifrost/api/values
#    │ GET    ... /_bifrost/api/scripts
#    │ GET    ... /_bifrost/api/system/overview
#    └───────────────────────────────────────────────
#
# 📌 ==== Bifrost 管理端基础验证完成 ====
#
# ============================================================
# 📊 执行报告
# ============================================================
#    场景: Bifrost 管理端基础验证
#    耗时: 15.92s
#    步骤: 33/33 通过
#    网络: 106 API 请求, 5 失败
#
#    结果: ✅ 通过
# ============================================================
```

### SSE/WebSocket 流式推送用例

先模拟代理流量，再跑场景验证 UI 展示：

```bash
cd .trae/skills/e2e-verify/scripts
bash examples/seed-streaming-traffic.sh
node browser-test.js scenario stream-sse --verbose
node browser-test.js scenario stream-ws --verbose
```

预期效果：

- SSE 记录选中后，Messages 里出现 SSE 列表与事件计数
- WebSocket 记录选中后，Messages 里出现 frames 列表与统计
- WebSocket 用例依赖 websocat，可用 `brew install websocat` 安装

## API 测试

管理端 API 的验证建议优先通过 devserver 代理或直连 admin 服务：

- **devserver 代理**：`http://localhost:3000/_bifrost/api/...`
- **admin 直连**：`http://127.0.0.1:9900/_bifrost/api/...`

### 管理端 API 验证方法

1. 启动管理端服务（默认 9900）与 Web devserver（默认 3000）
2. 选取管理端 API（参考 `crates/bifrost-admin/ADMIN_API.md`）
3. 用 `api-test.js` 发起请求，关注状态码、响应体与耗时
4. 对需要写入的接口，建议先在测试环境验证

```bash
node api-test.js --api /_bifrost/api/system/overview --target http://localhost:3000
node api-test.js --api /_bifrost/api/traffic --target http://127.0.0.1:9900
node api-test.js --api /_bifrost/api/rules --verbose --target http://localhost:3000
```

### Search/System Proxy 验证用例

```bash
node api-test.js --api /_bifrost/api/search -m POST -d '{"keyword":"login","scope":{"all":false,"url":true},"limit":10}' --target http://127.0.0.1:9900
node api-test.js --api /_bifrost/api/proxy/system --target http://127.0.0.1:9900
```

### 基础调用协议（api-test.js）

- `api` 会自动补齐前导 `/`，并支持 `/:appId` 占位符替换为 `.env/.app_config` 中的 `appId`
- `baseUrl` 取 `target` 或 `http://localhost:${port}`，最终 URL 为 `baseUrl + api`
- 默认请求头包含 `Content-Type: application/json`
- 仅在 `method` 非 GET/HEAD 且传入 `body` 时写入请求体
- 超时通过 `AbortSignal.timeout` 控制，默认 30000ms
- 根据响应 `content-type` 自动解析 JSON 或文本
- 返回结构包含 `success/status/statusText/duration/url/method/headers/body`

### 命令行参数

| 参数                 | 说明                                       |
| -------------------- | ------------------------------------------ |
| `--api <path>`       | API 路径（必需）                           |
| `-m, --method <M>`   | HTTP 方法（默认: GET）                     |
| `-d, --data <json>`  | 请求体 JSON                                |
| `-t, --target <url>` | 目标服务器 URL                             |
| `-p, --port <port>`  | 端口号（默认: 8000，本项目常用 3000/9900） |
| `--timeout <ms>`     | 超时时间（默认: 30000）                    |
| `-v, --verbose`      | 详细输出                                   |
| `-h, --help`         | 显示帮助                                   |

### 编程接口

```javascript
const { verifyAPI } = require("./api-test");

const result = await verifyAPI({
  api: "/_bifrost/api/system/overview",
  method: "GET",
  target: "http://localhost:3000",
  port: 3000,
  timeout: 30000,
  verbose: true,
  headers: {
    "X-Client-Id": "e2e-verify",
  },
});
```

## 交互模式命令

进入交互模式后可用的命令：

| 命令                | 说明                    |
| ------------------- | ----------------------- |
| `tools`             | 列出所有可用工具        |
| `tools <category>`  | 列出指定分类的工具      |
| `<tool> [params]`   | 执行工具（参数为 JSON） |
| `snapshot`          | 获取页面快照            |
| `screenshot [name]` | 截图                    |
| `save [session]`    | 保存浏览器会话          |
| `load [session]`    | 加载浏览器会话          |
| `vars`              | 显示存储的变量          |
| `exit`              | 退出交互模式            |
