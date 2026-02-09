# Bifrost 插件开发指南

## 概述

Bifrost 是一个高性能的 HTTP/HTTPS/SOCKS5 代理服务器，提供强大的插件系统，支持 **Rust 原生插件** 和 **Node.js 插件** 两种开发方式。

### 命名规范

> **重要**：所有插件名称必须以 `bifrost.` 为前缀

| 类型 | 规范 | 示例 |
|-----|------|------|
| Rust 插件 | `fn name()` 返回 `bifrost.xxx` | `"bifrost.logger"` |
| Node.js 插件目录 | `bifrost.xxx/` | `plugins/bifrost.demo/` |
| Node.js package.json | `name: "bifrost.xxx"` | `"bifrost.demo"` |

---

## Hook 系统

Bifrost 提供 22 种 Hook 点，覆盖代理请求的完整生命周期：

### Hook 分类

| 类别 | Hook | 说明 |
|-----|------|------|
| 认证 | `Auth` | 代理认证 |
| TLS | `Sni` | SNI（Server Name Indication）处理 |
| HTTP | `Http` | HTTP 请求/响应处理 |
| 数据流 | `ReqRead` / `ReqWrite` | 请求数据读取/写入 |
| 数据流 | `ResRead` / `ResWrite` | 响应数据读取/写入 |
| 规则 | `ReqRules` / `ResRules` / `TunnelRules` | 规则注入 |
| 隧道 | `Tunnel` | HTTPS 隧道建立 |
| 隧道数据 | `TunnelReqRead` / `TunnelReqWrite` | 隧道请求数据 |
| 隧道数据 | `TunnelResRead` / `TunnelResWrite` | 隧道响应数据 |
| WebSocket | `WsReqRead` / `WsReqWrite` | WebSocket 请求数据 |
| WebSocket | `WsResRead` / `WsResWrite` | WebSocket 响应数据 |
| 统计 | `ReqStats` / `ResStats` | 请求/响应统计 |

### 执行顺序

```
客户端请求
    │
    ▼
┌─────────┐
│  Auth   │ ← 代理认证
└────┬────┘
     │
     ▼
┌─────────┐
│   Sni   │ ← TLS SNI 处理
└────┬────┘
     │
     ▼
┌─────────┐
│  Http   │ ← HTTP 请求处理
└────┬────┘
     │
     ▼
┌──────────┐
│ ReqRules │ ← 请求规则注入
└────┬─────┘
     │
     ▼
┌──────────┐
│ ReqRead  │ ← 请求数据读取
└────┬─────┘
     │
     ▼
┌──────────┐
│ ReqWrite │ ← 请求数据写入
└────┬─────┘
     │
     ▼
  [转发到服务器]
     │
     ▼
┌──────────┐
│ ResRules │ ← 响应规则注入
└────┬─────┘
     │
     ▼
┌──────────┐
│ ResRead  │ ← 响应数据读取
└────┬─────┘
     │
     ▼
┌──────────┐
│ ResWrite │ ← 响应数据写入
└────┬─────┘
     │
     ▼
┌──────────┐
│ ReqStats │ ← 请求统计
└────┬─────┘
     │
     ▼
┌──────────┐
│ ResStats │ ← 响应统计
└────┬─────┘
     │
     ▼
返回客户端
```

### 优先级

- 数值越大，优先级越高
- 同一 Hook 点的多个插件按优先级降序执行
- 默认优先级：0

---

## Rust 插件开发

### BifrostPlugin Trait

```rust
use async_trait::async_trait;
use bifrost_plugin::{
    AuthContext, BifrostPlugin, DataContext, HttpContext, 
    PluginHook, Result, RulesContext, SniContext, 
    StatsContext, TunnelContext,
};

#[async_trait]
pub trait BifrostPlugin: Send + Sync {
    /// 插件名称（必须以 "bifrost." 为前缀）
    fn name(&self) -> &str;
    
    /// 插件版本
    fn version(&self) -> &str;
    
    /// 声明插件监听的 Hook 列表
    fn hooks(&self) -> Vec<PluginHook>;
    
    /// 插件优先级（默认 0，数值越大优先级越高）
    fn priority(&self) -> i32 { 0 }
    
    // Hook 回调方法
    async fn on_auth(&self, ctx: &mut AuthContext) -> Result<()> { Ok(()) }
    async fn on_sni(&self, ctx: &mut SniContext) -> Result<()> { Ok(()) }
    async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> { Ok(()) }
    async fn on_tunnel(&self, ctx: &mut TunnelContext) -> Result<()> { Ok(()) }
    async fn on_req_rules(&self, ctx: &mut RulesContext) -> Result<()> { Ok(()) }
    async fn on_res_rules(&self, ctx: &mut RulesContext) -> Result<()> { Ok(()) }
    async fn on_tunnel_rules(&self, ctx: &mut RulesContext) -> Result<()> { Ok(()) }
    async fn on_req_read(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_req_write(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_res_read(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_res_write(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_tunnel_req_read(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_tunnel_req_write(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_tunnel_res_read(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_tunnel_res_write(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_ws_req_read(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_ws_req_write(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_ws_res_read(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_ws_res_write(&self, ctx: &mut DataContext) -> Result<()> { Ok(()) }
    async fn on_req_stats(&self, ctx: &mut StatsContext) -> Result<()> { Ok(()) }
    async fn on_res_stats(&self, ctx: &mut StatsContext) -> Result<()> { Ok(()) }
}
```

