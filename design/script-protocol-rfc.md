# Bifrost Script Protocol 技术方案 v2

## 1. 背景与目标

### 1.1 需求概述

新增两个协议用于通过 JavaScript 脚本修改 HTTP 请求和响应：

- `reqScript://script_name` - 在请求阶段执行脚本，修改请求内容
- `resScript://script_name` - 在响应阶段执行脚本，修改响应内容

### 1.2 设计目标

1. **内置轻量级 JS 引擎**：使用 QuickJS (通过 rquickjs) 内嵌到 Bifrost，无需外部依赖
2. **沙箱执行环境**：仅支持基本逻辑运算，不支持文件系统、网络等危险操作
3. **请求/响应修改**：支持修改 headers、body、URL（请求阶段）、status（响应阶段）
4. **日志系统**：脚本执行的日志关联到具体请求，在 UI 详情页 Script Tab 展示
5. **脚本管理 UI**：在管理端提供独立的脚本编辑器 Tab，支持语法提示
6. **执行时机**：在 TLS 解包完成后执行（适用于 HTTP/HTTPS/SOCKS/HTTP3 代理）

### 1.3 关键设计决策

| 决策项 | 方案 | 理由 |
|--------|------|------|
| JS 引擎 | rquickjs (QuickJS) | 轻量级、内嵌、支持 ES2020、编译到单一二进制 |
| 脚本存储 | 用户数据目录 `scripts/` | 统一管理，持久化 |
| 安全模型 | 沙箱环境 | 仅提供受控 API，禁止文件/网络访问 |
| 脚本编辑 | 管理端独立 Tab | 内置 Monaco 编辑器，提供类型提示 |

## 2. 整体架构

```
┌────────────────────────────────────────────────────────────────────┐
│                        Bifrost Proxy                                │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────┐   TLS 解包后   ┌─────────────────┐                │
│  │ HTTP/HTTPS  │──────────────▶│  Script Engine  │                │
│  │ SOCKS/H3    │               │  (rquickjs)     │                │
│  │   Handler   │◀──────────────│   Sandbox       │                │
│  └─────────────┘   修改结果     └─────────────────┘                │
│         │                              │                            │
│         │                    ┌─────────┴─────────┐                 │
│         ▼                    ▼                   ▼                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐            │
│  │ Rules       │    │ Script      │    │ Script      │            │
│  │ Resolver    │    │ Store       │    │ Logs        │            │
│  └─────────────┘    │ (用户目录)  │    │ (per-req)   │            │
│                     └─────────────┘    └─────────────┘            │
│                                               │                    │
│  ┌────────────────────────────────────────────┼────────────────┐  │
│  │                   Admin UI                 │                │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐    │                │  │
│  │  │ Rules   │  │ Values  │  │ Scripts │◀───┘                │  │
│  │  │  Tab    │  │  Tab    │  │  Tab    │ (新增)               │  │
│  │  └─────────┘  └─────────┘  └─────────┘                     │  │
│  │                                                            │  │
│  │  Traffic Detail:                                           │  │
│  │  ┌──────────────────────────────────────────────────────┐  │  │
│  │  │ Request:  Overview │ Header │ Body │ Script Logs ✨  │  │  │
│  │  │ Response: Header │ Body │ Script Logs ✨             │  │  │
│  │  └──────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
```

## 3. JS 引擎选型：rquickjs

### 3.1 引擎对比

| 引擎 | 语言 | 大小 | ES 版本 | Rust 绑定 | 特点 |
|------|------|------|---------|-----------|------|
| **QuickJS (rquickjs)** | C | ~400KB | ES2020 | ✅ rquickjs | 轻量、高性能、活跃维护 |
| Boa | Rust | ~2MB | ES2022+ | 原生 | 纯 Rust、较大、部分特性不完整 |
| V8 (rusty_v8) | C++ | ~20MB+ | ES2023 | ✅ | 重量级、功能完整、依赖复杂 |
| Deno Core | C++/Rust | ~30MB+ | ES2023 | ✅ | 功能丰富但体积大 |

### 3.2 选择 rquickjs 的理由

1. **轻量级**：编译后仅增加约 400KB 二进制大小
2. **内嵌**：编译到 Bifrost 单一二进制，无需外部依赖
3. **ES2020 支持**：支持 async/await、Promise、现代语法
4. **成熟稳定**：QuickJS 由 FFmpeg/QEMU 作者 Fabrice Bellard 开发
5. **安全**：容易实现沙箱隔离，禁用危险 API
6. **性能**：JIT-less 但足够快，适合配置脚本场景

### 3.3 rquickjs 基本用法

```rust
use rquickjs::{Context, Runtime, Function, Object};

// 创建运行时和上下文
let runtime = Runtime::new()?;
let context = Context::full(&runtime)?;

context.with(|ctx| {
    // 注入全局对象
    let globals = ctx.globals();
    
    // 注入 request 对象
    let request = Object::new(ctx)?;
    request.set("url", "https://example.com")?;
    request.set("method", "GET")?;
    globals.set("request", request)?;
    
    // 注入 log API
    let log = Object::new(ctx)?;
    log.set("info", Function::new(ctx, |msg: String| {
        println!("[INFO] {}", msg);
    })?)?;
    globals.set("log", log)?;
    
    // 执行脚本
    ctx.eval::<(), _>(r#"
        log.info("Processing: " + request.url);
        request.headers["X-Custom"] = "value";
    "#)?;
    
    Ok(())
})?;
```

## 4. 协议设计

### 4.1 协议格式

```
# 请求阶段脚本 - 脚本名称（不含扩展名）
example.com reqScript://add-auth-header

# 响应阶段脚本
example.com resScript://transform-response

# 组合使用
api.example.com reqScript://add-auth resScript://format-json
```

### 4.2 脚本存储位置

```
$BIFROST_DATA_DIR/
├── rules/
├── values/
└── scripts/                    # 新增脚本目录
    ├── request/                # 请求脚本
    │   ├── add-auth-header.js
    │   └── modify-params.js
    └── response/               # 响应脚本
        ├── transform-response.js
        └── add-cors.js
```

### 4.3 协议解析

| 协议 | Category | Multi-Match | 说明 |
|------|----------|-------------|------|
| `ReqScript` | Request | ✅ | 新增，请求阶段 |
| `ResScript` | Response | ✅ | 已存在，增强实现 |

## 5. 脚本 API 设计（沙箱环境）

### 5.1 全局对象

脚本在沙箱中执行，仅能访问以下预定义的全局对象：

```javascript
// ============================================
// 请求脚本 (reqScript) 可用的全局对象
// ============================================

/** 请求对象 - 可读可写 */
const request = {
  // 只读属性
  url: "https://api.example.com/users",      // 完整 URL
  host: "api.example.com",                   // 主机名
  path: "/users",                            // 路径
  protocol: "https",                         // 协议
  clientIp: "192.168.1.100",                 // 客户端 IP
  clientApp: "Chrome",                       // 客户端应用（可能为 null）
  
  // 可修改属性
  method: "GET",                             // 请求方法
  headers: {                                 // 请求头（对象形式）
    "User-Agent": "...",
    "Accept": "application/json"
  },
  body: null,                                // 请求体（string | null）
};

// ============================================
// 响应脚本 (resScript) 可用的全局对象
// ============================================

/** 响应对象 - 可读可写 */
const response = {
  // 可修改属性
  status: 200,                               // 状态码
  statusText: "OK",                          // 状态文本
  headers: {                                 // 响应头
    "Content-Type": "application/json"
  },
  body: '{"users": []}',                     // 响应体
  
  // 只读属性 - 原始请求信息
  request: {
    url: "https://api.example.com/users",
    method: "GET",
    host: "api.example.com",
    path: "/users",
    headers: { ... }
  }
};

// ============================================
// 通用全局对象（两种脚本都可用）
// ============================================

/** 日志 API - 日志会关联到当前请求 */
const log = {
  debug: (...args) => {},                    // 调试日志
  info: (...args) => {},                     // 信息日志
  warn: (...args) => {},                     // 警告日志
  error: (...args) => {}                     // 错误日志
};

/** 环境上下文 */
const ctx = {
  requestId: "abc-123",                      // 请求 ID
  scriptName: "add-auth-header",             // 脚本名称
  scriptType: "request",                     // 脚本类型: "request" | "response"
  
  // Values - 已加载的用户定义变量
  values: {
    "API_TOKEN": "xxx",
    "BASE_URL": "https://api.example.com"
  },
  
  // 匹配的规则列表
  matchedRules: [
    { pattern: "api.example.com", protocol: "reqScript", value: "add-auth" }
  ]
};

/** JSON 工具（内置） */
const JSON = {
  parse: (str) => {},
  stringify: (obj, replacer?, space?) => {}
};

/** 控制台（别名，指向 log） */
const console = log;
```