### Context 类型

#### PluginContext（基础上下文）

```rust
pub struct PluginContext {
    pub session_id: String,      // 会话 ID
    pub request_id: String,      // 请求 ID
    pub client_ip: String,       // 客户端 IP
    pub host: String,            // 目标主机
    pub url: String,             // 完整 URL
    pub method: String,          // HTTP 方法
    pub headers: HashMap<String, String>,  // HTTP 头
    pub status_code: Option<u16>,          // 状态码
}
```

#### AuthContext（认证上下文）

```rust
pub struct AuthContext {
    pub base: PluginContext,
    pub username: Option<String>,  // 用户名
    pub password: Option<String>,  // 密码
    pub authenticated: bool,       // 认证结果
}

impl AuthContext {
    pub fn approve(&mut self);  // 批准认证
    pub fn deny(&mut self);     // 拒绝认证
}
```

#### HttpContext（HTTP 上下文）

```rust
pub struct HttpContext {
    pub base: PluginContext,
    pub body: Option<Bytes>,   // 请求/响应体
    pub modified: bool,        // 是否已修改
}

impl HttpContext {
    pub fn set_header(&mut self, key: &str, value: &str);
    pub fn remove_header(&mut self, key: &str);
    pub fn set_body(&mut self, body: Bytes);
}
```

#### DataContext（数据流上下文）

```rust
pub struct DataContext {
    pub base: PluginContext,
    pub data: Bytes,           // 数据块
    pub is_last: bool,         // 是否为最后一块
}

impl DataContext {
    pub fn modify(&mut self, data: Bytes);
}
```

#### RulesContext（规则上下文）

```rust
pub struct RulesContext {
    pub base: PluginContext,
    pub rules: Vec<String>,    // 规则列表
}

impl RulesContext {
    pub fn add_rule(&mut self, rule: &str);
}
```

#### TunnelContext（隧道上下文）

```rust
pub struct TunnelContext {
    pub base: PluginContext,
    pub capture: bool,         // 是否捕获
}

impl TunnelContext {
    pub fn enable_capture(&mut self);
    pub fn disable_capture(&mut self);
}
```

#### StatsContext（统计上下文）

```rust
pub struct StatsContext {
    pub base: PluginContext,
    pub bytes_transferred: u64,  // 传输字节数
    pub duration_ms: u64,        // 持续时间（毫秒）
}
```

### 完整示例

```rust
use async_trait::async_trait;
use bifrost_plugin::{BifrostPlugin, HttpContext, PluginHook, Result};
use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct LoggerPlugin {
    log_count: AtomicU64,
}

impl LoggerPlugin {
    pub fn new() -> Self {
        Self {
            log_count: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl BifrostPlugin for LoggerPlugin {
    fn name(&self) -> &str {
        "bifrost.logger"  // 必须以 "bifrost." 为前缀
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Http]
    }

    fn priority(&self) -> i32 {
        100  // 高优先级
    }

    async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> {
        let count = self.log_count.fetch_add(1, Ordering::SeqCst) + 1;
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        tracing::info!(
            "[bifrost.logger] #{} {} {} {}",
            count,
            timestamp,
            ctx.base.method,
            ctx.base.url
        );

        ctx.set_header("X-Bifrost-Log-Id", &count.to_string());
        ctx.set_header("X-Bifrost-Log-Time", &timestamp);
        Ok(())
    }
}
```

### 注册插件