### 5.2 禁用的 API（沙箱安全）

以下标准 API 在沙箱中**不可用**：

```javascript
// ❌ 文件系统
require, import, fs, __dirname, __filename

// ❌ 网络
fetch, XMLHttpRequest, WebSocket

// ❌ 进程
process, child_process, exec

// ❌ 定时器（避免长时间占用）
setTimeout, setInterval, setImmediate

// ❌ 其他
eval, Function (动态代码执行)
```

### 5.3 示例脚本

**请求脚本：添加认证头** (`scripts/request/add-auth-header.js`)
```javascript
// 从 Values 获取 Token
const token = ctx.values.API_TOKEN;

if (token) {
  request.headers["Authorization"] = "Bearer " + token;
  log.info("Added auth header for: " + request.host);
} else {
  log.warn("No API_TOKEN found in values");
}

// 添加自定义头
request.headers["X-Request-ID"] = ctx.requestId;
request.headers["X-Processed-By"] = "bifrost-script";
```

**响应脚本：转换 JSON 响应** (`scripts/response/transform-response.js`)
```javascript
log.debug("Processing response for: " + response.request.url);

// 检查是否为 JSON 响应
const contentType = response.headers["Content-Type"] || "";
if (!contentType.includes("application/json")) {
  log.info("Skipping non-JSON response");
  return;
}

try {
  // 解析并修改 JSON
  const data = JSON.parse(response.body);
  
  // 添加元信息
  data._meta = {
    processedAt: Date.now(),
    processedBy: ctx.scriptName,
    originalUrl: response.request.url
  };
  
  // 更新响应体
  response.body = JSON.stringify(data, null, 2);
  log.info("Transformed JSON response successfully");
  
} catch (e) {
  log.error("Failed to parse JSON: " + e.message);
}
```

**响应脚本：添加 CORS 头** (`scripts/response/add-cors.js`)
```javascript
response.headers["Access-Control-Allow-Origin"] = "*";
response.headers["Access-Control-Allow-Methods"] = "GET, POST, PUT, DELETE, OPTIONS";
response.headers["Access-Control-Allow-Headers"] = "Content-Type, Authorization";

log.info("Added CORS headers");
```

## 6. 脚本执行时机

### 6.1 执行位置

脚本执行发生在 **TLS 解包完成后**，确保能够访问明文的 HTTP 数据：

```
客户端 ──▶ Bifrost Proxy ──▶ 目标服务器

HTTP 请求流程:
┌─────────┐    ┌──────────┐    ┌───────────┐    ┌────────────┐    ┌─────────┐
│ 接收    │───▶│ TLS 解包 │───▶│ reqScript │───▶│ 转发请求   │───▶│ 目标    │
│ 请求    │    │ (如需要) │    │ 执行      │    │            │    │ 服务器  │
└─────────┘    └──────────┘    └───────────┘    └────────────┘    └─────────┘

HTTP 响应流程:
┌─────────┐    ┌───────────┐    ┌──────────┐    ┌─────────┐
│ 接收    │───▶│ resScript │───▶│ TLS 重新 │───▶│ 返回    │
│ 响应    │    │ 执行      │    │ 加密     │    │ 客户端  │
└─────────┘    └───────────┘    └──────────┘    └─────────┘
```

### 6.2 适用场景

| 代理类型 | TLS 拦截 | 脚本可用 | 说明 |
|----------|----------|----------|------|
| HTTP | N/A | ✅ | 明文，直接可用 |
| HTTPS | tlsIntercept | ✅ | 需要启用 TLS 拦截 |
| HTTPS | tlsPassthrough | ❌ | 透传模式，无法访问内容 |
| SOCKS5 + HTTP | N/A | ✅ | 明文 |
| SOCKS5 + HTTPS | tlsIntercept | ✅ | 需要启用 TLS 拦截 |
| HTTP/3 (QUIC) | tlsIntercept | ✅ | 需要启用 TLS 拦截 |