```rust
use bifrost_plugin::PluginManager;

let manager = PluginManager::new();

// 注册 Rust 插件
manager.register(Arc::new(LoggerPlugin::new()));
manager.register(Arc::new(AuthPlugin::new(false)));
manager.register(Arc::new(RateLimitPlugin::new(60)));
```

---

## Node.js 插件开发

### 目录结构

```
plugins/
└── bifrost.demo/           # 目录名必须以 "bifrost." 为前缀
    ├── package.json
    ├── index.js
    └── README.md (可选)
```

### package.json 配置

```json
{
  "name": "bifrost.demo",   // name 必须以 "bifrost." 为前缀
  "version": "1.0.0",
  "description": "A demo plugin for Bifrost",
  "main": "index.js",
  "bifrost": {
    "hooks": [
      "http",
      "reqRead",
      "resRead",
      "reqRules",
      "resRules"
    ],
    "priority": 100
  }
}
```

### bifrost 配置字段

| 字段 | 类型 | 说明 |
|-----|------|------|
| `hooks` | `string[]` | 监听的 Hook 列表 |
| `priority` | `number` | 插件优先级（默认 0） |

### Hook 名称映射

| Rust Hook | Node.js Hook |
|-----------|--------------|
| `Auth` | `auth` |
| `Sni` | `sni` |
| `Http` | `http` |
| `ReqRead` | `reqRead` |
| `ReqWrite` | `reqWrite` |
| `ResRead` | `resRead` |
| `ResWrite` | `resWrite` |
| `ReqRules` | `reqRules` |
| `ResRules` | `resRules` |
| `TunnelRules` | `tunnelRules` |
| `Tunnel` | `tunnel` |
| `TunnelReqRead` | `tunnelReqRead` |
| `TunnelReqWrite` | `tunnelReqWrite` |
| `TunnelResRead` | `tunnelResRead` |
| `TunnelResWrite` | `tunnelResWrite` |
| `WsReqRead` | `wsReqRead` |
| `WsReqWrite` | `wsReqWrite` |
| `WsResRead` | `wsResRead` |
| `WsResWrite` | `wsResWrite` |
| `ReqStats` | `reqStats` |
| `ResStats` | `resStats` |

### HTTP 协议

Node.js 插件通过 HTTP 协议与 Bifrost 通信：

**请求格式：**
- Method: `POST`
- URL: `http://localhost:{port}/{hook}`
- Content-Type: `application/json`
- Body: JSON 格式的 Context 对象

**响应格式：**
```json
{
  "modified": true,
  "ctx": { /* 修改后的 Context */ },
  "rules": ["rule1", "rule2"]  // 仅 Rules Hook
}
```

### 完整示例

```javascript
const http = require('http');

const PLUGIN_NAME = 'bifrost.demo';
const PORT = process.env.BIFROST_PLUGIN_PORT || 18000;

const handlers = {
  http: async (ctx) => {
    ctx.headers = ctx.headers || {};
    ctx.headers['X-Demo-Plugin'] = PLUGIN_NAME;
    ctx.headers['X-Demo-Timestamp'] = new Date().toISOString();
    
    console.log(`[${PLUGIN_NAME}] ${ctx.method} ${ctx.url}`);
    
    return { modified: true, ctx };
  },

  reqRules: async (ctx) => {
    const rules = [];
    
    if (ctx.url && ctx.url.includes('/mock/')) {
      rules.push('statusCode://200');
    }
    
    return { rules, ctx };
  },

  reqStats: async (ctx) => {
    console.log(`[${PLUGIN_NAME}] Request: ${ctx.bytesTransferred} bytes`);
    return { ctx };
  },
};

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const hook = url.pathname.slice(1);

  if (!handlers[hook]) {
    res.writeHead(404);
    res.end(JSON.stringify({ error: `Unknown hook: ${hook}` }));
    return;
  }

  let body = '';
  for await (const chunk of req) {
    body += chunk;
  }

  try {
    const ctx = JSON.parse(body);
    const result = await handlers[hook](ctx);
    
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(result));
  } catch (err) {
    res.writeHead(500);
    res.end(JSON.stringify({ error: err.message }));
  }
});

server.listen(PORT, () => {
  console.log(`[${PLUGIN_NAME}] Started on port ${PORT}`);
});
```

---

## 示例插件

项目提供了多个示例插件供参考：

### Rust 示例（bifrost-plugin-examples）