## 7. 日志系统设计

### 7.1 日志数据结构

```rust
// crates/bifrost-core/src/script/types.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptLogEntry {
    pub timestamp: u64,
    pub level: ScriptLogLevel,
    pub message: String,
    pub args: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScriptLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExecutionResult {
    pub script_name: String,
    pub script_type: ScriptType,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub logs: Vec<ScriptLogEntry>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScriptType {
    Request,
    Response,
}
```

### 7.2 TrafficRecord 扩展

```rust
// crates/bifrost-admin/src/traffic.rs

pub struct TrafficRecord {
    // ... 现有字段 ...
    
    /// 请求阶段脚本执行结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_script_results: Option<Vec<ScriptExecutionResult>>,
    
    /// 响应阶段脚本执行结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub res_script_results: Option<Vec<ScriptExecutionResult>>,
}
```

## 8. UI 设计

### 8.1 脚本管理 Tab（新增页面）

在管理端主导航新增 **Scripts** Tab：

```
┌─────────────────────────────────────────────────────────────────────┐
│  Bifrost                                                            │
├──────────┬──────────┬──────────┬──────────┬──────────┬─────────────┤
│ Overview │  Rules   │  Values  │ Scripts✨│ Traffic  │  Settings   │
└──────────┴──────────┴──────────┴──────────┴──────────┴─────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Scripts                                                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐  ┌─────────────────────────────────────────┐  │
│  │ 📁 Request       │  │                                         │  │
│  │   add-auth.js    │  │  // add-auth.js                         │  │
│  │   modify-url.js  │  │                                         │  │
│  │                  │  │  const token = ctx.values.API_TOKEN;    │  │
│  │ 📁 Response      │  │  if (token) {                           │  │
│  │   transform.js   │  │    request.headers["Authorization"]     │  │
│  │   add-cors.js    │  │      = "Bearer " + token;               │  │
│  │                  │  │    log.info("Added auth header");       │  │
│  │ [+ New Script]   │  │  }                                      │  │
│  │                  │  │                                         │  │
│  └─────────────────┘  │  ─────────────────────────────────────── │  │
│                       │  [Save] [Test] [Delete]                  │  │
│                       └─────────────────────────────────────────┘  │
│                                                                     │
│  Type Hints Panel:                                                  │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │ Available Objects:                                            │ │
│  │ • request.url, request.method, request.headers, request.body  │ │
│  │ • ctx.values, ctx.matchedRules, ctx.requestId                 │ │
│  │ • log.debug(), log.info(), log.warn(), log.error()           │ │
│  └───────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### 8.2 Monaco 编辑器集成

```typescript
// web/src/pages/Scripts/index.tsx

import MonacoEditor from '@monaco-editor/react';

// 为 Bifrost 脚本环境提供类型定义
const bifrostTypes = `
interface BifrostRequest {
  readonly url: string;
  readonly host: string;
  readonly path: string;
  readonly protocol: string;
  readonly clientIp: string;
  readonly clientApp: string | null;
  method: string;
  headers: Record<string, string>;
  body: string | null;
}

interface BifrostResponse {
  status: number;
  statusText: string;
  headers: Record<string, string>;
  body: string | null;
  readonly request: {
    url: string;
    method: string;
    host: string;
    path: string;
    headers: Record<string, string>;
  };
}

interface BifrostContext {
  readonly requestId: string;
  readonly scriptName: string;
  readonly scriptType: 'request' | 'response';
  readonly values: Record<string, string>;
  readonly matchedRules: Array<{
    pattern: string;
    protocol: string;
    value: string;
  }>;
}

interface BifrostLog {
  debug(...args: any[]): void;
  info(...args: any[]): void;
  warn(...args: any[]): void;
  error(...args: any[]): void;
}

declare const request: BifrostRequest;
declare const response: BifrostResponse;
declare const ctx: BifrostContext;
declare const log: BifrostLog;
declare const console: BifrostLog;
`;

function ScriptEditor({ scriptType, scriptName, content, onChange }) {
  return (
    <MonacoEditor
      height="400px"
      language="javascript"
      theme="vs-dark"
      value={content}
      onChange={onChange}
      beforeMount={(monaco) => {
        // 添加 Bifrost 类型定义
        monaco.languages.typescript.javascriptDefaults.addExtraLib(
          bifrostTypes,
          'bifrost.d.ts'
        );
      }}
      options={{
        minimap: { enabled: false },
        fontSize: 14,
        automaticLayout: true,
      }}
    />
  );
}
```

### 8.3 规则编辑器脚本选择

在规则编辑器中，输入 `reqScript://` 或 `resScript://` 时提供脚本名称智能提示：

```typescript
// web/src/pages/Rules/RuleEditor/index.tsx

// 脚本协议的自动补全
const getScriptSuggestions = (scriptType: 'request' | 'response') => {
  const scripts = useScriptStore(s => 
    scriptType === 'request' ? s.requestScripts : s.responseScripts
  );
  
  return scripts.map(script => ({
    label: script.name,
    insertText: script.name,
    detail: `${scriptType} script`,
    documentation: script.description || `Script: ${script.name}`,
  }));
};
```

### 8.4 流量详情 Script Logs Tab

```
Request Panel:
┌──────────────────────────────────────────────────────────────────┐
│ Request                                                          │
├──────────────────────────────────────────────────────────────────┤
│ Overview │ Header │ Query │ Body │ Raw │ Scripts ✨              │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  📜 add-auth-header                                              │
│  ────────────────────────────────────────────────────────────── │
│  Status: ✅ Success (2ms)                                        │
│                                                                  │
│  Logs:                                                          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 10:23:45.123 [INFO]  Added auth header for: api.example.com│ │
│  │ 10:23:45.124 [DEBUG] Token length: 32                      │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  📜 modify-params                                               │
│  ────────────────────────────────────────────────────────────── │
│  Status: ⚠️ Error (1ms)                                         │
│  Error: ReferenceError: params is not defined                   │
│                                                                  │
│  Logs:                                                          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 10:23:45.130 [ERROR] Script execution failed               │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

## 9. 脚本执行引擎实现

### 9.1 ScriptEngine 结构

```rust
// crates/bifrost-script/src/engine.rs