| 插件 | 功能 | Hook |
|-----|------|------|
| `bifrost.logger` | 请求日志记录 | `Http`, `ReqStats`, `ResStats` |
| `bifrost.header-injector` | HTTP 头注入 | `Http` |
| `bifrost.auth` | 代理认证 | `Auth` |
| `bifrost.rate-limit` | 请求限流 | `Http` |
| `bifrost.mock` | Mock 响应 | `Http`, `ReqRules` |
| `bifrost.data-transform` | 数据转换 | `ReqRead`, `ResRead` |
| `bifrost.tunnel-inspector` | 隧道检查 | `Tunnel`, `TunnelRules` |

### Node.js 示例（bifrost.demo）

| 插件 | 功能 | Hook |
|-----|------|------|
| `bifrost.demo` | 完整示例 | `http`, `reqRead`, `resRead`, `reqRules`, `resRules`, `reqStats`, `resStats` |

---

## 最佳实践

### 1. 命名规范

- 插件名称必须以 `bifrost.` 为前缀
- 使用小写字母和连字符：`bifrost.my-plugin`
- 避免使用下划线或大写字母

### 2. 性能优化

```rust
// ✅ 使用原子操作计数
use std::sync::atomic::{AtomicU64, Ordering};
let count = self.counter.fetch_add(1, Ordering::Relaxed);

// ✅ 使用 parking_lot 替代 std::sync
use parking_lot::RwLock;
let data = self.cache.read();

// ❌ 避免在热路径上使用锁
// let data = self.cache.lock().unwrap();
```

### 3. 错误处理

```rust
async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> {
    // 使用 ? 操作符传播错误
    let value = parse_header(&ctx.base.headers)?;
    
    // 记录错误但不中断流程
    if let Err(e) = self.process(&value) {
        tracing::warn!("Process failed: {}", e);
    }
    
    Ok(())
}
```

### 4. 日志规范

```rust
// 使用插件名称作为前缀
tracing::info!("[bifrost.my-plugin] Processing request: {}", ctx.base.url);
tracing::warn!("[bifrost.my-plugin] Rate limit exceeded for: {}", client_ip);
tracing::error!("[bifrost.my-plugin] Failed to process: {}", err);
```

### 5. 线程安全

```rust
// ✅ 使用 Send + Sync trait
pub struct MyPlugin {
    cache: Arc<RwLock<HashMap<String, String>>>,
}

// ✅ 使用原子类型
pub struct Counter {
    value: AtomicU64,
}
```

---

## API 参考

### PluginHook 枚举

```rust
pub enum PluginHook {
    Auth,
    Sni,
    Http,
    Tunnel,
    ReqRules,
    ResRules,
    TunnelRules,
    ReqRead,
    ReqWrite,
    ResRead,
    ResWrite,
    TunnelReqRead,
    TunnelReqWrite,
    TunnelResRead,
    TunnelResWrite,
    WsReqRead,
    WsReqWrite,
    WsResRead,
    WsResWrite,
    ReqStats,
    ResStats,
}
```

### PluginManager 方法

```rust
impl PluginManager {
    /// 创建插件管理器
    pub fn new() -> Self;
    
    /// 注册 Rust 插件
    pub fn register(&self, plugin: Arc<dyn BifrostPlugin>);
    
    /// 注销插件
    pub fn unregister(&self, name: &str);
    
    /// 获取插件
    pub fn get(&self, name: &str) -> Option<Arc<dyn BifrostPlugin>>;
    
    /// 获取指定 Hook 的所有插件（按优先级排序）
    pub fn get_plugins_for_hook(&self, hook: PluginHook) -> Vec<Arc<dyn BifrostPlugin>>;
    
    /// 执行 Hook
    pub async fn execute_hook<C>(&self, hook: PluginHook, ctx: &mut C) -> Result<()>;
}
```

---

## 常见问题

### Q: 插件加载失败？

1. 检查插件名称是否以 `bifrost.` 为前缀
2. 检查 `package.json` 中的 `bifrost.hooks` 配置是否正确
3. 查看日志获取详细错误信息

### Q: Hook 没有被触发？

1. 确认插件已正确注册到 PluginManager
2. 检查 `hooks()` 方法返回的 Hook 列表是否包含目标 Hook
3. 确认请求类型与 Hook 类型匹配

### Q: 如何调试 Node.js 插件？

1. 设置环境变量 `DEBUG=bifrost:*`
2. 在插件中添加 `console.log` 语句
3. 检查插件服务器日志输出

---

## 版本历史

| 版本 | 日期 | 说明 |
|-----|------|------|
| 1.0.0 | 2024-01 | 初始版本 |