use rquickjs::{Context, Runtime, Function, Object, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ScriptEngine {
    /// QuickJS 运行时
    runtime: Runtime,
    /// 脚本缓存
    cache: Arc<RwLock<HashMap<String, CompiledScript>>>,
    /// 配置
    config: ScriptEngineConfig,
}

pub struct ScriptEngineConfig {
    /// 脚本执行超时（毫秒）
    pub timeout_ms: u64,
    /// 脚本目录
    pub scripts_dir: PathBuf,
    /// 最大内存（字节）
    pub max_memory: usize,
}

impl ScriptEngine {
    pub fn new(config: ScriptEngineConfig) -> Result<Self> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(config.max_memory);
        
        Ok(Self {
            runtime,
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }
    
    /// 执行请求脚本
    pub async fn execute_request_script(
        &self,
        script_name: &str,
        request: &mut RequestContext,
        ctx: &ScriptContext,
    ) -> Result<ScriptExecutionResult> {
        let script_path = self.config.scripts_dir
            .join("request")
            .join(format!("{}.js", script_name));
        
        self.execute_script(&script_path, ScriptType::Request, request, None, ctx).await
    }
    
    /// 执行响应脚本
    pub async fn execute_response_script(
        &self,
        script_name: &str,
        response: &mut ResponseContext,
        ctx: &ScriptContext,
    ) -> Result<ScriptExecutionResult> {
        let script_path = self.config.scripts_dir
            .join("response")
            .join(format!("{}.js", script_name));
        
        self.execute_script(&script_path, ScriptType::Response, None, response, ctx).await
    }
}
```

### 9.2 沙箱环境创建

```rust
// crates/bifrost-script/src/sandbox.rs

use rquickjs::{Context, Object, Function, Value};

pub fn create_sandbox_context(
    ctx: &Context,
    script_type: ScriptType,
    request: Option<&RequestContext>,
    response: Option<&ResponseContext>,
    script_ctx: &ScriptContext,
    log_collector: &mut Vec<ScriptLogEntry>,
) -> Result<()> {
    ctx.with(|ctx| {
        let globals = ctx.globals();
        
        // 创建 log 对象
        let log = create_log_object(ctx, log_collector)?;
        globals.set("log", log.clone())?;
        globals.set("console", log)?;
        
        // 创建 ctx 对象（环境上下文）
        let ctx_obj = create_ctx_object(ctx, script_ctx)?;
        globals.set("ctx", ctx_obj)?;
        
        // 根据脚本类型创建 request 或 response 对象
        match script_type {
            ScriptType::Request => {
                let req = create_request_object(ctx, request.unwrap())?;
                globals.set("request", req)?;
            }
            ScriptType::Response => {
                let res = create_response_object(ctx, response.unwrap())?;
                globals.set("response", res)?;
            }
        }
        
        // 移除危险的全局对象
        remove_dangerous_globals(ctx)?;
        
        Ok(())
    })
}

fn remove_dangerous_globals(ctx: &Ctx) -> Result<()> {
    let globals = ctx.globals();
    
    // 移除可能的危险 API
    let dangerous = ["eval", "Function", "setTimeout", "setInterval"];
    for name in dangerous {
        globals.remove(name)?;
    }
    
    Ok(())
}

fn create_log_object(
    ctx: &Ctx, 
    collector: &mut Vec<ScriptLogEntry>
) -> Result<Object> {
    let log = Object::new(ctx)?;
    
    let collector = Arc::new(Mutex::new(collector));
    
    for level in ["debug", "info", "warn", "error"] {
        let collector = collector.clone();
        let level_enum = match level {
            "debug" => ScriptLogLevel::Debug,
            "info" => ScriptLogLevel::Info,
            "warn" => ScriptLogLevel::Warn,
            "error" => ScriptLogLevel::Error,
            _ => unreachable!(),
        };
        
        log.set(level, Function::new(ctx, move |args: Vec<Value>| {
            let message = args.iter()
                .map(|v| format!("{:?}", v))
                .collect::<Vec<_>>()
                .join(" ");
            
            collector.lock().unwrap().push(ScriptLogEntry {
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                level: level_enum,
                message,
                args: None,
            });
        })?)?;
    }
    
    Ok(log)
}
```

## 10. API 接口设计

### 10.1 脚本管理 API

```rust
// crates/bifrost-admin/src/handlers/scripts.rs

/// 获取所有脚本列表
/// GET /api/scripts
pub async fn list_scripts() -> Json<ScriptsResponse> {
    // {
    //   "request": [
    //     { "name": "add-auth", "description": "...", "created_at": ... }
    //   ],
    //   "response": [
    //     { "name": "transform", "description": "...", "created_at": ... }
    //   ]
    // }
}

/// 获取单个脚本内容
/// GET /api/scripts/:type/:name
pub async fn get_script(type: ScriptType, name: String) -> Json<ScriptDetail> {
    // { "name": "add-auth", "type": "request", "content": "...", ... }
}

/// 创建或更新脚本
/// PUT /api/scripts/:type/:name
pub async fn save_script(
    type: ScriptType, 
    name: String, 
    body: Json<SaveScriptRequest>
) -> Json<ScriptDetail> {
    // 验证脚本语法
    // 保存到文件系统
}

/// 删除脚本
/// DELETE /api/scripts/:type/:name
pub async fn delete_script(type: ScriptType, name: String) -> StatusCode {
    // 删除文件
}

/// 测试脚本（不保存）
/// POST /api/scripts/test
pub async fn test_script(body: Json<TestScriptRequest>) -> Json<TestScriptResponse> {
    // 在沙箱中执行脚本
    // 返回执行结果和日志
}
```

### 10.2 类型定义

```typescript
// web/src/types/scripts.ts

export type ScriptType = 'request' | 'response';

export interface ScriptSummary {
  name: string;
  type: ScriptType;
  description?: string;
  created_at: number;
  updated_at: number;
}

export interface ScriptDetail extends ScriptSummary {
  content: string;
}

export interface ScriptsResponse {
  request: ScriptSummary[];
  response: ScriptSummary[];
}

export interface SaveScriptRequest {
  content: string;
  description?: string;
}

export interface TestScriptRequest {
  type: ScriptType;
  content: string;
  mock_data: {
    request?: MockRequestData;
    response?: MockResponseData;
  };
}

export interface TestScriptResponse {
  success: boolean;
  error?: string;
  duration_ms: number;
  logs: ScriptLogEntry[];
  result?: {
    modified_headers?: Record<string, string>;
    modified_body?: string;
    modified_status?: number;
  };
}
```

## 11. 代码变更清单

### 11.1 新增文件

| 路径 | 说明 |
|------|------|
| `crates/bifrost-script/` | 新增 crate：脚本执行引擎 |
| `crates/bifrost-script/src/lib.rs` | 模块入口 |
| `crates/bifrost-script/src/engine.rs` | ScriptEngine 实现 |
| `crates/bifrost-script/src/sandbox.rs` | 沙箱环境创建 |
| `crates/bifrost-script/src/types.rs` | 类型定义 |
| `crates/bifrost-admin/src/handlers/scripts.rs` | 脚本管理 API |
| `web/src/pages/Scripts/` | 脚本管理页面 |
| `web/src/pages/Scripts/ScriptEditor.tsx` | Monaco 编辑器组件 |
| `web/src/pages/Scripts/ScriptList.tsx` | 脚本列表组件 |
| `web/src/stores/useScriptStore.ts` | 脚本状态管理 |
| `web/src/components/TrafficDetail/panes/ScriptLogs/` | 脚本日志 Tab |

### 11.2 修改文件

| 路径 | 变更内容 |
|------|----------|
| `Cargo.toml` | 新增 bifrost-script 依赖 |
| `crates/bifrost-core/src/protocol.rs` | 新增 `ReqScript` 协议（确认 ResScript 行为） |
| `crates/bifrost-proxy/src/proxy/http/handler.rs` | 集成脚本执行 |
| `crates/bifrost-admin/src/traffic.rs` | 新增 script_results 字段 |
| `crates/bifrost-admin/src/router.rs` | 添加脚本 API 路由 |
| `web/src/types/index.ts` | 新增 Script 相关类型 |
| `web/src/components/Layout/index.tsx` | 添加 Scripts 导航项 |
| `web/src/components/TrafficDetail/index.tsx` | 新增 Script Logs Tab |
| `web/src/pages/Rules/RuleEditor/` | 添加脚本名称智能提示 |

## 12. 实现阶段

### Phase 1: 基础引擎 (2-3 天)

1. 创建 `bifrost-script` crate
2. 集成 rquickjs
3. 实现沙箱环境
4. 实现基本的脚本执行

### Phase 2: 协议集成 (1-2 天)

1. 新增/确认协议定义
2. 在 handler 中集成脚本执行
3. 实现请求/响应修改应用

### Phase 3: 存储与 API (1-2 天)

1. 实现脚本文件存储
2. 实现脚本管理 API
3. TrafficRecord 扩展

### Phase 4: 脚本管理 UI (2-3 天)

1. 新增 Scripts 页面
2. 集成 Monaco 编辑器
3. 实现脚本列表、编辑、测试功能
4. 添加类型提示

### Phase 5: 流量详情集成 (1 天)

1. 新增 Script Logs Tab
2. 日志展示组件

### Phase 6: 规则编辑器集成 (1 天)

1. 添加脚本名称智能提示
2. 测试完整流程

### Phase 7: 测试与文档 (1 天)

1. 单元测试
2. E2E 测试
3. 文档更新

## 13. 安全考虑

### 13.1 沙箱限制

| 限制项 | 实现方式 |
|--------|----------|
| 无文件访问 | 不注入 fs、require 等 API |
| 无网络访问 | 不注入 fetch、XMLHttpRequest |
| 无进程访问 | 不注入 process、child_process |
| 执行超时 | QuickJS 内置超时机制 |
| 内存限制 | `runtime.set_memory_limit()` |
| 无 eval | 移除 eval、Function 全局对象 |

### 13.2 资源限制

```rust
pub const DEFAULT_SCRIPT_CONFIG: ScriptEngineConfig = ScriptEngineConfig {
    timeout_ms: 1000,           // 1 秒超时
    max_memory: 16 * 1024 * 1024, // 16MB 内存限制
    // ...
};
```

## 14. 参考资料

- [rquickjs 文档](https://docs.rs/rquickjs/)
- [QuickJS 官网](https://bellard.org/quickjs/)
- [Monaco Editor](https://microsoft.github.io/monaco-editor/)
